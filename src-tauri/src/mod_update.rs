use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Component, Path, PathBuf};

const WORKSPACE_PREFIX: &str = ".openacai-update-";
const LOCK_PATH: &str = ".openacai-native-update.lock";
const MANIFEST_PATH: &str = "transaction.json";
const FINISHED_MANIFEST_PATH: &str = "finished-transaction.json";
const TRANSACTION_KIND: &str = "openacai-native-mod-update";
const LOADER_DESCRIPTORS: [&str; 2] = [
    "OpenACAI/Runtime/openacai-loader.manifest.json",
    "BepInEx/plugins/OpenACAILoader/openacai-loader.manifest.json",
];
const LOADER_FILE_MANIFESTS: [&str; 2] = [
    "OpenACAI/Runtime/openacai-loader-files.json",
    "BepInEx/plugins/OpenACAILoader/openacai-loader-files.json",
];

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct UpdateTransaction {
    kind: String,
    version: u32,
    root_sha256: String,
    targets: Vec<UpdateTarget>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateTarget {
    pub(crate) relative_path: PathBuf,
    pub(crate) originally_existed: bool,
    pub(crate) backup_path: Option<PathBuf>,
    pub(crate) original_sha256: Option<String>,
    // Version 1 always contains a hash. Version 2 also permits null for deletion.
    pub(crate) replacement_sha256: Option<String>,
}

enum RecoveryOutcome {
    Restored,
    Committed,
    Cleaned,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviousInstall {
    pub(crate) assembly_path: Option<String>,
    pub(crate) enabled_assembly_path: Option<String>,
    pub(crate) package_path: Option<String>,
    pub(crate) enabled_package_path: Option<String>,
    pub(crate) is_enabled: bool,
}

struct Replacement {
    source: Option<PathBuf>,
    relative_path: PathBuf,
    existed: bool,
    removal_sha256: Option<String>,
}

struct NativePackagePolicy {
    package_path: PathBuf,
    old_files: Vec<(PathBuf, String)>,
}

#[derive(Deserialize)]
struct LoaderDescriptor {
    schema: u32,
    id: String,
    version: String,
}

#[derive(Deserialize)]
struct LoaderFileManifest {
    schema: u32,
    version: String,
    files: Vec<LoaderFile>,
}

#[derive(Deserialize)]
struct LoaderFile {
    path: PathBuf,
    sha256: String,
    size: u64,
}

#[tauri::command]
pub async fn update_native_mod(
    source: String,
    destination: String,
    previous_install: PreviousInstall,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        replace_archive(source, destination, Some(previous_install))
    })
    .await
    .map_err(|error| format!("The mod update could not finish: {error}"))?
}

pub fn install_zip_archive(source: String, destination: String) -> Result<(), String> {
    replace_archive(source, destination, None)
}

fn replace_archive(
    source: String,
    destination: String,
    previous_install: Option<PreviousInstall>,
) -> Result<(), String> {
    let selected_root = Path::new(&destination);
    let root = fs::canonicalize(selected_root).map_err(|error| error.to_string())?;
    let _lock = acquire_update_lock(&root)?;
    recover_pending_updates(&root)?;
    replace_archive_locked(source, destination, previous_install, &mut |_, _| Ok(()))
}

// The caller must hold the canonical game root's update lock and recover pending
// transactions. The callback durably records package intent before live writes.
pub(crate) fn replace_archive_locked(
    source: String,
    destination: String,
    previous_install: Option<PreviousInstall>,
    before_commit: &mut dyn FnMut(&[UpdateTarget], &Path) -> Result<(), String>,
) -> Result<(), String> {
    replace_archive_locked_inner(source, destination, previous_install, None, before_commit)
}

pub(crate) fn replace_native_archive_locked(
    source: String,
    destination: String,
    package_path: PathBuf,
    old_files: Vec<(PathBuf, String)>,
    before_commit: &mut dyn FnMut(&[UpdateTarget], &Path) -> Result<(), String>,
) -> Result<(), String> {
    let policy = NativePackagePolicy { package_path, old_files };
    validate_native_policy(&policy)?;
    replace_archive_locked_inner(source, destination, None, Some(policy), before_commit)
}

