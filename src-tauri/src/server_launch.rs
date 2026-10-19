use crate::{installation_profiles, mod_profile, mod_update, server_native_config, server_process};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command};
use std::sync::Mutex;
use tauri::Manager;

const MAX_SETTINGS_BYTES: u64 = 64 * 1024;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Settings {
    schema_version: u32,
    installation_id: String,
    executable: String,
    data_directory: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessExit {
    success: bool,
    code: Option<i32>,
}

struct OwnedServer {
    installation_id: String,
    child: Child,
    _installation_lock: File,
    _data_lock: File,
}

struct Processes {
    running: Option<OwnedServer>,
    last_exit: Option<(String, ProcessExit)>,
}

static PROCESSES: Mutex<Processes> = Mutex::new(Processes { running: None, last_exit: None });

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerLaunchState {
    installation_id: String,
    root: String,
    data_directory: Option<String>,
    revision: Option<String>,
    config_path: Option<String>,
    config_revision: Option<String>,
    config: Option<server_native_config::ConfigSummary>,
    config_problem: Option<String>,
    process_state: String,
    process_id: Option<u32>,
    last_exit: Option<ProcessExit>,
    process_problem: Option<String>,
    can_save: bool,
    save_problem: Option<String>,
    can_launch: bool,
    launch_problem: Option<String>,
}

fn require_installation(executable: &str, installation_id: &str) -> Result<(String, String), String> {
    let inspected = installation_profiles::inspect_target(executable)?;
    if inspected.target.kind != installation_profiles::TargetKind::Server {
        return Err("Select a dedicated-server installation before preparing its launch.".into());
    }
    if !inspected.available {
        return Err(inspected.problem.unwrap_or_else(|| "The dedicated-server installation is unavailable.".into()));
    }
    if inspected.target.id != installation_id {
        return Err("The dedicated-server installation changed. Select its current executable again.".into());
    }
    Ok((inspected.root.ok_or("The server installation has no resolved folder.")?, inspected.target.executable))
}

fn ordinary_path(path: &Path) -> Result<String, String> {
    let text = path.to_str().ok_or("The server data folder is not valid Unicode.")?;
    if text.get(..8).is_some_and(|prefix| prefix.eq_ignore_ascii_case("\\\\?\\UNC\\")) {
        return Ok(format!("\\\\{}", &text[8..]));
    }
    Ok(text.strip_prefix("\\\\?\\").unwrap_or(text).to_owned())
}

fn validate_path(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if value.is_empty() || value.len() > 32767 || value.chars().any(char::is_control)
        || !path.is_absolute() || value.starts_with("\\\\?\\") || value.starts_with("\\\\.\\")
        || value.split(['/', '\\']).any(|part| part == "." || part == "..")
        || path.components().any(|part| matches!(part, Component::CurDir | Component::ParentDir)) {
        return Err("Select an ordinary absolute path to an existing server data folder.".into());
    }
    Ok(())
}

fn canonical_data_directory(value: &str) -> Result<String, String> {
    validate_path(value)?;
    mod_update::validate_directory(Path::new(value))?;
    let path = fs::canonicalize(value).map_err(|error| format!("Could not resolve the server data folder: {error}"))?;
    mod_update::validate_directory(&path)?;
    ordinary_path(&path)
}

fn settings_folder(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path().app_config_dir().map(|folder| folder.join("server-launch"))
        .map_err(|error| format!("Could not find the server profile folder: {error}"))
}

fn settings_file(installation_id: &str) -> String {
    // The ID has already been matched to inspect_target's canonical SHA-256 identity.
    format!("{installation_id}.json")
}

fn read_settings(folder: &Path, installation_id: &str, executable: &str) -> Result<(Option<Settings>, Option<String>), String> {
    match fs::symlink_metadata(folder) {
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok((None, None)),
        Err(error) => return Err(format!("Could not inspect server launch settings: {error}")),
        Ok(_) => {}
    }
    let filename = settings_file(installation_id);
    if !mod_update::validate_target(folder, Path::new(&filename))? { return Ok((None, None)); }
    let file = File::open(folder.join(&filename)).map_err(|error| format!("Could not read server launch settings: {error}"))?;
    let mut bytes = Vec::new();
    file.take(MAX_SETTINGS_BYTES + 1).read_to_end(&mut bytes).map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_SETTINGS_BYTES {
        return Err("Server launch settings exceed 64 KiB. The saved file was preserved.".into());
    }
    let settings: Settings = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Server launch settings are invalid or unsupported: {error}. The saved file was preserved."))?;
    if settings.schema_version != 1 || settings.installation_id != installation_id || settings.executable != executable {
        return Err("Server launch settings do not match this installation or use an unsupported schema. The saved file was preserved.".into());
    }
    validate_path(&settings.data_directory)?;
    Ok((Some(settings), Some(format!("{:x}", Sha256::digest(&bytes)))))
}

fn create_settings_folder(folder: &Path) -> Result<(), String> {
    for ancestor in folder.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(_) => { mod_update::validate_directory(ancestor)?; break; }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(format!("Could not inspect the server profile folder: {error}")),
        }
    }
    fs::create_dir_all(folder).map_err(|error| format!("Could not create the server profile folder: {error}"))?;
    mod_update::validate_directory(folder)
}

