use crate::{installation_profiles, mod_update};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{ErrorKind, Read, Write};
use std::ops::Range;
use std::path::Path;

const CONFIG_PATH: &str = "BepInEx/config/modstoke.client.cfg";
const MANIFEST_PATH: &str = "OpenACAI/Mods/Modstoke.Client/openacai.mod.json";
const ASSEMBLY_PATH: &str = "OpenACAI/Mods/Modstoke.Client/Modstoke.Client.dll";
const MAX_CONFIG_BYTES: usize = 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientAdmissionState {
    pub installation_id: String,
    pub root: String,
    pub config_path: String,
    pub state: String,
    pub enabled: Option<bool>,
    pub revision: Option<String>,
    pub config_problem: Option<String>,
    pub client_available: bool,
    pub client_version: Option<String>,
    pub client_problem: Option<String>,
    pub process_state: String,
    pub process_problem: Option<String>,
    pub can_write: bool,
    pub write_problem: Option<String>,
}

struct Config {
    bytes: Option<Vec<u8>>,
    boolean_span: Option<Range<usize>>,
    enabled: bool,
    revision: Option<String>,
}

struct Snapshot {
    state: ClientAdmissionState,
    config: Option<Config>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientManifest {
    id: String,
    #[serde(rename = "type")]
    package_type: String,
    version: String,
    assembly: String,
    entry_type: String,
    entry_method: String,
}

#[tauri::command]
pub async fn read_client_admission(
    executable: String,
    installation_id: String,
) -> Result<ClientAdmissionState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = require_installation(&executable, &installation_id)?;
        Ok(snapshot(&root, &installation_id).state)
    })
    .await
    .map_err(|error| format!("Could not read the managed admission preference: {error}"))?
}

#[tauri::command]
pub async fn write_client_admission(
    executable: String,
    installation_id: String,
    expected_revision: Option<String>,
    enabled: bool,
) -> Result<ClientAdmissionState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        write_preference(&executable, &installation_id, expected_revision, enabled)
    })
    .await
    .map_err(|error| format!("Could not save the managed admission preference: {error}"))?
}

fn require_installation(executable: &str, installation_id: &str) -> Result<String, String> {
    let inspected = installation_profiles::inspect_target(executable)?;
    if inspected.target.kind != installation_profiles::TargetKind::Player {
        return Err("Managed client admission requires a player installation.".into());
    }
    if !inspected.available {
        return Err(inspected.problem.unwrap_or_else(|| "The player installation is unavailable.".into()));
    }
    if inspected.target.id != installation_id {
        return Err("The player installation no longer matches its selected identity. Select it again before changing managed admission.".into());
    }
    inspected.root.ok_or_else(|| "The player installation has no resolved folder.".into())
}

fn read_bounded(root: &Path, relative: &Path, limit: usize) -> Result<Option<Vec<u8>>, String> {
    if !mod_update::validate_target(root, relative)? {
        return Ok(None);
    }
    let path = root.join(relative);
    let file = File::open(&path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    let mut bytes = Vec::new();
    file.take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    if bytes.len() > limit {
        return Err(format!("{} exceeds the {limit}-byte limit.", path.display()));
    }
    Ok(Some(bytes))
}

fn read_config(root: &Path) -> Result<Config, String> {
    let Some(bytes) = read_bounded(root, Path::new(CONFIG_PATH), MAX_CONFIG_BYTES)? else {
        return Ok(Config { bytes: None, boolean_span: None, enabled: false, revision: None });
    };
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| "The managed admission config must contain valid UTF-8. Its contents were preserved.")?;
    let body = text.trim_start_matches('\u{feff}');
    let mut offset = text.len() - body.len();
    let mut setting = None;
    for line in body.split_inclusive('\n') {
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            let (key, value) = trimmed.split_once('=').ok_or_else(invalid_config)?;
            if key.trim() != "EnableManagedAdmission" || setting.is_some() {
                return Err(invalid_config());
            }
            let boolean = value.trim();
            let enabled = if boolean.eq_ignore_ascii_case("true") {
                true
            } else if boolean.eq_ignore_ascii_case("false") {
                false
            } else {
                return Err(invalid_config());
            };
            // Offsets are UTF-8 byte offsets into the original, unmodified file.
            let start = offset + (line.len() - line.trim_start().len())
                + key.len() + 1 + (value.len() - value.trim_start().len());
            setting = Some((enabled, start..start + boolean.len()));
        }
        offset += line.len();
    }
    let (enabled, boolean_span) = setting.ok_or_else(invalid_config)?;
    let revision = Some(format!("{:x}", Sha256::digest(&bytes)));
    Ok(Config { bytes: Some(bytes), boolean_span: Some(boolean_span), enabled, revision })
}