fn replace_archive_locked_inner(
    source: String,
    destination: String,
    previous_install: Option<PreviousInstall>,
    native: Option<NativePackagePolicy>,
    before_commit: &mut dyn FnMut(&[UpdateTarget], &Path) -> Result<(), String>,
) -> Result<(), String> {
    let selected_root = Path::new(&destination);
    let root = fs::canonicalize(selected_root).map_err(|error| error.to_string())?;
    let (assembly, enabled_assembly, package, enabled_package, disabled) =
        if let Some(previous) = previous_install {
            (
                installed_path(selected_root, previous.assembly_path)?,
                installed_path(selected_root, previous.enabled_assembly_path)?,
                installed_path(selected_root, previous.package_path)?,
                installed_path(selected_root, previous.enabled_package_path)?,
                !previous.is_enabled,
            )
        } else {
            (None, None, None, None, false)
        };
    if disabled && assembly.is_none() {
        return Err("Cannot preserve this mod's disabled state without its assembly path. Refresh the installed mods before updating it.".into());
    }

    let workspace = tempfile::Builder::new()
        .prefix(WORKSPACE_PREFIX)
        .tempdir_in(&root)
        .map_err(|error| format!("Could not create an update staging folder: {error}"))?;
    let staging = workspace.path().join("staging");
    let backup = workspace.path().join("backup");
    fs::create_dir(&staging).map_err(|error| error.to_string())?;
    validate_archive(&source)?;
    if let Some(policy) = &native {
        validate_native_archive(&source, policy)?;
    }
    super::unzip_file(&source, &staging.to_string_lossy()).map_err(|error| {
        format!(
            "Could not prepare the replacement archive. The installed mod was not changed. {error}"
        )
    })?;

    let mut staged_files = Vec::new();
    collect_files(&staging, &mut staged_files)?;
    if staged_files.is_empty() {
        return Err(
            "The replacement archive contains no files. The installed mod was not changed.".into(),
        );
    }
    staged_files.sort();
    if let Some(policy) = &native {
        let mut spellings = HashMap::new();
        record_native_path_case(&policy.package_path, &mut spellings)?;
        for (path, _) in &policy.old_files {
            record_native_path_case(path, &mut spellings)?;
        }
        for source in &staged_files {
            let relative = source.strip_prefix(&staging).map_err(|error| error.to_string())?;
            validate_native_payload_path(relative, &policy.package_path)?;
            record_native_path_case(relative, &mut spellings)?;
        }
    }
    let loader_package = validate_staged_loader(&staging, &staged_files)?;
    if native.is_some() && loader_package {
        return Err("A Loader package cannot be installed as a native mod.".into());
    }
    let staged_paths: HashSet<String> = staged_files
        .iter()
        .map(|path| path_key(path.strip_prefix(&staging).expect("staged file")))
        .collect();

    let vortex_files = vortex_deployed_files(&root)?;
    let mut targets = HashSet::new();
    let mut replacements = Vec::new();
    for source in staged_files {
        let original = source
            .strip_prefix(&staging)
            .map_err(|error| error.to_string())?;
        let mut relative_path = original.to_owned();
        if disabled {
            if let (Some(package), Some(enabled_package)) = (&package, &enabled_package) {
                if let Some(suffix) = strip_path_prefix(original, enabled_package) {
                    relative_path = package.join(suffix);
                }
            }
            if let (Some(assembly), Some(enabled_assembly)) = (&assembly, &enabled_assembly) {
                if path_key(original) == path_key(enabled_assembly)
                    || path_key(&relative_path) == path_key(enabled_assembly)
                {
                    relative_path = assembly.clone();
                }
            }
        }

        if native.is_some()
            && is_native_user_file(&relative_path)
            && validate_target(&root, &relative_path)?
        {
            continue;
        }
        let key = path_key(&relative_path);
        if !targets.insert(key.clone()) {
            return Err(format!(
                "The archive maps multiple files to {}.",
                relative_path.display()
            ));
        }
        validate_mod_path(&relative_path)?;
        let existed = validate_target(&root, &relative_path)?;
        if existed
            && (is_bepinex_config(original)
                || is_bepinex_config(&relative_path)
                || (loader_package && is_loader_user_file(&relative_path)))
        {
            continue;
        }
        if key == "vortex.deployment.json" || vortex_files.contains(&key) {
            return Err(format!(
                "{} is managed by Vortex. Update it through Vortex before retrying this mod.",
                relative_path.display()
            ));
        }
        replacements.push(Replacement {
            source: Some(source),
            relative_path,
            existed,
            removal_sha256: None,
        });
    }

    if let Some(assembly) = &assembly {
        if !targets.contains(&path_key(assembly)) {
            return Err(format!("The replacement archive does not contain {}. The installed mod was not changed; review the package layout before updating it.", assembly.display()));
        }
    }

    if let Some(policy) = &native {
        plan_native_removals(&root, policy, &staged_paths, &mut replacements)?;
    } else if loader_package {
        plan_loader_removals(&root, &staged_paths, &mut replacements)?;
    }
    if replacements.is_empty() {
        before_commit(&[], &staging)?;
        return Ok(()); // An archive containing only existing configuration defaults.
    }

    // Publish a complete, immutable manifest only after every backup is durable.
    fs::create_dir(&backup).map_err(|error| error.to_string())?;
    let mut transaction = UpdateTransaction {
        kind: TRANSACTION_KIND.into(),
        version: 2,
        root_sha256: hash_bytes(path_key(&root).as_bytes()),
        targets: Vec::new(),
    };
    for (index, replacement) in replacements.iter().enumerate() {
        let original_sha256 = target_hash(&root, &replacement.relative_path)?;
        if original_sha256.is_some() != replacement.existed {
            return Err(format!("{} changed while the update was being prepared. The installed mod was not changed.", replacement.relative_path.display()));
        }
        if replacement.source.is_none() && original_sha256 != replacement.removal_sha256 {
            return Err(format!("{} changed while its removal was being prepared. No installed files were changed.", replacement.relative_path.display()));
        }
        let backup_path = if let Some(expected) = &original_sha256 {
            let relative = PathBuf::from("backup").join(index.to_string());
            let saved = workspace.path().join(&relative);
            copy_durable(&root.join(&replacement.relative_path), &saved)?;
            if hash_file(&saved)? != *expected {
                return Err(format!(
                    "{} changed while it was being backed up. The installed mod was not changed.",
                    replacement.relative_path.display()
                ));
            }
            Some(relative)
        } else {
            None
        };
        transaction.targets.push(UpdateTarget {
            relative_path: replacement.relative_path.clone(),
            originally_existed: replacement.existed,
            backup_path,
            original_sha256,
            replacement_sha256: replacement.source.as_deref().map(hash_file).transpose()?,
        });
        // The source must reach disk before it becomes an installed file.
        if let Some(source) = &replacement.source {
            OpenOptions::new()
                .write(true)
                .open(source)
                .and_then(|file| file.sync_all())
                .map_err(|error| error.to_string())?;
        }
    }
    let manifest = serde_json::to_vec(&transaction).map_err(|error| error.to_string())?;
    let manifest_hash = hash_bytes(&manifest);
    write_durable(workspace.path(), MANIFEST_PATH, &manifest)?;
    before_commit(&transaction.targets, &staging)?;

    // From here on, neither an error nor unwinding may discard recovery data.
    let workspace = workspace.into_path();
    let result = (|| {
        for (index, (replacement, target)) in
            replacements.iter().zip(&transaction.targets).enumerate()
        {
            ensure_unmanaged(&root, &target.relative_path)?;
            require_original(&root, target)?;
            write_durable(
                &workspace,
                &format!("intent-{index}"),
                manifest_hash.as_bytes(),
            )?;
            if let Some(source) = &replacement.source {
                create_parent_dirs(&root, &target.relative_path)?;
                // Recheck after preparation, immediately before the live replacement.
                if Some(hash_file(source)?) != target.replacement_sha256 {
                    return Err(format!(
                        "The staged replacement for {} changed.",
                        target.relative_path.display()
                    ));
                }
                ensure_unmanaged(&root, &target.relative_path)?;
                require_original(&root, target)?;
                durable_rename(
                    source,
                    &root.join(&target.relative_path),
                    target.originally_existed,
                )?;
            } else {
                ensure_unmanaged(&root, &target.relative_path)?;
                require_original(&root, target)?;
                // Keep both the verified backup and the removed file until commit.
                durable_rename(
                    &root.join(&target.relative_path),
                    &workspace.join(format!("deleted-{index}")),
                    false,
                )?;
            }
        }
        write_durable(&workspace, "committed", manifest_hash.as_bytes())
    })();
    if let Err(error) = result {
        return match recover_workspace(&root, &workspace) {
            Ok(RecoveryOutcome::Committed | RecoveryOutcome::Cleaned) => Err(format!(
                "The update was applied, but finalization reported an error: {error}"
            )),
            Ok(RecoveryOutcome::Restored) => Err(format!(
                "Update failed. Previous files were restored. {error}"
            )),
            Err(recovery_error) => Err(format!("Update failed: {error}\n{recovery_error}")),
        };
    }
    cleanup_transaction(&workspace, false).map_err(|error| retained_error(
        &workspace,
        &format!("The update was committed, but its recovery workspace could not be cleaned up: {error}"),
    ))
}

