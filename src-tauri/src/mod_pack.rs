use crate::mod_update::{self, UpdateTarget};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

const WORKSPACE_PREFIX: &str = ".modstoke-pack-";
const WORKSPACE_SUFFIX_LENGTH: usize = 12;
const MAX_PACKAGES: usize = 128;
const MAX_TARGETS: usize = 8192;
const MAX_RECEIPT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RetainedPackage {
    pub(crate) targets: Vec<RetainedTarget>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RetainedTarget {
    pub(crate) relative_path: PathBuf,
    pub(crate) original_sha256: Option<String>,
    pub(crate) replacement_sha256: Option<String>,
    pub(crate) backup_path: Option<PathBuf>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SlotReceipt {
    version: u32,
    root_sha256: String,
    workspace_name: String,
    slot: usize,
    package: RetainedPackage,
}

struct StoredReceipt {
    receipt: SlotReceipt,
    sha256: String,
}

// All operations run under the canonical game root's native update lock. The
// caller publishes receipts durably before allowing their package's live writes.
pub(crate) fn create_workspace(root: &Path) -> Result<String, String> {
    let root = canonical_root(root)?;
    let workspace = tempfile::Builder::new()
        .prefix(WORKSPACE_PREFIX)
        .rand_bytes(WORKSPACE_SUFFIX_LENGTH)
        .tempdir_in(&root)
        .map_err(|error| format!("Could not create a retained pack folder: {error}"))?;
    let name = workspace
        .path()
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or("Invalid retained pack folder name")?
        .to_owned();
    validate_workspace_name(&name)?;
    mod_update::validate_directory(workspace.path())?;
    // This workspace outlives both the native update transaction and the process.
    let _ = workspace.into_path();
    Ok(name)
}

pub(crate) fn retain_package(
    root: &Path,
    workspace_name: &str,
    slot: usize,
    targets: &[UpdateTarget],
) -> Result<RetainedPackage, String> {
    if targets.len() > MAX_TARGETS {
        return Err("A retained package contains too many files.".into());
    }
    let package = RetainedPackage {
        targets: targets
            .iter()
            .enumerate()
            .map(|(index, target)| RetainedTarget {
                relative_path: target.relative_path.clone(),
                original_sha256: target.original_sha256.clone(),
                replacement_sha256: target.replacement_sha256.clone(),
                backup_path: target.original_sha256.as_ref().map(|_| backup_path(slot, index)),
            })
            .collect(),
    };
    validate_receipt(workspace_name, slot, &package)?;
    if targets.iter().any(|target| target.originally_existed != target.original_sha256.is_some()) {
        return Err("The retained package has an invalid original-file state.".into());
    }
    let root = canonical_root(root)?;
    let workspace = existing_workspace(&root, workspace_name)?;
    // Record ownership before any original is copied. If profile publication
    // fails, discard can still identify every completed or partial slot.
    retain_slot_receipt(&root, &workspace, workspace_name, slot, &package)?;
    ensure_directory(&workspace.join("backup"))?;
    ensure_directory(&workspace.join("backup").join(slot.to_string()))?;
    for (index, target) in package.targets.iter().enumerate() {
        mod_update::ensure_unmanaged(&root, &target.relative_path)?;
        if target_hash(&root, &target.relative_path)? != target.original_sha256 {
            return Err(format!("{} changed before its pack backup could be retained.", target.relative_path.display()));
        }
        let relative_backup = backup_path(slot, index);
        let saved_hash = target_hash(&workspace, &relative_backup)?;
        if let Some(expected) = &target.original_sha256 {
            if let Some(saved) = saved_hash {
                if saved != *expected {
                    return Err(format!("The retained original for {} has changed.", target.relative_path.display()));
                }
            } else {
                mod_update::copy_durable(
                    &root.join(&target.relative_path),
                    &workspace.join(&relative_backup),
                )?;
            }
            if target_hash(&workspace, &relative_backup)?.as_ref() != Some(expected) {
                return Err(format!("The retained original for {} does not match its recorded hash.", target.relative_path.display()));
            }
        } else if saved_hash.is_some() {
            return Err(format!("An unexpected original backup exists at {}.", workspace.join(relative_backup).display()));
        }
    }
    // Recheck all originals and immutable backups before the caller can publish
    // the receipt, including files copied earlier in this package.
    for target in &package.targets {
        mod_update::ensure_unmanaged(&root, &target.relative_path)?;
        if target_hash(&root, &target.relative_path)? != target.original_sha256 {
            return Err(format!("{} changed while its pack backup was being prepared.", target.relative_path.display()));
        }
        validate_backup(&workspace, target, false)?;
    }
    let mut remaining = MAX_RECEIPT_BYTES;
    require_slot_receipt(&root, &workspace, workspace_name, slot, &package, &mut remaining)?;
    Ok(package)
}

pub(crate) fn validate_receipt(
    workspace_name: &str,
    slot: usize,
    package: &RetainedPackage,
) -> Result<(), String> {
    validate_workspace_name(workspace_name)?;
    if slot >= MAX_PACKAGES || package.targets.len() > MAX_TARGETS {
        return Err("The retained pack receipt exceeds its package or file limit.".into());
    }
    check_receipt_size(package)?;
    let mut paths = HashSet::new();
    for (index, target) in package.targets.iter().enumerate() {
        mod_update::validate_mod_path(&target.relative_path)?;
        let key = path_key(&target.relative_path);
        let expected_backup = target.original_sha256.as_ref().map(|_| backup_path(slot, index));
        if key.split('/').any(|part| part.is_empty() || matches!(part, "." | ".."))
            || !paths.insert(key)
            || target.original_sha256.is_none() && target.replacement_sha256.is_none()
            || target.original_sha256.as_deref().is_some_and(|hash| !valid_sha256(hash))
            || target.replacement_sha256.as_deref().is_some_and(|hash| !valid_sha256(hash))
            || target.backup_path.as_deref().map(Path::as_os_str)
                != expected_backup.as_deref().map(Path::as_os_str)
        {
            return Err(format!("Invalid retained pack target: {}", target.relative_path.display()));
        }
    }
    Ok(())
}

pub(crate) fn preflight_restore(
    root: &Path,
    workspace_name: &str,
    packages: &[(usize, &RetainedPackage)],
) -> Result<(), String> {
    validate_packages(workspace_name, packages)?;
    let root = canonical_root(root)?;
    let workspace = existing_workspace(&root, workspace_name)?;
    let mut remaining = MAX_RECEIPT_BYTES;
    for (slot, package) in packages {
        require_slot_receipt(&root, &workspace, workspace_name, *slot, package, &mut remaining)?;
    }
    let mut current_files = HashMap::<String, Option<String>>::new();
    // Receipts arrive in application order. Simulating reversal allows a later
    // package's original to be an earlier package's replacement at the same path.
    for (slot, package) in packages.iter().rev() {
        for (index, target) in package.targets.iter().enumerate().rev() {
            mod_update::ensure_unmanaged(&root, &target.relative_path)?;
            validate_backup(&workspace, target, false)?;
            validate_removed(&workspace, *slot, index, target)?;
            let key = path_key(&target.relative_path);
            let current = match current_files.get(&key) {
                Some(hash) => hash.clone(),
                None => target_hash(&root, &target.relative_path)?,
            };
            require_restorable(target, &current)?;
            current_files.insert(key, target.original_sha256.clone());
        }
    }
    Ok(())
}

pub(crate) fn restore_package(
    root: &Path,
    workspace_name: &str,
    slot: usize,
    package: &RetainedPackage,
) -> Result<(), String> {
    preflight_restore(root, workspace_name, &[(slot, package)])?;
    let root = canonical_root(root)?;
    let workspace = existing_workspace(&root, workspace_name)?;
    for (index, target) in package.targets.iter().enumerate().rev() {
        mod_update::ensure_unmanaged(&root, &target.relative_path)?;
        validate_backup(&workspace, target, false)?;
        validate_removed(&workspace, slot, index, target)?;
        require_restorable(target, &target_hash(&root, &target.relative_path)?)?;
        let update_target = UpdateTarget {
            relative_path: target.relative_path.clone(),
            originally_existed: target.original_sha256.is_some(),
            backup_path: target.backup_path.clone(),
            original_sha256: target.original_sha256.clone(),
            replacement_sha256: target.replacement_sha256.clone(),
        };
        mod_update::restore_pack_target(&root, &workspace, slot, index, &update_target)?;
        if target_hash(&root, &target.relative_path)? != target.original_sha256 {
            return Err(format!("{} changed while its original contents were being restored.", target.relative_path.display()));
        }
    }
    Ok(())
}

pub(crate) fn discard_workspace(
    root: &Path,
    workspace_name: &str,
    packages: &[(usize, &RetainedPackage)],
) -> Result<(), String> {
    validate_packages(workspace_name, packages)?;
    let root = canonical_root(root)?;
    let workspace = root.join(workspace_name);
    if !directory_exists(&workspace)? {
        return Ok(());
    }
    let mut remaining = MAX_RECEIPT_BYTES;
    let mut retained = Vec::new();
    for slot in 0..MAX_PACKAGES {
        let published = packages.iter().find(|(published_slot, _)| *published_slot == slot).map(|(_, package)| *package);
        match read_slot_receipt(&root, &workspace, workspace_name, slot, &mut remaining)? {
            Some(stored) => {
                if published.is_some_and(|package| package != &stored.receipt.package) {
                    return Err(format!("The retained pack receipt for slot {slot} does not match the profile."));
                }
                retained.push((slot, stored.receipt.package, Some(stored.sha256)));
            }
            None => {
                if let Some(package) = published {
                    // A previous discard may already have removed this slot's
                    // metadata. Its profile receipt still identifies known paths.
                    retained.push((slot, package.clone(), None));
                }
            }
        }
    }
    // Check every known file before deleting any. Unknown entries are never
    // enumerated or removed; removing only empty directories preserves them.
    directory_exists(&workspace.join("backup"))?;
    for slot in 0..MAX_PACKAGES {
        directory_exists(&workspace.join("backup").join(slot.to_string()))?;
    }
    for (slot, package, _) in &retained {
        for (index, target) in package.targets.iter().enumerate() {
            validate_backup(&workspace, target, true)?;
            validate_removed(&workspace, *slot, index, target)?;
        }
    }
    for (slot, package, receipt_sha256) in &retained {
        for (index, target) in package.targets.iter().enumerate() {
            if let (Some(backup), Some(expected)) = (&target.backup_path, &target.original_sha256) {
                remove_known_file(&workspace, backup, expected)?;
            } else if let Some(expected) = &target.replacement_sha256 {
                remove_known_file(&workspace, &removed_path(*slot, index), expected)?;
            }
        }
        remove_empty_directory(&workspace.join("backup").join(slot.to_string()))?;
        // Remove ownership last so an interrupted cleanup retains enough
        // information even when the profile never received this slot.
        if let Some(expected) = receipt_sha256 {
            remove_known_file(&workspace, &receipt_path(*slot), expected)?;
        }
    }
    for slot in 0..MAX_PACKAGES {
        remove_empty_directory(&workspace.join("backup").join(slot.to_string()))?;
    }
    remove_empty_directory(&workspace.join("backup"))?;
    remove_empty_directory(&workspace)
}

fn validate_packages(workspace_name: &str, packages: &[(usize, &RetainedPackage)]) -> Result<(), String> {
    validate_workspace_name(workspace_name)?;
    if packages.len() > MAX_PACKAGES {
        return Err("The retained pack contains too many packages.".into());
    }
    let mut slots = HashSet::new();
    for (slot, package) in packages {
        if !slots.insert(*slot) {
            return Err("The retained pack contains a repeated package slot.".into());
        }
        validate_receipt(workspace_name, *slot, package)?;
    }
    check_receipt_size(packages)
}

fn retain_slot_receipt(
    root: &Path,
    workspace: &Path,
    workspace_name: &str,
    slot: usize,
    package: &RetainedPackage,
) -> Result<(), String> {
    let mut remaining = MAX_RECEIPT_BYTES;
    let mut already_recorded = false;
    for existing_slot in 0..MAX_PACKAGES {
        if let Some(stored) = read_slot_receipt(root, workspace, workspace_name, existing_slot, &mut remaining)? {
            if existing_slot == slot {
                if stored.receipt.package != *package {
                    return Err(format!("The retained pack receipt for slot {slot} differs from this package. Its original backups were left untouched."));
                }
                already_recorded = true;
            }
        }
    }
    if already_recorded {
        return Ok(());
    }
    let receipt = SlotReceipt {
        version: 1,
        root_sha256: root_identity(root),
        workspace_name: workspace_name.to_owned(),
        slot,
        package: package.clone(),
    };
    check_receipt_size(&receipt)?;
    let bytes = serde_json::to_vec(&receipt).map_err(|error| error.to_string())?;
    if bytes.len() > remaining {
        return Err("The retained pack receipts exceed their combined size limit.".into());
    }
    let relative = receipt_path(slot);
    mod_update::validate_target(workspace, &relative)?;
    let mut temporary = tempfile::Builder::new()
        .prefix("pending-")
        .tempfile_in(workspace)
        .map_err(|error| error.to_string())?;
    temporary.write_all(&bytes)
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|error| format!("Could not retain the pack receipt: {error}"))?;
    let temporary = temporary.into_temp_path();
    mod_update::durable_rename(&temporary, &workspace.join(relative), false)?;
    let mut remaining = MAX_RECEIPT_BYTES;
    require_slot_receipt(root, workspace, workspace_name, slot, package, &mut remaining)
}

fn require_slot_receipt(
    root: &Path,
    workspace: &Path,
    workspace_name: &str,
    slot: usize,
    package: &RetainedPackage,
    remaining: &mut usize,
) -> Result<(), String> {
    let stored = read_slot_receipt(root, workspace, workspace_name, slot, remaining)?
        .ok_or_else(|| format!("The retained pack receipt for slot {slot} is missing."))?;
    if stored.receipt.package != *package {
        return Err(format!("The retained pack receipt for slot {slot} does not match the profile."));
    }
    Ok(())
}

fn read_slot_receipt(
    root: &Path,
    workspace: &Path,
    workspace_name: &str,
    slot: usize,
    remaining: &mut usize,
) -> Result<Option<StoredReceipt>, String> {
    let relative = receipt_path(slot);
    if !mod_update::validate_target(workspace, &relative)? {
        return Ok(None);
    }
    let path = workspace.join(relative);
    let file = open_single_file(&path)?;
    if file.metadata().map_err(|error| error.to_string())?.len() > *remaining as u64 {
        return Err("The retained pack receipts exceed their combined size limit.".into());
    }
    let mut bytes = Vec::new();
    file.take(*remaining as u64 + 1).read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    *remaining = remaining.checked_sub(bytes.len())
        .ok_or("The retained pack receipts exceed their combined size limit.")?;
    let receipt: SlotReceipt = serde_json::from_slice(&bytes)
        .map_err(|error| format!("The retained pack receipt at {} is invalid: {error}", path.display()))?;
    if receipt.version != 1
        || receipt.slot != slot
        || receipt.workspace_name != workspace_name
        || receipt.root_sha256 != root_identity(root)
    {
        return Err(format!("The retained pack receipt at {} has an invalid identity.", path.display()));
    }
    validate_receipt(workspace_name, slot, &receipt.package)?;
    Ok(Some(StoredReceipt {
        receipt,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
    }))
}

fn receipt_path(slot: usize) -> PathBuf {
    PathBuf::from(format!("receipt-{slot}.json"))
}

fn root_identity(root: &Path) -> String {
    format!("{:x}", Sha256::digest(path_key(root).as_bytes()))
}

fn validate_workspace_name(name: &str) -> Result<(), String> {
    if !name.strip_prefix(WORKSPACE_PREFIX).is_some_and(|suffix| {
        suffix.len() == WORKSPACE_SUFFIX_LENGTH && suffix.bytes().all(|byte| byte.is_ascii_alphanumeric())
    }) {
        return Err("Invalid retained pack folder name.".into());
    }
    Ok(())
}

fn canonical_root(root: &Path) -> Result<PathBuf, String> {
    mod_update::validate_directory(root)?;
    fs::canonicalize(root).map_err(|error| format!("Could not resolve the game folder: {error}"))
}

fn existing_workspace(root: &Path, name: &str) -> Result<PathBuf, String> {
    validate_workspace_name(name)?;
    let workspace = root.join(name);
    mod_update::validate_directory(&workspace)?;
    Ok(workspace)
}

fn directory_exists(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            mod_update::validate_directory(path)?;
            Ok(true)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            // Even missing children must not conceal a linked ancestor.
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    directory_exists(parent)?;
                }
            }
            Ok(false)
        }
        Err(error) => Err(format!("Could not inspect {}: {error}", path.display())),
    }
}

