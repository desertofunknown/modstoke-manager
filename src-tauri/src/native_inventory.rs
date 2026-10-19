use crate::mod_update::{validate_directory, validate_target};
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

const MAX_DIRECTORIES: usize = 4096;
const MAX_ENTRIES: usize = 16384;
const MAX_MANIFESTS: usize = 1024;
const MAX_MANIFEST_BYTES: usize = 512 * 1024;
const MAX_TOTAL_BYTES: usize = 32 * 1024 * 1024;
const MANIFEST_DEPTH: usize = 4;
const ASSEMBLY_DEPTH: usize = 16;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeInventory {
    pub(crate) root: String,
    pub(crate) packages: Vec<NativePackage>,
    pub(crate) notices: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativePackage {
    pub(crate) manifest_path: String,
    pub(crate) package_path: String,
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) version: Option<String>,
    pub(crate) author: Option<String>,
    pub(crate) package_type: String,
    pub(crate) entry_type: Option<String>,
    pub(crate) entry_method: Option<String>,
    pub(crate) assembly_path: Option<String>,
    pub(crate) assembly_candidates: Vec<String>,
    pub(crate) state: String,
    pub(crate) details: Vec<String>,
}

#[tauri::command]
pub async fn read_native_mod_inventory(game_root: String) -> Result<NativeInventory, String> {
    tauri::async_runtime::spawn_blocking(move || read_inventory(&game_root))
        .await
        .map_err(|error| format!("Native package inspection could not finish: {error}"))?
}

pub(crate) fn read_inventory(game_root: &str) -> Result<NativeInventory, String> {
    let selected = Path::new(game_root);
    validate_directory(selected)?;
    let root = fs::canonicalize(selected)
        .map_err(|error| format!("Could not resolve the game folder: {error}"))?;
    validate_directory(&root)?;
    if !validate_target(&root, Path::new("SonsOfTheForest.exe"))?
        && !validate_target(&root, Path::new("SonsOfTheForestDS.exe"))?
    {
        return Err("Select a Sons of the Forest game or dedicated server folder containing its executable.".into());
    }

    let mut scan = Scan::default();
    let mods = root.join("OpenACAI").join("Mods");
    // Validate the existing ancestry even when Mods itself has not been created.
    let mut existing = mods.as_path();
    loop {
        match fs::symlink_metadata(existing) {
            Ok(_) => {
                validate_directory(existing)?;
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                existing = existing.parent().ok_or("Invalid native mod folder")?;
            }
            Err(error) => {
                return Err(format!(
                    "Could not inspect {}: {error}",
                    display_path(existing)
                ))
            }
        }
    }
    if existing != mods {
        scan.notices
            .push("The OpenACAI/Mods folder does not exist.".into());
        return Ok(NativeInventory {
            root: display_path(&root),
            packages: Vec::new(),
            notices: scan.notices,
        });
    }

    let manifests = scan.manifests(&mods);
    let mut packages: Vec<_> = manifests.iter().map(|path| scan.package(path)).collect();
    mark_duplicates(&mut packages);
    packages.sort_by(|left, right| left.manifest_path.cmp(&right.manifest_path));
    Ok(NativeInventory {
        root: display_path(&root),
        packages,
        notices: scan.notices,
    })
}

#[derive(Clone, Default)]
struct Directory {
    folders: Vec<PathBuf>,
    files: Vec<PathBuf>,
    complete: bool,
}

#[derive(Default)]
struct Scan {
    directories: HashMap<PathBuf, Directory>,
    examined_entries: usize,
    manifest_bytes: usize,
    notices: Vec<String>,
}

impl Scan {
    fn notice(&mut self, message: String) {
        if !self.notices.contains(&message) {
            self.notices.push(message);
        }
    }

