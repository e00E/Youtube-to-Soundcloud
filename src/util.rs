pub fn path_to_str<T: AsRef<std::path::Path>>(path: T) -> String {
    path.as_ref().to_string_lossy().into_owned()
}

pub fn handle_status_code(response: reqwest::Response) -> Result<reqwest::Response, String> {
    if response.status().is_success() {
        Ok(response)
    } else {
        Err(format!(
            "response has bad status code: {}",
            response.status(),
        ))
    }
}

pub fn download_file<T: AsRef<std::path::Path>>(
    url: &str,
    path: T,
    client: &reqwest::Client,
) -> Result<(), String> {
    client
        .get(url)
        .send()
        .map_err(|err| format!("download file request {} failed: {}", url, err))
        .and_then(handle_status_code)
        .and_then(|mut response| {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open(path.as_ref())
                .map_err(|err| {
                    format!(
                        "failed to open {}: {}",
                        path.as_ref().to_str().unwrap_or("undisplayable"),
                        err
                    )
                })?;
            std::io::copy(&mut response, &mut file)
                .map_err(|err| {
                    format!(
                        "failed to write {} to file {}: {}",
                        url,
                        path_to_str(&path),
                        err
                    )
                })
                .map(|_| ())
        })
}
