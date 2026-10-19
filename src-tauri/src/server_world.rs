use crate::{mod_update, server_launch, server_world_restore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const STORE: &str = ".modstoke-world-backups";
pub(crate) const PENDING: &str = "pending.json";
pub(crate) const MAX_BACKUPS: usize = 64;
const MAX_FILES: usize = 4096;
const MAX_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_DEPTH: usize = 16;
const MAX_RECORD_BYTES: u64 = 40 * 1024 * 1024;
const STORE_PREPARATION_PREFIX: &str = ".modstoke-world-store-";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldState {
    installation_id: String,
    data_directory: String,
    save_directory: String,
    save_slot: u32,
    world_revision: Option<String>,
    files: usize,
    bytes: u64,
    backups: Vec<BackupSummary>,
    pending: Option<PendingSummary>,
    problem: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BackupSummary {
    id: String,
    revision: String,
    created_utc: String,
    label: String,
    files: usize,
    bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PendingSummary {
    revision: String,
    detail: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WorldFile {
    pub(crate) path: String,
    pub(crate) bytes: u64,
    pub(crate) sha256: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Inventory {
    pub(crate) directories: Vec<String>,
    pub(crate) files: Vec<WorldFile>,
}

impl Inventory {
    pub(crate) fn revision(&self) -> Result<String, String> {
        Ok(hash_bytes(&serde_json::to_vec(self).map_err(|error| error.to_string())?))
    }

    fn bytes(&self) -> u64 { self.files.iter().map(|file| file.bytes).sum() }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Manifest {
    schema_version: u32,
    pub(crate) id: String,
    pub(crate) installation_id: String,
    pub(crate) installation_root: String,
    pub(crate) data_directory: String,
    pub(crate) save_directory: String,
    created_utc: String,
    label: String,
    pub(crate) inventory: Inventory,
}

pub(crate) struct Backup {
    pub(crate) manifest: Manifest,
    pub(crate) revision: String,
    pub(crate) payload: PathBuf,
}

pub(crate) fn hash_bytes(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

pub(crate) fn valid_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn valid_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 80
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn valid_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 255 || name == "." || name == ".."
        || name.ends_with(['.', ' ']) || name.chars().any(char::is_control)
        || name.contains(['<', '>', ':', '"', '/', '\\', '|', '?', '*']) {
        return false;
    }
    let stem = name.split('.').next().unwrap_or_default().to_uppercase();
    !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$")
        && !["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(*prefix).is_some_and(|suffix| {
                matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³")
            })
        })
}

fn validate_relative(value: &str) -> Result<(), String> {
    if value.is_empty() || value.contains('\\') || value.split('/').count() > MAX_DEPTH
        || !value.split('/').all(valid_name) {
        return Err("A save-folder path is unsafe or exceeds the depth limit of 16.".into());
    }
    Ok(())
}

fn ordinary(path: &Path) -> Result<String, String> {
    let text = path.to_str().ok_or("Save and backup paths must be valid Unicode.")?;
    if text.get(..8).is_some_and(|prefix| prefix.eq_ignore_ascii_case("\\\\?\\UNC\\")) {
        return Ok(format!("\\\\{}", &text[8..]));
    }
    Ok(text.strip_prefix("\\\\?\\").unwrap_or(text).to_owned())
}

fn path_key(value: &str) -> String { value.replace('\\', "/").trim_end_matches('/').to_lowercase() }

fn validate_absolute(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if value.is_empty() || value.len() > 32767 || !path.is_absolute()
        || value.chars().any(char::is_control) || value.starts_with("\\\\?\\") || value.starts_with("\\\\.\\")
        || value.split(['/', '\\']).any(|part| part == "." || part == "..") {
        return Err("Select an ordinary absolute path to the complete native save folder.".into());
    }
    for part in path.components() {
        match part {
            Component::Normal(name) if name.to_str().is_some_and(valid_name) => {}
            Component::Prefix(_) | Component::RootDir => {}
            _ => return Err("The selected save folder contains an unsafe path component.".into()),
        }
    }
    Ok(())
}

// A missing final component is legitimate between the two restore renames. Its
// existing parent is still resolved and validated; no missing ancestry is guessed.
pub(crate) fn selected_folder(data: &str, value: &str) -> Result<PathBuf, String> {
    validate_absolute(value)?;
    mod_update::validate_directory(Path::new(data))?;
    let data_path = fs::canonicalize(data).map_err(|error| error.to_string())?;
    let requested = Path::new(value);
    let resolved = match fs::symlink_metadata(requested) {
        Ok(_) => {
            mod_update::validate_directory(requested)?;
            fs::canonicalize(requested).map_err(|error| error.to_string())?
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let parent = requested.parent().ok_or("The selected save folder has no parent.")?;
            mod_update::validate_directory(parent)?;
            fs::canonicalize(parent).map_err(|error| error.to_string())?
                .join(requested.file_name().ok_or("The selected save folder has no name.")?)
        }
        Err(error) => return Err(format!("Could not inspect the selected save folder: {error}")),
    };
    let canonical = ordinary(&resolved)?;
    if path_key(&canonical) != path_key(value) {
        return Err("The save folder resolves through a different path or alias. Select its canonical folder again.".into());
    }
    let data_key = path_key(&ordinary(&data_path)?);
    let selected_key = path_key(&canonical);
    let relative = selected_key.strip_prefix(&format!("{data_key}/"))
        .filter(|value| !value.is_empty()).ok_or("Choose a save folder inside the configured data folder, not the data folder itself.")?;
    if relative.split('/').next().is_some_and(|name| name == STORE || name.starts_with(STORE_PREPARATION_PREFIX)) {
        return Err("The backup store cannot be selected as a native save folder.".into());
    }
    Ok(PathBuf::from(canonical))
}

pub(crate) fn directory_present(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => { mod_update::validate_directory(path)?; Ok(true) }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            mod_update::validate_directory(path.parent().ok_or("Missing folder parent.")?)?;
            Ok(false)
        }
        Err(error) => Err(format!("Could not inspect a save or backup folder: {error}")),
    }
}

fn open_file(folder: &Path, relative: &str) -> Result<File, String> {
    if !mod_update::validate_target(folder, Path::new(relative))? {
        return Err("A required save or recovery file is missing. Current files were preserved.".into());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1).custom_flags(0x00200000); // Read sharing only; open the reparse point itself.
    }
    let file = options.open(folder.join(relative)).map_err(|error| format!("Could not read a save or recovery file: {error}"))?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    #[cfg(windows)]
    let linked = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let linked = metadata.file_type().is_symlink();
    if linked || !metadata.is_file() {
        return Err("Save and recovery files must be ordinary files, without links or reparse points.".into());
    }
    Ok(file)
}