fn ensure_directory(path: &Path) -> Result<(), String> {
    if !directory_exists(path)? {
        fs::create_dir(path).map_err(|error| format!("Could not create {}: {error}", path.display()))?;
    }
    mod_update::validate_directory(path)
}

fn backup_path(slot: usize, index: usize) -> PathBuf {
    PathBuf::from("backup").join(slot.to_string()).join(index.to_string())
}

fn removed_path(slot: usize, index: usize) -> PathBuf {
    PathBuf::from(format!("removed-{slot}-{index}"))
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_lowercase()
}

fn valid_sha256(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn target_hash(root: &Path, relative: &Path) -> Result<Option<String>, String> {
    if !mod_update::validate_target(root, relative)? {
        return Ok(None);
    }
    let path = root.join(relative);
    let mut file = open_single_file(&path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let length = file.read(&mut buffer).map_err(|error| format!("Could not read {}: {error}", path.display()))?;
        if length == 0 {
            break;
        }
        hash.update(&buffer[..length]);
    }
    Ok(Some(format!("{:x}", hash.finalize())))
}

fn open_single_file(path: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1).custom_flags(0x00200000); // Read sharing; open the reparse point itself.
    }
    let file = options.open(path).map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    require_single_file(path, &file)?;
    Ok(file)
}

