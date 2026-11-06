//! Native package operations for modstoke-native. Inspect and install run the Manager's own
//! checks and transaction; removal uses the same backed-up transaction with no new files.
use super::native_archive::{self, NativeArchive};
use super::native_local::{self, mods_layout, InstallCheck};
use super::*;
use crate::{admin_paths, native_inventory, native_root};
use serde_json::Value;

pub(crate) const PENDING_PLAN: &str = "An unfinished install is recorded in this folder. Run recover, or apply the queue, to roll it back before changing mods.";

pub(crate) struct Violation {
    pub(crate) path: String,
    pub(crate) rule: &'static str,
    pub(crate) message: String,
}

pub(crate) struct FileInfo {
    pub(crate) path: String,
    pub(crate) size: u64,
    pub(crate) sha256: String,
}

// The manifest fields the Loader reads besides identity: author and its fallbacks, and `client`.
// The Loader never reads a description; one is passed on only as the author's free text.
pub(crate) struct Extras {
    pub(crate) author: Option<String>,
    pub(crate) client: String,
    pub(crate) description: Option<String>,
    pub(crate) min_loader: Option<String>,
}

pub(crate) struct ArchiveInfo {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) package_path: String,
    pub(crate) manifest_path: String,
    pub(crate) assembly_path: String,
    pub(crate) sha256: String,
    pub(crate) size: u64,
    pub(crate) files: Vec<FileInfo>,
    pub(crate) required: Vec<(String, String)>,
    pub(crate) extras: Extras,
}

pub(crate) struct Inspection {
    pub(crate) archive: ArchiveInfo,
    pub(crate) action: Option<String>,
    pub(crate) previous_version: Option<String>,
    pub(crate) previous_owner: Option<&'static str>,
    pub(crate) violations: Vec<Violation>,
}

pub(crate) struct Installation {
    pub(crate) archive: ArchiveInfo,
    pub(crate) action: String,
    pub(crate) previous_version: Option<String>,
    pub(crate) written: usize,
    pub(crate) removed: usize,
}

pub(crate) struct Removal {
    pub(crate) id: String,
    pub(crate) version: String,
    pub(crate) removed: usize,
    pub(crate) kept: Vec<String>,
    pub(crate) notices: Vec<String>,
}

pub(crate) struct OwnedPackage {
    pub(crate) id: String,
    pub(crate) version: String,
    pub(crate) package_path: String,
}

pub(crate) fn extras(bytes: &[u8]) -> Extras {
    let object = serde_json::from_slice::<Value>(bytes).ok().and_then(|value| value.as_object().cloned()).unwrap_or_default();
    let text = |name: &str| object.iter()
        .find(|(key, value)| key.eq_ignore_ascii_case(name) && value.as_str().is_some_and(|text| !text.trim().is_empty()))
        .and_then(|(_, value)| value.as_str().map(|text| text.trim().to_owned()));
    let author = ["author", "creator", "createdBy", "created_by", "owner"].into_iter().find_map(text);
    let client = match object.get("client").and_then(Value::as_str) {
        Some(value @ ("required" | "optional" | "server")) => value.to_owned(),
        _ => "unknown".to_owned(),
    };
    Extras { author, client, description: text("description"), min_loader: text("minLoaderVersion") }
}

fn archive_info(archive: &NativeArchive, source: &Path) -> ArchiveInfo {
    ArchiveInfo {
        id: archive.id.clone(),
        name: archive.name.clone(),
        version: archive.version.clone(),
        package_path: archive.package_path.clone(),
        manifest_path: archive.manifest_path.clone(),
        assembly_path: archive.assembly_path.clone(),
        sha256: archive.sha256.clone(),
        size: archive.size,
        files: archive.files.iter().map(|file| FileInfo { path: file.path.clone(), size: file.size, sha256: file.sha256.clone() }).collect(),
        required: archive.required.iter().map(|dependency| (dependency.id.clone(), dependency.version.clone())).collect(),
        extras: archive_manifest(source, archive).map(|bytes| extras(&bytes)).unwrap_or(Extras { author: None, client: "unknown".into(), description: None, min_loader: None }),
    }
}