pub(crate) fn read_record<T: serde::de::DeserializeOwned>(folder: &Path, name: &str) -> Result<(T, String), String> {
    let file = open_file(folder, name)?;
    if file.metadata().map_err(|error| error.to_string())?.len() > MAX_RECORD_BYTES {
        return Err("Recovery metadata exceeds the supported size. It was preserved.".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_RECORD_BYTES + 1).read_to_end(&mut bytes).map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_RECORD_BYTES { return Err("Recovery metadata exceeds the supported size.".into()); }
    let record = serde_json::from_slice(&bytes).map_err(|_| "Recovery metadata is invalid or unsupported. It was preserved.")?;
    Ok((record, hash_bytes(&bytes)))
}

pub(crate) fn write_record<T: Serialize>(folder: &Path, name: &str, record: &T) -> Result<String, String> {
    if mod_update::validate_target(folder, Path::new(name))? {
        return Err("Recovery metadata already exists. Its contents were preserved.".into());
    }
    let bytes = serde_json::to_vec_pretty(record).map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_RECORD_BYTES { return Err("Recovery metadata exceeds the supported size.".into()); }
    let mut temporary = tempfile::NamedTempFile::new_in(folder).map_err(|error| error.to_string())?;
    temporary.write_all(&bytes).and_then(|_| temporary.as_file().sync_all()).map_err(|error| error.to_string())?;
    let temporary = temporary.into_temp_path();
    mod_update::durable_rename(&temporary, &folder.join(name), false)?;
    Ok(hash_bytes(&bytes))
}

fn hash_or_copy(folder: &Path, entry: &str, destination: Option<&Path>, limit: u64) -> Result<(u64, String), String> {
    let mut source = open_file(folder, entry)?;
    if source.metadata().map_err(|error| error.to_string())?.len() > limit {
        return Err("A save file exceeds its recorded size, the 1 GiB file limit, or the 4 GiB folder limit.".into());
    }
    let mut temporary = if let Some(destination) = destination {
        let parent = destination.parent().ok_or("Missing copy destination parent.")?;
        mod_update::validate_directory(parent)?;
        Some(tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?)
    } else { None };
    let mut hash = Sha256::new();
    let mut bytes = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let count = source.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 { break; }
        bytes += count as u64;
        if bytes > limit { return Err("A save file grew beyond its recorded size or supported limits. Current files were preserved.".into()); }
        hash.update(&buffer[..count]);
        if let Some(output) = temporary.as_mut() {
            output.write_all(&buffer[..count]).map_err(|error| error.to_string())?;
        }
    }
    if let (Some(output), Some(destination)) = (temporary, destination) {
        output.as_file().sync_all().map_err(|error| error.to_string())?;
        let output = output.into_temp_path();
        mod_update::durable_rename(&output, destination, false)?;
    }
    Ok((bytes, format!("{:x}", hash.finalize())))
}

