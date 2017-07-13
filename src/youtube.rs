extern crate reqwest;

use std;

pub const YOUTUBE_API_PLAYLIST_ITEMS: &str = "https://www.googleapis.com/youtube/v3/playlistItems";

pub fn make_playlist_items_url(id: &str, youtube_api_key: &str) -> Result<reqwest::Url, reqwest::UrlError> {
    reqwest::Url::parse_with_params(
        YOUTUBE_API_PLAYLIST_ITEMS,
        &[
            ("key", youtube_api_key),
            ("playlistId", id),
            ("maxResults", "50"),
            ("part", "contentDetails,snippet"),
        ],
    )
}

pub fn download_audio(video_id: &str) -> Result<String, String> {
    let output = std::process::Command::new("youtube-dl")
        .arg(format!("https://youtube.com/watch?v={}", video_id))
        .args(&["-f", "bestaudio"])
        .arg("--restrict-filenames")
        .output()
        .expect("execution of youtube-dl failed");
    if !output.status.success() {
        return Err("youtube-dl did not signal success".to_string());
    };
    let stdout = std::str::from_utf8(&output.stdout).map_err(|err| {
        format!("parsing of youtube-dl output failed: {}", err)
    })?;

    let end = " has already been downloaded";
    let start = "[download] ";
    for line in stdout.split("\n") {
        if line.starts_with(start) && line.ends_with(end) {
            return Ok(line[start.len()..line.len() - end.len()].to_string());
        }
    }

    let target = "[download] Destination: ";
    let start = stdout.find(target).ok_or(format!(
        "youtube-dl failed to download audio"
    ))? + target.len();
    let end = stdout[start..].find("\n").unwrap();
    Ok(stdout[start..start + end].to_string())
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct PlaylistItemsResource {
    pub nextPageToken: Option<String>,
    pub pageInfo: PageInfo,
    pub items: Vec<PlaylistItem>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct PageInfo {
    pub totalResults: u64,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct PlaylistItem {
    pub contentDetails: ContentDetails,
    pub snippet: Snippet,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct ContentDetails {
    pub videoId: String,
    pub videoPublishedAt: String,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct Snippet {
    pub title: String,
    pub description: String,
    pub position: u64,
    pub publishedAt: String,
    pub thumbnails: Thumbnails,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct Thumbnails {
    pub default: Thumbnail,
    pub medium: Option<Thumbnail>,
    pub high: Option<Thumbnail>,
    pub standard: Option<Thumbnail>,
    pub maxres: Option<Thumbnail>,
}

impl Thumbnails {
    pub fn get_best_thumbnail(&self) -> &Thumbnail {
        let thumbnails = &[&self.maxres, &self.standard, &self.high, &self.medium];
        for i in thumbnails.iter() {
            match *i {
                &Some(ref t) => return t,
                &None => continue,
            }
        }
        &self.default
    }
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct Thumbnail {
    pub url: String,
}