fn exclusive_lock(folder: &Path, name: &str) -> Result<File, String> {
    mod_update::validate_target(folder, Path::new(name))?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0).custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let file = options.open(folder.join(name))
        .map_err(|error| format!("Could not lock {}. Another Manager operation may be using it: {error}", folder.join(name).display()))?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    #[cfg(windows)]
    let linked = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let linked = metadata.file_type().is_symlink();
    if linked || !metadata.is_file() {
        return Err("The server profile lock is not an ordinary file.".into());
    }
    #[cfg(not(windows))]
    file.try_lock().map_err(|error| format!("Could not lock server launch settings: {error}"))?;
    Ok(file)
}

fn reap(processes: &mut Processes) -> Result<(), String> {
    let Some(server) = processes.running.as_mut() else { return Ok(()); };
    if let Some(exit) = server.child.try_wait().map_err(|error| format!("Could not read the launched server's process status: {error}"))? {
        processes.last_exit = Some((server.installation_id.clone(), ProcessExit { success: exit.success(), code: exit.code() }));
        // Releasing these handles permits Manager recovery only after this child has exited.
        processes.running = None;
    }
    Ok(())
}

pub(crate) fn reap_exited() -> Result<(), String> {
    let mut processes = PROCESSES.lock().map_err(|_| "The server process record is unavailable. Restart Manager before changing this installation.")?;
    reap(&mut processes)
}

pub(crate) struct ServerWorldAccess {
    pub(crate) installation_id: String,
    pub(crate) root: String,
    pub(crate) data_directory: String,
    pub(crate) save_slot: u32,
    executable: String,
    settings_folder: PathBuf,
    settings_revision: String,
    config_revision: String,
    _installation_lock: File,
    _launch_lock: File,
    _data_lock: File,
}

impl ServerWorldAccess {
    pub(crate) fn require_unchanged(&self) -> Result<(), String> {
        require_world_stopped()?;
        let (root, executable) = require_installation(&self.executable, &self.installation_id)?;
        if root != self.root || executable != self.executable
            || canonical_data_directory(&self.data_directory)? != self.data_directory {
            return Err("The server installation or data folder changed. Read its current settings before continuing.".into());
        }
        let (settings, revision) = read_settings(&self.settings_folder, &self.installation_id, &self.executable)?;
        if revision.as_deref() != Some(self.settings_revision.as_str())
            || settings.as_ref().map(|value| value.data_directory.as_str()) != Some(self.data_directory.as_str()) {
            return Err("Server data-folder settings changed. Read them again before continuing.".into());
        }
        let config = server_native_config::read(Path::new(&self.data_directory))?;
        if config.revision != self.config_revision || config.summary.save_slot != self.save_slot {
            return Err("dedicatedserver.cfg changed. Read the server settings again before continuing.".into());
        }
        mod_profile::require_launch_ready(Path::new(&self.root))?;
        require_world_stopped()
    }
}