fn scan(folder: &Path, relative: &str, inventory: &mut Inventory, names: &mut HashSet<String>, bytes: &mut u64) -> Result<(), String> {
    let current = if relative.is_empty() { folder.to_owned() } else { folder.join(relative) };
    mod_update::validate_directory(&current)?;
    for entry in fs::read_dir(&current).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name().into_string().map_err(|_| "Save filenames must be valid Unicode.")?;
        let path = if relative.is_empty() { name } else { format!("{relative}/{name}") };
        validate_relative(&path)?;
        if !names.insert(path.to_lowercase()) { return Err("The save folder contains ambiguous filename aliases.".into()); }
        let metadata = fs::symlink_metadata(entry.path()).map_err(|error| error.to_string())?;
        if metadata.is_dir() {
            if inventory.directories.len() >= MAX_FILES { return Err("The save folder exceeds 4096 subfolders.".into()); }
            mod_update::validate_directory(&entry.path())?;
            inventory.directories.push(path.clone());
            scan(folder, &path, inventory, names, bytes)?;
        } else {
            if inventory.files.len() >= MAX_FILES { return Err("The save folder exceeds 4096 files.".into()); }
            let (size, sha256) = hash_or_copy(folder, &path, None, MAX_FILE_BYTES.min(MAX_BYTES - *bytes))?;
            *bytes += size;
            if *bytes > MAX_BYTES { return Err("The complete save folder exceeds 4 GiB.".into()); }
            inventory.files.push(WorldFile { path, bytes: size, sha256 });
        }
    }
    Ok(())
}

pub(crate) fn inspect(folder: &Path) -> Result<Inventory, String> {
    let mut inventory = Inventory { directories: Vec::new(), files: Vec::new() };
    let mut bytes = 0;
    scan(folder, "", &mut inventory, &mut HashSet::new(), &mut bytes)?;
    inventory.directories.sort();
    inventory.files.sort_by(|left, right| left.path.cmp(&right.path));
    validate_inventory(&inventory)?;
    Ok(inventory)
}