// The manifest as the archive stores it, under either root folder. inspect already checked its size.
fn archive_manifest(source: &Path, archive: &NativeArchive) -> Option<Vec<u8>> {
    let mut zip = zip::ZipArchive::new(File::open(source).ok()?).ok()?;
    let wanted = key(&archive.manifest_path);
    let name = zip.file_names()
        .find(|name| key(&native_root::rebase(name, mods_layout_of(&archive.package_path))) == wanted)?
        .to_owned();
    let mut bytes = Vec::new();
    zip.by_name(&name).ok()?.take(512 * 1024).read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

fn mods_layout_of(package_path: &str) -> &'static str {
    if package_path.split('/').next().is_some_and(|root| root.eq_ignore_ascii_case(native_root::LEGACY_ROOT)) {
        native_root::LEGACY_ROOT
    } else {
        native_root::ROOT
    }
}

// Everything install would check, without the server stopped and without changing anything.
// Archive problems end the inspection; everything after them is collected.
pub(crate) fn inspect(root: &Path, source: &Path) -> Result<Inspection, String> {
    let archive = native_archive::inspect(source, mods_layout(root))?;
    let state = read_relaxed(root)?;
    let profile = state.profile.as_ref();
    let known = native_local::known_package(profile, &archive.id);
    check_archive(source, &archive, recorded_files(known))?;
    let mut violations = Vec::new();
    for file in &archive.files {
        let path = Path::new(&file.path);
        if native_root::is_protected_content(path) {
            violations.push(Violation { path: file.path.clone(), rule: "protected-content", message: format!("{} is Virginia's private content and is never replaced.", file.path) });
        }
        if admin_paths::is_admin_data(&mod_update::path_key(path)) {
            violations.push(Violation { path: file.path.clone(), rule: "admin-data", message: format!("{} belongs to Modstoke Admin's staff permissions.", file.path) });
        }
        if let Err(message) = mod_update::ensure_unmanaged(root, path) {
            violations.push(Violation { path: file.path.clone(), rule: "vortex", message });
        }
        if is_executable(&file.path) {
            violations.push(Violation { path: file.path.clone(), rule: "policy", message: format!("{} is a program, which mod packages may not install.", file.path) });
        }
    }
    if profile.is_some_and(|profile| profile.pending_plan.is_some()) {
        violations.push(Violation { path: String::new(), rule: "pending", message: PENDING_PLAN.into() });
    }
    let previous_version = known.map(|package| package.version.clone());
    let previous_owner = if known.is_some_and(|package| package.native.is_some()) {
        Some("manager")
    } else if known.is_some() {
        Some("catalog")
    } else {
        native_inventory::read_inventory(&root.to_string_lossy())?.packages.iter()
            .any(|entry| key(&entry.id) == key(&archive.id)).then_some("manual")
    };
    let action = match native_local::prepare_many(root, profile, std::slice::from_ref(&archive)) {
        Ok(prepared) => Some(prepared.action),
        Err(message) => {
            violations.push(Violation { path: archive.package_path.clone(), rule: plan_rule(&message), message });
            None
        }
    };
    // Any problem stops the install, an unfinished plan included, so there is no action to offer.
    let action = action.filter(|_| violations.is_empty());
    Ok(Inspection { archive: archive_info(&archive, source), action, previous_version, previous_owner, violations })
}

// What the install's own archive checks would refuse, and a Mods folder spelled differently: on a
// case-sensitive file system that would be a second folder the Loader never reads.
fn check_archive(source: &Path, archive: &NativeArchive, old_files: Vec<(PathBuf, String)>) -> Result<(), String> {
    if archive.package_path.split('/').nth(1) != Some("Mods") {
        return Err(format!("The archive spells its Mods folder as in {}. Use Modstoke/Mods/<folder> exactly.", archive.package_path));
    }
    mod_update::check_native_archive(source, PathBuf::from(&archive.package_path), old_files)
}

fn recorded_files(package: Option<&ProfilePackage>) -> Vec<(PathBuf, String)> {
    package.and_then(|package| package.native.as_ref()).into_iter().flat_map(|record| &record.files)
        .map(|file| (PathBuf::from(&file.path), file.sha256.clone())).collect()
}

