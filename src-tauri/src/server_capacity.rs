use crate::{mod_update, server_launch, server_native_config, server_world};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use server_native_config::{NativeConfig, CONFIG_FILE, MAX_CONFIG_BYTES};

const BACKUP_DIRECTORY: &str = ".modstoke-config-backups";
const MAX_BACKUPS: usize = 64;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityChange {
    changed: bool,
    max_players: u32,
    config_revision: String,
    backup_path: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigChange {
    changed: bool,
    config_revision: String,
    backup_path: Option<String>,
}

fn backup_directory(data: &Path) -> Result<PathBuf, String> {
    mod_update::validate_directory(data)?;
    let directory = data.join(BACKUP_DIRECTORY);
    if !server_world::directory_present(&directory)? {
        fs::create_dir(&directory)
            .map_err(|error| format!("Could not create the configuration backup folder: {error}"))?;
    }
    mod_update::validate_directory(&directory)?;
    Ok(directory)
}

fn backup_count(directory: &Path) -> Result<usize, String> {
    mod_update::validate_directory(directory)?;
    let mut count = 0;
    for entry in fs::read_dir(directory)
        .map_err(|error| format!("Could not inspect the configuration backup folder: {error}"))? {
        if count == MAX_BACKUPS {
            return Err("The configuration backup folder exceeds 64 entries. Preserve its backups and move older entries elsewhere before changing server settings.".into());
        }
        let entry = entry
            .map_err(|error| format!("Could not inspect a configuration backup entry: {error}"))?;
        if !mod_update::validate_target(directory, Path::new(&entry.file_name()))? {
            return Err("The configuration backup folder changed while it was being read. Read server settings before trying again.".into());
        }
        count += 1;
    }
    Ok(count)
}

fn require_exact_file(directory: &Path, name: &Path, expected: &NativeConfig) -> Result<(), String> {
    if !mod_update::validate_target(directory, name)? {
        return Err("A required configuration or backup file is missing.".into());
    }
    let file = File::open(directory.join(name))
        .map_err(|error| format!("Could not read a configuration or backup file: {error}"))?;
    let metadata = file.metadata()
        .map_err(|error| format!("Could not inspect a configuration or backup file: {error}"))?;
    if !metadata.is_file() || metadata.len() > MAX_CONFIG_BYTES as u64 {
        return Err("Configuration and backup files must be ordinary files no larger than 64 KiB.".into());
    }
    let mut bytes = Vec::new();
    file.take((MAX_CONFIG_BYTES + 1) as u64).read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read a configuration or backup file: {error}"))?;
    if bytes != expected.original_bytes()
        || format!("{:x}", Sha256::digest(&bytes)) != expected.revision {
        return Err("A configuration or backup file changed or does not match its expected original bytes. Read server settings before trying again.".into());
    }
    Ok(())
}

fn durable_temporary(directory: &Path, bytes: &[u8]) -> Result<tempfile::TempPath, String> {
    mod_update::validate_directory(directory)?;
    let mut temporary = tempfile::NamedTempFile::new_in(directory)
        .map_err(|error| format!("Could not stage the configuration file: {error}"))?;
    temporary.write_all(bytes)
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|error| format!("Could not flush the staged configuration file: {error}"))?;
    // Close the handle before MoveFileExW. TempPath removes only the unpublished
    // source name on failure; the hash-named backup is never removed or replaced.
    Ok(temporary.into_temp_path())
}

fn ensure_backup(data: &Path, original: &NativeConfig) -> Result<PathBuf, String> {
    let directory = backup_directory(data)?;
    let count = backup_count(&directory)?;
    let name = format!("{}.{}", original.revision, CONFIG_FILE);
    if !mod_update::validate_target(&directory, Path::new(&name))? {
        if count >= MAX_BACKUPS {
            return Err("The configuration backup folder already has 64 entries. Preserve its backups and move older entries elsewhere before changing server settings.".into());
        }
        let temporary = durable_temporary(&directory, original.original_bytes())?;
        require_exact_file(&directory, Path::new(temporary.file_name()
            .ok_or("The configuration backup staging path has no filename.")?), original)?;
        mod_update::validate_target(&directory, Path::new(&name))?;
        mod_update::durable_rename(&temporary, &directory.join(&name), false)?;
    }
    require_exact_file(&directory, Path::new(&name), original)?;
    Ok(directory.join(name))
}

