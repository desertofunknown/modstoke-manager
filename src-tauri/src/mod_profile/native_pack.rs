use super::native_archive;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const FORMAT: &str = "modstoke.native-pack";
const MAX_DOCUMENT_BYTES: usize = 256 * 1024;
const MAX_PACKAGES: usize = 128;
const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_TOTAL_FILES: usize = 32768;

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativePackDependency {
    pub id: String,
    pub version: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativePackPackage {
    pub id: String,
    pub name: String,
    pub version: String,
    pub folder: String,
    pub required: Vec<NativePackDependency>,
    pub sha256: String,
    pub size: u64,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativePackDocument {
    pub format: String,
    pub schema_version: u32,
    pub name: String,
    pub packages: Vec<NativePackPackage>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativePackPreparation {
    pub document: NativePackDocument,
    pub total_archive_bytes: u64,
    pub total_decoded_bytes: u64,
    pub file_count: usize,
}

pub(super) struct InspectedPack {
    pub(super) preparation: NativePackPreparation,
    pub(super) archives: Vec<native_archive::NativeArchive>,
}

pub(super) fn describe_archive(archive: &native_archive::NativeArchive) -> Result<NativePackPackage, String> {
    let mut package = NativePackPackage {
        id: archive.id.clone(), name: archive.name.clone(), version: archive.version.clone(),
        folder: archive.package_path.rsplit('/').next().ok_or("The native archive has no package folder.")?.into(),
        required: archive.required.iter().map(|dependency| NativePackDependency {
            id: dependency.id.clone(), version: dependency.version.clone(),
        }).collect(),
        sha256: archive.sha256.clone(), size: archive.size,
    };
    canonicalize_package(&mut package)?;
    Ok(package)
}

fn validate_pack_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.trim() != name
        || name.chars().count() > 120
        || name.chars().any(char::is_control)
    {
        return Err("The pack name must contain 1–120 Unicode characters without surrounding whitespace or control characters.".into());
    }
    Ok(())
}

fn canonicalize_package(package: &mut NativePackPackage) -> Result<(), String> {
    native_archive::validate_id(&package.id)?;
    native_archive::validate_name(&package.name)?;
    native_archive::validate_version(&package.version)?;
    native_archive::validate_hash(&package.sha256)?;
    native_archive::validate_package_folder(&package.folder)?;
    native_archive::validate_path(&format!("OpenACAI/Mods/{}", package.folder))?;
    if package.size == 0 || package.size > MAX_ARCHIVE_BYTES {
        return Err(format!("Package {} must describe an archive of 1 byte to 256 MiB.", package.id));
    }
    if package.required.len() > 64 {
        return Err(format!("Package {} declares more than 64 required dependencies.", package.id));
    }
    let mut dependencies = BTreeSet::new();
    for dependency in &package.required {
        native_archive::validate_id(&dependency.id)?;
        native_archive::validate_version(&dependency.version)?;
        if dependency.id.eq_ignore_ascii_case(&package.id) {
            return Err(format!("Package {} cannot require itself.", package.id));
        }
        if !dependencies.insert(dependency.id.to_ascii_lowercase()) {
            return Err(format!("Package {} repeats required dependency {}.", package.id, dependency.id));
        }
    }
    package.required.sort_by_cached_key(|dependency| dependency.id.to_ascii_lowercase());
    Ok(())
}

pub(super) fn canonicalize_document(mut document: NativePackDocument) -> Result<NativePackDocument, String> {
    if document.format != FORMAT || document.schema_version != 1 {
        return Err("Expected a modstoke.native-pack document with schemaVersion 1.".into());
    }
    validate_pack_name(&document.name)?;
    if document.packages.is_empty() || document.packages.len() > MAX_PACKAGES {
        return Err("A native pack must contain 1–128 packages.".into());
    }
    let mut ids: BTreeMap<String, usize> = BTreeMap::new();
    let mut folders: BTreeMap<String, String> = BTreeMap::new();
    let mut total_bytes = 0;
    for (index, package) in document.packages.iter_mut().enumerate() {
        canonicalize_package(package)?;
        if ids.insert(package.id.to_ascii_lowercase(), index).is_some() {
            return Err(format!("Native pack ID conflict: package {} is selected more than once.", package.id));
        }
        if let Some(other) = folders.insert(package.folder.to_ascii_lowercase(), package.id.clone()) {
            return Err(format!("Native pack folder conflict: {} and {} both use {}.", other, package.id, package.folder));
        }
        if package.size > MAX_TOTAL_BYTES - total_bytes {
            return Err("The native pack exceeds 2 GiB of archive bytes.".into());
        }
        total_bytes += package.size;
    }

    let mut remaining = vec![0usize; document.packages.len()];
    let mut dependents = vec![Vec::new(); document.packages.len()];
    let mut ready = BTreeSet::new();
    for (index, package) in document.packages.iter().enumerate() {
        remaining[index] = package.required.len();
        for dependency in &package.required {
            let dependency_index = ids.get(&dependency.id.to_ascii_lowercase()).copied()
                .ok_or_else(|| format!("Package {} requires {} version {}, which is missing from the selected pack.", package.id, dependency.id, dependency.version))?;
            let selected = &document.packages[dependency_index];
            if selected.version != dependency.version {
                return Err(format!("Dependency version conflict: {} requires {} version {}, but the pack selects version {}.", package.id, dependency.id, dependency.version, selected.version));
            }
            dependents[dependency_index].push(index);
        }
        if remaining[index] == 0 {
            ready.insert(package.id.to_ascii_lowercase());
        }
    }
    let mut ordered = Vec::with_capacity(document.packages.len());
    while let Some(id) = ready.pop_first() {
        let index = ids[&id];
        ordered.push(document.packages[index].clone());
        for &dependent in &dependents[index] {
            remaining[dependent] -= 1;
            if remaining[dependent] == 0 {
                ready.insert(document.packages[dependent].id.to_ascii_lowercase());
            }
        }
    }
    if ordered.len() != document.packages.len() {
        let blocked: Vec<_> = ids.iter()
            .filter(|(_, index)| remaining[**index] != 0)
            .map(|(_, index)| document.packages[*index].id.as_str())
            .collect();
        return Err(format!("A native dependency cycle prevents ordering these packages: {}.", blocked.join(", ")));
    }
    document.packages = ordered;
    // The UI exports two-space pretty JSON followed by one newline.
    let bytes = serde_json::to_vec_pretty(&document)
        .map_err(|error| format!("Could not encode the native pack: {error}"))?;
    if bytes.len() + 1 > MAX_DOCUMENT_BYTES {
        return Err("The native pack descriptor exceeds 256 KiB.".into());
    }
    Ok(document)
}

/// Validates descriptor metadata only; this does not verify any archive bytes.
#[tauri::command]
pub fn parse_native_pack(text: String) -> Result<NativePackDocument, String> {
    if text.len() > MAX_DOCUMENT_BYTES {
        return Err("The native pack descriptor exceeds 256 KiB.".into());
    }
    let document: NativePackDocument = serde_json::from_str(&text)
        .map_err(|error| format!("Invalid native pack JSON: {error}"))?;
    canonicalize_document(document)
}

#[tauri::command]
pub async fn prepare_native_pack(
    name: String,
    sources: Vec<String>,
    expected: Option<String>,
) -> Result<NativePackPreparation, String> {
    // Validate imported claims before opening any local archives.
    let expected = expected.map(parse_native_pack).transpose()?;
    let name = expected.as_ref().map_or_else(|| name.trim().to_owned(), |document| document.name.clone());
    validate_pack_name(&name)?;
    if sources.is_empty() || sources.len() > MAX_PACKAGES {
        return Err("Select 1–128 native ZIP archives for the pack.".into());
    }
    for source in &sources {
        if source.trim().is_empty() || source.encode_utf16().count() > 32768 {
            return Err("Each native archive source must be nonblank and at most 32768 UTF-16 units.".into());
        }
    }
    if expected.as_ref().is_some_and(|document| document.packages.len() != sources.len()) {
        return Err("The selected archive count does not match the native pack descriptor.".into());
    }
    tauri::async_runtime::spawn_blocking(move || prepare_archives(name, sources, expected).map(|result| result.preparation))
        .await.map_err(|error| format!("Native pack preparation could not finish: {error}"))?
}

pub(super) fn prepare_archives(
    name: String,
    sources: Vec<String>,
    expected: Option<NativePackDocument>,
) -> Result<InspectedPack, String> {
    validate_pack_name(&name)?;
    if sources.is_empty() || sources.len() > MAX_PACKAGES
        || sources.iter().any(|source| source.trim().is_empty() || source.encode_utf16().count() > 32768) {
        return Err("Select 1–128 native ZIP paths, each at most 32768 UTF-16 units.".into());
    }
    if expected.as_ref().is_some_and(|document| document.packages.len() != sources.len()) {
        return Err("The selected archive count does not match the native pack descriptor.".into());
    }
    let mut total_archive_bytes = 0;
    let mut total_decoded_bytes = 0;
    let mut file_count = 0;
    let mut packages = Vec::with_capacity(sources.len());
    let mut archives = Vec::with_capacity(sources.len());
    let mut selected_ids = BTreeSet::new();
    let mut selected_folders = BTreeMap::new();
    for (index, source) in sources.iter().enumerate() {
        let archive = native_archive::inspect(Path::new(source))
            .map_err(|error| format!("Selected archive {}: {error}", index + 1))?;
        if archive.size > MAX_TOTAL_BYTES - total_archive_bytes {
            return Err("The native pack exceeds 2 GiB of archive bytes.".into());
        }
        total_archive_bytes += archive.size;
        if archive.files.len() > MAX_TOTAL_FILES - file_count {
            return Err("The native pack exceeds 32768 decoded files.".into());
        }
        file_count += archive.files.len();
        for file in &archive.files {
            if file.size > MAX_TOTAL_BYTES - total_decoded_bytes {
                return Err("The native pack exceeds 2 GiB of decoded files.".into());
            }
            total_decoded_bytes += file.size;
        }
        let package = describe_archive(&archive)?;
        if !selected_ids.insert(package.id.to_ascii_lowercase()) {
            return Err(format!("Native pack ID conflict: package {} is selected more than once.", package.id));
        }
        if let Some(other) = selected_folders.insert(package.folder.to_ascii_lowercase(), package.id.clone()) {
            return Err(format!("Native pack folder conflict: {} and {} both use {}.", other, package.id, package.folder));
        }
        if let Some(document) = &expected {
            let claim = document.packages.iter()
                .find(|claim| claim.id.eq_ignore_ascii_case(&package.id))
                .ok_or_else(|| format!("Selected package {} is not declared in the native pack.", package.id))?;
            if claim.version != package.version {
                return Err(format!("Package version conflict: the descriptor requires {} version {}, but the selected archive is version {}.", package.id, claim.version, package.version));
            }
            if claim != &package {
                return Err(format!("Selected package {} does not match the descriptor's ID spelling, name, version, folder, required dependencies, SHA-256, or archive size.", package.id));
            }
        }
        packages.push(package);
        archives.push(archive);
    }
    let document = canonicalize_document(NativePackDocument {
        format: FORMAT.into(),
        schema_version: 1,
        name,
        packages,
    })?;
    Ok(InspectedPack { preparation: NativePackPreparation {
        // Return the imported claims after comparison, preserving their exact strings.
        document: expected.unwrap_or(document),
        total_archive_bytes,
        total_decoded_bytes,
        file_count,
    }, archives })
}
