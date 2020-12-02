use crate::util;
use reqwest::{
    blocking::{multipart::Form, Client},
    StatusCode,
};
use serde::Deserialize;
use std::collections::HashMap;

const SOUNDCLOUD_API_TOKEN: &str = "https://api.soundcloud.com/oauth2/token";
const SOUNDCLOUD_API_RESOLVE: &str = "https://api.soundcloud.com/resolve.json";
const SOUNDCLOUD_API_UPLOAD: &str = "https://api.soundcloud.com/tracks";
const SOUNDCLOUD_API_ME: &str = "https://api.soundcloud.com/me";

#[derive(Debug, Deserialize)]
pub struct AuthenticateResponse {
    pub access_token: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadResponse {}

#[derive(Debug, Deserialize)]
pub struct ResolveResponse {
    location: String,
}

#[derive(Debug, Deserialize)]
pub struct PlaylistGetResponse {
    tracks: Vec<Track>,
}

#[derive(Debug, Deserialize)]
pub struct Track {
    id: u64,
}

pub fn authenticate(
    client_id: &str,
    client_secret: &str,
    username: &str,
    password: &str,
    request_client: &Client,
) -> Result<Option<AuthenticateResponse>, String> {
    let mut params = HashMap::new();
    params.insert("client_id", client_id);
    params.insert("client_secret", client_secret);
    params.insert("username", username);
    params.insert("password", password);
    params.insert("grant_type", "password");
    params.insert("scope", "non-expiring");
    let response = request_client
        .post(SOUNDCLOUD_API_TOKEN)
        .form(&params)
        .send()
        .map_err(|err| format!("failed to send authenticate request: {}", err))?;
    match response.status() {
        StatusCode::UNAUTHORIZED => Ok(None),
        other if other.is_success() => response
            .json()
            .map_err(|err| format!("failed to parse authenticate response: {}", err)),
        other => Err(format!("response has bad status code: {}", other)),
    }
}

pub fn is_token_valid(
    client_id: &str,
    access_token: &str,
    request_client: &Client,
) -> Result<bool, String> {
    let url = reqwest::Url::parse_with_params(
        SOUNDCLOUD_API_ME,
        &[("client_id", client_id), ("oauth_token", access_token)],
    )
    .expect("creation of me url failed");
    let response = request_client
        .get(url)
        .send()
        .map_err(|err| format!("failed to send resolve request: {}", err))?;
    match response.status() {
        StatusCode::UNAUTHORIZED => Ok(false),
        other if other.is_success() => Ok(true),
        other => Err(format!("response has bad status code: {}", other)),
    }
}

pub fn resolve(
    url: &str,
    client_id: &str,
    request_client: &Client,
) -> Result<Option<ResolveResponse>, String> {
    let url = reqwest::Url::parse_with_params(
        SOUNDCLOUD_API_RESOLVE,
        &[("url", url), ("client_id", client_id)],
    )
    .expect("creation of resolve url failed");
    let response = request_client
        .get(url)
        .send()
        .map_err(|err| format!("failed to send resolve request: {}", err))?;
    match response.status() {
        StatusCode::NOT_FOUND => Ok(None),
        StatusCode::FOUND => response
            .json()
            .map_err(|err| format!("failed to parse resolve response: {}", err)),
        other => Err(format!("response has bad status code: {}", other)),
    }
}

pub fn playlist_url_to_api_url(
    url: &str,
    client_id: &str,
    request_client: &Client,
) -> Result<Option<String>, String> {
    resolve(url, client_id, request_client)
        .map(|response| response.map(|response| response.location))
}

pub fn get_tracks(
    playlist_api_url: &str,
    client_id: &str,
    request_client: &Client,
) -> Result<PlaylistGetResponse, String> {
    let url = reqwest::Url::parse_with_params(
        playlist_api_url,
        &[("client_id", client_id), ("representation", "id")],
    )
    .expect("creation of playlist url failed");
    request_client
        .get(url)
        .send()
        .map_err(|err| format!("failed to send get tracks request: {}", err))
        .and_then(util::handle_status_code)?
        .json()
        .map_err(|err| format!("failed to parse of get tracks response: {}", err))
}

pub fn add_to_playlist(
    track_id: u64,
    playlist_api_url: &str,
    client_id: &str,
    access_token: &str,
    request_client: &Client,
) -> Result<(), String> {
    let previous_tracks = get_tracks(playlist_api_url, client_id, request_client)?.tracks;
    let track_id = format!("{}", track_id);
    let mut params = vec![
        ("client_id", client_id.to_string()),
        ("oauth_token", access_token.to_string()),
        ("representation", "compact".to_string()),
    ];
    for track in previous_tracks.iter() {
        params.push(("playlist[tracks][][id]", track.id.to_string()));
    }
    params.push(("playlist[tracks][][id]", track_id));
    request_client
        .put(playlist_api_url)
        .form(&params)
        .send()
        .map_err(|err| format!("failed to send playlist put request: {}", err))
        .and_then(util::handle_status_code)
        .map(|_| ())
}

pub fn upload<T: AsRef<std::path::Path>, U: AsRef<std::path::Path>>(
    file_path: T,
    artwork_path: &Option<U>,
    metadata: &HashMap<&str, &str>,
    client_id: &str,
    access_token: &str,
    request_client: &Client,
) -> Result<u64, String> {
    let mut params = Form::new()
        .text("client_id", client_id.to_string())
        .text("oauth_token", access_token.to_string());
    for (key, value) in metadata {
        params = params.text(format!("track[{}]", key), value.to_string());
    }
    // Not being able to access the specified files is a panic because the caller should have made
    // sure that they exist
    params = params
        .file("track[asset_data]", &file_path)
        .map_err(|err| {
            format!(
                "failed to open audio file {}: {}",
                util::path_to_str(&file_path),
                err
            )
        })
        .unwrap();
    if let Some(artwork_path) = artwork_path {
        params = params
            .file("track[artwork_data]", artwork_path)
            .map_err(|err| {
                format!(
                    "failed to open artwork file {}: {}",
                    util::path_to_str(artwork_path),
                    err
                )
            })
            .unwrap();
    }
    let track: Track = request_client
        .post(SOUNDCLOUD_API_UPLOAD)
        .multipart(params)
        .send()
        .map_err(|err| format!("failed to send upload request: {}", err))
        .and_then(util::handle_status_code)?
        .json()
        .map_err(|err| format!("failed to parse upload response: {}", err))?;
    Ok(track.id)
}