fn invalid_config() -> String {
    "The managed admission config must contain exactly one EnableManagedAdmission = true or false setting, with only blank lines or # comments otherwise. Its contents were preserved.".into()
}

fn installed_client(root: &Path) -> Result<String, String> {
    let bytes = read_bounded(root, Path::new(MANIFEST_PATH), 64 * 1024)?
        .ok_or("Install the Modstoke Client package in this player installation before changing managed admission.")?;
    let manifest: ClientManifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("The Modstoke Client package manifest is unreadable or unsupported: {error}"))?;
    if manifest.id != "modstoke.client"
        || manifest.package_type != "OpenACAIMod"
        || manifest.assembly != "Modstoke.Client.dll"
        || manifest.entry_type != "Modstoke.Client.Mod"
        || manifest.entry_method != "Initialize"
    {
        return Err("The installed Modstoke Client manifest does not identify the supported client entry point.".into());
    }
    if manifest.version.is_empty()
        || manifest.version.chars().count() > 128
        || manifest.version.trim() != manifest.version
        || manifest.version.chars().any(char::is_control)
    {
        return Err("The Modstoke Client manifest has an invalid version.".into());
    }
    if !mod_update::validate_target(root, Path::new(ASSEMBLY_PATH))? {
        return Err("The Modstoke Client assembly is missing. Install the complete client package before changing managed admission.".into());
    }
    let metadata = fs::metadata(root.join(ASSEMBLY_PATH))
        .map_err(|error| format!("Could not inspect the Modstoke Client assembly: {error}"))?;
    if metadata.len() == 0 {
        return Err("The Modstoke Client assembly is empty. Install the complete client package before changing managed admission.".into());
    }
    // This establishes package-file presence only, not loading, uniqueness or authenticity.
    Ok(manifest.version)
}

fn snapshot(root: &str, installation_id: &str) -> Snapshot {
    let folder = Path::new(root);
    let (config, config_problem) = match read_config(folder) {
        Ok(config) => (Some(config), None),
        Err(problem) => (None, Some(problem)),
    };
    let (client_version, client_problem) = match installed_client(folder) {
        Ok(version) => (Some(version), None),
        Err(problem) => (None, Some(problem)),
    };
    let (process_state, process_problem) = match game_process_running() {
        Ok(false) => ("stopped", None),
        Ok(true) => ("running", Some("Close all Sons of the Forest game and dedicated-server processes before changing managed admission.".into())),
        Err(problem) => ("unknown", Some(problem)),
    };
    let ownership_problem = mod_update::ensure_unmanaged(folder, Path::new(CONFIG_PATH)).err();
    let write_problem = config_problem.clone()
        .or(ownership_problem)
        .or_else(|| client_problem.clone())
        .or_else(|| process_problem.clone());
    let state = ClientAdmissionState {
        installation_id: installation_id.into(),
        root: root.into(),
        config_path: folder.join(CONFIG_PATH).to_string_lossy().into_owned(),
        state: match &config {
            Some(config) if config.bytes.is_none() => "missing",
            Some(_) => "configured",
            None => "unreadable",
        }.into(),
        enabled: config.as_ref().map(|config| config.enabled),
        revision: config.as_ref().and_then(|config| config.revision.clone()),
        config_problem,
        client_available: client_version.is_some(),
        client_version,
        client_problem,
        process_state: process_state.into(),
        process_problem,
        can_write: write_problem.is_none(),
        write_problem,
    };
    Snapshot { state, config }
}

fn require_writable(snapshot: &Snapshot) -> Result<(), String> {
    if let Some(problem) = &snapshot.state.write_problem {
        return Err(problem.clone());
    }
    Ok(())
}

fn create_config_parents(root: &Path) -> Result<(), String> {
    mod_update::validate_target(root, Path::new(CONFIG_PATH))?;
    let mut parent = root.to_owned();
    for part in ["BepInEx", "config"] {
        parent.push(part);
        match fs::symlink_metadata(&parent) {
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {
                fs::create_dir(&parent)
                    .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
            }
            Err(error) => return Err(format!("Could not inspect {}: {error}", parent.display())),
        }
        mod_update::validate_directory(&parent)?;
    }
    Ok(())
}

