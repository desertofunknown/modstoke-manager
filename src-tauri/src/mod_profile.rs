use crate::mod_update;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[path = "mod_profile_rollback.rs"]
mod rollback;
#[path = "native_package_archive.rs"]
mod native_archive;
#[path = "native_package_install.rs"]
pub(crate) mod native_local;
#[path = "native_pack_prepare.rs"]
pub(crate) mod native_pack;
#[path = "native_pack_install.rs"]
pub(crate) mod native_pack_install;

const PROFILE_FILE: &str = "modstoke-profile.lock.json";
const MAX_PROFILE_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModProfile {
    schema_version: u32,
    game_root: String,
    requested_roots: Vec<String>,
    packages: Vec<ProfilePackage>,
    archives: Vec<ArchivePin>,
    pending_plan: Option<InstallPlan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rollback: Option<rollback::PackCheckpoint>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProfilePackage {
    id: String,
    name: String,
    version: String,
    dependencies: Vec<String>,
    dependency_basis: String,
    source: PackageSource,
    ownership: String,
    enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    archive_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    native: Option<native_archive::NativeArchive>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PackageSource {
    provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ArchivePin {
    id: String,
    version: String,
    url: String,
    sha256: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstallPlan {
    id: String,
    requested_roots: Vec<String>,
    steps: Vec<InstallStep>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    native_pack: Option<native_pack::NativePackDocument>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InstallStep {
    package: ProfilePackage,
    action: String,
    before: Option<InstallObservation>,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    targets: Option<Vec<TargetReceipt>>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstallObservation {
    id: String,
    version: String,
    enabled: bool,
    ownership: String,
    loader_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    manifest_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    package_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assembly_path: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TargetReceipt {
    relative_path: PathBuf,
    original_sha256: Option<String>,
    replacement_sha256: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedFile {
    path: String,
    sha256: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtectedPath {
    path: String,
    directory: bool,
}

#[derive(Clone, Serialize)]
pub struct ProfileState {
    profile: Option<ModProfile>,
    revision: Option<String>,
}

fn hash(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

fn key(value: &str) -> String { value.replace('\\', "/").to_lowercase() }

fn valid_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 2048 && !value.chars().any(char::is_control)
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn invalid_profile() -> String {
    format!("{PROFILE_FILE} has incomplete or unsupported data. It was preserved; restore a complete supported profile or move it aside after reviewing its pending operation.")
}

fn game_root(value: &str) -> Result<PathBuf, String> {
    profile_root(value, false)
}

fn profile_root(value: &str, allow_server: bool) -> Result<PathBuf, String> {
    let selected = Path::new(value);
    mod_update::validate_directory(selected)?;
    let root = fs::canonicalize(selected).map_err(|error| error.to_string())?;
    let player = mod_update::validate_target(&root, Path::new("SonsOfTheForest.exe"))?;
    let server = allow_server && mod_update::validate_target(&root, Path::new("SonsOfTheForestDS.exe"))?;
    if !player && !server {
        return Err(if allow_server {
            "Select a folder containing SonsOfTheForest.exe or SonsOfTheForestDS.exe to read its mod profile."
        } else {
            "Select a player folder containing SonsOfTheForest.exe for catalog package changes. Use Native packages for dedicated-server ZIP installation."
        }.into());
    }
    Ok(root)
}

fn relative(root: &Path, value: &str) -> Result<PathBuf, String> {
    // Observed paths may use the UI's ordinary Windows spelling, while root is
    // canonical. Resolve the parent without following the final file itself.
    let path = Path::new(value);
    let parent = path.parent().ok_or_else(invalid_profile)?;
    mod_update::validate_directory(parent)?;
    let absolute = fs::canonicalize(parent).map_err(|error| error.to_string())?
        .join(path.file_name().ok_or_else(invalid_profile)?);
    let suffix = absolute.strip_prefix(root).map_err(|_| "An observed install path is outside the selected game folder.".to_string())?;
    mod_update::validate_mod_path(suffix)?;
    Ok(suffix.to_owned())
}

fn encode_coordinate(value: &str) -> String {
    value.bytes().map(|byte| {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            (byte as char).to_string()
        } else { format!("%{byte:02X}") }
    }).collect()
}

fn validate_package(pkg: &ProfilePackage) -> Result<(), String> {
    if ![&pkg.id, &pkg.name, &pkg.version].into_iter().all(|value| valid_text(value))
        || !["native", "vortex", "manual"].contains(&pkg.ownership.as_str())
        || pkg.dependencies.len() > 64
        || !["catalog-current-identities", native_local::DEPENDENCY_BASIS].contains(&pkg.dependency_basis.as_str())
        || pkg.dependencies.iter().any(|value| !valid_text(value))
        || pkg.archive_sha256.as_deref().is_some_and(|value| !valid_hash(value)) {
        return Err(invalid_profile());
    }
    native_local::validate_package_record(pkg)?;
    match pkg.source.provider.as_str() {
        "installed" if pkg.source.author.is_none() && pkg.source.slug.is_none() && pkg.source.url.is_none() => {}
        "sotf-mods" => {
            let author = pkg.source.author.as_deref().filter(|value| valid_text(value)).ok_or_else(invalid_profile)?;
            let slug = pkg.source.slug.as_deref().filter(|value| valid_text(value)).ok_or_else(invalid_profile)?;
            let expected = format!("https://sotf-mods.com/mods/{}/{}/download/{}", encode_coordinate(author), encode_coordinate(slug), encode_coordinate(&pkg.version));
            if pkg.source.url.as_deref() != Some(&expected) { return Err(invalid_profile()); }
        }
        native_local::PROVIDER => {}
        _ => return Err(invalid_profile()),
    }
    Ok(())
}

fn unique_ids(values: impl Iterator<Item = String>) -> bool {
    let mut seen = HashSet::new();
    values.into_iter().all(|value| valid_text(&value) && seen.insert(value.to_lowercase()))
}

fn validate_profile(root: &Path, profile: &ModProfile) -> Result<(), String> {
    if ![1, 2].contains(&profile.schema_version) {
        return Err(format!("{PROFILE_FILE} uses unsupported schema {}. Update Manager to read it; the file was preserved.", profile.schema_version));
    }
    if fs::canonicalize(&profile.game_root).map_err(|_| invalid_profile())? != root {
        return Err(format!("{PROFILE_FILE} belongs to a different game folder. It was preserved."));
    }
    if profile.packages.len() > 1024 || profile.requested_roots.len() > 1024 || profile.archives.len() > 4096
        || !unique_ids(profile.packages.iter().map(|pkg| pkg.id.clone()))
        || !unique_ids(profile.requested_roots.iter().cloned()) {
        return Err(invalid_profile());
    }
    for pkg in &profile.packages { validate_package(pkg)?; }
    let mut archives = HashSet::new();
    let mut observed_versions = HashMap::new();
    for pin in &profile.archives {
        if !valid_text(&pin.id) || !valid_text(&pin.version) || !valid_text(&pin.url) || !valid_hash(&pin.sha256)
            || !archives.insert((key(&pin.id), &pin.version, &pin.url))
            || observed_versions.insert((key(&pin.id), &pin.version), &pin.sha256).is_some_and(|known| known != &pin.sha256) {
            return Err(invalid_profile());
        }
    }
    if let Some(plan) = &profile.pending_plan {
        if !valid_text(&plan.id) || plan.steps.is_empty() || plan.steps.len() > 128
            || (if plan.native_pack.is_some() { plan.requested_roots.is_empty() || plan.requested_roots.len() > 128 }
                else { plan.requested_roots.len() != 1 })
            || !unique_ids(plan.requested_roots.iter().cloned())
            || !unique_ids(plan.steps.iter().map(|step| step.package.id.clone())) {
            return Err(invalid_profile());
        }
        let mut earlier = HashSet::new();
        let native_plan = plan.steps[0].package.dependency_basis == native_local::DEPENDENCY_BASIS;
        let mut edges = 0;
        let mut applying = 0;
        for step in &plan.steps {
            validate_package(&step.package)?;
            if (step.package.dependency_basis == native_local::DEPENDENCY_BASIS) != native_plan { return Err(invalid_profile()); }
            edges += step.package.dependencies.len();
            if edges > 1024 || step.package.dependencies.iter().any(|dep| !earlier.contains(dep))
                || !["install", "update", "preserve"].contains(&step.action.as_str())
                || !["planned", "downloaded", "applying", "applied"].contains(&step.status.as_str()) {
                return Err(invalid_profile());
            }
            if step.action == "install" && step.before.is_some()
                || step.action != "install" && step.before.is_none()
                || step.action != "preserve" && (step.package.ownership != "native"
                    || step.package.source.provider != if native_plan { native_local::PROVIDER } else { "sotf-mods" })
                || step.action == "preserve" && !step.package.enabled {
                return Err(invalid_profile());
            }
            if let Some(before) = &step.before {
                if !valid_text(&before.id) || !valid_text(&before.version) || key(&before.id) != key(&step.package.id)
                    || !["native", "vortex", "manual"].contains(&before.ownership.as_str())
                    || (if native_plan { before.loader_type != "modstoke-native-mod" }
                        else { !["redloader-mod", "redloader-library", "bepinex-plugin"].contains(&before.loader_type.as_str()) })
                    || step.action == "update" && (before.ownership != "native" || before.enabled != step.package.enabled)
                    || step.action == "preserve" && (before.ownership != step.package.ownership || before.version != step.package.version || !before.enabled) {
                    return Err(invalid_profile());
                }
            }
            if step.action != "preserve" && step.status != "planned" {
                let digest = step.package.archive_sha256.as_ref().ok_or_else(invalid_profile)?;
                if !profile.archives.iter().any(|pin| key(&pin.id) == key(&step.package.id) && pin.version == step.package.version
                    && Some(&pin.url) == step.package.source.url.as_ref() && &pin.sha256 == digest) {
                    return Err(invalid_profile());
                }
            }
            if step.status == "applying" {
                applying += 1;
                if applying > 1 || step.targets.as_ref().is_none_or(|targets| targets.is_empty()) { return Err(invalid_profile()); }
            }
            if let Some(targets) = &step.targets {
                if targets.len() > 8192 || !["applying", "applied"].contains(&step.status.as_str()) { return Err(invalid_profile()); }
                let mut seen = HashSet::new();
                for target in targets {
                    mod_update::validate_mod_path(&target.relative_path)?;
                    if !seen.insert(key(&target.relative_path.to_string_lossy()))
                        || target.original_sha256.as_deref().is_some_and(|value| !valid_hash(value))
                        || target.replacement_sha256.as_deref().is_some_and(|value| !valid_hash(value)) {
                        return Err(invalid_profile());
                    }
                }
            }
            earlier.insert(step.package.id.clone());
        }
        if plan.requested_roots.iter().any(|id| !earlier.contains(id)) { return Err(invalid_profile()); }
        native_pack_install::validate_plan(plan)?;
    }
    rollback::validate(profile)
}

fn read_locked(root: &Path) -> Result<ProfileState, String> {
    if !mod_update::validate_target(root, Path::new(PROFILE_FILE))? {
        return Ok(ProfileState { profile: None, revision: None });
    }
    let file = File::open(root.join(PROFILE_FILE)).map_err(|error| error.to_string())?;
    if file.metadata().map_err(|error| error.to_string())?.len() > MAX_PROFILE_BYTES { return Err(invalid_profile()); }
    let mut bytes = Vec::new();
    file.take(MAX_PROFILE_BYTES + 1).read_to_end(&mut bytes).map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_PROFILE_BYTES { return Err(invalid_profile()); }
    let json: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| invalid_profile())?;
    if !matches!(json.get("schemaVersion").and_then(|value| value.as_u64()), Some(1 | 2)) {
        return Err(format!("{PROFILE_FILE} has an unsupported schema. Update Manager to read it; the file was preserved."));
    }
    let profile: ModProfile = serde_json::from_value(json).map_err(|_| invalid_profile())?;
    validate_profile(root, &profile)?;
    Ok(ProfileState { profile: Some(profile), revision: Some(hash(&bytes)) })
}

fn encode_profile(root: &Path, profile: &ModProfile) -> Result<Vec<u8>, String> {
    validate_profile(root, profile)?;
    let bytes = serde_json::to_vec_pretty(profile).map_err(|error| error.to_string())?;
    // Reserve room for recovery phase transitions even when the profile is full.
    let limit = if profile.rollback.as_ref().is_some_and(|checkpoint| checkpoint.phase == "available") { MAX_PROFILE_BYTES - 32 } else { MAX_PROFILE_BYTES };
    if bytes.len() as u64 > limit { return Err(invalid_profile()); }
    rollback::check_restored_size(profile, MAX_PROFILE_BYTES)?;
    Ok(bytes)
}

fn write_locked(root: &Path, profile: &ModProfile) -> Result<ProfileState, String> {
    let bytes = encode_profile(root, profile)?;
    let replace = mod_update::validate_target(root, Path::new(PROFILE_FILE))?;
    let mut temporary = tempfile::Builder::new().prefix(".modstoke-profile-").tempfile_in(root).map_err(|error| error.to_string())?;
    temporary.write_all(&bytes).and_then(|_| temporary.as_file().sync_all()).map_err(|error| error.to_string())?;
    let temporary = temporary.into_temp_path();
    mod_update::durable_rename(&temporary, &root.join(PROFILE_FILE), replace)?;
    Ok(ProfileState { profile: Some(profile.clone()), revision: Some(hash(&bytes)) })
}

fn complete_package(profile: &mut ModProfile, index: usize) {
    let plan = profile.pending_plan.as_mut().expect("validated pending plan");
    plan.steps[index].status = "applied".into();
    let pkg = plan.steps[index].package.clone();
    if let Some(existing) = profile.packages.iter_mut().find(|entry| key(&entry.id) == key(&pkg.id)) {
        *existing = pkg;
    } else { profile.packages.push(pkg); }
}

fn reconcile(root: &Path, state: ProfileState) -> Result<ProfileState, String> {
    let Some(mut profile) = state.profile.clone() else { return Ok(state); };
    if profile.rollback.as_ref().is_some_and(|checkpoint| checkpoint.phase != "available") { return Ok(state); }
    let Some(plan) = &profile.pending_plan else { return Ok(state); };
    let Some(index) = plan.steps.iter().position(|step| step.status == "applying") else { return Ok(state); };
    if plan.steps[index].package.dependency_basis == native_local::DEPENDENCY_BASIS {
        native_local::require_stopped()?;
    }
    mod_update::recover_pending_updates(root)?;
    let step = &plan.steps[index];
    let mut original = true;
    let mut replacement = true;
    for target in step.targets.as_ref().ok_or_else(invalid_profile)? {
        let current = mod_update::target_hash(root, &target.relative_path)?;
        original &= current == target.original_sha256;
        replacement &= current == target.replacement_sha256;
    }
    if replacement {
        complete_package(&mut profile, index);
    } else if original {
        let step = &mut profile.pending_plan.as_mut().expect("pending plan").steps[index];
        step.status = "downloaded".into();
        step.targets = None;
    } else {
        return Err(format!("Interrupted package {} conflicts with current files in {}. Its profile and recovery backups were preserved. Restore or reconcile this package's listed original/replacement hashes before retrying; other completed packages are still recorded.", step.package.id, root.join(PROFILE_FILE).display()));
    }
    write_locked(root, &profile)
}

fn require_revision(state: &ProfileState, expected: &Option<String>) -> Result<(), String> {
    if &state.revision != expected {
        return Err("The game profile changed during this operation. Retry from the installed mod list to resume its current plan.".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn read_mod_profile(game_root: String) -> Result<ProfileState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = profile_root(&game_root, true)?;
        let _lock = mod_update::acquire_update_lock(&root)?;
        let state = read_locked(&root)?;
        let pack_recovery_active = state.profile.as_ref().is_some_and(|profile|
            profile.rollback.as_ref().is_some_and(|checkpoint| checkpoint.phase != "available"));
        if !pack_recovery_active && mod_update::validate_target(&root, Path::new("SonsOfTheForestDS.exe"))? {
            mod_update::recover_pending_updates(&root)?;
        }
        reconcile(&root, state)
    }).await.map_err(|error| error.to_string())?
}

pub(crate) fn require_launch_ready(root: &Path) -> Result<(), String> {
    let state = read_locked(root)?;
    if let Some(profile) = state.profile {
        if profile.pending_plan.is_some() {
            return Err("This installation has an unfinished Manager package plan. Finish or roll back that plan before starting; launch does not install or recover packages.".into());
        }
        if profile.rollback.as_ref().is_some_and(|checkpoint| checkpoint.phase != "available") {
            return Err("This installation has unfinished pack recovery. Finish recovery before starting.".into());
        }
    }
    mod_update::require_no_pending_updates(root)
}

#[tauri::command]
pub async fn check_player_launch_ready(game_root: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = profile_root(&game_root, false)?;
        let _lock = mod_update::acquire_update_lock(&root)?;
        require_launch_ready(&root)
    }).await.map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn write_mod_profile(game_root: String, expected_revision: Option<String>, mut profile: ModProfile) -> Result<ProfileState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = self::game_root(&game_root)?;
        let _lock = mod_update::acquire_update_lock(&root)?;
        let current = read_locked(&root)?;
        require_revision(&current, &expected_revision)?;
        native_local::require_catalog_write(&current, &profile)?;
        rollback::validate_write(current.profile.as_ref(), &profile)?;
        let mut validation = profile.clone();
        if current.profile.as_ref().is_none_or(|value| value.pending_plan.is_none()) && profile.pending_plan.is_some() {
            // The incoming copy still carries the previous completed checkpoint.
            // The native begin operation replaces it only after the new plan validates.
            validation.rollback = None;
        }
        validate_profile(&root, &validation)?;
        rollback::begin(&root, &current, &mut profile)?;
        write_locked(&root, &profile)
    }).await.map_err(|error| error.to_string())?
}

fn validate_previous(root: &Path, before: &InstallObservation) -> Result<mod_update::PreviousInstall, String> {
    let manifest = before.manifest_path.as_deref().ok_or("The installed mod has no manifest path. Refresh before updating.")?;
    let relative_manifest = relative(root, manifest)?;
    mod_update::validate_target(root, &relative_manifest)?;
    let manifest: serde_json::Value = serde_json::from_slice(&fs::read(root.join(relative_manifest)).map_err(|error| error.to_string())?)
        .map_err(|_| "The installed manifest changed. Refresh before updating.")?;
    if manifest.get("id").and_then(|value| value.as_str()).map(key) != Some(key(&before.id))
        || manifest.get("version").and_then(|value| value.as_str()) != Some(before.version.as_str()) {
        return Err("The installed identity or version changed before commit. Refresh before updating.".into());
    }
    let package = before.package_path.as_deref().map(|value| relative(root, value)).transpose()?;
    let enabled_package = package.as_ref().map(|value| {
        let components: Vec<_> = value.components().collect();
        if components.len() == 3 && components[1].as_os_str().to_string_lossy().eq_ignore_ascii_case("_Disabled") {
            PathBuf::from(components[0].as_os_str()).join(components[2].as_os_str())
        } else { value.clone() }
    });
    let package_disabled = package != enabled_package;
    let assembly = before.assembly_path.as_deref().map(|value| relative(root, value)).transpose()?;
    let mut enabled_assembly = None;
    if let Some(assembly) = &assembly {
        if !mod_update::validate_target(root, assembly)? { return Err("The installed assembly moved before commit. Refresh before updating.".into()); }
        let disabled_extension = assembly.extension().is_some_and(|value| value.eq_ignore_ascii_case("disabled"));
        if before.enabled == (disabled_extension || package_disabled) {
            return Err("The installed enabled/disabled state changed. Refresh before updating.".into());
        }
        let mut enabled = assembly.with_extension("dll");
        if package_disabled {
            if let (Some(package), Some(enabled_package)) = (&package, &enabled_package) {
                if let Ok(suffix) = enabled.strip_prefix(package) { enabled = enabled_package.join(suffix); }
            }
        }
        let opposite = if before.enabled { assembly.with_extension("disabled") } else { enabled.clone() };
        if mod_update::validate_target(root, &opposite)? {
            return Err("Both enabled and disabled assembly paths exist. Resolve this ambiguity before updating.".into());
        }
        enabled_assembly = Some(root.join(enabled).to_string_lossy().into_owned());
    } else if !before.enabled {
        return Err("Cannot preserve the disabled state without its assembly path. Refresh before updating.".into());
    }
    Ok(mod_update::PreviousInstall {
        assembly_path: assembly.map(|value| root.join(value).to_string_lossy().into_owned()),
        enabled_assembly_path: enabled_assembly,
        package_path: package.map(|value| root.join(value).to_string_lossy().into_owned()),
        enabled_package_path: enabled_package.map(|value| root.join(value).to_string_lossy().into_owned()),
        is_enabled: before.enabled,
    })
}

fn validate_observed_files(root: &Path, expected_files: &[ExpectedFile]) -> Result<(), String> {
    if expected_files.len() > 4096 { return Err("Too many inventory observations for this plan.".into()); }
    for expected in expected_files {
        if expected.sha256.as_deref().is_some_and(|value| !valid_hash(value)) { return Err(invalid_profile()); }
        let path = relative(root, &expected.path)?;
        if mod_update::target_hash(root, &path)? != expected.sha256 {
            return Err(format!("{} changed before package commit. The pending plan was preserved; refresh before retrying.", path.display()));
        }
    }
    Ok(())
}

fn validate_staged_identity(staging: &Path, pkg: &ProfilePackage) -> Result<(PathBuf, String), String> {
    let mut folders = vec![staging.to_owned()];
    let mut files = 0;
    let mut matching = 0;
    let mut identity_manifest = None;
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(folder).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            files += 1;
            if files > 8192 { return Err("The package exceeds 8192 staged entries.".into()); }
            if entry.file_type().map_err(|error| error.to_string())?.is_dir() {
                folders.push(entry.path());
            } else if entry.file_name().to_string_lossy().eq_ignore_ascii_case("manifest.json") {
                let mut bytes = Vec::new();
                File::open(entry.path()).map_err(|error| error.to_string())?.take(256 * 1024 + 1)
                    .read_to_end(&mut bytes).map_err(|error| error.to_string())?;
                if bytes.len() > 256 * 1024 { return Err("The package manifest is too large.".into()); }
                let manifest: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| "The package contains an invalid manifest.")?;
                if manifest.get("id").and_then(|value| value.as_str()).map(key) == Some(key(&pkg.id)) {
                    let file = entry.path();
                    let relative = file.strip_prefix(staging).map_err(|error| error.to_string())?;
                    let parts: Vec<_> = relative.components().collect();
                    if parts.len() < 3 || parts.len() > 6
                        || !["mods", "libs"].contains(&parts[0].as_os_str().to_string_lossy().to_lowercase().as_str())
                        || manifest.get("version").and_then(|value| value.as_str()) != Some(pkg.version.as_str()) {
                        return Err(format!("The archive for {} does not contain its pinned version {} in a supported Mods/Libs inventory location.", pkg.id, pkg.version));
                    }
                    matching += 1;
                    identity_manifest = Some((relative.to_owned(), hash(&bytes)));
                }
            }
        }
    }
    if matching != 1 { return Err(format!("The archive must contain exactly one manifest for the pinned package {} {}.", pkg.id, pkg.version)); }
    identity_manifest.ok_or_else(invalid_profile)
}

#[tauri::command]
pub async fn apply_mod_profile_package(
    game_root: String,
    expected_revision: Option<String>,
    package_id: String,
    source: String,
    expected_files: Vec<ExpectedFile>,
    protected_paths: Vec<ProtectedPath>,
) -> Result<ProfileState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = self::game_root(&game_root)?;
        let _lock = mod_update::acquire_update_lock(&root)?;
        let state = reconcile(&root, read_locked(&root)?)?;
        require_revision(&state, &expected_revision)?;
        let mut profile = state.profile.ok_or_else(invalid_profile)?;
        rollback::require_forward(&profile)?;
        let plan = profile.pending_plan.as_ref().ok_or_else(invalid_profile)?;
        let index = plan.steps.iter().position(|step| step.status != "applied").ok_or_else(invalid_profile)?;
        let step = &plan.steps[index];
        if step.package.source.provider != "sotf-mods" {
            return Err("Resume this native package from Native packages using its original ZIP.".into());
        }
        if key(&step.package.id) != key(&package_id) || step.status != "downloaded" || step.action == "preserve" {
            return Err("The requested package is not the next downloaded step in this profile.".into());
        }
        mod_update::recover_pending_updates(&root)?;
        validate_observed_files(&root, &expected_files)?;
        if protected_paths.len() > 4096 { return Err("Too many installed package paths for this plan.".into()); }
        let protected: Vec<_> = protected_paths.iter().map(|entry| {
            relative(&root, &entry.path).map(|path| (key(&path.to_string_lossy()), entry.directory))
        }).collect::<Result<_, _>>()?;
        let previous = if step.action == "update" {
            Some(validate_previous(&root, step.before.as_ref().ok_or_else(invalid_profile)?)?)
        } else { None };
        // Copy first so the verified bytes cannot be swapped by a cache writer
        // between hashing the download and extracting it.
        let staged = tempfile::Builder::new().prefix(".modstoke-profile-").tempfile_in(&root).map_err(|error| error.to_string())?;
        fs::copy(&source, staged.path()).map_err(|error| error.to_string())?;
        staged.as_file().sync_all().map_err(|error| error.to_string())?;
        if Some(mod_update::hash_file(staged.path())?) != step.package.archive_sha256 {
            return Err("The downloaded archive no longer matches its recorded SHA-256. The pending plan was preserved.".into());
        }
        mod_update::replace_archive_locked(
            staged.path().to_string_lossy().into_owned(), root.to_string_lossy().into_owned(), previous,
            &mut |targets, staging| {
                validate_observed_files(&root, &expected_files)?;
                let planned_step = &profile.pending_plan.as_ref().expect("pending plan").steps[index];
                let (manifest, manifest_hash) = validate_staged_identity(staging, &planned_step.package)?;
                let destination_manifest = if let Some(before) = &planned_step.before {
                    relative(&root, before.manifest_path.as_deref().ok_or_else(invalid_profile)?)?
                } else { manifest };
                let identity_target = targets.iter().find(|target| key(&target.relative_path.to_string_lossy()) == key(&destination_manifest.to_string_lossy()))
                    .ok_or("The archive does not replace the selected package's manifest in its expected location.")?;
                if identity_target.replacement_sha256.as_ref() != Some(&manifest_hash)
                    || planned_step.action == "install" && identity_target.original_sha256.is_some() {
                    return Err("The selected package's install location changed or conflicts with existing files. Refresh before retrying.".into());
                }
                for target in targets {
                    let target_key = key(&target.relative_path.to_string_lossy());
                    if target.original_sha256 != target.replacement_sha256 && protected.iter().any(|(path, directory)|
                        target_key == *path || *directory && target_key.starts_with(&format!("{path}/"))) {
                        return Err(format!("The archive would change another installed package at {}. Its files were preserved; resolve the overlapping package contents before retrying.", target.relative_path.display()));
                    }
                }
                if targets.is_empty() { return Ok(()); }
                rollback::retain_step(&root, &mut profile, index, targets)?;
                let step = &mut profile.pending_plan.as_mut().expect("pending plan").steps[index];
                step.status = "applying".into();
                step.targets = Some(targets.iter().map(|target| TargetReceipt {
                    relative_path: target.relative_path.clone(),
                    original_sha256: target.original_sha256.clone(),
                    replacement_sha256: target.replacement_sha256.clone(),
                }).collect());
                // A committed package must still fit once it becomes a completed
                // profile entry. Refuse before live writes if that receipt cannot fit.
                let mut completed = profile.clone();
                complete_package(&mut completed, index);
                encode_profile(&root, &completed)?;
                write_locked(&root, &profile).map(|_| ())
            },
        )?;
        complete_package(&mut profile, index);
        write_locked(&root, &profile)
    }).await.map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn rollback_mod_profile(game_root: String, expected_revision: Option<String>, plan_id: String) -> Result<ProfileState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = profile_root(&game_root, true)?;
        native_local::require_stopped()?;
        let _lock = mod_update::acquire_update_lock(&root)?;
        let state = reconcile(&root, read_locked(&root)?)?;
        require_revision(&state, &expected_revision)?;
        // Resolve any native package cleanup before reversing the enclosing plan.
        mod_update::recover_pending_updates(&root)?;
        rollback::restore(&root, state, &plan_id)
    }).await.map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn discard_mod_profile_rollback(game_root: String, expected_revision: Option<String>, plan_id: String) -> Result<ProfileState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = profile_root(&game_root, true)?;
        native_local::require_stopped()?;
        let _lock = mod_update::acquire_update_lock(&root)?;
        let state = reconcile(&root, read_locked(&root)?)?;
        require_revision(&state, &expected_revision)?;
        rollback::discard(&root, state, Some(&plan_id))
    }).await.map_err(|error| error.to_string())?
}
