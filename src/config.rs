extern crate serde;
extern crate serde_json;

use std;

pub const PLAYLISTS_FILE: &str = "playlists.json";
pub const PLAYLISTS_BACKUP_FILE: &str = "playlists_backup.json";
pub const CONFIG_FILE: &str = "config.json";


#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub soundcloud_client_id: String,
    pub soundcloud_client_secret: String,
    pub soundcloud_username: String,
    pub soundcloud_password: String,
    pub youtube_api_key: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Playlists {
    pub playlists: Vec<Playlist>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Playlist {
    pub youtube: String,
    pub soundcloud: String,
    #[serde(default)]
    pub position: u64,
}

pub fn read_playlists() -> Result<Playlists, String> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .open(PLAYLISTS_FILE)
        .map_err(|err| format!("failed to open {}: {}", PLAYLISTS_FILE, err))?;
    serde_json::from_reader(file).map_err(|err| {
        format!("parsing of {} failed: {}", PLAYLISTS_FILE, err)
    })
}

pub fn write_playlists(playlists: &Playlists, path: &str) -> Result<(), String> {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|err| format!("opening {} failed: {}", path, err))?;
    serde_json::to_writer_pretty(file, playlists).map_err(|err| format!("writing to {} failed: {}", path, err))
}

pub fn write_playlists_safe(playlists: &Playlists) {
    std::fs::rename(PLAYLISTS_FILE, PLAYLISTS_BACKUP_FILE).expect("creation of playlists file backup failed");
    write_playlists(&playlists, PLAYLISTS_FILE).expect("failed to save new version of playlists file");
}

pub fn read_config() -> Result<Config, String> {
    let path = CONFIG_FILE;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|err| format!("failed to open {}: {}", path, err))?;
    serde_json::from_reader(file).map_err(|err| format!("parsing of {} failed: {}", path, err))
}