    fn directory(&mut self, path: &Path) -> Directory {
        if let Some(cached) = self.directories.get(path) {
            return cached.clone();
        }
        if self.directories.len() >= MAX_DIRECTORIES || self.examined_entries >= MAX_ENTRIES {
            self.notice("The shared directory or entry inspection limit was reached; this inventory is incomplete.".into());
            return Directory::default();
        }
        let mut result = Directory {
            complete: true,
            ..Directory::default()
        };
        let read = validate_directory(path).and_then(|()| {
            fs::read_dir(path)
                .map_err(|error| format!("Could not read {}: {error}", display_path(path)))
        });
        match read {
            Err(error) => {
                result.complete = false;
                self.notice(error);
            }
            Ok(entries) => {
                for entry in entries {
                    if self.examined_entries >= MAX_ENTRIES {
                        result.complete = false;
                        self.notice("The shared entry inspection limit was reached; this inventory is incomplete.".into());
                        break;
                    }
                    self.examined_entries += 1;
                    match entry.and_then(|entry| Ok((entry.path(), entry.file_type()?))) {
                        Ok((entry, kind)) if kind.is_symlink() => {
                            result.complete = false;
                            self.notice(format!(
                                "Linked filesystem entry was not followed: {}",
                                display_path(&entry)
                            ));
                            // Retain a linked manifest name so its rejected read remains visible.
                            result.files.push(entry);
                        }
                        Ok((entry, kind)) if kind.is_dir() => result.folders.push(entry),
                        Ok((entry, kind)) if kind.is_file() => result.files.push(entry),
                        Ok((entry, _)) => {
                            result.complete = false;
                            self.notice(format!(
                                "Unsupported filesystem entry was skipped: {}",
                                display_path(&entry)
                            ));
                        }
                        Err(error) => {
                            result.complete = false;
                            self.notice(format!(
                                "Could not inspect an entry in {}: {error}",
                                display_path(path)
                            ));
                        }
                    }
                }
            }
        }
        result.folders.sort();
        result.files.sort();
        self.directories.insert(path.to_owned(), result.clone());
        result
    }

    fn manifests(&mut self, root: &Path) -> Vec<PathBuf> {
        let mut queue = VecDeque::from([(root.to_owned(), 0)]);
        let mut manifests = Vec::new();
        while let Some((folder, depth)) = queue.pop_front() {
            let directory = self.directory(&folder);
            for file in directory.files {
                let name = file.file_name().unwrap_or_default().to_string_lossy();
                if ["openacai.mod.json", "openacai.json", "manifest.json"]
                    .iter()
                    .any(|known| name.eq_ignore_ascii_case(known))
                {
                    if manifests.len() == MAX_MANIFESTS {
                        self.notice("The 1024-manifest inspection limit was reached; additional manifests were omitted.".into());
                        return manifests;
                    }
                    manifests.push(file);
                }
            }
            for child in directory
                .folders
                .into_iter()
                .filter(|path| !ignored_folder(path))
            {
                if depth < MANIFEST_DEPTH {
                    queue.push_back((child, depth + 1));
                } else {
                    self.notice("Manifest discovery stops at four nested folders, matching the native loader; deeper folders were not inspected for manifests.".into());
                }
            }
        }
        manifests
    }