fn installed_path(root: &Path, value: Option<String>) -> Result<Option<PathBuf>, String> {
    value
        .map(|value| {
            let relative = strip_path_prefix(Path::new(&value), root).ok_or_else(|| {
                format!("Installed path is outside the selected game folder: {value}")
            })?;
            validate_relative_path(&relative)?;
            Ok(relative)
        })
        .transpose()
}

fn validate_archive(source: &str) -> Result<(), String> {
    let reader = File::open(source).map_err(|error| error.to_string())?;
    let mut archive = zip::ZipArchive::new(reader).map_err(|error| error.to_string())?;
    let mut entries = HashMap::new();
    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(|error| error.to_string())?;
        let relative = file
            .enclosed_name()
            .ok_or_else(|| format!("Unsafe path in zip entry: {}", file.name()))?;
        validate_mod_path(relative)?;
        let kind = file.unix_mode().unwrap_or_default() & 0o170000;
        if kind != 0 && kind != 0o100000 && kind != 0o040000 {
            return Err(format!("The archive contains a linked or special file: {}", file.name()));
        }
        let key = path_key(relative);
        if entries.insert(key, file.is_dir()).is_some() {
            return Err(format!("The archive contains duplicate paths: {}", file.name()));
        }
    }
    for key in entries.keys() {
        let mut parent = Path::new(key).parent();
        while let Some(path) = parent {
            if entries.get(&path_key(path)) == Some(&false) {
                return Err(format!("The archive uses a file as a folder: {}", path.display()));
            }
            parent = path.parent();
        }
    }
    Ok(())
}

fn is_bepinex_config(path: &Path) -> bool {
    path_key(path).starts_with("bepinex/config/")
}

fn is_loader_user_file(path: &Path) -> bool {
    // These directories contain mutable user data, never package-owned removals.
    // Bootstrap INI and runtimeconfig.json files are runtime payload, not settings.
    path_key(path)
        .split('/')
        .any(|part| part == "config" || part == "userdata")
}

pub(crate) fn is_native_user_file(path: &Path) -> bool {
    is_loader_user_file(path)
        || path.extension().is_some_and(|extension| {
            let extension = extension.to_string_lossy();
            extension.eq_ignore_ascii_case("cfg") || extension.eq_ignore_ascii_case("ini")
        })
}

fn validate_native_path(path: &Path) -> Result<(), String> {
    validate_mod_path(path)?;
    let text = path.to_str().ok_or("Native mod paths must be valid Unicode.")?;
    for part in text.split(['/', '\\']) {
        let stem = part.split('.').next().unwrap_or_default().trim_end().to_ascii_lowercase();
        let device = matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
            || ["com", "lpt"].iter().any(|prefix| {
                stem.strip_prefix(*prefix).is_some_and(|suffix| {
                    matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³")
                })
            });
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.ends_with(['.', ' '])
            || part.chars().any(|character| character.is_control() || "<>:\"|?*".contains(character))
            || device
        {
            return Err(format!("Invalid native mod path: {}", path.display()));
        }
    }
    Ok(())
}

fn validate_native_payload_path(path: &Path, package: &Path) -> Result<(), String> {
    validate_native_path(path)?;
    if !strip_path_prefix(path, package).is_some_and(|suffix| !suffix.as_os_str().is_empty()) {
        return Err(format!("Native mod file {} is outside {}.", path.display(), package.display()));
    }
    Ok(())
}

fn record_native_path_case(path: &Path, spellings: &mut HashMap<String, PathBuf>) -> Result<(), String> {
    let mut prefix = PathBuf::new();
    for component in path.components() {
        prefix.push(component);
        if let Some(previous) = spellings.insert(path_key(&prefix), prefix.clone()) {
            if previous != prefix {
                return Err(format!("Native mod paths have conflicting capitalization: {} and {}.", previous.display(), prefix.display()));
            }
        }
    }
    Ok(())
}