fn plan_rule(message: &str) -> &'static str {
    if message.contains("pack recovery") || message.contains("unfinished") {
        "pending"
    } else if message.contains("requires native mod") {
        "dependency"
    } else if message.contains("not owned") || message.contains("unowned") || message.contains("Vortex") || message.contains("without Manager ownership") {
        "ownership"
    } else {
        "plan"
    }
}

// Installs or updates one native ZIP. The caller holds the update lock and confirmed the server is stopped.
pub(crate) fn install_locked(root: &Path, source: &Path, expected_sha256: &str, stopped: StoppedCheck) -> Result<Installation, String> {
    let archive = native_archive::inspect(source, mods_layout(root))?;
    let state = read_locked(root)?;
    if state.profile.as_ref().is_some_and(|profile| profile.pending_plan.is_some()) {
        return Err(PENDING_PLAN.into());
    }
    check_archive(source, &archive, recorded_files(native_local::known_package(state.profile.as_ref(), &archive.id)))?;
    if folder_state(root, &archive.package_path) == Some(false) {
        return Err(format!("{} is disabled. Enable it before updating it.", archive.id));
    }
    let installed = native_local::install_archive_locked(root, source, expected_sha256, InstallCheck::Headless, stopped)?;
    Ok(Installation {
        archive: archive_info(&installed.archive, source),
        action: installed.action,
        previous_version: installed.previous_version,
        written: installed.written,
        removed: installed.removed,
    })
}

// Removes one Manager-owned native package: its recorded files, unless changed, and its record.
// Configuration files and protected content stay, and so does the package folder while they do.
// A disabled package's files are removed where they are, so it never becomes enabled on the way.
pub(crate) fn remove_locked(root: &Path, id: &str, stopped: StoppedCheck) -> Result<Removal, String> {
    let current = read_locked(root)?;
    if let Some(profile) = &current.profile {
        if profile.pending_plan.is_some() {
            return Err(PENDING_PLAN.into());
        }
        rollback::require_forward(profile)?;
    }
    mod_update::recover_pending_updates(root)?;
    let current = reconcile(root, current, stopped)?;
    let package = native_local::known_package(current.profile.as_ref(), id).ok_or_else(|| format!("No mod with ID {id} is installed."))?;
    let archive = package.native.clone().ok_or_else(|| format!("{} was installed from a mod catalog, not as a native package. Remove it with the Manager.", package.id))?;
    let dependents = native_local::dependents(root, &archive.id)?;
    if !dependents.is_empty() {
        return Err(format!("{} is required by {}. Remove those first.", archive.id, dependents.join(", ")));
    }
    let folder = if folder_state(root, &archive.package_path) == Some(false) { format!("{}.disabled", archive.package_path) } else { archive.package_path.clone() };
    let old_files = archive.files.iter()
        .map(|file| (PathBuf::from(format!("{folder}{}", &file.path[archive.package_path.len().min(file.path.len())..])), file.sha256.clone()))
        .collect();
    let mut removed = 0;
    let result = mod_update::remove_native_files_locked(root, PathBuf::from(&folder), old_files, &mut |targets, _| {
        stopped()?;
        removed = targets.len();
        Ok(())
    });
    let mut notices = Vec::new();
    match result {
        Ok(()) => {}
        // The files are gone; only the transaction's own cleanup failed, and recover retries it.
        Err(message) if message.starts_with("The update was applied") || message.starts_with("The update was committed") => notices.push(message),
        Err(message) => return Err(message),
    }
    // The record goes once the files have. A crash before this leaves a record whose files are
    // gone, and removing the mod again finishes the job.
    let mut profile = current.profile.ok_or_else(invalid_profile)?;
    profile.packages.retain(|package| key(&package.id) != key(&archive.id));
    profile.requested_roots.retain(|requested| key(requested) != key(&archive.id));
    // The recovery point is marked for release in the same write, so a crash after it leaves
    // the release for recover to finish.
    if let Some(point) = profile.rollback.as_mut() {
        point.phase = "discarding".into();
    }
    let written = write_locked(root, &profile)?;
    // A change retires the previous recovery point, as a new install does; only now that the
    // removal is done. A cleanup that stops part way stays marked, and recover finishes it.
    if written.profile.as_ref().is_some_and(|profile| profile.rollback.is_some()) {
        if let Err(message) = rollback::discard(root, written, None) {
            notices.push(message);
        }
    }
    // A folder something else still holds open stays, listed with what it keeps.
    let kept = prune(root, Path::new(&folder)).unwrap_or_else(|_| vec![format!("{folder}/")]);
    Ok(Removal { id: archive.id, version: archive.version, removed, kept, notices })
}