fn validate_inventory(inventory: &Inventory) -> Result<(), String> {
    if inventory.files.len() > MAX_FILES || inventory.directories.len() > MAX_FILES {
        return Err("The save inventory exceeds supported file or folder limits.".into());
    }
    let mut names = HashSet::new();
    let mut directories = HashSet::new();
    let mut previous: Option<&str> = None;
    for path in &inventory.directories {
        validate_relative(path)?;
        if previous.is_some_and(|previous| previous >= path.as_str()) || !names.insert(path.to_lowercase()) {
            return Err("The save inventory contains duplicate or unordered folders.".into());
        }
        directories.insert(path.as_str());
        previous = Some(path);
    }
    let mut bytes = 0u64;
    previous = None;
    for file in &inventory.files {
        validate_relative(&file.path)?;
        if file.bytes > MAX_FILE_BYTES || !valid_hash(&file.sha256)
            || previous.is_some_and(|previous| previous >= file.path.as_str()) || !names.insert(file.path.to_lowercase()) {
            return Err("The save inventory contains invalid or duplicate files.".into());
        }
        bytes += file.bytes;
        previous = Some(&file.path);
    }
    if bytes > MAX_BYTES { return Err("The save inventory exceeds 4 GiB.".into()); }
    for path in inventory.directories.iter().map(String::as_str).chain(inventory.files.iter().map(|file| file.path.as_str())) {
        if let Some((parent, _)) = path.rsplit_once('/') {
            if !directories.contains(parent) { return Err("The save inventory omits a parent folder.".into()); }
        }
    }
    if !inventory.files.iter().any(|file| file.path.eq_ignore_ascii_case("SaveData.zip") || file.path.eq_ignore_ascii_case("SaveData.json")) {
        return Err("Select the complete native save folder containing SaveData.zip or SaveData.json at its root.".into());
    }
    Ok(())
}

pub(crate) fn require_inventory(folder: &Path, expected: &Inventory) -> Result<(), String> {
    if &inspect(folder)? != expected {
        return Err("A save or recovery folder changed after it was read. Current files were preserved; read its state again.".into());
    }
    Ok(())
}

pub(crate) fn copy_inventory(source: &Path, destination: &Path, expected: &Inventory) -> Result<(), String> {
    validate_inventory(expected)?;
    mod_update::validate_directory(source)?;
    if directory_present(destination)? { return Err("The prepared save folder already exists. It was preserved.".into()); }
    fs::create_dir(destination).map_err(|error| error.to_string())?;
    for relative in &expected.directories {
        let path = destination.join(relative);
        mod_update::validate_directory(path.parent().ok_or("Missing backup parent.")?)?;
        fs::create_dir(&path).map_err(|error| error.to_string())?;
    }
    let mut total = 0u64;
    for entry in &expected.files {
        let (bytes, hash) = hash_or_copy(source, &entry.path, Some(&destination.join(&entry.path)), entry.bytes)?;
        total += bytes;
        if bytes != entry.bytes || hash != entry.sha256 || total > MAX_BYTES {
            return Err("The source save changed while copying. The prepared folder was retained and no live folder was replaced.".into());
        }
    }
    require_inventory(destination, expected)?;
    require_inventory(source, expected)
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoreMarker { schema_version: u32, format: String }

pub(crate) fn store(data: &Path, create: bool) -> Result<Option<PathBuf>, String> {
    mod_update::validate_directory(data)?;
    let folder = data.join(STORE);
    if !directory_present(&folder)? {
        if !create { return Ok(None); }
        let mut retained = 0;
        for entry in fs::read_dir(data).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            if entry.file_name().to_str().is_some_and(|name| name.to_lowercase().starts_with(STORE_PREPARATION_PREFIX)) {
                mod_update::validate_directory(&entry.path())?;
                retained += 1;
                if retained >= MAX_BACKUPS {
                    return Err("There are already 64 retained backup-store preparations. Preserve or archive them before continuing.".into());
                }
            }
        }
        let preparation = tempfile::Builder::new().prefix(STORE_PREPARATION_PREFIX).rand_bytes(16)
            .tempdir_in(data).map_err(|error| format!("Could not prepare the backup store: {error}"))?.into_path();
        write_record(&preparation, "store.json", &StoreMarker { schema_version: 1, format: "modstoke-native-world-backups".into() })?;
        for name in ["backups", "preparations", "transactions"] {
            fs::create_dir(preparation.join(name)).map_err(|error| error.to_string())?;
        }
        // Only publish the final store name after its marker and layout exist.
        // Failed first-use preparation cannot masquerade as a usable store.
        rename_directory(&preparation, &folder)?;
    }
    let (marker, _): (StoreMarker, _) = read_record(&folder, "store.json")?;
    if marker.schema_version != 1 || marker.format != "modstoke-native-world-backups" {
        return Err("The selected data folder contains an unsupported backup store. Its files were preserved.".into());
    }
    for name in ["backups", "preparations", "transactions"] { mod_update::validate_directory(&folder.join(name))?; }
    Ok(Some(folder))
}

