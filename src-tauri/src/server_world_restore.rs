use crate::{mod_update, server_launch, server_world};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use server_world::{Backup, Inventory, WorldState, MAX_BACKUPS, PENDING};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PendingRecord {
    schema_version: u32,
    transaction_id: String,
    installation_id: String,
    installation_root: String,
    data_directory: String,
    pub(crate) save_directory: String,
    before_backup_id: String,
    before_backup_revision: String,
    after_backup_id: String,
    after_backup_revision: String,
    before_world_revision: String,
    after_world_revision: String,
}

pub(crate) struct Pending {
    pub(crate) record: PendingRecord,
    pub(crate) revision: String,
}

pub(crate) fn read_pending(store: &Path) -> Result<Option<Pending>, String> {
    mod_update::validate_directory(store)?;
    match fs::symlink_metadata(store.join(PENDING)) {
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Could not inspect pending save recovery: {error}")),
        Ok(_) => {}
    }
    let (record, revision): (PendingRecord, _) = server_world::read_record(store, PENDING)?;
    if record.schema_version != 1 || !server_world::valid_id(&record.transaction_id)
        || !server_world::valid_id(&record.before_backup_id) || !server_world::valid_id(&record.after_backup_id)
        || record.before_backup_id == record.after_backup_id
        || !server_world::valid_hash(&record.installation_id)
        || !server_world::valid_hash(&record.before_backup_revision) || !server_world::valid_hash(&record.after_backup_revision)
        || !server_world::valid_hash(&record.before_world_revision) || !server_world::valid_hash(&record.after_world_revision)
        || record.before_world_revision == record.after_world_revision {
        return Err("The pending save-restore journal is invalid or unsupported. All folders were preserved and server launch remains blocked.".into());
    }
    mod_update::validate_directory(&store.join("transactions").join(&record.transaction_id))?;
    Ok(Some(Pending { record, revision }))
}

pub(crate) fn require_pending_binding(access: &server_launch::ServerWorldAccess, selected: &Path, pending: &Pending) -> Result<(), String> {
    let record = &pending.record;
    if record.installation_id != access.installation_id || record.installation_root != access.root
        || record.data_directory != access.data_directory || selected.to_str() != Some(record.save_directory.as_str()) {
        return Err("The pending restore belongs to a different installation or selected save folder. Read pending recovery for its original profile; current files were preserved.".into());
    }
    Ok(())
}

fn require_journal(store: &Path, expected: &str) -> Result<(), String> {
    if read_pending(store)?.as_ref().map(|pending| pending.revision.as_str()) != Some(expected) {
        return Err("The pending restore changed. Read its current state before choosing finish or undo.".into());
    }
    Ok(())
}

#[derive(PartialEq, Eq)]
enum Position {
    Before,
    Between,
    After,
}

struct Observed {
    before: Backup,
    after: Backup,
    transaction: PathBuf,
    position: Position,
}

fn optional_inventory(path: &Path) -> Result<Option<Inventory>, String> {
    if server_world::directory_present(path)? { server_world::inspect(path).map(Some) } else { Ok(None) }
}

fn require_transaction_layout(transaction: &Path) -> Result<(), String> {
    mod_update::validate_directory(transaction)?;
    let mut count = 0;
    for entry in fs::read_dir(transaction).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name();
        if !matches!(name.to_str(), Some("staged" | "displaced")) {
            return Err("The restore workspace contains unrecognized files. All files were preserved; recovery cannot guess their ownership.".into());
        }
        mod_update::validate_directory(&entry.path())?;
        count += 1;
        if count > 2 { return Err("The restore workspace contains ambiguous folder names.".into()); }
    }
    Ok(())
}