// Some(true) when the package folder is in place, Some(false) when only `<folder>.disabled` is.
pub(crate) fn folder_state(root: &Path, package_path: &str) -> Option<bool> {
    let exists = |path: &str| fs::symlink_metadata(root.join(path)).is_ok();
    if exists(package_path) {
        Some(true)
    } else if exists(&format!("{package_path}.disabled")) {
        Some(false)
    } else {
        None
    }
}

// Clears what an interrupted change left behind, so the folder can change again: an unfinished
// file transaction, archive copies a killed install left, an install whose plan was recorded but
// not finished (rolled back with the Manager's own recovery point, removing the folders it made),
// a rollback that stopped part way, and a recovery point whose cleanup stopped part way.
pub(crate) fn recover_locked(root: &Path, stopped: StoppedCheck) -> Result<Vec<String>, String> {
    let mut done = Vec::new();
    let interrupted = mod_update::interrupted_changes(root);
    mod_update::recover_pending_updates(root)?;
    let finished = interrupted.saturating_sub(mod_update::interrupted_changes(root));
    if finished > 0 {
        done.push(format!("Finished or restored {finished} interrupted file change{}.", if finished == 1 { "" } else { "s" }));
    }
    let preparations = mod_update::remove_interrupted_preparations(root);
    if preparations > 0 {
        done.push(format!("Removed {preparations} file change{} interrupted before it began.", if preparations == 1 { "" } else { "s" }));
    }
    let leftovers = remove_leftover_copies(root);
    if leftovers > 0 {
        done.push(format!("Removed {leftovers} archive cop{} an interrupted install left.", if leftovers == 1 { "y" } else { "ies" }));
    }
    let state = read_locked(root)?;
    let state = reconcile(root, state, stopped)?;
    let Some(profile) = &state.profile else { return Ok(done) };
    let phase = profile.rollback.as_ref().map(|point| point.phase.clone());
    if let Some(plan) = &profile.pending_plan {
        let (plan_id, roots) = (plan.id.clone(), plan.requested_roots.join(", "));
        let folders: Vec<(String, String)> = plan.steps.iter()
            .filter_map(|step| Some((step.package.id.clone(), step.package.native.as_ref()?.package_path.clone())))
            .collect();
        stopped()?;
        let restored = rollback::restore(root, state, &plan_id)?;
        // A package the plan was installing for the first time leaves no folder behind.
        for (id, folder) in folders {
            if native_local::known_package(restored.profile.as_ref(), &id).is_none() {
                let _ = prune(root, Path::new(&folder));
            }
        }
        done.push(format!("Rolled back the unfinished install of {roots}."));
    } else if phase.as_deref() == Some("restoring") {
        let plan_id = profile.rollback.as_ref().map(|point| point.plan_id.clone()).unwrap_or_default();
        stopped()?;
        rollback::restore(root, state, &plan_id)?;
        done.push("Finished a rollback that had stopped part way.".into());
    } else if phase.as_deref() == Some("discarding") {
        rollback::discard(root, state, None)?;
        done.push("Finished cleaning up an earlier recovery point.".into());
    }
    Ok(done)
}

// Private archive copies an install makes in the root; one a killed install left is never used again.
fn remove_leftover_copies(root: &Path) -> usize {
    let Ok(entries) = fs::read_dir(root) else { return 0 };
    entries.flatten()
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            (name.starts_with(".modstoke-native-") || name.starts_with(".modstoke-profile-"))
                && entry.file_type().is_ok_and(|kind| kind.is_file())
        })
        .filter(|entry| fs::remove_file(entry.path()).is_ok())
        .count()
}

