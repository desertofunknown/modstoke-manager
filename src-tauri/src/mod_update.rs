use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviousInstall {
    assembly_path: Option<String>,
    enabled_assembly_path: Option<String>,
    package_path: Option<String>,
    enabled_package_path: Option<String>,
    is_enabled: bool,
}

struct Replacement {
    source: PathBuf,
    relative_path: PathBuf,
    existed: bool,
}

#[tauri::command]
pub async fn update_native_mod(
    source: String,
    destination: String,
    previous_install: PreviousInstall,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        replace_mod_files(source, destination, previous_install)
    })
    .await
    .map_err(|error| format!("The mod update could not finish: {error}"))?
}

fn replace_mod_files(
    source: String,
    destination: String,
    previous_install: PreviousInstall,
) -> Result<(), String> {
    let selected_root = Path::new(&destination);
    let root = fs::canonicalize(selected_root).map_err(|error| error.to_string())?;
    let assembly = installed_path(selected_root, previous_install.assembly_path)?;
    let enabled_assembly = installed_path(selected_root, previous_install.enabled_assembly_path)?;
    let package = installed_path(selected_root, previous_install.package_path)?;
    let enabled_package = installed_path(selected_root, previous_install.enabled_package_path)?;
    if !previous_install.is_enabled && assembly.is_none() {
        return Err("Cannot preserve this mod's disabled state without its assembly path. Refresh the installed mods before updating it.".into());
    }

    let workspace = tempfile::Builder::new()
        .prefix(".openacai-update-")
        .tempdir_in(&root)
        .map_err(|error| format!("Could not create an update staging folder: {error}"))?;
    let staging = workspace.path().join("staging");
    let backup = workspace.path().join("backup");
    fs::create_dir(&staging).map_err(|error| error.to_string())?;
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

    let vortex_files = vortex_deployed_files(&root)?;
    let mut targets = HashSet::new();
    let mut replacements = Vec::new();
    for source in staged_files {
        let original = source
            .strip_prefix(&staging)
            .map_err(|error| error.to_string())?;
        let mut relative_path = original.to_owned();
        if !previous_install.is_enabled {
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

        let key = path_key(&relative_path);
        if !targets.insert(key.clone()) {
            return Err(format!(
                "The archive maps multiple files to {}.",
                relative_path.display()
            ));
        }
        if key == "vortex.deployment.json" || vortex_files.contains(&key) {
            return Err(format!(
                "{} is managed by Vortex. Update it through Vortex before retrying this mod.",
                relative_path.display()
            ));
        }
        if root.join(&relative_path).starts_with(workspace.path()) {
            return Err("The archive contains an update staging path.".into());
        }
        let existed = validate_target(&root, &relative_path)?;
        replacements.push(Replacement {
            source,
            relative_path,
            existed,
        });
    }

    if let Some(assembly) = &assembly {
        if !targets.contains(&path_key(assembly)) {
            return Err(format!("The replacement archive does not contain {}. The installed mod was not changed; review the package layout before updating it.", assembly.display()));
        }
    }

    // Finish every backup before replacing any installed file.
    for replacement in &replacements {
        if replacement.existed {
            let saved = backup.join(&replacement.relative_path);
            fs::create_dir_all(saved.parent().ok_or("Invalid backup path")?)
                .map_err(|error| error.to_string())?;
            fs::copy(root.join(&replacement.relative_path), &saved).map_err(|error| {
                format!(
                    "Could not back up {}. The installed mod was not changed. {error}",
                    replacement.relative_path.display()
                )
            })?;
        }
    }

    let mut written = Vec::new();
    let mut created_dirs = Vec::new();
    for replacement in &replacements {
        let target = root.join(&replacement.relative_path);
        let result = (|| {
            if validate_target(&root, &replacement.relative_path)? != replacement.existed {
                return Err(format!("{} changed while the update was being prepared. Refresh the installed mods and retry.", replacement.relative_path.display()));
            }
            create_parent_dirs(&root, &replacement.relative_path, &mut created_dirs)?;
            fs::rename(&replacement.source, &target).map_err(|error| {
                format!(
                    "Could not replace {}: {error}",
                    replacement.relative_path.display()
                )
            })
        })();
        if let Err(error) = result {
            let failures = restore_files(&root, &backup, &written, &created_dirs);
            if failures.is_empty() {
                return Err(format!(
                    "Update failed. Previous files were restored. {error}"
                ));
            }
            let recovery_path = workspace.into_path();
            return Err(format!(
                "Update failed: {error}\nSome files could not be restored: {}\nBackups are retained at {}. Close the game and restore those files before retrying.",
                failures.join("; "), recovery_path.join("backup").display()
            ));
        }
        written.push(replacement);
    }

    Ok(())
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
            !matches!(part, Component::Normal(_))
                || part.as_os_str().to_string_lossy().contains(':')
        })
    {
        return Err(format!("Invalid update path: {}", path.display()));
    }
    Ok(())
}

fn validate_target(root: &Path, relative: &Path) -> Result<bool, String> {
    validate_relative_path(relative)?;
    let mut target = root.to_owned();
    let mut parts = relative.components().peekable();
    let mut file_exists = false;
    while let Some(part) = parts.next() {
        target.push(part);
        match fs::symlink_metadata(&target) {
            Ok(metadata) => {
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
                        target.display()
                    ));
                }
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
    for entry in fs::read_dir(folder).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            collect_files(&entry.path(), files)?;
        } else {
            files.push(entry.path());
        }
    }
    Ok(())
}

fn create_parent_dirs(
    root: &Path,
    relative: &Path,
    created: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let mut parent = root.to_owned();
    if let Some(relative_parent) = relative.parent() {
        for part in relative_parent.components() {
            parent.push(part);
            if !parent.exists() {
                fs::create_dir(&parent).map_err(|error| error.to_string())?;
                created.push(parent.clone());
            }
        }
    }
    Ok(())
}

fn restore_files(
    root: &Path,
    backup: &Path,
    written: &[&Replacement],
    created_dirs: &[PathBuf],
) -> Vec<String> {
    let mut failures = Vec::new();
    for replacement in written.iter().rev() {
        let target = root.join(&replacement.relative_path);
        if let Err(error) = validate_target(root, &replacement.relative_path) {
            failures.push(error);
            continue;
        }
        let result = if replacement.existed {
            fs::rename(backup.join(&replacement.relative_path), &target)
        } else {
            fs::remove_file(&target)
        };
        if let Err(error) = result {
            if replacement.existed || error.kind() != ErrorKind::NotFound {
                failures.push(format!("{}: {error}", replacement.relative_path.display()));
            }
        }
    }
    for folder in created_dirs.iter().rev() {
        // Only remove empty folders created by this update.
        let _ = fs::remove_dir(folder);
    }
    failures
}

fn vortex_deployed_files(root: &Path) -> Result<HashSet<String>, String> {
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
