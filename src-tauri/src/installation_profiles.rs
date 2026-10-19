use crate::mod_update;
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Component, Path, PathBuf};
use tauri::Manager;

const REGISTRY_FILE: &str = "installation-profiles.json";
const LOCK_FILE: &str = ".installation-profiles.lock";
const MAX_BYTES: u64 = 64 * 1024;
const MAX_TARGETS: usize = 32;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Registry {
    pub schema_version: u32,
    #[serde(deserialize_with = "required_nullable")]
    pub selected_id: Option<String>,
    pub targets: Vec<Target>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub id: String,
    pub name: String,
    pub executable: String,
    pub kind: TargetKind,
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetKind {
    Player,
    Server,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectedTarget {
    pub target: Target,
    pub available: bool,
    pub problem: Option<String>,
    pub root: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryState {
    pub registry: Registry,
    pub revision: Option<String>,
    pub selected: Option<InspectedTarget>,
}

fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

fn empty_registry() -> Registry {
    Registry {
        schema_version: 1,
        selected_id: None,
        targets: Vec::new(),
    }
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn path_key(value: &str) -> String {
    value
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_lowercase()
}

fn root_id(root: &str) -> String {
    hash(path_key(root).as_bytes())
}

fn ordinary_path(path: &Path) -> Result<String, String> {
    let value = path
        .to_str()
        .ok_or("The installation path is not valid Unicode.")?;
    if value
        .get(..8)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("\\\\?\\UNC\\"))
    {
        return Ok(format!("\\\\{}", &value[8..]));
    }
    Ok(value.strip_prefix("\\\\?\\").unwrap_or(value).to_owned())
}

fn executable_parts(value: &str) -> Result<(&Path, TargetKind), String> {
    let path = Path::new(value);
    if value.is_empty()
        || value.len() > 32767
        || value.chars().any(char::is_control)
        || !path.is_absolute()
        || value.starts_with("\\\\?\\")
        || value.starts_with("\\\\.\\")
        || value
            .split(['/', '\\'])
            .any(|part| part == "." || part == "..")
        || path
            .components()
            .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
        || value.ends_with(['/', '\\'])
    {
        return Err(
            "Select an ordinary absolute path to the game or dedicated-server executable.".into(),
        );
    }
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("The installation executable has no filename.")?;
    let kind = if filename.eq_ignore_ascii_case("SonsOfTheForest.exe") {
        TargetKind::Player
    } else if filename.eq_ignore_ascii_case("SonsOfTheForestDS.exe") {
        TargetKind::Server
    } else {
        return Err("Select SonsOfTheForest.exe or SonsOfTheForestDS.exe.".into());
    };
    Ok((
        path.parent()
            .ok_or("The installation executable has no parent folder.")?,
        kind,
    ))
}

fn canonical_target(executable: &str) -> Result<(String, String, TargetKind), String> {
    let (parent, kind) = executable_parts(executable)?;
    mod_update::validate_directory(parent)?;
    let filename = Path::new(executable)
        .file_name()
        .ok_or("Missing executable filename.")?;
    if !mod_update::validate_target(parent, Path::new(filename))? {
        return Err(format!(
            "The installation executable is missing: {executable}"
        ));
    }
    let canonical = fs::canonicalize(executable)
        .map_err(|error| format!("Could not resolve {executable}: {error}"))?;
    let root = canonical
        .parent()
        .ok_or("The installation executable has no parent folder.")?;
    let canonical_name = canonical
        .file_name()
        .ok_or("Missing executable filename.")?;
    if !mod_update::validate_target(root, Path::new(canonical_name))? {
        return Err(format!(
            "The installation executable is missing: {executable}"
        ));
    }
    let ordinary_executable = ordinary_path(&canonical)?;
    let (_, canonical_kind) = executable_parts(&ordinary_executable)?;
    if canonical_kind != kind {
        return Err("The resolved executable does not match the installation kind.".into());
    }
    Ok((ordinary_executable, ordinary_path(root)?, kind))
}

fn inspect_saved(target: &Target) -> InspectedTarget {
    let result = canonical_target(&target.executable).and_then(|(_, root, kind)| {
        if kind != target.kind || root_id(&root) != target.id {
            return Err("The installation no longer matches its saved identity. Remove it and add its current executable again.".into());
        }
        Ok(root)
    });
    match result {
        Ok(root) => InspectedTarget {
            target: target.clone(),
            available: true,
            problem: None,
            root: Some(root),
        },
        Err(problem) => InspectedTarget {
            target: target.clone(),
            available: false,
            problem: Some(problem),
            root: None,
        },
    }
}

fn invalid_registry(reason: impl std::fmt::Display) -> String {
    format!(
        "{REGISTRY_FILE} has invalid or unsupported data: {reason}. The saved file was preserved."
    )
}

fn validate_registry(registry: &Registry) -> Result<(), String> {
    if registry.schema_version != 1 || registry.targets.len() > MAX_TARGETS {
        return Err(invalid_registry(
            "expected schema version 1 and at most 32 installations",
        ));
    }
    let mut ids = HashSet::new();
    let mut roots = HashSet::new();
    for target in &registry.targets {
        if target.name.trim().is_empty()
            || target.name.chars().count() > 64
            || target.name.chars().any(char::is_control)
        {
            return Err(invalid_registry(
                "installation names must contain 1 to 64 characters without control characters",
            ));
        }
        let (parent, kind) = executable_parts(&target.executable).map_err(invalid_registry)?;
        let root = ordinary_path(parent).map_err(invalid_registry)?;
        if target.id != root_id(&root) || target.kind != kind {
            return Err(invalid_registry(
                "an installation ID or kind does not match its executable path",
            ));
        }
        if !ids.insert(target.id.clone()) || !roots.insert(path_key(&root)) {
            return Err(invalid_registry(
                "each installation folder may appear only once",
            ));
        }
    }
    if registry
        .selected_id
        .as_ref()
        .is_some_and(|selected| !ids.contains(selected))
    {
        return Err(invalid_registry(
            "the selected installation does not exist in the registry",
        ));
    }
    Ok(())
}

fn read_registry(folder: &Path) -> Result<(Registry, Option<String>), String> {
    match fs::symlink_metadata(folder) {
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok((empty_registry(), None)),
        Err(error) => {
            return Err(format!(
                "Could not inspect the installation profile folder: {error}"
            ))
        }
        Ok(_) => {}
    }
    mod_update::validate_directory(folder)?;
    if !mod_update::validate_target(folder, Path::new(REGISTRY_FILE))? {
        return Ok((empty_registry(), None));
    }
    let file = File::open(folder.join(REGISTRY_FILE))
        .map_err(|error| format!("Could not read {REGISTRY_FILE}: {error}"))?;
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read {REGISTRY_FILE}: {error}"))?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(invalid_registry("the file exceeds 64 KiB"));
    }
    let registry: Registry = serde_json::from_slice(&bytes).map_err(invalid_registry)?;
    validate_registry(&registry)?;
    Ok((registry, Some(hash(&bytes))))
}

fn state(registry: Registry, revision: Option<String>) -> RegistryState {
    let selected = registry
        .selected_id
        .as_ref()
        .and_then(|id| registry.targets.iter().find(|target| &target.id == id))
        .map(inspect_saved);
    RegistryState {
        registry,
        revision,
        selected,
    }
}

fn config_folder(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|error| format!("Could not find the installation profile folder: {error}"))
}

fn acquire_lock(folder: &Path) -> Result<File, String> {
    mod_update::validate_target(folder, Path::new(LOCK_FILE))?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0).custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let lock = options.open(folder.join(LOCK_FILE)).map_err(|error| {
        format!("Could not lock installation profiles. Another save may be running: {error}")
    })?;
    let metadata = lock.metadata().map_err(|error| error.to_string())?;
    #[cfg(windows)]
    let linked = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let linked = metadata.file_type().is_symlink();
    if linked || !metadata.is_file() {
        return Err("The installation profile lock is not an ordinary file.".into());
    }
    #[cfg(not(windows))]
    lock.try_lock()
        .map_err(|error| format!("Could not lock installation profiles: {error}"))?;
    Ok(lock)
}