// Why the profile cannot be read even as a moved folder's, if it cannot.
pub(crate) fn profile_problem(root: &Path) -> Option<String> {
    read_relaxed(root).err()
}

pub(crate) fn has_pending_plan(root: &Path) -> bool {
    read_locked(root).ok().and_then(|state| state.profile).is_some_and(|profile| profile.pending_plan.is_some())
}

fn raw_profile(root: &Path) -> Result<Option<ModProfile>, String> {
    if !safe_fs::validate_target(root, Path::new(PROFILE_FILE))? {
        return Ok(None);
    }
    let file = File::open(root.join(PROFILE_FILE)).map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_PROFILE_BYTES + 1).read_to_end(&mut bytes).map_err(|error| error.to_string())?;
    Ok(serde_json::from_slice::<ModProfile>(&bytes).ok())
}

// The profile as a read-only command sees it. One written for this folder before it was moved is
// read as if it named this folder, without writing anything or touching its recovery point.
fn read_relaxed(root: &Path) -> Result<ProfileState, String> {
    let error = match read_locked(root) {
        Ok(state) => return Ok(state),
        Err(error) => error,
    };
    let Some(mut profile) = raw_profile(root)? else { return Err(error) };
    if fs::canonicalize(&profile.game_root).ok().as_deref() == Some(root) {
        return Err(error);
    }
    profile.game_root = root.to_string_lossy().into_owned();
    profile.rollback = None;
    profile.schema_version = 1;
    native_local::rebase_records(root, &mut profile);
    validate_profile(root, &profile).map_err(|_| error)?;
    Ok(ProfileState { profile: Some(profile), revision: None })
}

// A Windows path recorded in a profile that Linux under WSL reaches as /mnt/<drive>/..., when that
// path leads to `root`: the same folder seen from the other system, not a moved one.
fn same_folder_from_wsl(recorded: &str, root: &Path) -> bool {
    if cfg!(windows) {
        return false;
    }
    let path = recorded.trim_start_matches("\\\\?\\");
    let bytes = path.as_bytes();
    if bytes.len() < 2 || bytes[1] != b':' || !bytes[0].is_ascii_alphabetic() {
        return false;
    }
    let mounted = format!("/mnt/{}{}", (bytes[0] as char).to_ascii_lowercase(), path[2..].replace('\\', "/"));
    same_directory(Path::new(&mounted), root)
}

#[cfg(unix)]
fn same_directory(left: &Path, right: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (fs::metadata(left), fs::metadata(right)) {
        (Ok(left), Ok(right)) => left.dev() == right.dev() && left.ino() == right.ino(),
        _ => false,
    }
}

// Windows has no stable file identity here, so an old folder that still exists is never assumed
// to be a different one.
#[cfg(not(unix))]
fn same_directory(_: &Path, _: &Path) -> bool {
    true
}

// Why the profile cannot be pointed at `root`, if it names another folder: Some((pending, message)),
// where `pending` means recovering in the old folder clears it. None when it needs no change or
// can be re-pointed.
fn rehome_refusal(root: &Path, profile: &ModProfile) -> Option<(bool, String)> {
    let recorded = &profile.game_root;
    let old = fs::canonicalize(recorded);
    let shown = safe_fs::strip_verbatim_prefix(recorded);
    if same_folder_from_wsl(recorded, root) {
        return Some((false, format!("{PROFILE_FILE} records this folder by its Windows path, {shown}. Change mods from Windows, or pass the folder's own path from a Linux host.")));
    }
    if let Ok(old) = &old {
        // A link left at the old path leads here; the folder itself moved.
        if old != root && same_directory(old, root) {
            return Some((false, format!("{PROFILE_FILE} records this folder as {}. Pass that path as --root.", safe_fs::strip_verbatim_prefix(&old.to_string_lossy()))));
        }
    }
    let linked = old.as_ref().ok().map(PathBuf::as_path) == Some(root);
    let back = if linked { format!("Replace the link at {shown} with the folder itself") } else { format!("Move the folder back to {shown}") };
    if profile.pending_plan.is_some() {
        return Some((true, format!("{PROFILE_FILE} records an unfinished install that can only be rolled back in {shown}. {back}, run recover there, then move it again.")));
    }
    if profile.rollback.as_ref().is_some_and(|point| point.phase == "restoring") {
        return Some((true, format!("{PROFILE_FILE} records a rollback that stopped part way, which can only be finished in {shown}. {back}, run recover there, then move it again.")));
    }
    None
}