    fn read_manifest(&mut self, path: &Path) -> Result<Manifest, String> {
        let folder = path.parent().ok_or("Manifest has no package folder")?;
        let name = Path::new(path.file_name().ok_or("Manifest has no file name")?);
        if !validate_target(folder, name)? {
            return Err("The manifest disappeared during inspection.".into());
        }
        let file = File::open(path).map_err(|error| format!("Could not read manifest: {error}"))?;
        let length = file.metadata().map_err(|error| error.to_string())?.len();
        if length > MAX_MANIFEST_BYTES as u64 {
            return Err("Manifest exceeds the 512 KiB inspection limit.".into());
        }
        let remaining = MAX_TOTAL_BYTES.saturating_sub(self.manifest_bytes);
        if length > remaining as u64 || remaining == 0 {
            self.notice("The shared 32 MiB manifest inspection limit was reached; some manifest metadata is incomplete.".into());
            return Err("The shared manifest byte limit prevents reading this manifest.".into());
        }
        let limit = (MAX_MANIFEST_BYTES + 1).min(remaining);
        let mut bytes = Vec::new();
        let read = file.take(limit as u64).read_to_end(&mut bytes);
        self.manifest_bytes += bytes.len();
        read.map_err(|error| format!("Could not read manifest: {error}"))?;
        if bytes.len() == remaining {
            self.notice("The shared 32 MiB manifest inspection limit was reached; some manifest metadata is incomplete.".into());
            return Err(
                "The shared manifest byte limit prevented confirming the end of this manifest."
                    .into(),
            );
        }
        if bytes.len() > MAX_MANIFEST_BYTES || bytes.len() as u64 != length {
            return Err(
                "Manifest exceeded its inspection limit or changed while being read.".into(),
            );
        }
        if bytes.starts_with(&[0xff, 0xfe])
            || bytes.starts_with(&[0xfe, 0xff])
            || bytes.starts_with(&[0x00, 0x00, 0xfe, 0xff])
        {
            return Err("This manifest uses UTF-16 or UTF-32 encoding. The loader can read that encoding, but this inventory currently inspects only UTF-8 manifests.".into());
        }
        let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
        serde_json::from_slice(bytes).map_err(|error| format!("Invalid manifest JSON: {error}"))
    }

    fn package(&mut self, path: &Path) -> NativePackage {
        let folder = path.parent().expect("discovered manifest has a parent");
        let folder_name = folder
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let mut package = NativePackage {
            manifest_path: display_path(path),
            package_path: display_path(folder),
            id: folder_name.clone(),
            name: folder_name,
            version: None,
            author: None,
            package_type: "mod".into(),
            entry_type: None,
            entry_method: None,
            assembly_path: None,
            assembly_candidates: Vec::new(),
            state: "incomplete".into(),
            details: Vec::new(),
        };
        let manifest = match self.read_manifest(path) {
            Ok(manifest) => manifest,
            Err(error) => {
                package.details.push(error);
                package.details.push("Metadata could not be inspected reliably. The loader may use empty metadata and attempt DLL fallback; this inventory does not guess that result.".into());
                return package;
            }
        };
        package.id = manifest.value(&["id"]).unwrap_or(package.id);
        package.name = manifest
            .value(&["name"])
            .unwrap_or_else(|| package.id.clone());
        package.version = manifest.value(&["version"]);
        package.author = manifest.value(&["author", "creator", "createdBy", "created_by", "owner"]);
        let library = manifest.value(&["type"]).is_some_and(|kind| {
            [
                "Library",
                "OpenACAILibrary",
                "OpenACAI.Library",
                "OpenACAI-Library",
            ]
            .iter()
            .any(|alias| kind.eq_ignore_ascii_case(alias))
        });
        if library {
            package.package_type = "library".into();
            package
                .details
                .push("Library packages are not independently executed as native mods.".into());
        }
        package.entry_type = manifest.value(&[
            "entryType",
            "entry_type",
            "mainType",
            "pluginType",
            "typeName",
            "class",
            "entrypoint",
            "entryPoint",
            "main",
            "plugin",
        ]);
        package.entry_method = manifest.value(&[
            "entryMethod",
            "entry_method",
            "method",
            "initializeMethod",
            "initMethod",
        ]);
        if !library && package.entry_type.is_none() {
            package.details.push("No entrypoint type is declared; the native runtime skips execution of this metadata-only package.".into());
        }

        let (assembly, candidates, complete) =
            self.assembly(folder, &manifest, &mut package.details);
        package.assembly_path = assembly.as_deref().map(display_path);
        package.assembly_candidates = candidates.iter().map(|path| display_path(path)).collect();
        package.state = if disabled_path(folder) {
            package.details.push("The package path is disabled or pending.".into());
            "disabled"
        } else if package.assembly_candidates.len() > 1 && assembly.is_none() {
            "ambiguous"
        } else if !complete {
            "incomplete"
        } else if library {
            if assembly.is_some() { "library" } else { "incomplete" }
        } else if package.entry_type.is_none() {
            "metadata-only"
        } else if assembly.is_none() {
            "incomplete"
        } else {
            package.details.push("Manifest and assembly candidate found. Assembly metadata, entrypoint validity, dependencies, and runtime loading have not been verified.".into());
            "candidate"
        }.into();
        package
    }