// Every accepted position corresponds to one complete same-volume directory
// rename. The same three positions work in either direction, even if the process
// exits during recovery. No unknown or externally changed folder is overwritten.
fn observe(access: &server_launch::ServerWorldAccess, selected: &Path, store: &Path, pending: &Pending) -> Result<Observed, String> {
    access.require_unchanged()?;
    require_pending_binding(access, selected, pending)?;
    require_journal(store, &pending.revision)?;
    if server_world::selected_folder(&access.data_directory, &pending.record.save_directory)? != selected {
        return Err("The selected save folder changed while recovery was prepared.".into());
    }
    let record = &pending.record;
    let before = server_world::require_backup(access, selected, store, &record.before_backup_id, &record.before_backup_revision)?;
    let after = server_world::require_backup(access, selected, store, &record.after_backup_id, &record.after_backup_revision)?;
    if before.manifest.inventory.revision()? != record.before_world_revision
        || after.manifest.inventory.revision()? != record.after_world_revision {
        return Err("The pending journal no longer matches its retained backups. All files were preserved.".into());
    }
    let transaction = store.join("transactions").join(&record.transaction_id);
    require_transaction_layout(&transaction)?;
    let staged = optional_inventory(&transaction.join("staged"))?;
    let displaced = optional_inventory(&transaction.join("displaced"))?;
    let live = optional_inventory(selected)?;
    let old = &before.manifest.inventory;
    let new = &after.manifest.inventory;
    let position = if live.as_ref() == Some(old) && staged.as_ref() == Some(new) && displaced.is_none() {
        Position::Before
    } else if live.is_none() && staged.as_ref() == Some(new) && displaced.as_ref() == Some(old) {
        Position::Between
    } else if live.as_ref() == Some(new) && staged.is_none() && displaced.as_ref() == Some(old) {
        Position::After
    } else {
        return Err("The live save or retained restore folders no longer match a recognized before, between, or after state. All folders were preserved; finish and undo are blocked.".into());
    };
    access.require_unchanged()?;
    require_journal(store, &pending.revision)?;
    Ok(Observed { before, after, transaction, position })
}

fn move_recognized(access: &server_launch::ServerWorldAccess, store: &Path, pending: &Pending,
    source: &Path, destination: &Path, inventory: &Inventory) -> Result<(), String> {
    access.require_unchanged()?;
    require_journal(store, &pending.revision)?;
    server_world::require_inventory(source, inventory)?;
    access.require_unchanged()?;
    require_journal(store, &pending.revision)?;
    server_world::rename_directory(source, destination)
}

fn reconcile(access: &server_launch::ServerWorldAccess, selected: &Path, store: &Path,
    pending: &Pending, finish: bool) -> Result<(), String> {
    let mut observed = observe(access, selected, store, pending)?;
    if finish && observed.position == Position::Before {
        move_recognized(access, store, pending, selected, &observed.transaction.join("displaced"), &observed.before.manifest.inventory)?;
        observed = observe(access, selected, store, pending)?;
    } else if !finish && observed.position == Position::After {
        move_recognized(access, store, pending, selected, &observed.transaction.join("staged"), &observed.after.manifest.inventory)?;
        observed = observe(access, selected, store, pending)?;
    }
    if observed.position == Position::Between {
        let (source, inventory) = if finish {
            (observed.transaction.join("staged"), &observed.after.manifest.inventory)
        } else {
            (observed.transaction.join("displaced"), &observed.before.manifest.inventory)
        };
        move_recognized(access, store, pending, &source, selected, inventory)?;
        observed = observe(access, selected, store, pending)?;
    }
    if observed.position != (if finish { Position::After } else { Position::Before }) {
        return Err("The restore did not reach the chosen final state. Its journal and folders were retained.".into());
    }
    access.require_unchanged()?;
    require_journal(store, &pending.revision)?;
    let complete = if finish { "finished.json" } else { "undone.json" };
    if mod_update::validate_target(&observed.transaction, Path::new(complete))? {
        return Err("A completion record already exists. The pending restore was preserved.".into());
    }
    // Archiving the intent is the commit point. Until this durable move succeeds,
    // launch remains blocked and either explicit recovery direction is available.
    mod_update::durable_rename(&store.join(PENDING), &observed.transaction.join(complete), false)
}