fn needs_rehome(root: &Path, profile: &ModProfile) -> bool {
    safe_fs::strip_verbatim_prefix(&profile.game_root) != safe_fs::strip_verbatim_prefix(&root.to_string_lossy())
        || fs::canonicalize(&profile.game_root).ok().as_deref() != Some(root)
}

// What would stop a changing command from pointing the profile at this folder, for read-only
// commands to report before a request is queued: Some((pending, message)).
pub(crate) fn rehome_blocker(root: &Path) -> Option<(bool, String)> {
    let profile = raw_profile(root).ok()??;
    if !needs_rehome(root, &profile) {
        return None;
    }
    rehome_refusal(root, &profile)
}

// A profile written for this folder before it was moved or restored elsewhere, or copied from
// another system, names another folder. Changing commands point it at this one; package records
// are relative to the folder. The recovery point cannot follow, since its receipts name the old
// folder, so it is released. Refused as Err((pending, message)), see rehome_refusal.
pub(crate) fn rehome_locked(root: &Path) -> Result<bool, (bool, String)> {
    let Some(mut profile) = raw_profile(root).map_err(|message| (false, message))? else { return Ok(false) };
    if !needs_rehome(root, &profile) {
        return Ok(false);
    }
    if let Some(refusal) = rehome_refusal(root, &profile) {
        return Err(refusal);
    }
    rollback::release_for_move(root, &mut profile).map_err(|message| (false, message))?;
    profile.game_root = root.to_string_lossy().into_owned();
    write_locked(root, &profile).map_err(|message| (false, message))?;
    Ok(true)
}

// Deletes empty folders below and including `folder`, deepest first. Returns what stays, naming a
// protected folder itself and never what is inside it.
fn prune(root: &Path, folder: &Path) -> Result<Vec<String>, String> {
    fn walk(root: &Path, relative: &Path, kept: &mut Vec<String>) -> Result<bool, String> {
        let absolute = root.join(relative);
        if fs::symlink_metadata(&absolute).is_err() {
            return Ok(true);
        }
        if native_root::is_protected_content(relative) {
            kept.push(format!("{}/", relative.to_string_lossy().replace('\\', "/")));
            return Ok(false);
        }
        safe_fs::validate_directory(&absolute)?;
        let mut empty = true;
        for entry in fs::read_dir(&absolute).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let child = relative.join(entry.file_name());
            if entry.file_type().map_err(|error| error.to_string())?.is_dir() {
                empty &= walk(root, &child, kept)?;
            } else {
                kept.push(child.to_string_lossy().replace('\\', "/"));
                empty = false;
            }
        }
        if empty {
            fs::remove_dir(&absolute).map_err(|error| error.to_string())?;
        }
        Ok(empty)
    }
    let mut kept = Vec::new();
    walk(root, folder, &mut kept)?;
    kept.sort();
    Ok(kept)
}

// Disables a native package by renaming its folder to `<folder>.disabled`, which the Loader lists
// but never loads, or enables it again. The profile record keeps the enabled location.
pub(crate) fn set_enabled_locked(root: &Path, id: &str, enabled: bool) -> Result<bool, String> {
    let state = read_locked(root)?;
    if state.profile.as_ref().is_some_and(|profile| profile.pending_plan.is_some()) {
        return Err(PENDING_PLAN.into());
    }
    let package = native_local::known_package(state.profile.as_ref(), id).ok_or_else(|| format!("No mod with ID {id} is installed."))?;
    let archive = package.native.as_ref().ok_or_else(|| format!("{} was installed from a mod catalog. Enable or disable it with the Manager.", package.id))?;
    let active = PathBuf::from(&archive.package_path);
    let disabled = PathBuf::from(format!("{}.disabled", archive.package_path));
    let exists = |relative: &Path| fs::symlink_metadata(root.join(relative)).is_ok();
    let (from, to) = if enabled { (&disabled, &active) } else { (&active, &disabled) };
    if !exists(from) && exists(to) {
        return Ok(false);
    }
    if !exists(from) {
        return Err(format!("{} is missing, so {} cannot be {}.", archive.package_path, archive.id, if enabled { "enabled" } else { "disabled" }));
    }
    if exists(to) {
        return Err(format!("Both {} and {} exist. Remove one by hand first.", active.display(), disabled.display()));
    }
    safe_fs::validate_directory(&root.join(from))?;
    fs::rename(root.join(from), root.join(to)).map_err(|error| error.to_string())?;
    Ok(true)
}

