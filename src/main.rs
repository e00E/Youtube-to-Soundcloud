extern crate reqwest;
#[macro_use]
extern crate serde_derive;

extern crate backoff;
extern crate chrono;
extern crate serde;
extern crate serde_json;

use chrono::Datelike;
use std::str::FromStr;

mod config;
mod soundcloud;
mod util;
mod youtube;

fn default_backoff() -> backoff::ExponentialBackoff {
    backoff::ExponentialBackoff {
        max_interval: std::time::Duration::new(1024, 0),
        max_elapsed_time: None,
        ..Default::default()
    }
}

struct App {
    config: config::Config,
    playlists: config::Playlists,
    client: reqwest::Client,
    access_token: String,
}

impl App {
    fn new() -> Result<App, String> {
        let mut config = config::Config::read()?;
        let mut client = reqwest::ClientBuilder::new().unwrap();
        // Currently soundclouds playlisturl to api url needs redirects to be disabled for resolve to
        // work correctly.
        client.redirect(reqwest::RedirectPolicy::none());
        client.timeout(std::time::Duration::new(256, 0));
        // For debugging with Fiddler:
        // client.proxy(reqwest::Proxy::https("http://localhost:8888").unwrap());
        let client = client.build().unwrap();

        println!("Checking validity of existing Soundcloud access token.");
        let need_new_token = match config.soundcloud_access_token {
            Some(ref access_token) => {
                let mut op = || {
                    soundcloud::is_token_valid(&config.soundcloud_client_id, access_token, &client).map_err(|err| {
                        println!("Error: {}\nRetrying...", err);
                        backoff::Error::Transient(err)
                    })
                };
                !backoff::Operation::retry(&mut op, &mut default_backoff()).unwrap()
            }
            None => true,
        };
        let access_token;
        if need_new_token {
            let mut op = || {
                println!("No valid existing access token found. Authenticating with Soundcloud.");
                soundcloud::authenticate(
                    &config.soundcloud_client_id,
                    &config.soundcloud_client_secret,
                    &config.soundcloud_username,
                    &config.soundcloud_password,
                    &client,
                ).map_err(|err| {
                    println!("Error: {}\nRetrying...", err);
                    backoff::Error::Transient(err)
                })
            };
            match backoff::Operation::retry(&mut op, &mut default_backoff()).unwrap() {
                Some(response) => {
                    access_token = response.access_token;
                }
                None => return Err("The Soundcloud account details are not valid.".to_string()),
            }
        } else {
            access_token = config.soundcloud_access_token.clone().unwrap()
        };
        if need_new_token {
            config.soundcloud_access_token = Some(access_token.clone());
            config.write_safe()?;
        };

        // Load playlists
        let playlists = config::Playlists::read()?;

        Ok(App {
            config: config,
            playlists: playlists,
            client: client,
            access_token: access_token,
        })
    }

    fn download_audio(video: &youtube::PlaylistItem) -> String {
        println!(
            "Downloading new video with id {} and title {}.",
            &video.contentDetails.videoId, &video.snippet.title,
        );
        let mut op = || {
            youtube::download_audio(&video.contentDetails.videoId).map_err(|err| {
                println!("Error: {}\nRetrying...", err);
                backoff::Error::Transient(err)
            })
        };
        backoff::Operation::retry(&mut op, &mut default_backoff()).unwrap()
    }

    fn download_thumbnail(&self, video: &youtube::PlaylistItem) -> Option<String> {
        println!("Downloading thumbnail.");
        let mut op = || {
            let url = &video.snippet.thumbnails.get_best_thumbnail().url;
            let allowed_extensions = &["jpg", "png"];
            let start = url.rfind(".").ok_or(backoff::Error::Permanent(format!(
                "thumbnail url {} has no extension",
                url
            )))?;
            let extension = &url[start + 1..];
            if allowed_extensions.contains(&extension) {
                let path = format!("thumbnail.{}", extension).to_string();
                util::download_file(url, &path, &self.client)
                    .map_err(|err| {
                        println!("Error: {}\nRetrying...", err);
                        backoff::Error::Transient(err)
                    })
                    .and_then(|_| Ok(path))
            } else {
                Err(backoff::Error::Permanent(format!(
                    "thumbnail has illegal extension {}",
                    extension
                )))
            }
        };

        match backoff::Operation::retry(&mut op, &mut default_backoff()) {
            Ok(path) => Some(path),
            Err(err) => {
                println!(
                    "Failed to retrieve thumbnail, it will not be uploaded to Soundcloud: {}.",
                    err
                );
                None
            }
        }
    }

    fn get_youtube_playlist_data(&self, url: reqwest::Url) -> Result<youtube::PlaylistItemsResource, String> {
        println!("Getting Youtube playlist data.");
        let mut op = || -> Result<youtube::PlaylistItemsResource, backoff::Error<String>> {
            self.client
                .get(url.clone())
                .unwrap()
                .send()
                .map_err(|err| {
                    backoff::Error::Transient(format!("failed to send youtube playlist get request: {}", err))
                })
                .and_then(|response| match response.status() {
                    status if status.is_success() => Ok(response),
                    status if status.is_client_error() => Err(backoff::Error::Permanent(format!(
                        "response indicates client error: {}",
                        status
                    ))),
                    status => Err(backoff::Error::Transient(format!(
                        "response indicates server error: {}",
                        status
                    ))),
                })?
                .json()
                .map_err(|err| {
                    backoff::Error::Transient(format!("failed to parse youtube playlist get response: {}", err))
                })
                .map_err(|err| match err {
                    backoff::Error::Transient(err) => {
                        println!("Error: {}\nRetrying...", err);
                        backoff::Error::Transient(err)
                    }
                    other => other,
                })
        };
        backoff::Operation::retry(&mut op, &mut default_backoff()).map_err(|err| {
            format!(
                "could not get the youtube playlist: {}. \
                 Make sure the id is set correctly in the config file.",
                err
            )
        })
    }

