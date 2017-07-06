extern crate reqwest;

use std;
use std::io::Read;

pub fn handle_status_code(mut response: reqwest::Response) -> Result<reqwest::Response, String> {
    if response.status().is_success() {
        Ok(response)
    } else {
        let mut body: String = String::new();
        let result = response.read_to_string(&mut body);
        Err(format!(
            "response has bad status code: {}, body: {}",
            response.status(),
            if result.is_ok() { body } else { body }
        ))
    }
}

pub fn download_file(url: &str, path: &str, client: &reqwest::Client) -> Result<(), String> {
    client
        .get(url)
        .send()
        .map_err(|err| {
            format!("download file request {} failed: {}", url, err)
        })
        .and_then(handle_status_code)
        .and_then(|mut response| {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open(path)
                .map_err(|err| format!("failed to open {}: {}", path, err))?;
            std::io::copy(&mut response, &mut file)
                .map_err(|err| {
                    format!("write of {} to file {} failed: {}", url, path, err)
                })
                .and_then(|_| Ok(()))
        })
}