fn require_world_stopped() -> Result<(), String> {
    if crate::client_admission::game_process_running()? {
        return Err("Close Sons of the Forest and its dedicated server before changing server settings or accessing world backups and recovery.".into());
    }
    Ok(())
}

pub(crate) fn acquire_world_access(app: &tauri::AppHandle, executable: &str, installation_id: &str,
    expected_revision: &str, expected_config_revision: &str) -> Result<ServerWorldAccess, String> {
    let (root, executable) = require_installation(executable, installation_id)?;
    let installation_lock = mod_update::acquire_update_lock(Path::new(&root))?;
    let folder = settings_folder(app)?;
    let launch_lock = exclusive_lock(&folder, ".launch.lock")?;
    require_world_stopped()?;
    let (settings, revision) = read_settings(&folder, installation_id, &executable)?;
    if revision.as_deref() != Some(expected_revision) {
        return Err("Server launch settings changed. Read them again before accessing saves.".into());
    }
    let settings = settings.ok_or("Choose the server data folder before accessing saves.")?;
    let data_directory = canonical_data_directory(&settings.data_directory)?;
    if data_directory != settings.data_directory {
        return Err("The server data folder changed. Choose it again before accessing saves.".into());
    }
    let data_lock = exclusive_lock(Path::new(&data_directory), ".modstoke-server.lock")?;
    let config = server_native_config::read(Path::new(&data_directory))?;
    let access = ServerWorldAccess {
        installation_id: installation_id.into(), root, data_directory, save_slot: config.summary.save_slot,
        executable, settings_folder: folder, settings_revision: expected_revision.into(),
        config_revision: expected_config_revision.into(), _installation_lock: installation_lock,
        _launch_lock: launch_lock, _data_lock: data_lock,
    };
    access.require_unchanged()?;
    Ok(access)
}

fn snapshot(folder: &Path, root: &str, executable: &str, installation_id: &str) -> Result<ServerLaunchState, String> {
    let (settings, revision) = read_settings(folder, installation_id, executable)?;
    let data_directory = settings.map(|settings| settings.data_directory);
    let config_path = data_directory.as_ref().map(|path| Path::new(path).join("dedicatedserver.cfg").to_string_lossy().into_owned());
    let config = data_directory.as_deref().ok_or_else(|| "Choose the data folder used by this dedicated server.".to_string())
        .and_then(|saved| {
            let canonical = canonical_data_directory(saved)?;
            if canonical != saved { return Err("The server data folder changed. Choose it again before starting.".into()); }
            Ok(canonical)
        })
        .and_then(|path| server_native_config::read(Path::new(&path)));
    let (config, config_revision, config_problem) = match config {
        Ok(config) => (Some(config.summary), Some(config.revision), None),
        Err(problem) => (None, None, Some(problem)),
    };
    let observed = (|| {
        let mut processes = PROCESSES.lock().map_err(|_| "The server process record is unavailable. Restart Manager before trying again.")?;
        reap(&mut processes)?;
        let last_exit = processes.last_exit.as_ref().filter(|(id, _)| id == installation_id).map(|(_, exit)| exit.clone());
        if let Some(server) = &processes.running {
            return Ok::<_, String>((if server.installation_id == installation_id { "running" } else { "other" },
                (server.installation_id == installation_id).then_some(server.child.id()), last_exit));
        }
        Ok((if server_process::is_running()? { "other" } else { "stopped" }, None, last_exit))
    })();
    let (process_state, process_id, last_exit, process_problem) = match observed {
        Ok((state, pid, exit)) => (state, pid, exit, None),
        Err(problem) => ("unknown", None, None, Some(problem)),
    };
    let save_problem = process_problem.clone().or_else(|| (process_state != "stopped")
        .then(|| "Stop all Sons of the Forest dedicated-server processes before changing the data folder or starting another server.".into()))
        .or_else(|| data_directory.as_deref().and_then(|data| crate::server_world::require_no_pending(Path::new(data)).err()));
    let launch_problem = save_problem.clone().or_else(|| config_problem.clone())
        .or_else(|| config.as_ref().filter(|config| config.save_mode == "New")
            .map(|_| "SaveMode is New. The native server would overwrite its selected save slot on every start. Set SaveMode to Continue in dedicatedserver.cfg before launching through Manager.".into()))
        .or_else(|| mod_profile::require_launch_ready(Path::new(root)).err());
    Ok(ServerLaunchState {
        installation_id: installation_id.into(), root: root.into(), data_directory, revision,
        config_path, config_revision, config, config_problem,
        process_state: process_state.into(), process_id, last_exit, process_problem,
        can_save: save_problem.is_none(), save_problem, can_launch: launch_problem.is_none(), launch_problem,
    })
}

