use crate::{mod_update, server_launch};
use serde::Serialize;
use std::fs::{self, File};
use std::io::{ErrorKind, Read, Seek, SeekFrom};
use std::path::Path;
use std::time::SystemTime;

const MAX_READ: u64 = 256 * 1024;
const FIRST_READ: u64 = 64 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleChunk {
    file: Option<String>,
    offset: u64,
    reset: bool,
    lines: Vec<String>,
    problem: Option<String>,
}

impl ConsoleChunk {
    fn problem(problem: impl Into<String>) -> Self {
        ConsoleChunk { file: None, offset: 0, reset: true, lines: Vec::new(), problem: Some(problem.into()) }
    }
}

fn is_game_log(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.starts_with("sotf_log") && name.ends_with(".txt")
}

// LogOutput.N.log are the Loader's fallbacks when another process holds LogOutput.log.
fn is_loader_log(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name == "logoutput.log" || name.strip_prefix("logoutput.").and_then(|rest| rest.strip_suffix(".log"))
        .is_some_and(|slot| matches!(slot, "1" | "2" | "3" | "4"))
}

fn newest_log(directory: &Path, matches: fn(&str) -> bool) -> Result<Option<String>, String> {
    match fs::symlink_metadata(directory) {
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Could not inspect {}: {error}", directory.display())),
        Ok(_) => mod_update::validate_directory(directory)?,
    }
    let mut newest: Option<(SystemTime, String)> = None;
    for entry in fs::read_dir(directory).map_err(|error| format!("Could not list {}: {error}", directory.display()))? {
        let Ok(entry) = entry else { continue };
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else { continue };
        let Ok(metadata) = entry.metadata() else { continue };
        if !matches(&name) || !metadata.is_file() { continue; }
        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        if newest.as_ref().is_none_or(|(time, current)| (modified, &name) > (*time, current)) {
            newest = Some((modified, name));
        }
    }
    Ok(newest.map(|(_, name)| name))
}

/// Reads complete lines after `offset`, or the tail of the file when there is no usable offset.
fn read_lines(directory: &Path, name: &str, offset: Option<u64>) -> Result<ConsoleChunk, String> {
    if !mod_update::validate_target(directory, Path::new(name))? {
        return Ok(ConsoleChunk::problem("The log file was removed. Waiting for a new one."));
    }
    let mut file = File::open(directory.join(name)).map_err(|error| format!("Could not open {name}: {error}"))?;
    let length = file.metadata().map_err(|error| format!("Could not inspect {name}: {error}"))?.len();
    // A shorter file means the server started over with the same name.
    let (start, reset) = match offset {
        Some(offset) if offset <= length => (offset, false),
        _ => (length.saturating_sub(FIRST_READ), true),
    };
    file.seek(SeekFrom::Start(start)).map_err(|error| format!("Could not read {name}: {error}"))?;
    let mut bytes = Vec::new();
    file.take(MAX_READ).read_to_end(&mut bytes).map_err(|error| format!("Could not read {name}: {error}"))?;

    let first = if reset && start > 0 {
        bytes.iter().position(|byte| *byte == b'\n').map_or(bytes.len(), |index| index + 1)
    } else { 0 };
    // Leave a partial last line for the next read unless it alone fills the read.
    let end = match bytes.iter().rposition(|byte| *byte == b'\n') {
        Some(index) if index >= first => index + 1,
        _ if bytes.len() as u64 == MAX_READ => bytes.len(),
        _ => first,
    };
    Ok(ConsoleChunk {
        file: Some(name.to_owned()),
        offset: start + end as u64,
        reset,
        lines: String::from_utf8_lossy(&bytes[first..end]).lines().map(str::to_owned).collect(),
        problem: None,
    })
}

#[tauri::command]
pub async fn read_server_console(app: tauri::AppHandle, executable: String, installation_id: String,
    source: String, file: Option<String>, offset: Option<u64>) -> Result<ConsoleChunk, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (root, data) = server_launch::server_folders(&app, &executable, &installation_id)?;
        let (directory, matches, missing) = match source.as_str() {
            "game" => {
                let Some(data) = data else {
                    return Ok(ConsoleChunk::problem("Choose the server data folder to read its game log."));
                };
                (Path::new(&data).join("logs"), is_game_log as fn(&str) -> bool,
                    "The server has not written a game log yet. Turn on Log files under Server settings, then start the server.")
            }
            "loader" => (Path::new(&root).join("BepInEx"), is_loader_log as fn(&str) -> bool,
                "No Modstoke Loader log in this server folder. The Loader is not installed here, or has not run yet."),
            _ => return Err("Unknown console source.".into()),
        };
        let Some(newest) = newest_log(&directory, matches)? else { return Ok(ConsoleChunk::problem(missing)); };
        // The name is only compared against files listed here, never used as a path itself.
        let offset = offset.filter(|_| file.as_deref() == Some(newest.as_str()));
        read_lines(&directory, &newest, offset)
    }).await.map_err(|error| error.to_string())?
}
