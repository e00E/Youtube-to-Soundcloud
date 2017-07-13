extern crate reqwest;
#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;
extern crate backoff;
extern crate chrono;

use std::str::FromStr;
use chrono::Datelike;

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

fn main() {
    let mut config = config::read_config().expect("reading configuration file failed");
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
            let mut op = || soundcloud::is_token_valid(&config.soundcloud_client_id, access_token, &client)
                .map_err(|err| {
                    println!("Error: {}\nRetrying...", err);
                    backoff::Error::Transient(err)
                });
            !backoff::Operation::retry(&mut op, &mut default_backoff()).unwrap()
        },
        None => true
    };
    let access_token = if need_new_token {
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
        Some(backoff::Operation::retry(&mut op, &mut default_backoff()).unwrap().access_token)
    } else {
        None
    };
    let access_token = match access_token {
        Some(access_token) => {
            config.soundcloud_access_token = Some(access_token.clone());
            access_token
        },
        None => config.soundcloud_access_token.clone().unwrap()
    };
    config::write_config_safe(&config).expect("failed to write updated config file");

    // Load playlists
    let mut playlists = config::read_playlists().expect("reading playlists file failed");

    println!("Checking youtube playlists for new videos.");
    for playlist in playlists.playlists.iter_mut() {
        println!(
            "Starting work on Youtube playlist with id: {}.",
            playlist.youtube
        );

        let soundcloud_playlist_api_url =
            soundcloud::playlist_url_to_api_url(&playlist.soundcloud, &config.soundcloud_client_id, &client).expect(
                format!(
                    "turning Soundcloud playlist url {} into api url failed.",
                    playlist.soundcloud
                ).as_str(),
            );


        let url = std::cell::RefCell::new(
            youtube::make_playlist_items_url(&playlist.youtube, &config.youtube_api_key)
                .expect("creation of youtube playlist url failed"),
        );
        let previous_position = playlist.position;
        let mut op = || {
            client
                .get(url.borrow().clone())
                .unwrap()
                .send()
                .map_err(|err| format!("sending request failed: {}", err))
                .and_then(util::handle_status_code)
                .and_then(|mut response| {
                    let body: Result<youtube::PlaylistItemsResource, String> = response.json().map_err(|err| {
                        format!("parsing response failed: {}", err)
                    });
                    body
                })
                .and_then(|resource| {
                    let mut count: u64 = 0;
                    for video in resource.items.iter().filter(|x| {
                        x.snippet.position > previous_position
                    })
                    {
                        count += 1;

                        println!(
                            "Downloading new video with id {} and title {}.",
                            &video.contentDetails.videoId,
                            &video.snippet.title,
                        );
                        let mut op = || {
                            youtube::download_audio(&video.contentDetails.videoId).map_err(|err| {
                                println!("Error: {}\nRetrying...", err);
                                backoff::Error::Transient(err)
                            })
                        };
                        let filename = backoff::Operation::retry(&mut op, &mut default_backoff()).unwrap();

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
                                util::download_file(url, &path, &client)
                                    .map_err(|err| {
                                        println!("Error: {}\nRetrying...", err);
                                        backoff::Error::Transient(err)
                                    })
                                    .and_then(|_| Ok(path))
                            } else {
                                Err(backoff::Error::Permanent(
                                    format!("thumbnail has illegal extension {}", extension),
                                ))
                            }
                        };
                        let thumbnail_path = match backoff::Operation::retry(&mut op, &mut default_backoff()) {
                            Ok(path) => Some(path),
                            Err(err) => {
                                println!(
                                    "Failed to retrieve thumbnail, it will not be uploaded to Soundcloud: {}.",
                                    err
                                );
                                None
                            }
                        };

                        println!("Uploading {} to Soundcloud.", &filename);
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
                            Err(err) => {
                                println!(
                                    "Parsing of timedate string {} failed, \
                                     release date will not be set on Soundcloud: {}.",
                                    &video.snippet.publishedAt,
                                    err
                                )
                            }
                        }
                        metadata.insert("downloadable", "1");
                        let mut op = || {
                            soundcloud::upload(
                                &filename,
                                thumbnail_path.as_ref(),
                                &metadata,
                                &config.soundcloud_client_id,
                                &access_token,
                                &client,
                            ).map_err(|err| {
                                println!("Error: {}\nRetrying...", err);
                                backoff::Error::Transient(err)
                            })
                        };
                        let audio_id = backoff::Operation::retry(&mut op, &mut default_backoff()).unwrap();

                        if let Some(ref path) = thumbnail_path {
                            println!("Cleaning up thumbnail file.");
                            if let Err(err) = std::fs::remove_file(path) {
                                println!("Error: {}\nFile will remain on disk.", err);
                            }
                        };

                        println!("Cleaning up audio file.");
                        if let Err(err) = std::fs::remove_file(&filename) {
                            println!("Error: {}\nFile will remain on disk.", err);
                        };

                        println!(
                            "Adding uploaded audio track with id {} to Soundcloud playlist.",
                            audio_id
                        );
                        let mut op = || {
                            soundcloud::add_to_playlist(
                                audio_id,
                                &soundcloud_playlist_api_url,
                                &config.soundcloud_client_id,
                                &access_token,
                                &client,
                            ).map_err(|err| {
                                println!("Error: {}\nRetrying...", err);
                                backoff::Error::Transient(err)
                            })
                        };
                        backoff::Operation::retry(&mut op, &mut default_backoff()).unwrap();
                    }
                    Ok((resource.nextPageToken, count))
                })
                .map_err(|err| {
                    println!(
                        "Receiving some of the youtube playlist data failed: {}\nRetrying...",
                        err
                    );
                    backoff::Error::Transient(err)
                })
        };
        loop {
            match backoff::Operation::retry(&mut op, &mut default_backoff()).unwrap() {
                (None, count) => {
                    playlist.position += count;
                    break;
                }
                (Some(token), count) => {
                    playlist.position += count;
                    let mut new_url = youtube::make_playlist_items_url(&playlist.youtube, &config.youtube_api_key)
                        .expect("creation youtube playlist url failed");
                    new_url.query_pairs_mut().append_pair("pageToken", &token);
                    *url.borrow_mut() = new_url;
                    continue;
                }
            }
        }
        println!();
    }
    config::write_playlists_safe(&playlists).expect("failed to write updated playlists file");
}