#[tauri::command]
pub async fn read_server_launch(app: tauri::AppHandle, executable: String, installation_id: String) -> Result<ServerLaunchState, String> {
    let folder = settings_folder(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let (root, executable) = require_installation(&executable, &installation_id)?;
        snapshot(&folder, &root, &executable, &installation_id)
    }).await.map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn write_server_capacity(app: tauri::AppHandle, executable: String, installation_id: String,
    expected_revision: String, expected_config_revision: String, max_players: u32,
    acknowledge_experimental: bool) -> Result<crate::server_capacity::CapacityChange, String> {
    if !matches!(max_players, 8 | 16 | 32) {
        return Err("Choose a configured player limit of 8, 16 or 32.".into());
    }
    if max_players > 8 && !acknowledge_experimental {
        return Err("Player limits above 8 are experimental. Review and acknowledge their limitations before saving.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let access = acquire_world_access(&app, &executable, &installation_id, &expected_revision, &expected_config_revision)?;
        crate::server_world::require_no_pending(Path::new(&access.data_directory))?;
        crate::server_capacity::write(&access, &expected_config_revision, max_players)
    }).await.map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn write_server_identity(app: tauri::AppHandle, executable: String, installation_id: String,
    expected_revision: String, expected_config_revision: String, server_name: String,
) -> Result<crate::server_capacity::ConfigChange, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let access = acquire_world_access(&app, &executable, &installation_id, &expected_revision, &expected_config_revision)?;
        crate::server_capacity::write_identity(&access, &expected_config_revision, &server_name)
    }).await.map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn write_server_launch(app: tauri::AppHandle, executable: String, installation_id: String,
    expected_revision: Option<String>, data_directory: String) -> Result<ServerLaunchState, String> {
    let folder = settings_folder(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let (root, executable) = require_installation(&executable, &installation_id)?;
        let data_directory = canonical_data_directory(&data_directory)?;
        let _installation_lock = mod_update::acquire_update_lock(Path::new(&root))?;
        create_settings_folder(&folder)?;
        let _launch_lock = exclusive_lock(&folder, ".launch.lock")?;
        server_process::require_stopped()?;
        let (previous, revision) = read_settings(&folder, &installation_id, &executable)?;
        if revision != expected_revision { return Err("Server launch settings changed. Read them again before saving.".into()); }
        if let Some(previous) = previous {
            crate::server_world::require_no_pending(Path::new(&previous.data_directory))?;
        }
        crate::server_world::require_no_pending(Path::new(&data_directory))?;
        let settings = Settings { schema_version: 1, installation_id: installation_id.clone(), executable: executable.clone(), data_directory };
        let bytes = serde_json::to_vec_pretty(&settings).map_err(|error| error.to_string())?;
        if bytes.len() as u64 > MAX_SETTINGS_BYTES { return Err("Server launch settings exceed 64 KiB.".into()); }
        let mut temporary = tempfile::NamedTempFile::new_in(&folder).map_err(|error| error.to_string())?;
        temporary.write_all(&bytes).and_then(|_| temporary.as_file().sync_all()).map_err(|error| error.to_string())?;
        let temporary = temporary.into_temp_path();
        if require_installation(&executable, &installation_id)?.0 != root
            || canonical_data_directory(&settings.data_directory)? != settings.data_directory {
            return Err("The selected server or data folder changed while settings were prepared. Read them again before saving.".into());
        }
        server_process::require_stopped()?;
        if read_settings(&folder, &installation_id, &executable)?.1 != expected_revision {
            return Err("Server launch settings changed while the save was prepared. Their current contents were preserved.".into());
        }
        let filename = settings_file(&installation_id);
        let exists = mod_update::validate_target(&folder, Path::new(&filename))?;
        mod_update::durable_rename(&temporary, &folder.join(filename), exists)?;
        let observed = snapshot(&folder, &root, &executable, &installation_id)?;
        if observed.revision.as_deref() != Some(format!("{:x}", Sha256::digest(&bytes)).as_str()) {
            return Err("Server launch settings changed after saving. Read their current values before continuing.".into());
        }
        Ok(observed)
    }).await.map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn launch_dedicated_server(app: tauri::AppHandle, executable: String, installation_id: String,
    expected_revision: String, expected_config_revision: String) -> Result<ServerLaunchState, String> {
    let folder = settings_folder(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let (root, executable) = require_installation(&executable, &installation_id)?;
        let installation_lock = mod_update::acquire_update_lock(Path::new(&root))?;
        let _launch_lock = exclusive_lock(&folder, ".launch.lock")?;
        let mut processes = PROCESSES.lock().map_err(|_| "The server process record is unavailable. Restart Manager before trying again.")?;
        reap(&mut processes)?;
        if processes.running.is_some() { return Err("A server launched by this Manager is still running.".into()); }
        server_process::require_stopped()?;
        let (settings, revision) = read_settings(&folder, &installation_id, &executable)?;
        if revision.as_deref() != Some(expected_revision.as_str()) { return Err("Server launch settings changed. Read them again before starting.".into()); }
        let settings = settings.ok_or("Choose the server data folder before starting.")?;
        let data_directory = canonical_data_directory(&settings.data_directory)?;
        if data_directory != settings.data_directory { return Err("The server data folder changed. Choose it again before starting.".into()); }
        let data_lock = exclusive_lock(Path::new(&data_directory), ".modstoke-server.lock")?;
        mod_profile::require_launch_ready(Path::new(&root))?;
        crate::server_world::require_no_pending(Path::new(&data_directory))?;
        let config = server_native_config::read(Path::new(&data_directory))?;
        if config.revision != expected_config_revision { return Err("dedicatedserver.cfg changed. Read its current save settings before starting.".into()); }
        if config.summary.save_mode != "Continue" {
            return Err("SaveMode must be Continue to launch through Manager. New would overwrite the selected save slot.".into());
        }
        if require_installation(&executable, &installation_id)?.0 != root {
            return Err("The dedicated-server installation changed before launch.".into());
        }
        server_process::require_stopped()?;
        crate::server_world::require_no_pending(Path::new(&data_directory))?;
        // Arguments go straight to the selected executable. There is no shell or inherited Steam player launch.
        let mut command = Command::new(&executable);
        command.current_dir(&root).arg("-userdatapath").arg(&data_directory);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x00000010); // CREATE_NEW_CONSOLE: allow a native operator console.
        }
        let child = command.spawn().map_err(|error| format!("Could not start the dedicated server: {error}"))?;
        processes.last_exit = None;
        processes.running = Some(OwnedServer { installation_id: installation_id.clone(), child,
            _installation_lock: installation_lock, _data_lock: data_lock });
        drop(processes);
        // If this observation fails, keep the owned child and its locks. Never kill or retry a possibly successful launch.
        snapshot(&folder, &root, &executable, &installation_id)
    }).await.map_err(|error| error.to_string())?
}