    fn assembly(
        &mut self,
        folder: &Path,
        manifest: &Manifest,
        details: &mut Vec<String>,
    ) -> (Option<PathBuf>, Vec<PathBuf>, bool) {
        let declarations: Vec<_> = ["assembly", "entryAssembly", "mainAssembly", "dll"]
            .iter()
            .filter_map(|key| manifest.value(&[key]))
            .collect();
        let mut complete = true;
        let ordinary_folder = PathBuf::from(display_path(folder));
        for declaration in &declarations {
            let file = if declaration.to_ascii_lowercase().ends_with(".dll") {
                declaration.clone()
            } else {
                format!("{declaration}.dll")
            };
            let mut checked = HashSet::new();
            let mut reported_outside = false;
            for prefix in [
                "",
                "bin",
                "bin/net11.0",
                "bin/net10.0",
                "managed",
                "Managed",
            ] {
                let candidate_path = ordinary_folder.join(prefix).join(&file);
                let Some(relative) = contained_relative(&ordinary_folder, &candidate_path) else {
                    if !reported_outside {
                        details.push(format!("Skipped assembly search paths outside the package for declaration: {declaration}"));
                        reported_outside = true;
                    }
                    continue;
                };
                if !checked.insert(relative.clone()) {
                    continue;
                }
                let candidate = folder.join(&relative);
                match validate_target(folder, &relative) {
                    Ok(true) if !disabled_path(&candidate) => {
                        return (Some(candidate.clone()), vec![candidate], complete)
                    }
                    Ok(_) => {}
                    Err(error) => {
                        complete = false;
                        details.push(format!(
                            "Could not inspect a declared assembly candidate: {error}"
                        ));
                    }
                }
            }
        }
        if !declarations.is_empty() {
            details.push("No enabled declared assembly was found. The native loader falls back to recursively discovered DLLs.".into());
        }
        let mut candidates = Vec::new();
        let mut queue = VecDeque::from([(folder.to_owned(), 0)]);
        while let Some((current, depth)) = queue.pop_front() {
            let directory = self.directory(&current);
            complete &= directory.complete;
            for file in directory.files {
                if file
                    .extension()
                    .is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("dll"))
                    && !disabled_path(&file)
                {
                    match file
                        .strip_prefix(folder)
                        .map_err(|error| error.to_string())
                        .and_then(|relative| validate_target(folder, relative))
                    {
                        Ok(true) => candidates.push(file),
                        Ok(false) => complete = false,
                        Err(error) => {
                            complete = false;
                            details.push(format!(
                                "Could not inspect a fallback assembly candidate: {error}"
                            ));
                        }
                    }
                }
            }
            for child in directory
                .folders
                .into_iter()
                .filter(|path| !has_disabled_segment(path))
            {
                if depth < ASSEMBLY_DEPTH {
                    queue.push_back((child, depth + 1));
                } else {
                    complete = false;
                    details.push("DLL fallback discovery stopped at 16 nested folders; deeper DLLs may exist.".into());
                }
            }
        }
        candidates.sort();
        candidates.dedup();
        if !complete {
            details.push("DLL fallback inspection is incomplete; no assembly was selected. See inventory notices for skipped folders or inspection limits.".into());
        }
        match candidates.len() {
            0 => details.push("No enabled assembly was found within the inspected package paths.".into()),
            1 => details.push("One DLL fallback candidate was found.".into()),
            _ => details.push("Multiple DLL fallback candidates were found. The native loader selects by filesystem enumeration order; the manager does not guess which DLL it will select.".into()),
        }
        let selected = if complete && candidates.len() == 1 {
            candidates.first().cloned()
        } else {
            None
        };
        (selected, candidates, complete)
    }
}

// Keep the loader's top-level, string-only, case-insensitive dictionary semantics,
// including rejecting duplicate string property names instead of silently picking one.
struct Manifest(HashMap<String, String>);