fn require_single_file(path: &Path, file: &File) -> Result<(), String> {
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() {
        return Err(format!("Expected an ordinary file at {}", path.display()));
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::Storage::FileSystem::{GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION};
        let mut information = BY_HANDLE_FILE_INFORMATION::default();
        if !unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle() as isize), &mut information) }.as_bool() {
            return Err(format!("Could not inspect {}: {}", path.display(), io::Error::last_os_error()));
        }
        if information.dwFileAttributes & 0x400 != 0 || information.nNumberOfLinks != 1 {
            return Err(format!("Refusing a linked retained pack file: {}", path.display()));
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(format!("Refusing a linked retained pack file: {}", path.display()));
        }
    }
    Ok(())
}

fn validate_backup(workspace: &Path, target: &RetainedTarget, allow_missing: bool) -> Result<(), String> {
    if let Some(backup) = &target.backup_path {
        let hash = target_hash(workspace, backup)?;
        if hash != target.original_sha256 && !(allow_missing && hash.is_none()) {
            return Err(format!("The original backup for {} is missing or changed at {}.", target.relative_path.display(), workspace.join(backup).display()));
        }
    }
    Ok(())
}

fn validate_removed(workspace: &Path, slot: usize, index: usize, target: &RetainedTarget) -> Result<(), String> {
    if target.original_sha256.is_none() {
        let removed = removed_path(slot, index);
        if let Some(hash) = target_hash(workspace, &removed)? {
            if Some(hash) != target.replacement_sha256 {
                return Err(format!("The removed pack file at {} has changed.", workspace.join(removed).display()));
            }
        }
    }
    Ok(())
}

