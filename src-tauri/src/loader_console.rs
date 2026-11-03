use crate::{installation_profiles, native_root, safe_fs};
use serde::{Deserialize, Deserializer, Serialize};
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// Modstoke Loader publishes its console pipe in Modstoke/UserData, or OpenACAI/UserData before the
// rename, while the game or server runs. The file can outlive a crash, so the process it names is
// checked before anything is sent.
const ENDPOINT_FILE: &str = "UserData/console-endpoint.json";
const PIPE_PREFIX: &str = "modstoke-console-";
const MAX_REQUEST_BYTES: usize = 16 * 1024;
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_COMMAND: usize = 256;
const MAX_ARGUMENTS: usize = 1024;
const PAGE: u32 = 256;
const FIRST_PAGE: i64 = 200;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Endpoint {
    schema: u32,
    pipe: String,
    process_id: u32,
    started_utc: String,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct Request<'a> {
    operation: &'a str,
    after: i64,
    limit: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    confirmed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    arguments: Option<&'a str>,
}

// The Loader writes System.Text.Json defaults: PascalCase names, null for missing text.
fn text<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Option::<String>::deserialize(deserializer).map(Option::unwrap_or_default)
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default, rename_all(deserialize = "PascalCase", serialize = "camelCase"))]
pub struct Snapshot {
    #[serde(deserialize_with = "text")]
    session_id: String,
    #[serde(deserialize_with = "text")]
    phase: String,
    #[serde(deserialize_with = "text")]
    summary: String,
    ready: bool,
    first_id: i64,
    last_id: i64,
    dropped: i64,
    entries: Vec<Entry>,
    commands: Vec<Command>,
    mods: Vec<String>,
    accepted: bool,
    detail: Option<String>,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default, rename_all(deserialize = "PascalCase", serialize = "camelCase"))]
struct Entry {
    id: i64,
    #[serde(deserialize_with = "text")]
    timestamp: String,
    #[serde(deserialize_with = "text")]
    level: String,
    #[serde(deserialize_with = "text")]
    source: String,
    #[serde(deserialize_with = "text")]
    message: String,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default, rename_all(deserialize = "PascalCase", serialize = "camelCase"))]
struct Command {
    #[serde(deserialize_with = "text")]
    name: String,
    #[serde(deserialize_with = "text")]
    description: String,
    enabled: bool,
    requires_confirmation: bool,
    accepts_arguments: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleRead {
    process_id: Option<u32>,
    problem: Option<String>,
    snapshot: Option<Snapshot>,
}

#[tauri::command]
pub async fn read_loader_console(executable: String, installation_id: String, after: Option<i64>) -> Result<ConsoleRead, String> {
    let (pipe, process_id) = match live_endpoint(executable, installation_id).await? {
        Ok(live) => live,
        Err(problem) => return Ok(ConsoleRead { process_id: None, problem: Some(problem), snapshot: None }),
    };
    let read = |after| {
        let pipe = pipe.clone();
        async move {
            exchange(&pipe, &Request { operation: "read", after, limit: PAGE, command: None, confirmed: None, arguments: None })
                .await
                .map_err(Failure::into_message)
        }
    };
    let snapshot = match after {
        Some(after) => read(after).await?,
        // Start from the most recent entries rather than the oldest the Loader still keeps.
        None => {
            let first = read(0).await?;
            read((first.last_id - FIRST_PAGE).max(first.first_id - 1).max(0)).await?
        }
    };
    Ok(ConsoleRead { process_id: Some(process_id), problem: None, snapshot: Some(snapshot) })
}

#[tauri::command]
pub async fn submit_loader_console(executable: String, installation_id: String, process_id: u32,
    command: String, arguments: String, confirmed: bool, after: i64) -> Result<Snapshot, String> {
    let name = command.trim();
    if name.is_empty() || name.chars().any(|character| character.is_whitespace() || character.is_control()) {
        return Err("Choose a command from the list, and put anything after its name under Arguments.".into());
    }
    if name.encode_utf16().count() > MAX_COMMAND {
        return Err("Command names are at most 256 characters.".into());
    }
    if arguments.encode_utf16().count() > MAX_ARGUMENTS || arguments.chars().any(char::is_control) {
        return Err("Arguments must be one line of up to 1024 characters.".into());
    }
    let (pipe, live) = live_endpoint(executable, installation_id).await??;
    if live != process_id {
        return Err("The game or server restarted since the console was read. Check the command and send it again.".into());
    }
    let arguments = arguments.trim();
    exchange(&pipe, &Request {
        operation: "submit", after, limit: PAGE, command: Some(name), confirmed: Some(confirmed),
        arguments: (!arguments.is_empty()).then_some(arguments),
    })
    .await
    .map_err(|failure| match failure {
        Failure::Unsent(message) => message,
        Failure::Sent(message) => format!("{message} The command may already be queued; check the output before sending it again."),
    })
}

/// The live pipe and process, or `Ok(Err(reason))` when there is no console to talk to.
async fn live_endpoint(executable: String, installation_id: String) -> Result<Result<(String, u32), String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = require_installation(&executable, &installation_id)?;
        Ok(endpoint(Path::new(&root), &executable))
    })
    .await
    .map_err(|error| format!("Could not find the Loader console: {error}"))?
}