fn begin_restore(access: &server_launch::ServerWorldAccess, save_directory: &str,
    expected_world_revision: &str, backup_id: &str, expected_backup_revision: &str) -> Result<(), String> {
    server_world::require_no_pending(Path::new(&access.data_directory))?;
    let selected = server_world::selected_folder(&access.data_directory, save_directory)?;
    let before = server_world::inspect(&selected)?;
    if before.revision()? != expected_world_revision {
        return Err("The current save folder changed. Read its state again before restoring.".into());
    }
    let store = server_world::store(Path::new(&access.data_directory), false)?.ok_or("No save backups were found in this data folder.")?;
    let after = server_world::require_backup(access, &selected, &store, backup_id, expected_backup_revision)?;
    if before == after.manifest.inventory {
        return Err("The selected backup already matches the complete native save folder. No restore is needed.".into());
    }
    if server_world::bounded_directories(&store.join("backups"), MAX_BACKUPS)?.len() >= MAX_BACKUPS {
        return Err("A restore needs space for a retained undo backup, but this data folder already has 64 backups. No files were changed.".into());
    }
    let transaction = server_world::new_workspace(&store.join("transactions"), "restore-")?;
    server_world::copy_inventory(&after.payload, &transaction.join("staged"), &after.manifest.inventory)?;
    let undo = server_world::create_backup(access, &selected, &store, &before, "Before restore")?;
    let record = PendingRecord { schema_version: 1,
        transaction_id: transaction.file_name().and_then(|name| name.to_str()).ok_or("Invalid restore workspace.")?.into(),
        installation_id: access.installation_id.clone(), installation_root: access.root.clone(),
        data_directory: access.data_directory.clone(), save_directory: selected.to_str().ok_or("Invalid save folder path.")?.into(),
        before_backup_id: undo.manifest.id.clone(), before_backup_revision: undo.revision.clone(),
        after_backup_id: after.manifest.id.clone(), after_backup_revision: after.revision.clone(),
        before_world_revision: before.revision()?, after_world_revision: after.manifest.inventory.revision()? };
    access.require_unchanged()?;
    server_world::require_no_pending(Path::new(&access.data_directory))?;
    server_world::require_backup(access, &selected, &store, &record.before_backup_id, &record.before_backup_revision)?;
    server_world::require_backup(access, &selected, &store, &record.after_backup_id, &record.after_backup_revision)?;
    require_transaction_layout(&transaction)?;
    server_world::require_inventory(&transaction.join("staged"), &after.manifest.inventory)?;
    server_world::require_inventory(&selected, &before)?;
    if server_world::directory_present(&transaction.join("displaced"))? {
        return Err("The restore workspace changed during preparation. All folders were preserved.".into());
    }
    access.require_unchanged()?;
    if server_world::selected_folder(&access.data_directory, save_directory)? != selected {
        return Err("The selected save folder changed before its restore intent was published.".into());
    }
    // No live-folder rename is permitted until all snapshot bytes and this
    // immutable intent have been flushed and durably published.
    let revision = server_world::write_record(&store, PENDING, &record)?;
    reconcile(access, &selected, &store, &Pending { record, revision }, true)
}

#[tauri::command]
pub async fn restore_server_world_backup(app: tauri::AppHandle, executable: String, installation_id: String,
    expected_revision: String, expected_config_revision: String, save_directory: String,
    expected_world_revision: String, backup_id: String, expected_backup_revision: String) -> Result<WorldState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let access = server_launch::acquire_world_access(&app, &executable, &installation_id, &expected_revision, &expected_config_revision)?;
        begin_restore(&access, &save_directory, &expected_world_revision, &backup_id, &expected_backup_revision)
            .map_err(|error| format!("{error} Read World recovery before taking another action; a prepared or pending restore may have been retained."))?;
        server_world::snapshot(&access, &save_directory)
    }).await.map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn recover_server_world_restore(app: tauri::AppHandle, executable: String, installation_id: String,
    expected_revision: String, expected_config_revision: String, save_directory: String,
    expected_pending_revision: String, direction: String) -> Result<WorldState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let finish = match direction.as_str() { "finish" => true, "undo" => false, _ => return Err("Choose finish or undo for this restore.".into()) };
        if save_directory.is_empty() { return Err("Read pending recovery and use its exact selected save folder before choosing finish or undo.".into()); }
        let access = server_launch::acquire_world_access(&app, &executable, &installation_id, &expected_revision, &expected_config_revision)?;
        let selected = server_world::selected_folder(&access.data_directory, &save_directory)?;
        let store = server_world::store(Path::new(&access.data_directory), false)?.ok_or("No pending save recovery was found.")?;
        let pending = read_pending(&store)?.ok_or("No pending save recovery was found. Read its current state before continuing.")?;
        if pending.revision != expected_pending_revision {
            return Err("The pending save restore changed. Read its current state before choosing finish or undo.".into());
        }
        reconcile(&access, &selected, &store, &pending, finish)
            .map_err(|error| format!("{error} Recovery remains pending. Read its current state before choosing another action."))?;
        server_world::snapshot(&access, &save_directory)
    }).await.map_err(|error| error.to_string())?
}
