extern crate reqwest;
extern crate cpython;

use std::collections::HashMap;
use cpython::{Python, PyDict, PyTuple, ObjectProtocol, ToPyObject, PythonObject};
use util;

const SOUNDCLOUD_API_TOKEN: &str = "https://api.soundcloud.com/oauth2/token";
const SOUNDCLOUD_API_RESOLVE: &str = "https://api.soundcloud.com/resolve.json";

#[derive(Debug, Deserialize)]
pub struct AuthenticateResponse {
    pub access_token: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadResponse {}

// TODO: handle refreshing tokens? or just get new token each upload? need to see what error
// happens when token is too old


#[derive(Debug, Deserialize)]
pub struct ResolveResponse {
    status: String,
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
    request_client: &reqwest::Client,
) -> Result<AuthenticateResponse, String> {
    let mut params = HashMap::new();
    params.insert("client_id", client_id);
    params.insert("client_secret", client_secret);
    params.insert("username", username);
    params.insert("password", password);
    params.insert("grant_type", "password");
    params.insert("scope", "non-expiring");
    request_client
        .post(SOUNDCLOUD_API_TOKEN)
        .form(&params)
        .send()
        .map_err(|err| format!("authenticate request failed: {}", err))
        .and_then(util::handle_status_code)
        .and_then(|mut response| {
            let body: Result<AuthenticateResponse, String> = response.json().map_err(|err| {
                format!("parsing of authenticate response failed: {}", err)
            });
            body
        })
}

pub fn resolve(url: &str, client_id: &str, request_client: &reqwest::Client) -> Result<ResolveResponse, String> {
    let url = reqwest::Url::parse_with_params(
        SOUNDCLOUD_API_RESOLVE,
        &[("url", url), ("client_id", client_id)],
    ).expect("creation of resolve url failed");
    request_client
        .get(url)
        .send()
        .map_err(|err| format!("resolve request failed: {}", err))
        .and_then(|mut response| {
            let body: Result<ResolveResponse, String> = response
                .json()
                .map_err(|err| format!("parsing of resolve response failed: {}", err));
            body
        })
}

pub fn playlist_url_to_api_url(url: &str, client_id: &str, request_client: &reqwest::Client) -> Result<String, String> {
    resolve(url, client_id, request_client).and_then(|response| if response.status == "302 - Found" {
        Ok(response.location)
    } else {
        Err(format!(
            "resolve response status is not 302 but {}",
            response.status
        ))
    })
}

pub fn get_tracks(
    playlist_api_url: &str,
    client_id: &str,
    request_client: &reqwest::Client,
) -> Result<PlaylistGetResponse, String> {
    let url = reqwest::Url::parse_with_params(
        playlist_api_url,
        &[("client_id", client_id), ("representation", "id")],
    ).expect("creation of playlist url failed");
    request_client
        .get(url)
        .send()
        .map_err(|err| format!("get tracks request failed: {}", err))
        .and_then(|mut response| {
            let body: Result<PlaylistGetResponse, String> = response.json().map_err(|err| {
                format!("parsing of get tracks response failed: {}", err)
            });
            body
        })
}

pub fn add_to_playlist(
    track_id: u64,
    playlist_api_url: &str,
    client_id: &str,
    access_token: &str,
    request_client: &reqwest::Client,
) -> Result<(), String> {
    let previous_tracks = get_tracks(playlist_api_url, client_id, request_client)?
        .tracks;
    let track_id = format!("{}", track_id);
    let mut params = vec![
        ("client_id", client_id.to_string()),
        ("oauth_token", access_token.to_string()),
        ("representation", "compact".to_string()),
    ];
    for track in previous_tracks.iter() {
        params.push(("playlist[tracks][][id]", track.id.to_string()));
    }
    params.push(("playlist[tracks][][id]", track_id.to_string()));
    request_client
        .put(playlist_api_url)
        .form(&params)
        .send()
        .map_err(|err| format!("playlist put request failed: {}", err))
        .and_then(util::handle_status_code)
        .and_then(|_| Ok(()))
}

pub fn upload(
    file_path: &str,
    artwork_path: Option<&str>,
    metadata: &HashMap<&str, &str>,
    access_token: &str,
) -> Result<u64, String> {
    let gil = Python::acquire_gil();
    let py = gil.python();
    let soundcloud = py.import("soundcloud")
        .expect("import of python soundcloud module failed");
    let kwargs = PyDict::new(py);

    kwargs.set_item(py, "access_token", access_token).unwrap();
    let client = soundcloud
        .call(py, "Client", PyTuple::empty(py), Some(&kwargs))
        .unwrap();

    let kwargs = PyDict::new(py);
    let track = PyDict::new(py);
    for (key, value) in metadata.iter() {
        track.set_item(py, key, value).unwrap();
    }
    let file = py.eval(&format!("open(\"{}\", \"rb\")", file_path), None, None)
        .expect(&format!("failed to open file {}", file_path));
    track.set_item(py, "asset_data", file).unwrap();
    match artwork_path {
        Some(path) => {
            let file = py.eval(&format!("open(\"{}\", \"rb\")", path), None, None)
                .expect(&format!("failed to open file {}", path));
            track.set_item(py, "artwork_data", file).unwrap();
        }
        _ => (),
    }
    kwargs.set_item(py, "track", track).unwrap();

    let track = client
        .call_method(
            py,
            "post",
            PyTuple::new(py, &["/tracks".to_py_object(py).into_object()]),
            Some(&kwargs),
        )
        .map_err(|err| {
            format!("Upload of file {} failed: {:?}", file_path, err)
        })?;
    let id: u64 = track.getattr(py, "id").unwrap().extract(py).unwrap();
    Ok(id)
}