fn require_installation(executable: &str, installation_id: &str) -> Result<String, String> {
    let selected = installation_profiles::require_selected(executable, installation_id, None,
        "The installation is unavailable.",
        "The installation no longer matches its saved identity. Select it again.")?;
    selected.root.ok_or_else(|| "The installation has no resolved folder.".into())
}

fn valid_pipe(pipe: &str, process_id: u32) -> bool {
    let Some(rest) = pipe.strip_prefix(PIPE_PREFIX) else { return false };
    let Some((pid, guid)) = rest.split_once('-') else { return false };
    pid == process_id.to_string()
        && guid.len() == 32
        && guid.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

// Seconds since the Unix epoch for an ISO-8601 time such as 2026-11-02T18:00:00.1234567+00:00.
fn unix_seconds(value: &str) -> Option<i64> {
    let (date, time) = value.split_once('T')?;
    let mut date_parts = date.splitn(3, '-').map(|part| part.parse::<i64>().ok());
    let (year, month, day) = (date_parts.next()??, date_parts.next()??, date_parts.next()??);
    let (clock, offset) = if let Some(clock) = time.strip_suffix('Z') {
        (clock, 0)
    } else {
        let split = time.rfind(['+', '-'])?;
        let (hours, minutes) = time[split + 1..].split_once(':')?;
        let sign = if time.as_bytes()[split] == b'-' { -1 } else { 1 };
        (&time[..split], sign * (hours.parse::<i64>().ok()? * 3600 + minutes.parse::<i64>().ok()? * 60))
    };
    let mut clock_parts = clock.split(':');
    let hour = clock_parts.next()?.parse::<i64>().ok()?;
    let minute = clock_parts.next()?.parse::<i64>().ok()?;
    let second = clock_parts.next()?.split('.').next()?.parse::<i64>().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    // Days from the civil date, after Howard Hinnant's algorithm.
    let shifted = if month <= 2 { year - 1 } else { year };
    let era = shifted.div_euclid(400);
    let year_of_era = shifted - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146097 + day_of_era - 719468;
    Some(days * 86400 + hour * 3600 + minute * 60 + second - offset)
}

/// The pipe and process of a live console for this installation, or why there is none.
fn endpoint(root: &Path, executable: &str) -> Result<(String, u32), String> {
    let offline = "The Loader console is available while the game or server runs with Modstoke Loader.";
    let mut failure = None;
    for name in native_root::ROOTS {
        let path = format!("{name}/{ENDPOINT_FILE}");
        let Some(bytes) = safe_fs::read_bounded(root, Path::new(&path), 4096)? else {
            continue;
        };
        match read_endpoint(&bytes, executable, offline) {
            Ok(found) => return Ok(found),
            Err(error) => {
                failure.get_or_insert(error);
            }
        }
    }
    Err(failure.unwrap_or_else(|| offline.into()))
}

fn read_endpoint(bytes: &[u8], executable: &str, offline: &str) -> Result<(String, u32), String> {
    let endpoint: Endpoint = serde_json::from_slice(bytes)
        .map_err(|error| format!("console-endpoint.json could not be read: {error}"))?;
    if endpoint.schema != 1 {
        return Err("console-endpoint.json uses a version this Manager does not know. Update Manager to use the console.".into());
    }
    if !valid_pipe(&endpoint.pipe, endpoint.process_id) {
        return Err("console-endpoint.json does not name a Modstoke console pipe.".into());
    }
    let started = unix_seconds(&endpoint.started_utc)
        .ok_or("console-endpoint.json has an unreadable start time.")?;
    let (image, created) = process(endpoint.process_id).ok_or(offline)?;
    let same_executable = match (std::fs::canonicalize(&image), std::fs::canonicalize(executable)) {
        (Ok(running), Ok(expected)) => running.to_string_lossy().eq_ignore_ascii_case(&expected.to_string_lossy()),
        _ => false,
    };
    // A leftover file whose process ID now belongs to another program, or to a later start.
    if !same_executable || started < created - 2 || started > created + 600 {
        return Err(offline.into());
    }
    Ok((endpoint.pipe, endpoint.process_id))
}

#[cfg(windows)]
fn process(process_id: u32) -> Option<(String, i64)> {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
    use windows::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    struct ProcessHandle(HANDLE);
    impl Drop for ProcessHandle {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0); }
        }
    }
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) };
    if handle.is_invalid() {
        return None;
    }
    let handle = ProcessHandle(handle);
    let mut times = [FILETIME::default(); 4];
    let [created, exited, kernel, user] = &mut times;
    // An exited process stays openable while something still holds a handle to it.
    if !unsafe { GetProcessTimes(handle.0, created, exited, kernel, user) }.as_bool()
        || times[1].dwHighDateTime != 0 || times[1].dwLowDateTime != 0 {
        return None;
    }
    let mut name = vec![0u16; 32768];
    let mut length = name.len() as u32;
    if !unsafe { QueryFullProcessImageNameW(handle.0, PROCESS_NAME_WIN32, PWSTR(name.as_mut_ptr()), &mut length) }.as_bool() {
        return None;
    }
    let ticks = (u64::from(times[0].dwHighDateTime) << 32) | u64::from(times[0].dwLowDateTime);
    Some((String::from_utf16_lossy(&name[..length as usize]), (ticks / 10_000_000) as i64 - 11_644_473_600))
}