fn require_restorable(target: &RetainedTarget, current: &Option<String>) -> Result<(), String> {
    if current != &target.original_sha256 && current != &target.replacement_sha256 {
        return Err(format!("Pack restore conflict at {}: its current contents match neither the original nor the installed file. The current path was left untouched.", target.relative_path.display()));
    }
    Ok(())
}

fn remove_known_file(workspace: &Path, relative: &Path, expected: &str) -> Result<(), String> {
    if let Some(hash) = target_hash(workspace, relative)? {
        if hash != expected {
            return Err(format!("The retained file at {} changed and was not removed.", workspace.join(relative).display()));
        }
        fs::remove_file(workspace.join(relative)).map_err(|error| format!("Could not remove {}: {error}", workspace.join(relative).display()))?;
    }
    Ok(())
}

fn remove_empty_directory(path: &Path) -> Result<(), String> {
    if directory_exists(path)? {
        fs::remove_dir(path).map_err(|error| format!("Could not remove the retained pack folder {}. Any unexpected files were preserved: {error}", path.display()))?;
    }
    Ok(())
}

fn check_receipt_size<T: Serialize + ?Sized>(value: &T) -> Result<(), String> {
    struct BoundedWriter(usize);
    impl Write for BoundedWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0 = self.0.checked_sub(bytes.len()).ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "retained pack receipt limit exceeded"))?;
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(BoundedWriter(MAX_RECEIPT_BYTES), value)
        .map_err(|error| format!("The retained pack receipts exceed their size limit or cannot be serialized: {error}"))
}