#[tauri::command]
pub async fn read_installation_profiles(app: tauri::AppHandle) -> Result<RegistryState, String> {
    let folder = config_folder(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let (registry, revision) = read_registry(&folder)?;
        Ok(state(registry, revision))
    }).await.map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn inspect_installation_target(executable: String) -> Result<InspectedTarget, String> {
    tauri::async_runtime::spawn_blocking(move || inspect_target(&executable))
        .await.map_err(|error| error.to_string())?
}

pub(crate) fn inspect_target(executable: &str) -> Result<InspectedTarget, String> {
    let (parent, kind) = executable_parts(&executable)?;
    let mut target = Target {
        id: root_id(&ordinary_path(parent)?),
        name: match kind {
            TargetKind::Player => "Sons of the Forest",
            TargetKind::Server => "Dedicated server",
        }
        .into(),
        executable: executable.to_owned(),
        kind,
    };
    match canonical_target(&executable) {
        Ok((canonical_executable, root, _)) => {
            target.id = root_id(&root);
            target.executable = canonical_executable;
            Ok(InspectedTarget {
                target,
                available: true,
                problem: None,
                root: Some(root),
            })
        }
        Err(problem) => Ok(InspectedTarget {
            target,
            available: false,
            problem: Some(problem),
            root: None,
        }),
    }
}