pub(crate) fn bounded_directories(folder: &Path, maximum: usize) -> Result<Vec<String>, String> {
    mod_update::validate_directory(folder)?;
    let mut names = Vec::new();
    for entry in fs::read_dir(folder).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name().into_string().map_err(|_| "A recovery folder name is invalid.")?;
        if !valid_id(&name) { return Err("The backup store contains an unrecognized folder. Its files were preserved.".into()); }
        mod_update::validate_directory(&entry.path())?;
        names.push(name);
        if names.len() > maximum { return Err("The backup store exceeds its retained-folder limit. No files were removed.".into()); }
    }
    names.sort();
    Ok(names)
}

pub(crate) fn new_workspace(folder: &Path, prefix: &str) -> Result<PathBuf, String> {
    if bounded_directories(folder, MAX_BACKUPS)?.len() >= MAX_BACKUPS {
        return Err("The backup store already has 64 retained workspaces of this kind. Preserve or archive them before continuing; Manager does not prune them.".into());
    }
    let temporary = tempfile::Builder::new().prefix(prefix).rand_bytes(16).tempdir_in(folder)
        .map_err(|error| format!("Could not prepare a save workspace: {error}"))?;
    // Failed preparations remain available for inspection. Never recursively
    // delete a directory that may have been changed by another process.
    Ok(temporary.into_path())
}

pub(crate) fn rename_directory(source: &Path, destination: &Path) -> Result<(), String> {
    mod_update::validate_directory(source)?;
    if directory_present(destination)? { return Err("A restore destination already exists. Its contents were preserved.".into()); }
    #[cfg(windows)]
    { mod_update::durable_rename(source, destination, false) }
    #[cfg(not(windows))]
    { Err("Native save-folder swaps are supported on Windows only.".into()) }
}

fn utc_now() -> Result<String, String> {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|error| error.to_string())?.as_secs();
    let days = (seconds / 86400) as i64;
    // Gregorian civil date from Unix days; no locale-dependent timestamp parsing.
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    let time = seconds % 86400;
    Ok(format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", time / 3600, (time / 60) % 60, time % 60))
}

pub(crate) fn require_binding(access: &server_launch::ServerWorldAccess, selected: &Path, manifest: &Manifest) -> Result<(), String> {
    if manifest.installation_id != access.installation_id || manifest.installation_root != access.root
        || manifest.data_directory != access.data_directory || manifest.save_directory != ordinary(selected)? {
        return Err("This backup belongs to a different installation, data folder, or selected save folder.".into());
    }
    Ok(())
}

fn require_backup_layout(folder: &Path) -> Result<(), String> {
    let mut names = HashSet::new();
    for entry in fs::read_dir(folder).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name().into_string().map_err(|_| "The backup contains an invalid filename.")?;
        if !matches!(name.as_str(), "manifest.json" | "payload") || !names.insert(name) {
            return Err("The backup contains unrecognized files or aliases. Its files were preserved.".into());
        }
    }
    if names.len() != 2 { return Err("The backup is incomplete. Its files were preserved.".into()); }
    Ok(())
}

pub(crate) fn read_backup(store: &Path, id: &str) -> Result<Backup, String> {
    if !valid_id(id) { return Err("The backup ID is invalid.".into()); }
    let folder = store.join("backups").join(id);
    mod_update::validate_directory(&folder)?;
    require_backup_layout(&folder)?;
    let (manifest, revision): (Manifest, _) = read_record(&folder, "manifest.json")?;
    if manifest.schema_version != 1 || manifest.id != id || !valid_hash(&manifest.installation_id)
        || manifest.created_utc.len() != 20 || !matches!(manifest.label.as_str(), "Manual backup" | "Before restore") {
        return Err("The backup manifest is invalid or unsupported. It was preserved.".into());
    }
    validate_absolute(&manifest.installation_root)?;
    validate_absolute(&manifest.data_directory)?;
    validate_absolute(&manifest.save_directory)?;
    validate_inventory(&manifest.inventory)?;
    let payload = folder.join("payload");
    mod_update::validate_directory(&payload)?;
    Ok(Backup { manifest, revision, payload })
}