pub(crate) fn owned_packages(root: &Path) -> Result<Vec<OwnedPackage>, String> {
    Ok(read_relaxed(root)?.profile.map(|profile| profile.packages).unwrap_or_default().into_iter()
        .filter_map(|package| package.native.map(|archive| OwnedPackage { id: package.id, version: package.version, package_path: archive.package_path }))
        .collect())
}

// Programs and scripts a mod package may not install, whatever its kind.
pub(crate) fn is_executable(path: &str) -> bool {
    let name = path.rsplit(['/', '\\']).next().unwrap_or_default().to_ascii_lowercase();
    [".exe", ".com", ".bat", ".cmd", ".ps1", ".vbs", ".scr", ".msi", ".sh"].iter().any(|extension| name.ends_with(extension))
}

// Every installed native mod with its version and the exact versions it requires, disabled ones
// included, read from the manifests as dependency planning does.
// A mod's ID, version, and the ID and exact version of each mod it requires.
pub(crate) type Requirements = (String, String, Vec<(String, String)>);

// Installed native mods with what they require, and whether each is enabled: dependency planning
// only counts enabled mods as present, but a disabled one still blocks removing what it needs.
pub(crate) fn installed_requirements(root: &Path) -> Result<Vec<(Requirements, bool)>, String> {
    let inventory = native_inventory::read_inventory(&root.to_string_lossy())?;
    let mut found = Vec::new();
    for entry in &inventory.packages {
        let Some(version) = entry.version.clone() else { continue };
        let manifest = safe_fs::strip_verbatim_prefix(&entry.manifest_path);
        let Some(relative) = manifest.strip_prefix(&safe_fs::strip_verbatim_prefix(&root.to_string_lossy())).map(|rest| rest.trim_start_matches(['/', '\\']).to_owned()) else { continue };
        let required = fs::read(root.join(&relative)).ok()
            .and_then(|bytes| native_archive::parse_manifest(&bytes).ok())
            .map(|manifest| manifest.required.into_iter().map(|dependency| (dependency.id, dependency.version)).collect())
            .unwrap_or_default();
        found.push(((entry.id.clone(), version, required), entry.state != "disabled"));
    }
    Ok(found)
}

// A native archive's ID, version and the exact versions it requires.
pub(crate) fn archive_requirements(source: &Path) -> Result<Requirements, String> {
    let archive = native_archive::inspect(source, native_root::ROOT)?;
    Ok((archive.id, archive.version, archive.required.into_iter().map(|dependency| (dependency.id, dependency.version)).collect()))
}

// Whether a dependency problem is the archive's own ("<id> requires native mod ..."), which the
// queue may satisfy, rather than another installed mod's.
pub(crate) fn own_dependency(message: &str, id: &str) -> bool {
    message.split(" requires native mod ").next().is_some_and(|owner| owner.eq_ignore_ascii_case(id))
}

// Installed mods whose manifests require `id`, disabled ones included.
pub(crate) fn dependents(root: &Path, id: &str) -> Result<Vec<String>, String> {
    native_local::dependents(root, id)
}

// The folder modstoke-native was given, if it is a game or dedicated-server folder.
// Links in the path the host gives are resolved first; inside the folder they are still refused.
pub(crate) fn server_root(value: &str) -> Result<PathBuf, String> {
    match fs::canonicalize(value) {
        Ok(path) => profile_root(&safe_fs::strip_verbatim_prefix(&path.to_string_lossy()), true),
        Err(_) => profile_root(value, true),
    }
}