fn validate_native_policy(policy: &NativePackagePolicy) -> Result<(), String> {
    validate_native_path(&policy.package_path)?;
    let parts: Vec<_> = policy.package_path.components().collect();
    if parts.len() != 3
        || !parts[0].as_os_str().to_string_lossy().eq_ignore_ascii_case("OpenACAI")
        || !parts[1].as_os_str().to_string_lossy().eq_ignore_ascii_case("Mods")
    {
        return Err("Native mod packages must use OpenACAI/Mods/<folder>.".into());
    }
    if policy.old_files.len() > 8192 {
        return Err("The native mod ownership record contains too many files.".into());
    }
    let mut paths = HashSet::new();
    let mut spellings = HashMap::new();
    record_native_path_case(&policy.package_path, &mut spellings)?;
    for (path, hash) in &policy.old_files {
        validate_native_payload_path(path, &policy.package_path)?;
        if !valid_sha256(hash)
            || hash.bytes().any(|byte| byte.is_ascii_uppercase())
            || !paths.insert(path_key(path))
        {
            return Err(format!("Invalid or duplicate native mod ownership record for {}.", path.display()));
        }
        record_native_path_case(path, &mut spellings)?;
    }
    Ok(())
}

fn validate_native_archive(source: &str, policy: &NativePackagePolicy) -> Result<(), String> {
    let reader = File::open(source).map_err(|error| error.to_string())?;
    let mut archive = zip::ZipArchive::new(reader).map_err(|error| error.to_string())?;
    let mut spellings = HashMap::new();
    record_native_path_case(&policy.package_path, &mut spellings)?;
    for (path, _) in &policy.old_files {
        record_native_path_case(path, &mut spellings)?;
    }
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| error.to_string())?;
        let name = if entry.is_dir() { entry.name().trim_end_matches(['/', '\\']) } else { entry.name() };
        let path = Path::new(name);
        validate_native_path(path)?;
        // ZIPs may explicitly include the package's ancestor directories.
        if !entry.is_dir() || strip_path_prefix(&policy.package_path, path).is_none() {
            validate_native_payload_path(path, &policy.package_path)?;
        }
        record_native_path_case(path, &mut spellings)?;
    }
    Ok(())
}

fn plan_native_removals(
    root: &Path,
    policy: &NativePackagePolicy,
    staged_paths: &HashSet<String>,
    replacements: &mut Vec<Replacement>,
) -> Result<(), String> {
    for (path, expected) in &policy.old_files {
        if staged_paths.contains(&path_key(path)) || is_native_user_file(path) {
            continue;
        }
        let Some(current) = target_hash(root, path)? else { continue; };
        if current != *expected {
            return Err(format!("Native mod update conflict at {}: this obsolete package file has changed since installation. It was preserved.", root.join(path).display()));
        }
        ensure_unmanaged(root, path)?;
        replacements.push(Replacement {
            source: None,
            relative_path: path.clone(),
            existed: true,
            removal_sha256: Some(current),
        });
    }
    Ok(())
}

fn read_loader_json<T: serde::de::DeserializeOwned>(root: &Path, path: &str) -> Result<T, String> {
    let bytes = fs::read(root.join(path))
        .map_err(|error| format!("Could not read {}: {error}", root.join(path).display()))?;
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
    serde_json::from_slice(bytes)
        .map_err(|error| format!("Invalid Loader manifest at {}: {error}", root.join(path).display()))
}

fn read_loader_file_manifest(root: &Path) -> Result<Option<LoaderFileManifest>, String> {
    for path in LOADER_FILE_MANIFESTS {
        if !validate_target(root, Path::new(path))? {
            continue;
        }
        let manifest: LoaderFileManifest = read_loader_json(root, path)?;
        if manifest.schema != 1 || manifest.version.trim().is_empty() || manifest.files.is_empty() {
            return Err(format!("Unsupported or empty Loader file manifest at {}.", root.join(path).display()));
        }
        let mut paths = HashSet::new();
        for file in &manifest.files {
            validate_mod_path(&file.path)?;
            if !valid_sha256(&file.sha256) || !paths.insert(path_key(&file.path)) {
                return Err(format!("Invalid or duplicate file record for {} in {}.", file.path.display(), root.join(path).display()));
            }
        }
        return Ok(Some(manifest));
    }
    Ok(None)
}

fn validate_staged_loader(staging: &Path, staged_files: &[PathBuf]) -> Result<bool, String> {
    let mut version = None;
    for path in LOADER_DESCRIPTORS {
        if !validate_target(staging, Path::new(path))? {
            continue;
        }
        let descriptor: LoaderDescriptor = read_loader_json(staging, path)?;
        if descriptor.schema != 1
            || descriptor.id != "openacai.loader.sotf"
            || descriptor.version.trim().is_empty()
        {
            return Err(format!("Unsupported Loader descriptor at {path}."));
        }
        if version.as_ref().is_some_and(|value| value != &descriptor.version) {
            return Err("The Loader package contains conflicting descriptor versions.".into());
        }
        version = Some(descriptor.version);
    }
    let manifest = read_loader_file_manifest(staging)?;
    let Some(version) = version else {
        if manifest.is_some() {
            return Err("The archive contains a Loader file manifest without its package descriptor.".into());
        }
        return Ok(false);
    };
    let manifest = manifest.ok_or("The Loader package is missing its installed file manifest.")?;
    if manifest.version != version {
        return Err("The Loader descriptor and file manifest have different versions.".into());
    }
    // Both copies describe the same package; neither may silently override the other.
    if validate_target(staging, Path::new(LOADER_FILE_MANIFESTS[0]))?
        && validate_target(staging, Path::new(LOADER_FILE_MANIFESTS[1]))?
        && hash_file(&staging.join(LOADER_FILE_MANIFESTS[0]))?
            != hash_file(&staging.join(LOADER_FILE_MANIFESTS[1]))?
    {
        return Err("The Loader package contains conflicting file manifest copies.".into());
    }
    let mut listed = HashSet::new();
    for file in &manifest.files {
        listed.insert(path_key(&file.path));
        let hash = target_hash(staging, &file.path)?;
        if !hash.as_deref().is_some_and(|hash| hash.eq_ignore_ascii_case(&file.sha256)) {
            return Err(format!("Loader package file {} is missing or does not match its manifest SHA-256.", file.path.display()));
        }
        let size = fs::metadata(staging.join(&file.path))
            .map_err(|error| error.to_string())?
            .len();
        if size != file.size {
            return Err(format!("Loader package file {} does not match its manifest size.", file.path.display()));
        }
    }
    for source in staged_files {
        let relative = source.strip_prefix(staging).map_err(|error| error.to_string())?;
        let key = path_key(relative);
        if !listed.contains(&key)
            && !is_bepinex_config(relative)
            && !LOADER_FILE_MANIFESTS.iter().any(|path| key == path_key(Path::new(path)))
        {
            return Err(format!("Loader package file {} is absent from its file manifest.", relative.display()));
        }
    }
    Ok(true)
}