pub(crate) fn require_backup(access: &server_launch::ServerWorldAccess, selected: &Path, store: &Path,
    id: &str, expected_revision: &str) -> Result<Backup, String> {
    let backup = read_backup(store, id)?;
    require_binding(access, selected, &backup.manifest)?;
    if backup.revision != expected_revision { return Err("The selected backup changed. Read its current revision before continuing.".into()); }
    require_inventory(&backup.payload, &backup.manifest.inventory)?;
    if read_backup(store, id)?.revision != expected_revision {
        return Err("The backup manifest changed while its payload was read. All files were preserved.".into());
    }
    Ok(backup)
}

pub(crate) fn create_backup(access: &server_launch::ServerWorldAccess, selected: &Path,
    store: &Path, inventory: &Inventory, label: &str) -> Result<Backup, String> {
    if bounded_directories(&store.join("backups"), MAX_BACKUPS)?.len() >= MAX_BACKUPS {
        return Err("This data folder already contains 64 backups. Preserve or archive them before creating another; Manager does not prune backups.".into());
    }
    let preparation = new_workspace(&store.join("preparations"), "backup-")?;
    let id = preparation.file_name().and_then(|name| name.to_str()).ok_or("Invalid backup workspace.")?.to_owned();
    let payload = preparation.join("payload");
    copy_inventory(selected, &payload, inventory)?;
    let manifest = Manifest { schema_version: 1, id: id.clone(), installation_id: access.installation_id.clone(),
        installation_root: access.root.clone(), data_directory: access.data_directory.clone(), save_directory: ordinary(selected)?,
        created_utc: utc_now()?, label: label.into(), inventory: inventory.clone() };
    let revision = write_record(&preparation, "manifest.json", &manifest)?;
    access.require_unchanged()?;
    if selected_folder(&access.data_directory, &manifest.save_directory)? != selected {
        return Err("The selected save folder changed while the backup was prepared.".into());
    }
    require_inventory(&payload, inventory)?;
    require_backup_layout(&preparation)?;
    if read_record::<Manifest>(&preparation, "manifest.json")?.1 != revision {
        return Err("The prepared backup manifest changed. Its files were preserved.".into());
    }
    require_inventory(selected, inventory)?;
    access.require_unchanged()?;
    require_no_pending(Path::new(&access.data_directory))?;
    rename_directory(&preparation, &store.join("backups").join(&id))?;
    require_backup(access, selected, store, &id, &revision)
}

// Presence alone blocks launch, including a malformed file or a directory at the
// marker path. Reading an invalid journal must never make the server launchable.
pub(crate) fn require_no_pending(data: &Path) -> Result<(), String> {
    match fs::symlink_metadata(data) {
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("Could not inspect the server data folder: {error}")),
        Ok(_) => mod_update::validate_directory(data)?,
    }
    let folder = data.join(STORE);
    if !directory_present(&folder)? { return Ok(()); }
    match fs::symlink_metadata(folder.join(PENDING)) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Could not inspect pending save recovery: {error}. Starting the server is blocked.")),
        Ok(_) => Err("A native save-folder restore is pending. Read World recovery and explicitly finish or undo it before launching or changing the data folder.".into()),
    }
}