impl Manifest {
    fn value(&self, aliases: &[&str]) -> Option<String> {
        aliases.iter().find_map(|key| {
            self.0
                .get(&key.to_ascii_lowercase())
                .filter(|value| !value.trim().is_empty())
                .cloned()
        })
    }
}

impl<'de> Deserialize<'de> for Manifest {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ManifestVisitor;
        impl<'de> Visitor<'de> for ManifestVisitor {
            type Value = Manifest;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a JSON object")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Manifest, M::Error> {
                let mut values = HashMap::new();
                while let Some((key, value)) = map.next_entry::<String, serde_json::Value>()? {
                    if let serde_json::Value::String(value) = value {
                        if values.insert(key.to_ascii_lowercase(), value).is_some() {
                            return Err(de::Error::custom(format!(
                                "duplicate string property: {key}"
                            )));
                        }
                    }
                }
                Ok(Manifest(values))
            }
        }
        deserializer.deserialize_map(ManifestVisitor)
    }
}

fn contained_relative(folder: &Path, path: &Path) -> Option<PathBuf> {
    let ordinary = PathBuf::from(display_path(path));
    let mut normalized = PathBuf::new();
    for part in ordinary.components() {
        match part {
            Component::Normal(_) | Component::Prefix(_) | Component::RootDir => {
                normalized.push(part.as_os_str())
            }
            Component::CurDir => {}
            Component::ParentDir if normalized.pop() => {}
            _ => return None,
        }
    }
    let mut components = normalized.components();
    for expected in folder.components() {
        if !components
            .next()?
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(&expected.as_os_str().to_string_lossy())
        {
            return None;
        }
    }
    let relative: PathBuf = components.collect();
    if relative.as_os_str().is_empty() {
        None
    } else {
        Some(relative)
    }
}

fn ignored_folder(path: &Path) -> bool {
    path.file_name().is_some_and(|name| {
        let name = name.to_string_lossy();
        name.eq_ignore_ascii_case("_Disabled") || name.eq_ignore_ascii_case(".vs")
    })
}

fn disabled_path(path: &Path) -> bool {
    let value = path.to_string_lossy().to_lowercase();
    value.ends_with(".disabled") || value.ends_with(".pending") || has_disabled_segment(path)
}

fn has_disabled_segment(path: &Path) -> bool {
    path.components().any(|part| {
        part.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case("_Disabled")
    })
}

fn display_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    if let Some(unc) = value.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{unc}")
    } else {
        value.strip_prefix("\\\\?\\").unwrap_or(&value).to_owned()
    }
}

fn mark_duplicates(packages: &mut [NativePackage]) {
    let mut ids: HashMap<String, usize> = HashMap::new();
    let mut assemblies: HashMap<String, usize> = HashMap::new();
    let executable = |package: &NativePackage| {
        package.package_type == "mod" && package.entry_type.is_some() && package.state != "disabled"
    };
    for package in packages.iter().filter(|package| executable(package)) {
        *ids.entry(package.id.to_lowercase()).or_default() += 1;
        if let Some(assembly) = &package.assembly_path {
            *assemblies.entry(assembly.to_lowercase()).or_default() += 1;
        }
    }
    for package in packages.iter_mut().filter(|package| executable(package)) {
        let duplicate_id = ids
            .get(&package.id.to_lowercase())
            .is_some_and(|count| *count > 1);
        let duplicate_assembly = package.assembly_path.as_ref().is_some_and(|assembly| {
            assemblies
                .get(&assembly.to_lowercase())
                .is_some_and(|count| *count > 1)
        });
        if duplicate_id || duplicate_assembly {
            package.state = "ambiguous".into();
            if duplicate_id {
                package.details.push("More than one enabled executable native manifest declares this mod ID (case-insensitive), including manifests whose assembly is missing.".into());
            }
            if duplicate_assembly {
                package.details.push("More than one enabled executable native manifest selects this assembly path (case-insensitive).".into());
            }
        }
    }
}