fn plan_loader_removals(
    root: &Path,
    staged_paths: &HashSet<String>,
    replacements: &mut Vec<Replacement>,
) -> Result<(), String> {
    let mut removals = HashSet::new();
    if let Some(previous) = read_loader_file_manifest(root)? {
        for file in previous.files {
            let key = path_key(&file.path);
            if staged_paths.contains(&key) || is_loader_user_file(&file.path) {
                continue;
            }
            let Some(current) = target_hash(root, &file.path)? else {
                continue;
            };
            if !current.eq_ignore_ascii_case(&file.sha256) {
                return Err(format!("Loader update conflict at {}: this obsolete package file has changed since installation. It was preserved. Move it outside the game folder or resolve its ownership before retrying.", root.join(&file.path).display()));
            }
            ensure_unmanaged(root, &file.path)?;
            removals.insert(key);
            replacements.push(Replacement {
                source: None,
                relative_path: file.path,
                existed: true,
                removal_sha256: Some(current),
            });
        }
    }

    let mut legacy_files = Vec::new();
    if validate_target(root, Path::new("version.dll"))? {
        legacy_files.push(root.join("version.dll"));
    }
    for relative in ["_RedLoader", "BepInEx/plugins/RedLoaderBepInExCompat"] {
        let folder = root.join(relative);
        match fs::symlink_metadata(&folder) {
            Ok(metadata) => {
                reject_linked_path(&folder, &metadata)?;
                if metadata.is_dir() {
                    collect_files(&folder, &mut legacy_files)?;
                } else {
                    return Err(format!("Legacy Loader conflict at {}. Resolve this path before installing Modstoke Loader.", folder.display()));
                }
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(format!("Could not inspect {}: {error}", folder.display())),
        }
    }
    let mut conflicts = Vec::new();
    for file in legacy_files {
        let relative = file.strip_prefix(root).map_err(|error| error.to_string())?;
        let key = path_key(relative);
        if !staged_paths.contains(&key) && !removals.contains(&key) {
            conflicts.push(file.display().to_string());
        }
    }
    if !conflicts.is_empty() {
        return Err(format!("Legacy Loader files would remain outside the verified package replacement or removal plan. They were preserved. Remove or move the old loader outside the game folder before retrying:\n{}", conflicts.join("\n")));
    }
    Ok(())
}

fn strip_path_prefix(path: &Path, prefix: &Path) -> Option<PathBuf> {
    let mut components = path.components();
    for expected in prefix.components() {
        let actual = components.next()?;
        if !actual
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(&expected.as_os_str().to_string_lossy())
        {
            return None;
        }
    }
    Some(components.collect())
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_lowercase()
}

fn validate_relative_path(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty()
        || path.components().any(|part| {
            let name = part.as_os_str().to_string_lossy();
            !matches!(part, Component::Normal(_))
                || name.contains(':')
                || name.ends_with(['.', ' '])
        })
    {
        return Err(format!("Invalid update path: {}", path.display()));
    }
    Ok(())
}

pub(crate) fn validate_mod_path(path: &Path) -> Result<(), String> {
    validate_relative_path(path)?;
    let key = path_key(path);
    if key == "modstoke-profile.lock.json"
        || key.starts_with("modstoke-profile.lock.json/")
        || key.starts_with(".modstoke-profile-")
        || key.starts_with(".modstoke-pack-")
        || key == LOCK_PATH
        || key.starts_with(&format!("{LOCK_PATH}/"))
        || key
            .split('/')
            .next()
            .unwrap_or_default()
            .starts_with(WORKSPACE_PREFIX)
    {
        return Err(format!(
            "The archive contains a reserved update path: {}",
            path.display()
        ));
    }
    if key == "vortex.deployment.json" {
        return Err("Vortex deployment metadata cannot be replaced by a native mod update.".into());
    }
    Ok(())
}

fn reject_linked_path(path: &Path, metadata: &fs::Metadata) -> Result<(), String> {
    #[cfg(windows)]
    let linked = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let linked = metadata.file_type().is_symlink();
    if linked {
        return Err(format!(
            "Refusing to update through a linked path: {}",
            path.display()
        ));
    }
    Ok(())
}

pub(crate) fn validate_directory(path: &Path) -> Result<(), String> {
    // Include the root's ancestry, not just the paths supplied by the archive.
    for ancestor in path.ancestors() {
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        let metadata = fs::symlink_metadata(ancestor)
            .map_err(|error| format!("Could not inspect {}: {error}", ancestor.display()))?;
        reject_linked_path(ancestor, &metadata)?;
        if !metadata.is_dir() {
            return Err(format!("Expected a folder at {}", ancestor.display()));
        }
    }
    Ok(())
}

pub(crate) fn validate_target(root: &Path, relative: &Path) -> Result<bool, String> {
    validate_relative_path(relative)?;
    validate_directory(root)?;
    let mut target = root.to_owned();
    let mut parts = relative.components().peekable();
    let mut file_exists = false;
    while let Some(part) = parts.next() {
        target.push(part);
        match fs::symlink_metadata(&target) {
            Ok(metadata) => {
                reject_linked_path(&target, &metadata)?;
                if parts.peek().is_some() {
                    if !metadata.is_dir() {
                        return Err(format!("Expected a folder at {}", target.display()));
                    }
                } else if metadata.is_file() {
                    file_exists = true;
                } else {
                    return Err(format!(
                        "Refusing to replace a folder: {}",
                        target.display()
                    ));
                }
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(format!("Could not inspect {}: {error}", target.display())),
        }
    }
    Ok(file_exists)
}

fn collect_files(folder: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    validate_directory(folder)?;
    for entry in fs::read_dir(folder).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(|error| error.to_string())?;
        reject_linked_path(&entry.path(), &metadata)?;
        if metadata.is_dir() {
            collect_files(&entry.path(), files)?;
        } else if metadata.is_file() {
            files.push(entry.path());
        } else {
            return Err(format!(
                "Expected an ordinary file at {}",
                entry.path().display()
            ));
        }
    }
    Ok(())
}

fn create_parent_dirs(root: &Path, relative: &Path) -> Result<(), String> {
    validate_target(root, relative)?;
    let mut parent = root.to_owned();
    if let Some(relative_parent) = relative.parent() {
        for part in relative_parent.components() {
            parent.push(part);
            if !parent.exists() {
                fs::create_dir(&parent).map_err(|error| error.to_string())?;
            }
            validate_directory(&parent)?;
        }
    }
    Ok(())
}

pub(crate) fn acquire_update_lock(root: &Path) -> Result<File, String> {
    if validate_target(root, Path::new("SonsOfTheForestDS.exe"))? {
        crate::server_launch::reap_exited()?;
        crate::server_process::require_stopped()?;
    }
    validate_target(root, Path::new(LOCK_PATH))?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // An open handle owns this root's lock; a process exit releases it without
        // leaving a stale lock that would need to be guessed at or deleted.
        options.share_mode(0).custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let lock = options.open(root.join(LOCK_PATH)).map_err(|error| {
        format!(
            "Could not lock native mod updates in {}. Another update may be running: {error}",
            root.display()
        )
    })?;
    reject_linked_path(
        &root.join(LOCK_PATH),
        &lock.metadata().map_err(|error| error.to_string())?,
    )?;
    #[cfg(not(windows))]
    lock.try_lock().map_err(|error| {
        format!(
            "Could not lock native mod updates in {}: {error}",
            root.display()
        )
    })?;
    Ok(lock)
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(crate) fn hash_file(path: &Path) -> Result<String, String> {
    validate_directory(path.parent().ok_or("Invalid file path")?)?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Could not inspect {}: {error}", path.display()))?;
    reject_linked_path(path, &metadata)?;
    if !metadata.is_file() {
        return Err(format!("Expected an ordinary file at {}", path.display()));
    }
    let mut file =
        File::open(path).map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let length = file
            .read(&mut buffer)
            .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
        if length == 0 {
            break;
        }
        hash.update(&buffer[..length]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

pub(crate) fn target_hash(root: &Path, relative: &Path) -> Result<Option<String>, String> {
    if validate_target(root, relative)? {
        hash_file(&root.join(relative)).map(Some)
    } else {
        Ok(None)
    }
}

fn require_original(root: &Path, target: &UpdateTarget) -> Result<(), String> {
    if target_hash(root, &target.relative_path)? != target.original_sha256 {
        return Err(format!(
            "{} changed after it was backed up. Its current contents were not replaced.",
            target.relative_path.display()
        ));
    }
    Ok(())
}

pub(crate) fn ensure_unmanaged(root: &Path, relative: &Path) -> Result<(), String> {
    validate_mod_path(relative)?;
    if vortex_deployed_files(root)?.contains(&path_key(relative)) {
        return Err(format!("{} is managed by Vortex. Its current contents were not changed; resolve its ownership in Vortex before retrying.", relative.display()));
    }
    Ok(())
}

pub(crate) fn durable_rename(source: &Path, destination: &Path, replace: bool) -> Result<(), String> {
    #[cfg(windows)]
    let result = {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        };
        let from: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        let flags = if replace {
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH
        } else {
            MOVEFILE_WRITE_THROUGH
        };
        // Both paths are on the selected volume. Write-through makes publication
        // of the manifest, intents and completion marker precede later mutations.
        if unsafe { MoveFileExW(PCWSTR(from.as_ptr()), PCWSTR(to.as_ptr()), flags) }.as_bool() {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    };
    #[cfg(not(windows))]
    let result = (|| {
        if replace {
            fs::rename(source, destination)?;
        } else {
            // Publishing a link fails atomically if another writer created the
            // destination; do not emulate this with an exists-then-rename check.
            fs::hard_link(source, destination)?;
            fs::remove_file(source)?;
        }
        if let Some(parent) = destination.parent() {
            File::open(parent)?.sync_all()?;
        }
        if source.parent() != destination.parent() {
            if let Some(parent) = source.parent() {
                File::open(parent)?.sync_all()?;
            }
        }
        Ok::<(), std::io::Error>(())
    })();
    result.map_err(|error| {
        format!(
            "Could not move {} to {}: {error}",
            source.display(),
            destination.display()
        )
    })
}

fn write_durable(folder: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    validate_target(folder, Path::new(name))?;
    let mut temporary = tempfile::Builder::new()
        .prefix("pending-")
        .tempfile_in(folder)
        .map_err(|error| error.to_string())?;
    temporary
        .write_all(bytes)
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|error| error.to_string())?;
    let temporary = temporary.into_temp_path();
    durable_rename(&temporary, &folder.join(name), false)
}

pub(crate) fn copy_durable(source: &Path, destination: &Path) -> Result<(), String> {
    let folder = destination.parent().ok_or("Invalid backup folder")?;
    validate_directory(folder)?;
    let temporary = tempfile::Builder::new()
        .prefix("pending-")
        .tempfile_in(folder)
        .map_err(|error| error.to_string())?;
    fs::copy(source, temporary.path())
        .map_err(|error| format!("Could not back up {}: {error}", source.display()))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    let temporary = temporary.into_temp_path();
    durable_rename(&temporary, destination, false)
}

fn retained_error(workspace: &Path, error: &str) -> String {
    format!("{error}\nRecovery workspace retained at {}. Keep this folder, resolve the reported problem, and retry a ZIP install or native mod update in this game folder to resume recovery.", workspace.display())
}

pub(crate) fn recover_pending_updates(root: &Path) -> Result<(), String> {
    let mut workspaces = Vec::new();
    for entry in fs::read_dir(root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if entry
            .file_name()
            .to_string_lossy()
            .to_lowercase()
            .starts_with(WORKSPACE_PREFIX)
        {
            let metadata = fs::symlink_metadata(entry.path()).map_err(|error| error.to_string())?;
            reject_linked_path(&entry.path(), &metadata)
                .map_err(|error| retained_error(&entry.path(), &error))?;
            if metadata.is_dir() {
                workspaces.push(entry.path());
            }
        }
    }
    workspaces.sort();
    for workspace in workspaces {
        if validate_target(&workspace, Path::new(MANIFEST_PATH))
            .and_then(|active| {
                if active {
                    Ok(true)
                } else {
                    validate_target(&workspace, Path::new(FINISHED_MANIFEST_PATH))
                }
            })
            .map_err(|error| retained_error(&workspace, &error))?
        {
            recover_workspace(root, &workspace)?;
        }
        // Older, unjournaled workspaces and interrupted archive preparation have
        // no recognized transaction. Never guess how to restore their contents.
    }
    Ok(())
}

pub(crate) fn require_no_pending_updates(root: &Path) -> Result<(), String> {
    validate_directory(root)?;
    for entry in fs::read_dir(root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry.file_name().to_string_lossy().to_lowercase().starts_with(WORKSPACE_PREFIX) { continue; }
        let metadata = fs::symlink_metadata(entry.path()).map_err(|error| error.to_string())?;
        reject_linked_path(&entry.path(), &metadata)?;
        if metadata.is_dir() && (validate_target(&entry.path(), Path::new(MANIFEST_PATH))?
            || validate_target(&entry.path(), Path::new(FINISHED_MANIFEST_PATH))?) {
            return Err("A retained Manager update transaction needs recovery. Refresh server inventory while stopped, then read launch settings again. No launch or recovery was attempted here.".into());
        }
    }
    Ok(())
}

fn validate_transaction(root: &Path, transaction: &UpdateTransaction) -> Result<(), String> {
    if transaction.kind != TRANSACTION_KIND
        || !matches!(transaction.version, 1 | 2)
        || transaction.targets.is_empty()
    {
        return Err("The retained native update transaction has an unsupported format.".into());
    }
    if transaction.root_sha256 != hash_bytes(path_key(root).as_bytes()) {
        return Err(
            "The retained native update transaction belongs to a different game folder.".into(),
        );
    }
    let mut targets = HashSet::new();
    for (index, target) in transaction.targets.iter().enumerate() {
        validate_mod_path(&target.relative_path)?;
        if !targets.insert(path_key(&target.relative_path))
            || match &target.replacement_sha256 {
                Some(hash) => !valid_sha256(hash),
                None => transaction.version != 2 || !target.originally_existed,
            }
        {
            return Err(format!(
                "Invalid transaction target: {}",
                target.relative_path.display()
            ));
        }
        if target.originally_existed {
            if target.backup_path.as_ref() != Some(&PathBuf::from("backup").join(index.to_string()))
                || !target.original_sha256.as_deref().is_some_and(valid_sha256)
            {
                return Err(format!(
                    "Invalid backup record for {}",
                    target.relative_path.display()
                ));
            }
        } else if target.backup_path.is_some() || target.original_sha256.is_some() {
            return Err(format!(
                "Invalid original-file record for {}",
                target.relative_path.display()
            ));
        }
    }
    Ok(())
}

fn has_marker(workspace: &Path, name: &str, manifest_hash: &str) -> Result<bool, String> {
    if !validate_target(workspace, Path::new(name))? {
        return Ok(false);
    }
    let contents = fs::read(workspace.join(name)).map_err(|error| error.to_string())?;
    if contents != manifest_hash.as_bytes() {
        return Err(format!(
            "The transaction marker {} is damaged or belongs to a different manifest.",
            workspace.join(name).display()
        ));
    }
    Ok(true)
}

fn recover_workspace(root: &Path, workspace: &Path) -> Result<RecoveryOutcome, String> {
    (|| {
        validate_directory(workspace)?;
        let active = validate_target(workspace, Path::new(MANIFEST_PATH))?;
        let finished = validate_target(workspace, Path::new(FINISHED_MANIFEST_PATH))?;
        if active == finished {
            return Err("Expected exactly one active or finished transaction manifest.".into());
        }
        let manifest_name = if finished {
            FINISHED_MANIFEST_PATH
        } else {
            MANIFEST_PATH
        };
        let manifest =
            fs::read(workspace.join(manifest_name)).map_err(|error| error.to_string())?;
        let transaction: UpdateTransaction =
            serde_json::from_slice(&manifest).map_err(|error| {
                format!("The retained transaction manifest could not be read: {error}")
            })?;
        validate_transaction(root, &transaction)?;
        let manifest_hash = hash_bytes(&manifest);
        if finished {
            cleanup_transaction(workspace, true).map_err(|error| {
                format!("Update recovery is complete, but workspace cleanup failed: {error}")
            })?;
            return Ok(RecoveryOutcome::Cleaned);
        }
        let committed = has_marker(workspace, "committed", &manifest_hash)?;
        let restored = has_marker(workspace, "restored", &manifest_hash)?;
        if committed && restored {
            return Err("The transaction contains conflicting completion markers.".into());
        }
        if !committed && !restored {
            for (index, target) in transaction.targets.iter().enumerate().rev() {
                if !has_marker(workspace, &format!("intent-{index}"), &manifest_hash)? {
                    continue;
                }
                restore_target(root, workspace, index, target)?;
            }
            write_durable(workspace, "restored", manifest_hash.as_bytes())?;
        }
        cleanup_transaction(workspace, false).map_err(|error| {
            if committed {
                format!("The update was committed, but workspace cleanup failed: {error}")
            } else {
                format!("Previous files were restored, but workspace cleanup failed: {error}")
            }
        })?;
        Ok(if committed {
            RecoveryOutcome::Committed
        } else {
            RecoveryOutcome::Restored
        })
    })()
    .map_err(|error: String| retained_error(workspace, &error))
}

fn restore_target(
    root: &Path,
    workspace: &Path,
    index: usize,
    target: &UpdateTarget,
) -> Result<(), String> {
    restore_target_named(root, workspace, &format!("removed-{index}"), target)
}

pub(crate) fn restore_pack_target(
    root: &Path,
    workspace: &Path,
    slot: usize,
    index: usize,
    target: &UpdateTarget,
) -> Result<(), String> {
    restore_target_named(root, workspace, &format!("removed-{slot}-{index}"), target)
}

fn restore_target_named(
    root: &Path,
    workspace: &Path,
    removed_name: &str,
    target: &UpdateTarget,
) -> Result<(), String> {
    let current = target_hash(root, &target.relative_path)?;
    if current == target.original_sha256 {
        return Ok(()); // Already restored, or the replacement never happened.
    }
    require_replacement(target, &current)?;
    ensure_unmanaged(root, &target.relative_path)?;
    if let Some(backup) = &target.backup_path {
        if target_hash(workspace, backup)? != target.original_sha256 {
            return Err(format!(
                "The original backup for {} is missing or changed at {}.",
                target.relative_path.display(),
                workspace.join(backup).display()
            ));
        }
        // Never consume the backup: a second interruption must be recoverable too.
        let temporary = tempfile::Builder::new()
            .prefix("restore-")
            .tempfile_in(workspace)
            .map_err(|error| error.to_string())?;
        fs::copy(workspace.join(backup), temporary.path()).map_err(|error| error.to_string())?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|error| error.to_string())?;
        if Some(hash_file(temporary.path())?) != target.original_sha256 {
            return Err(format!(
                "The restore copy for {} does not match its original hash.",
                target.relative_path.display()
            ));
        }
        let temporary = temporary.into_temp_path();
        create_parent_dirs(root, &target.relative_path)?;
        ensure_unmanaged(root, &target.relative_path)?;
        require_replacement(target, &target_hash(root, &target.relative_path)?)?;
        durable_rename(&temporary, &root.join(&target.relative_path), current.is_some())?;
    } else {
        let removed = PathBuf::from(removed_name);
        if let Some(hash) = target_hash(workspace, &removed)? {
            if Some(hash) != target.replacement_sha256 {
                return Err(format!(
                    "The recovery copy at {} changed.",
                    workspace.join(removed).display()
                ));
            }
        }
        ensure_unmanaged(root, &target.relative_path)?;
        require_replacement(target, &target_hash(root, &target.relative_path)?)?;
        // A write-through move also makes removing a newly added live file durable.
        durable_rename(
            &root.join(&target.relative_path),
            &workspace.join(removed),
            true,
        )?;
    }
    Ok(())
}

fn require_replacement(target: &UpdateTarget, current: &Option<String>) -> Result<(), String> {
    if current != &target.replacement_sha256 {
        return Err(format!("Recovery conflict at {}: expected {}, found {}. The current path was left untouched.", target.relative_path.display(), target.replacement_sha256.as_deref().unwrap_or("a missing file"), current.as_deref().unwrap_or("a missing file")));
    }
    Ok(())
}

fn cleanup_transaction(workspace: &Path, finished: bool) -> Result<(), String> {
    // Validate the full private tree before deleting anything, and leave completion
    // records intact until all backups and payload files have been removed.
    collect_files(workspace, &mut Vec::new())?;
    for entry in fs::read_dir(workspace).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name();
        if [
            MANIFEST_PATH,
            FINISHED_MANIFEST_PATH,
            "committed",
            "restored",
        ]
        .iter()
        .any(|reserved| name == std::ffi::OsStr::new(reserved))
        {
            continue;
        }
        let result = if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            fs::remove_dir_all(entry.path())
        } else {
            fs::remove_file(entry.path())
        };
        result.map_err(|error| format!("Could not clean {}: {error}", entry.path().display()))?;
    }
    if !finished {
        // If cleanup itself is interrupted, this durable name prevents a missing
        // completion marker from making the next run roll back a committed update.
        durable_rename(
            &workspace.join(MANIFEST_PATH),
            &workspace.join(FINISHED_MANIFEST_PATH),
            false,
        )?;
    }
    for name in ["committed", "restored"] {
        match fs::remove_file(workspace.join(name)) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    fs::remove_file(workspace.join(FINISHED_MANIFEST_PATH)).map_err(|error| error.to_string())?;
    fs::remove_dir(workspace).map_err(|error| error.to_string())
}

fn vortex_deployed_files(root: &Path) -> Result<HashSet<String>, String> {
    validate_target(root, Path::new("vortex.deployment.json"))?;
    let text = match fs::read_to_string(root.join("vortex.deployment.json")) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(HashSet::new()),
        Err(error) => {
            return Err(format!(
                "Could not read Vortex deployment metadata: {error}"
            ))
        }
    };
    let deployment: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| format!("Could not read Vortex deployment metadata: {error}"))?;
    Ok(deployment
        .get("files")
        .and_then(|files| files.as_array())
        .into_iter()
        .flatten()
        .filter_map(|file| file.get("relPath").and_then(|path| path.as_str()))
        .map(|path| path_key(Path::new(path.trim_start_matches(['/', '\\']))))
        .collect())
}