pub(crate) fn snapshot(access: &server_launch::ServerWorldAccess, save_directory: &str) -> Result<WorldState, String> {
    let store = store(Path::new(&access.data_directory), false)?;
    let pending = store.as_deref().map(server_world_restore::read_pending).transpose()?.flatten();
    let requested = if save_directory.is_empty() {
        pending.as_ref().map(|pending| pending.record.save_directory.as_str())
            .ok_or("Choose the existing native save folder containing SaveData.zip or SaveData.json. No pending restore was found.")?
    } else { save_directory };
    let selected = selected_folder(&access.data_directory, requested)?;
    if let Some(pending) = &pending { server_world_restore::require_pending_binding(access, &selected, pending)?; }
    let listed = (|| {
        let mut backups = Vec::new();
        if let Some(store) = &store {
            for id in bounded_directories(&store.join("backups"), MAX_BACKUPS)? {
                let backup = read_backup(store, &id)?;
                if require_binding(access, &selected, &backup.manifest).is_ok() {
                    backups.push(BackupSummary { id, revision: backup.revision, created_utc: backup.manifest.created_utc,
                        label: backup.manifest.label, files: backup.manifest.inventory.files.len(), bytes: backup.manifest.inventory.bytes() });
                }
            }
        }
        Ok::<_, String>(backups)
    })();
    // An unrelated damaged backup must not hide an otherwise readable pending
    // journal. Recovery independently validates both of its referenced backups.
    let (mut backups, listing_problem) = match listed {
        Ok(backups) => (backups, None),
        Err(error) => (Vec::new(), Some(format!("The backup list is unavailable: {error}"))),
    };
    backups.sort_by(|left, right| right.created_utc.cmp(&left.created_utc).then_with(|| right.id.cmp(&left.id)));
    let (world_revision, files, bytes, problem) = if directory_present(&selected)? {
        match inspect(&selected) {
            Ok(inventory) => (Some(inventory.revision()?), inventory.files.len(), inventory.bytes(), None),
            Err(problem) => (None, 0, 0, Some(problem)),
        }
    } else {
        (None, 0, 0, pending.is_none().then(|| "The selected native save folder is missing. Existing backups were preserved.".into()))
    };
    access.require_unchanged()?;
    let current_pending = store.as_deref().map(server_world_restore::read_pending).transpose()?.flatten();
    if current_pending.as_ref().map(|pending| &pending.revision) != pending.as_ref().map(|pending| &pending.revision) {
        return Err("The pending restore changed while its state was read. Read World recovery again.".into());
    }
    Ok(WorldState { installation_id: access.installation_id.clone(), data_directory: access.data_directory.clone(),
        save_directory: ordinary(&selected)?, save_slot: access.save_slot, world_revision, files, bytes, backups,
        pending: pending.map(|pending| PendingSummary { revision: pending.revision,
            detail: "A folder restore was interrupted. Finish applies the selected backup; undo restores the complete before-restore folder. Both require unchanged recorded folders and backups.".into() }),
        problem: match (problem, listing_problem) {
            (Some(world), Some(backups)) => Some(format!("{world} {backups}")),
            (world, backups) => world.or(backups),
        } })
}

#[tauri::command]
pub async fn read_server_world(app: tauri::AppHandle, executable: String, installation_id: String,
    expected_revision: String, expected_config_revision: String, save_directory: String) -> Result<WorldState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let access = server_launch::acquire_world_access(&app, &executable, &installation_id, &expected_revision, &expected_config_revision)?;
        snapshot(&access, &save_directory)
    }).await.map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn create_server_world_backup(app: tauri::AppHandle, executable: String, installation_id: String,
    expected_revision: String, expected_config_revision: String, save_directory: String,
    expected_world_revision: String) -> Result<WorldState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let access = server_launch::acquire_world_access(&app, &executable, &installation_id, &expected_revision, &expected_config_revision)?;
        require_no_pending(Path::new(&access.data_directory))?;
        let selected = selected_folder(&access.data_directory, &save_directory)?;
        let inventory = inspect(&selected)?;
        if inventory.revision()? != expected_world_revision {
            return Err("The native save folder changed. Read its current state before creating a backup.".into());
        }
        let store = store(Path::new(&access.data_directory), true)?.ok_or("The backup store is unavailable.")?;
        create_backup(&access, &selected, &store, &inventory, "Manual backup")?;
        snapshot(&access, &save_directory)
    }).await.map_err(|error| error.to_string())?
}