fn uncertain_commit(error: &str, backup: &Path) -> String {
    format!("The server settings change may have been committed, but its final state could not be confirmed. {error} Read server settings before taking another action. Preserve the original configuration backup at {}.", backup.display())
}

pub(crate) fn write(access: &server_launch::ServerWorldAccess,
    expected_config_revision: &str, max_players: u32) -> Result<CapacityChange, String> {
    if !matches!(max_players, 8 | 16 | 32) {
        return Err("Choose a native server capacity of 8, 16, or 32 players.".into());
    }
    let result = write_config(access, expected_config_revision, |original| {
        if original.summary.max_players == max_players { return Ok(None); }
        original.with_max_players(max_players).map(Some)
    })?;
    Ok(CapacityChange {
        changed: result.changed, max_players, config_revision: result.config_revision,
        backup_path: result.backup_path,
    })
}

pub(crate) fn write_identity(access: &server_launch::ServerWorldAccess,
    expected_config_revision: &str, server_name: &str) -> Result<ConfigChange, String> {
    let name = server_name.trim();
    if name.is_empty() || name.chars().count() > 256 || server_name.chars().any(char::is_control) {
        return Err("Enter a server name of 1 to 256 characters without control characters.".into());
    }
    write_config(access, expected_config_revision, |original| {
        if original.summary.server_name == name { return Ok(None); }
        original.with_server_name(name).map(Some)
    })
}

fn write_config(access: &server_launch::ServerWorldAccess, expected_config_revision: &str,
    change: impl FnOnce(&NativeConfig) -> Result<Option<NativeConfig>, String>) -> Result<ConfigChange, String> {
    let data = Path::new(&access.data_directory);
    access.require_unchanged()?;
    server_world::require_no_pending(data)?;
    let original = server_native_config::read(data)?;
    if original.revision != expected_config_revision {
        return Err("dedicatedserver.cfg changed. Read server settings again before saving.".into());
    }
    let Some(changed) = change(&original)? else {
        access.require_unchanged()?;
        server_world::require_no_pending(data)?;
        return Ok(ConfigChange {
            changed: false, config_revision: original.revision, backup_path: None,
        });
    };

    let backup = ensure_backup(data, &original)?;
    let temporary = durable_temporary(data, changed.original_bytes())?;
    require_exact_file(data, Path::new(temporary.file_name()
        .ok_or("The configuration staging path has no filename.")?), &changed)?;
    let backup_folder = backup.parent().ok_or("The configuration backup has no parent folder.")?;
    let backup_name = Path::new(backup.file_name().ok_or("The configuration backup has no filename.")?);
    backup_count(backup_folder)?;
    require_exact_file(backup_folder, backup_name, &original)?;
    require_exact_file(data, Path::new(CONFIG_FILE), &original)?;
    // The access guard retains the installation, global launch and data locks.
    // Its last check verifies identity, settings, exact original config revision,
    // package readiness and stopped game/server processes immediately before commit.
    access.require_unchanged()?;
    server_world::require_no_pending(data)?;
    mod_update::durable_rename(&temporary, &data.join(CONFIG_FILE), true)
        .map_err(|error| uncertain_commit(&error, &backup))?;
    let installed = server_native_config::read(data)
        .map_err(|error| uncertain_commit(&error, &backup))?;
    if installed.revision != changed.revision || installed.original_bytes() != changed.original_bytes() {
        return Err(uncertain_commit("dedicatedserver.cfg did not match the expected new bytes.", &backup));
    }
    Ok(ConfigChange {
        changed: true, config_revision: installed.revision,
        backup_path: Some(backup.to_string_lossy().into_owned()),
    })
}