#[tauri::command]
pub async fn write_installation_profiles(
    app: tauri::AppHandle,
    expected_revision: Option<String>,
    registry: Registry,
) -> Result<RegistryState, String> {
    let folder = config_folder(&app)?;
    tauri::async_runtime::spawn_blocking(move || write_registry(&folder, expected_revision, registry))
        .await.map_err(|error| error.to_string())?
}

fn write_registry(folder: &Path, expected_revision: Option<String>, registry: Registry) -> Result<RegistryState, String> {
    validate_registry(&registry)?;
    let bytes = serde_json::to_vec_pretty(&registry).map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Installation profiles exceed the 64 KiB limit. No changes were saved.".into());
    }
    // Validate existing ancestors before creating the app's configuration folder.
    for ancestor in folder.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(_) => {
                mod_update::validate_directory(ancestor)?;
                break;
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Could not inspect the installation profile folder: {error}"
                ))
            }
        }
    }
    fs::create_dir_all(&folder)
        .map_err(|error| format!("Could not create the installation profile folder: {error}"))?;
    mod_update::validate_directory(&folder)?;
    let _lock = acquire_lock(&folder)?;
    let (previous, revision) = read_registry(&folder)?;
    if expected_revision != revision {
        return Err("Installation profiles changed since they were loaded. Reload them before saving again.".into());
    }
    for target in &registry.targets {
        if let Some(old) = previous.targets.iter().find(|old| old.id == target.id) {
            if old.executable != target.executable || old.kind != target.kind {
                return Err("A saved installation's executable and kind cannot be changed. Remove it and add the new executable instead.".into());
            }
        } else {
            let inspected = inspect_saved(target);
            if !inspected.available {
                return Err(format!(
                    "Could not add {}: {}",
                    target.name,
                    inspected.problem.unwrap_or_default()
                ));
            }
        }
    }
    let result = state(registry, Some(hash(&bytes)));
    if result.registry.selected_id != previous.selected_id {
        if let Some(selected) = &result.selected {
            if !selected.available {
                return Err(format!(
                    "Could not select {}: {}",
                    selected.target.name,
                    selected
                        .problem
                        .as_deref()
                        .unwrap_or("The installation is unavailable.")
                ));
            }
        }
    }
    let mut temporary = tempfile::NamedTempFile::new_in(&folder)
        .map_err(|error| format!("Could not prepare installation profiles: {error}"))?;
    temporary
        .write_all(&bytes)
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|error| format!("Could not write installation profiles: {error}"))?;
    mod_update::validate_target(&folder, Path::new(REGISTRY_FILE))?;
    temporary
        .persist(folder.join(REGISTRY_FILE))
        .map_err(|error| {
            format!(
                "Could not replace {REGISTRY_FILE}. The previous file was preserved: {}",
                error.error
            )
        })?;
    Ok(result)
}
