use crate::hashing::sha256_reader;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Component, Path, PathBuf};

#[cfg(windows)]
pub(crate) const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
#[cfg(windows)]
pub(crate) const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

pub(crate) fn validate_relative_path(path: &Path) -> Result<(), String> {
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

pub(crate) fn is_linked(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

pub(crate) fn reject_linked_path(path: &Path, metadata: &fs::Metadata) -> Result<(), String> {
    if is_linked(metadata) {
        return Err(format!(
            "Refusing to update through a linked path: {}",
            path.display()
        ));
    }
    Ok(())
}

pub(crate) fn exclusive_lock_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // An open handle owns the lock; a process exit releases it without
        // leaving a stale lock that would need to be guessed at or deleted.
        options.share_mode(0).custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options
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

pub(crate) fn collect_files(folder: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
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

pub(crate) fn create_parent_dirs(root: &Path, relative: &Path) -> Result<(), String> {
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

pub(crate) fn read_bounded(root: &Path, relative: &Path, limit: usize) -> Result<Option<Vec<u8>>, String> {
    if !validate_target(root, relative)? {
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

pub(crate) fn hash_file(path: &Path) -> Result<String, String> {
    validate_directory(path.parent().ok_or("Invalid file path")?)?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Could not inspect {}: {error}", path.display()))?;
    reject_linked_path(path, &metadata)?;
    if !metadata.is_file() {
        return Err(format!("Expected an ordinary file at {}", path.display()));
    }
    let file =
        File::open(path).map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    sha256_reader(file).map_err(|error| format!("Could not read {}: {error}", path.display()))
}

pub(crate) fn target_hash(root: &Path, relative: &Path) -> Result<Option<String>, String> {
    if validate_target(root, relative)? {
        hash_file(&root.join(relative)).map(Some)
    } else {
        Ok(None)
    }
}

pub(crate) fn durable_rename(source: &Path, destination: &Path, replace: bool) -> Result<(), String> {
    move_durably(source, destination, replace).map_err(|error| {
        format!(
            "Could not move {} to {}: {error}",
            source.display(),
            destination.display()
        )
    })
}

pub(crate) fn move_durably(source: &Path, destination: &Path, replace: bool) -> std::io::Result<()> {
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
    result
}

pub(crate) fn write_durable(folder: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
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