    fn upload_audio(&self, audio_path: &str, video: &youtube::PlaylistItem, thumbnail_path: &Option<String>) -> u64 {
        println!("Uploading {} to Soundcloud.", audio_path);
        let year;
        let month;
        let day;
        let mut metadata = std::collections::HashMap::<&str, &str>::new();
        metadata.insert("sharing", "public");
        metadata.insert("title", &video.snippet.title);
        metadata.insert("description", &video.snippet.description);
        metadata.insert("downloadable", "1");
        let datetime = chrono::DateTime::<chrono::offset::Utc>::from_str(&video.snippet.publishedAt);
        match datetime {
            Ok(datetime) => {
                let date = datetime.date();
                year = date.year().to_string();
                month = date.month().to_string();
                day = date.day().to_string();
                metadata.insert("release_year", &year);
                metadata.insert("release_month", &month);
                metadata.insert("release_day", &day);
            }
            Err(err) => println!(
                "Failed to parse timedate string {}, \
                 release date will not be set on Soundcloud: {}.",
                &video.snippet.publishedAt, err
            ),
        }
        metadata.insert("downloadable", "1");
        let mut op = || {
            soundcloud::upload(
                audio_path,
                thumbnail_path,
                &metadata,
                &self.config.soundcloud_client_id,
                &self.access_token,
                &self.client,
            ).map_err(|err| {
                println!("Error: {}\nRetrying...", err);
                backoff::Error::Transient(err)
            })
        };
        let audio_id = backoff::Operation::retry(&mut op, &mut default_backoff()).unwrap();

        if let &Some(ref path) = thumbnail_path {
            println!("Cleaning up thumbnail file.");
            if let Err(err) = std::fs::remove_file(path) {
                println!("Error: {}\nFile will remain on disk.", err);
            }
        };

        println!("Cleaning up audio file.");
        if let Err(err) = std::fs::remove_file(audio_path) {
            println!("Error: {}\nFile will remain on disk.", err);
        };

        audio_id
    }

    fn add_to_playlist(&self, audio_id: u64, soundcloud_playlist_api_url: &str) {
        println!(
            "Adding uploaded audio track with id {} to Soundcloud playlist.",
            audio_id
        );
        let mut op = || {
            soundcloud::add_to_playlist(
                audio_id,
                soundcloud_playlist_api_url,
                &self.config.soundcloud_client_id,
                &self.access_token,
                &self.client,
            ).map_err(|err| {
                println!("Error: {}\nRetrying...", err);
                backoff::Error::Transient(err)
            })
        };
        backoff::Operation::retry(&mut op, &mut default_backoff()).unwrap();
    }

    fn resolve_soundcloud_playlist_url(&self, url: &str) -> Result<String, String> {
        println!("Resolving Soundcloud playlist url {}.", url);
        let mut op = || {
            soundcloud::playlist_url_to_api_url(&url, &self.config.soundcloud_client_id, &self.client).map_err(|err| {
                println!("Error: {}\nRetrying...", err);
                backoff::Error::Transient(err)
            })
        };
        match backoff::Operation::retry(&mut op, &mut default_backoff()).unwrap() {
            Some(url) => Ok(url),
            None => {
                return Err("The Soundcloud playlist url is not valid. \
                            Make sure you correctly set the full url in the config file."
                    .to_string());
            }
        }
    }

    fn run(&mut self) -> Result<(), String> {
        println!();
        for playlist in self.playlists.playlists.iter() {
            println!("Starting work on Youtube playlist with id: {}.", playlist.youtube);

            let soundcloud_playlist_api_url = self.resolve_soundcloud_playlist_url(&playlist.soundcloud)?;

            let mut url = youtube::make_playlist_items_url(&playlist.youtube, &self.config.youtube_api_key).unwrap();
            let previous_position = playlist.position.get();
            loop {
                let resource = self.get_youtube_playlist_data(url.clone())?;

                for video in resource
                    .items
                    .iter()
                    .filter(|x| x.snippet.position >= previous_position)
                {
                    let filename = App::download_audio(&video);

                    let thumbnail_path = self.download_thumbnail(&video);

                    let audio_id = self.upload_audio(&filename, &video, &thumbnail_path);

                    self.add_to_playlist(audio_id, &soundcloud_playlist_api_url);

                    // Save new playlist position
                    playlist.position.set(playlist.position.get() + 1);
                    self.playlists.write_safe()?;
                }
                match resource.nextPageToken {
                    Some(token) => {
                        url =
                            youtube::make_playlist_items_url(&playlist.youtube, &self.config.youtube_api_key).unwrap();
                        url.query_pairs_mut().append_pair("pageToken", &token);
                        continue;
                    }
                    None => {
                        break;
                    }
                };
            }
            println!("Done.\n");
        }
        Ok(())
    }
}

fn main() {
    let mut app = match App::new() {
        Ok(app) => app,
        Err(err) => {
            println!("Error: {}", err);
            return;
        }
    };

    if let Err(err) = app.run() {
        println!("Error: {}", err);
    }
}