fn write_preference(
    executable: &str,
    installation_id: &str,
    expected_revision: Option<String>,
    enabled: bool,
) -> Result<ClientAdmissionState, String> {
    let root = require_installation(executable, installation_id)?;
    let folder = Path::new(&root);
    let _lock = mod_update::acquire_update_lock(folder)?;
    let initial = snapshot(&root, installation_id);
    require_writable(&initial)?;
    let original = initial.config.ok_or_else(invalid_config)?;
    if original.revision != expected_revision {
        return Err("The managed admission config changed since it was loaded. Reload it before saving again.".into());
    }
    let bytes = match (&original.bytes, &original.boolean_span) {
        (Some(bytes), Some(span)) if original.enabled != enabled => {
            let mut replacement = bytes.clone();
            replacement.splice(span.clone(), if enabled { b"true".as_slice() } else { b"false".as_slice() }.iter().copied());
            Some(replacement)
        }
        (None, _) if enabled => Some(b"EnableManagedAdmission=true\n".to_vec()),
        _ => original.bytes.clone(),
    };
    if bytes.as_ref().is_some_and(|bytes| bytes.len() > MAX_CONFIG_BYTES) {
        return Err("The changed managed admission config would exceed the 1024-byte limit. Its contents were preserved.".into());
    }
    let changed = bytes != original.bytes;
    let temporary = if changed {
        create_config_parents(folder)?;
        let parent = folder.join("BepInEx/config");
        let mut temporary = tempfile::NamedTempFile::new_in(&parent)
            .map_err(|error| format!("Could not prepare the managed admission config: {error}"))?;
        temporary.write_all(bytes.as_deref().ok_or_else(invalid_config)?)
            .and_then(|_| temporary.flush())
            .and_then(|_| temporary.as_file().sync_all())
            .map_err(|error| format!("Could not write the managed admission config: {error}"))?;
        // Keep cleanup ownership of the path, but close the handle before rename.
        Some(temporary.into_temp_path())
    } else {
        None
    };

    if require_installation(executable, installation_id)? != root {
        return Err("The selected installation changed while the preference was being prepared. No config was replaced.".into());
    }
    let current = snapshot(&root, installation_id);
    require_writable(&current)?;
    // Re-read exact bytes after the other final guards, immediately before publication.
    let current_config = read_config(folder)?;
    if current_config.revision != expected_revision || current_config.bytes != original.bytes {
        return Err("The managed admission config changed while the preference was being prepared. Its current contents were preserved; reload before saving again.".into());
    }
    if let Some(temporary) = temporary {
        mod_update::durable_rename(&temporary, &folder.join(CONFIG_PATH), original.bytes.is_some())?;
        return Ok(snapshot(&root, installation_id).state);
    }
    Ok(current.state)
}

#[cfg(windows)]
pub(crate) fn game_process_running() -> Result<bool, String> {
    // This is a conservative system-wide snapshot, not a lock against a later launch.
    use windows::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE, ERROR_NO_MORE_FILES};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    struct SnapshotHandle(HANDLE);
    impl Drop for SnapshotHandle {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0); }
        }
    }
    let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if handle == INVALID_HANDLE_VALUE {
        return Err(format!("Could not determine whether Sons of the Forest is running: {}", std::io::Error::last_os_error()));
    }
    let handle = SnapshotHandle(handle);
    let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
    let mut present = unsafe { Process32FirstW(handle.0, &mut entry) }.as_bool();
    let mut running = false;
    loop {
        if !present {
            let error = unsafe { GetLastError() };
            return if error == ERROR_NO_MORE_FILES {
                Ok(running)
            } else {
                Err(format!("Could not enumerate running processes (Windows error {}). Managed admission cannot be changed until process status is known.", error.0))
            };
        }
        let end = entry.szExeFile.iter().position(|character| *character == 0)
            .ok_or("A running process has an unreadable executable name. Managed admission cannot be changed until process status is known.")?;
        let name = String::from_utf16(&entry.szExeFile[..end])
            .map_err(|_| "A running process has an unreadable executable name. Managed admission cannot be changed until process status is known.")?;
        running |= name.eq_ignore_ascii_case("SonsOfTheForest.exe")
            || name.eq_ignore_ascii_case("SonsOfTheForestDS.exe");
        present = unsafe { Process32NextW(handle.0, &mut entry) }.as_bool();
    }
}

#[cfg(not(windows))]
pub(crate) fn game_process_running() -> Result<bool, String> {
    Err("Managed admission process checks are supported only on Windows. The preference cannot be changed on this platform.".into())
}
