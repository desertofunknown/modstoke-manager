use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{io::Write, time::Duration};
use tauri::{ipc::Channel, Manager};

const MAX_DOWNLOAD_BYTES: u64 = 512 * 1024 * 1024;
// The SOTF Mods catalog, the Loader's and UnityExplorer's GitHub releases, and the storage both redirect to.
const SOURCES: [&str; 7] = [
    "https://sotf-mods.com/mods/",
    "https://r2.sotf-mods.com/",
    "https://modstoke.com/releases/",
    "https://github.com/desertofunknown/",
    "https://github.com/ToniMacaroni/UnityExplorer_Sons/releases/",
    "https://release-assets.githubusercontent.com/",
    "https://objects.githubusercontent.com/",
];

#[derive(Clone, Serialize)]
pub struct DownloadProgress {
    received: u64,
    total: Option<u64>,
}

// The URL crate has already resolved dot segments and lowercased the host, so a prefix check holds.
fn trusted(url: &reqwest::Url) -> bool {
    SOURCES.iter().any(|source| url.as_str().starts_with(source))
}

fn too_large() -> String {
    "The download is larger than the Manager accepts.".into()
}

// Saves an archive to the downloads folder and returns its path; the caller removes it after installing.
// When the publisher lists a SHA-256, it is checked here against the same bytes that get installed.
#[tauri::command]
pub async fn download_to_cache(
    app: tauri::AppHandle,
    url: String,
    sha256: Option<String>,
    on_progress: Channel<DownloadProgress>,
) -> Result<String, String> {
    let url = reqwest::Url::parse(&url).map_err(|_| "The download link is not valid.")?;
    if !trusted(&url) {
        return Err(format!("Downloads from {} are not allowed.", url.host_str().unwrap_or("this address")));
    }
    let client = reqwest::Client::builder()
        .user_agent(concat!("Modstoke-Manager/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 10 {
                attempt.error("too many redirects")
            } else if trusted(attempt.url()) {
                attempt.follow()
            } else {
                let host = attempt.url().host_str().unwrap_or("an unknown address").to_owned();
                attempt.error(format!("redirected to {host}, which is not an allowed download source"))
            }
        }))
        .build()
        .map_err(|error| error.to_string())?;
    let mut response = client.get(url).send().await.map_err(|error| format!("The download failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("The download failed: {}", response.status()));
    }
    let total = response.content_length();
    if total.is_some_and(|size| size > MAX_DOWNLOAD_BYTES) {
        return Err(too_large());
    }

    // On Windows the app cache is also where the webview keeps its profile, so the webview only
    // gets this one subfolder.
    let folder = app.path().app_cache_dir().map_err(|error| error.to_string())?.join("downloads");
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    let mut file = tempfile::Builder::new()
        .prefix("download-")
        .suffix(".tmp")
        .tempfile_in(&folder)
        .map_err(|error| format!("Could not create a download file: {error}"))?;
    let mut hasher = Sha256::new();
    let (mut received, mut reported) = (0u64, 0u64);
    while let Some(chunk) = response.chunk().await.map_err(|error| format!("The download failed: {error}"))? {
        received += chunk.len() as u64;
        if received > MAX_DOWNLOAD_BYTES {
            return Err(too_large());
        }
        hasher.update(&chunk);
        file.write_all(&chunk).map_err(|error| format!("Could not save the download: {error}"))?;
        if received - reported >= 256 * 1024 {
            reported = received;
            let _ = on_progress.send(DownloadProgress { received, total });
        }
    }
    let _ = on_progress.send(DownloadProgress { received, total });

    if let Some(expected) = sha256 {
        let actual = format!("{:x}", hasher.finalize());
        if !actual.eq_ignore_ascii_case(expected.trim()) {
            return Err(format!("The download does not match its published hash. Expected {expected}, got {actual}."));
        }
    }
    file.as_file().sync_all().map_err(|error| format!("Could not save the download: {error}"))?;
    let (_, path) = file.keep().map_err(|error| format!("Could not save the download: {error}"))?;
    Ok(path.to_string_lossy().into_owned())
}