#[cfg(not(windows))]
fn process(_process_id: u32) -> Option<(String, i64)> {
    None
}

/// Whether a failed request may still have reached the Loader, which matters for a command.
enum Failure {
    Unsent(String),
    Sent(String),
}

impl Failure {
    fn into_message(self) -> String {
        match self {
            Failure::Unsent(message) | Failure::Sent(message) => message,
        }
    }
}

// One request per connection: a 4-byte little-endian length, then UTF-8 JSON, each way. The
// Loader's own 5 s deadline can't fire while the game is suspended, so this one bounds the wait.
#[cfg(windows)]
async fn exchange(pipe: &str, request: &Request<'_>) -> Result<Snapshot, Failure> {
    use tokio::net::windows::named_pipe::ClientOptions;

    let body = serde_json::to_vec(request).map_err(|error| Failure::Unsent(error.to_string()))?;
    if body.len() > MAX_REQUEST_BYTES {
        return Err(Failure::Unsent("The command is too long for the Loader console.".into()));
    }
    let path = format!(r"\\.\pipe\{pipe}");
    let mut written = false;
    let work = async {
        let mut attempt = 0;
        let mut stream = loop {
            match ClientOptions::new().open(&path) {
                Ok(stream) => break stream,
                // ERROR_PIPE_BUSY: other clients hold the pipe's instances for a moment.
                Err(error) if error.raw_os_error() == Some(231) && attempt < 20 => {
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(error) => return Err(Failure::Unsent(format!("Could not connect to the Loader console: {error}"))),
            }
        };
        let mut frame = (body.len() as u32).to_le_bytes().to_vec();
        frame.extend_from_slice(&body);
        written = true;
        stream.write_all(&frame).await
            .map_err(|error| Failure::Sent(format!("Could not send to the Loader console: {error}")))?;
        let mut length = [0u8; 4];
        stream.read_exact(&mut length).await
            .map_err(|error| Failure::Sent(format!("The Loader console did not answer: {error}")))?;
        let length = u32::from_le_bytes(length) as usize;
        if length > MAX_RESPONSE_BYTES {
            return Err(Failure::Sent("The Loader console sent an answer larger than 8 MiB.".into()));
        }
        let mut bytes = vec![0; length];
        stream.read_exact(&mut bytes).await
            .map_err(|error| Failure::Sent(format!("The Loader console's answer was cut off: {error}")))?;
        Ok(bytes)
    };
    let result = tokio::time::timeout(Duration::from_secs(6), work).await;
    let bytes = match result {
        Ok(result) => result?,
        Err(_) if written => return Err(Failure::Sent("The Loader console did not answer within 6 seconds.".into())),
        Err(_) => return Err(Failure::Unsent("The Loader console did not accept a connection within 6 seconds.".into())),
    };
    serde_json::from_slice(&bytes)
        .map_err(|error| Failure::Sent(format!("The Loader console's answer could not be read: {error}")))
}

#[cfg(not(windows))]
async fn exchange(_pipe: &str, _request: &Request<'_>) -> Result<Snapshot, Failure> {
    Err(Failure::Unsent("The Loader console is only available on Windows.".into()))
}
