use crate::mod_update;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_DECODED_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ENTRIES: usize = 8192;
const MAX_MANIFEST_BYTES: usize = 512 * 1024;
const MAX_JSON_DEPTH: usize = 16;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct NativeDependency {
    pub(super) id: String,
    pub(super) version: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct NativeArchiveFile {
    pub(super) path: String,
    pub(super) sha256: String,
    pub(super) size: u64,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct NativeArchive {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) version: String,
    pub(super) package_path: String,
    pub(super) manifest_path: String,
    pub(super) assembly_path: String,
    pub(super) required: Vec<NativeDependency>,
    pub(super) files: Vec<NativeArchiveFile>,
    pub(super) sha256: String,
    pub(super) size: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct NativeManifest {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) version: String,
    pub(super) assembly: String,
    pub(super) entry_type: String,
    pub(super) required: Vec<NativeDependency>,
}

pub(super) fn inspect(source: &Path) -> Result<NativeArchive, String> {
    let source = if source.is_absolute() {
        source.to_owned()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("Could not resolve the archive folder: {error}"))?
            .join(source)
    };
    let parent = source.parent().ok_or("The archive has no parent folder.")?;
    let name = Path::new(source.file_name().ok_or("The archive has no file name.")?);
    if !mod_update::validate_target(parent, name)? {
        return Err("The native archive does not exist.".into());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Retain this handle throughout inspection; deny concurrent writes and deletion.
        options.share_mode(1).custom_flags(0x00200000);
    }
    let mut reader = options
        .open(&source)
        .map_err(|error| format!("Could not open the native archive: {error}"))?;
    let metadata = reader.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() || is_linked(&metadata) {
        return Err("The native archive must be a regular, nonlinked file.".into());
    }
    if !mod_update::validate_target(parent, name)? {
        return Err("The native archive disappeared during inspection.".into());
    }
    let size = metadata.len();
    if size > MAX_ARCHIVE_BYTES {
        return Err("The native archive exceeds 256 MiB.".into());
    }
    let (sha256, actual_size) = hash_reader(&mut reader, MAX_ARCHIVE_BYTES, None)?;
    if actual_size != size {
        return Err("The native archive changed while being inspected.".into());
    }
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|error| error.to_string())?;
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|error| format!("Could not read the native ZIP archive: {error}"))?;
    if archive.len() > MAX_ENTRIES {
        return Err("The native archive contains more than 8192 entries.".into());
    }
    let mut entries = HashMap::new();
    let mut files = Vec::new();
    let mut package_path: Option<String> = None;
    let mut manifest: Option<(String, NativeManifest)> = None;
    let mut decoded = 0u64;
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|error| format!("Could not read a native ZIP entry: {error}"))?;
        let directory = file.is_dir();
        let normalized = file.name().replace('\\', "/");
        let path = if directory {
            normalized.strip_suffix('/').unwrap_or(&normalized)
        } else {
            &normalized
        };
        let path = validate_path(path)?;
        let kind = file.unix_mode().unwrap_or_default() & 0o170000;
        if (kind != 0 && kind != 0o100000 && kind != 0o040000)
            || (kind == 0o040000 && !directory)
            || (kind == 0o100000 && directory)
        {
            return Err(format!(
                "Linked, special, or inconsistent ZIP entry: {path}"
            ));
        }
        let parts: Vec<_> = path.split('/').collect();
        if !parts[0].eq_ignore_ascii_case("OpenACAI")
            || (parts.len() > 1 && !parts[1].eq_ignore_ascii_case("Mods"))
            || (!directory && parts.len() < 4)
        {
            return Err(format!(
                "Native payload must be under OpenACAI/Mods/<package>/: {path}"
            ));
        }
        if parts.len() >= 3 {
            validate_package_folder(parts[2])?;
            let package = parts[..3].join("/");
            if let Some(existing) = &package_path {
                if existing.to_uppercase() != package.to_uppercase() {
                    return Err("A native archive must contain exactly one package folder.".into());
                }
            } else {
                package_path = Some(package);
            }
        }
        let key = path.to_uppercase();
        if entries.insert(key, directory).is_some() {
            return Err(format!(
                "The native archive contains duplicate paths: {path}"
            ));
        }
        let is_manifest = is_manifest_name(parts.last().copied().unwrap_or_default());
        if is_manifest && (directory || parts.len() != 4) {
            return Err(format!(
                "Native manifests must be files directly in the package folder: {path}"
            ));
        }
        if is_manifest && manifest.is_some() {
            return Err("The native archive contains multiple recognized manifests.".into());
        }
        let remaining = MAX_DECODED_BYTES - decoded;
        let limit = if is_manifest {
            remaining.min(MAX_MANIFEST_BYTES as u64)
        } else {
            remaining
        };
        let expected_size = file.size();
        if expected_size > limit || (directory && expected_size != 0) {
            return Err(format!(
                "ZIP entry exceeds its permitted decoded size: {path}"
            ));
        }
        let mut bytes = Vec::new();
        let (hash, file_size) = hash_reader(
            &mut file,
            limit,
            if is_manifest { Some(&mut bytes) } else { None },
        )?;
        decoded += file_size;
        if file_size != expected_size || (directory && file_size != 0) {
            return Err(format!(
                "ZIP entry size does not match its declared size: {path}"
            ));
        }
        if is_manifest {
            manifest = Some((path.clone(), parse_manifest(&bytes)?));
        }
        if !directory {
            files.push(NativeArchiveFile {
                path,
                sha256: hash,
                size: file_size,
            });
        }
    }
    for path in entries.keys() {
        let mut ancestor = path.as_str();
        while let Some((parent, _)) = ancestor.rsplit_once('/') {
            if entries.get(parent) == Some(&false) {
                return Err(format!(
                    "The native archive uses a file as a folder: {parent}"
                ));
            }
            ancestor = parent;
        }
    }
    let package_path = package_path.ok_or("The native archive has no package folder.")?;
    let (manifest_path, manifest) =
        manifest.ok_or("The native archive has no recognized manifest.")?;
    let assembly_key = format!("{package_path}/{}", manifest.assembly).to_uppercase();
    let assembly_path = files
        .iter()
        .find(|file| file.path.to_uppercase() == assembly_key)
        .map(|file| file.path.clone())
        .ok_or("The explicitly declared native DLL does not exist in the archive.")?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let record = NativeArchive {
        id: manifest.id,
        name: manifest.name,
        version: manifest.version,
        package_path,
        manifest_path,
        assembly_path,
        required: manifest.required,
        files,
        sha256,
        size,
    };
    validate_record(&record)?;
    Ok(record)
}

pub(super) fn validate_record(record: &NativeArchive) -> Result<(), String> {
    validate_id(&record.id)?;
    validate_version(&record.version)?;
    validate_name(&record.name)?;
    validate_hash(&record.sha256)?;
    if record.size == 0 || record.size > MAX_ARCHIVE_BYTES {
        return Err("The native archive record has an invalid compressed size.".into());
    }
    let package = validate_path(&record.package_path)?;
    let parts: Vec<_> = package.split('/').collect();
    if package != record.package_path
        || parts.len() != 3
        || !parts[0].eq_ignore_ascii_case("OpenACAI")
        || !parts[1].eq_ignore_ascii_case("Mods")
    {
        return Err("The native archive record must name one OpenACAI/Mods package folder.".into());
    }
    validate_package_folder(parts[2])?;
    let manifest_path = validate_path(&record.manifest_path)?;
    let assembly_path = validate_path(&record.assembly_path)?;
    if manifest_path != record.manifest_path
        || assembly_path != record.assembly_path
        || !assembly_path.to_ascii_lowercase().ends_with(".dll")
    {
        return Err("The native archive record has an invalid manifest or assembly path.".into());
    }
    if record.required.len() > 64 {
        return Err("At most 64 required native dependencies may be recorded.".into());
    }
    let mut dependencies = HashSet::new();
    for dependency in &record.required {
        validate_id(&dependency.id)?;
        validate_version(&dependency.version)?;
        if dependency.id.eq_ignore_ascii_case(&record.id)
            || !dependencies.insert(dependency.id.to_ascii_lowercase())
        {
            return Err(
                "The native archive record contains duplicate or self dependencies.".into(),
            );
        }
    }
    if record.files.is_empty() || record.files.len() > MAX_ENTRIES {
        return Err("The native archive record has an invalid file count.".into());
    }
    let prefix = format!("{}/", package.to_uppercase());
    let mut files = HashSet::new();
    let mut decoded = 0u64;
    let mut manifest_count = 0;
    let mut assembly_found = false;
    for file in &record.files {
        let path = validate_path(&file.path)?;
        let key = path.to_uppercase();
        if mod_update::is_native_user_file(Path::new(&path))
            && (is_manifest_name(path.rsplit('/').next().unwrap_or_default())
                || path.to_ascii_lowercase().ends_with(".dll")
                || path.to_ascii_lowercase().ends_with(".exe"))
        {
            return Err("Native manifests and executable files must not be placed in preserved configuration paths.".into());
        }
        if path != file.path || !key.starts_with(&prefix) || !files.insert(key.clone()) {
            return Err(format!(
                "Duplicate or out-of-package native file record: {}",
                file.path
            ));
        }
        validate_hash(&file.sha256)?;
        if file.size > MAX_DECODED_BYTES - decoded {
            return Err("The native archive record exceeds 512 MiB of decoded files.".into());
        }
        decoded += file.size;
        if is_manifest_name(path.rsplit('/').next().unwrap_or_default()) {
            if path.split('/').count() != 4
                || path != manifest_path
                || file.size == 0
                || file.size > MAX_MANIFEST_BYTES as u64
            {
                return Err("The native archive record has an invalid manifest file.".into());
            }
            manifest_count += 1;
        }
        if path == assembly_path {
            assembly_found = true;
        }
    }
    for path in &files {
        let mut ancestor = path.as_str();
        while let Some((parent, _)) = ancestor.rsplit_once('/') {
            if files.contains(parent) {
                return Err(format!(
                    "The native archive record uses a file as a folder: {parent}"
                ));
            }
            ancestor = parent;
        }
    }
    if manifest_count != 1 || !assembly_found {
        return Err(
            "The native archive record must contain its one manifest and declared DLL.".into(),
        );
    }
    Ok(())
}

pub(super) fn validate_hash(value: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err("Native archive hashes must be 64 lowercase hexadecimal characters.".into());
    }
    Ok(())
}

pub(super) fn validate_name(value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > 2048 || value.chars().any(char::is_control) {
        return Err(
            "The native package name must be nonblank, bounded text without control characters."
                .into(),
        );
    }
    Ok(())
}

fn is_linked(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn hash_reader(
    reader: &mut impl Read,
    limit: u64,
    mut retained: Option<&mut Vec<u8>>,
) -> Result<(String, u64), String> {
    let mut reader = reader.take(limit + 1);
    let mut hash = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|error| format!("Could not read native package bytes: {error}"))?;
        if count == 0 {
            break;
        }
        size += count as u64;
        if size > limit {
            return Err("Native package data exceeds its inspection byte limit.".into());
        }
        hash.update(&buffer[..count]);
        if let Some(bytes) = retained.as_mut() {
            bytes.extend_from_slice(&buffer[..count]);
        }
    }
    Ok((format!("{:x}", hash.finalize()), size))
}

pub(super) fn validate_path(value: &str) -> Result<String, String> {
    let path = value.replace('\\', "/");
    mod_update::validate_mod_path(Path::new(&path))?;
    if path.encode_utf16().count() > 32767 {
        return Err("Native package paths exceed the Win32 path length limit.".into());
    }
    for component in path.split('/') {
        let lower = component.to_lowercase();
        let device = lower
            .split('.')
            .next()
            .unwrap_or_default()
            .trim_end_matches(' ');
        let numbered_device = ["com", "lpt"].iter().any(|prefix| {
            device.strip_prefix(*prefix).is_some_and(|suffix| {
                matches!(
                    suffix,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        });
        if component.is_empty()
            || component.encode_utf16().count() > 255
            || component == "."
            || component == ".."
            || component.ends_with(['.', ' '])
            || component
                .chars()
                .any(|c| c.is_control() || "<>:\"|?*".contains(c))
            || matches!(device, "con" | "prn" | "aux" | "nul" | "conin$" | "conout$")
            || numbered_device
            || matches!(lower.as_str(), "_disabled" | ".vs" | ".git")
            || lower.ends_with(".disabled")
            || lower.ends_with(".pending")
        {
            return Err(format!("Unsafe or disabled native package path: {path}"));
        }
    }
    Ok(path)
}

fn is_manifest_name(name: &str) -> bool {
    ["openacai.mod.json", "openacai.json", "manifest.json"]
        .iter()
        .any(|alias| name.eq_ignore_ascii_case(alias))
}

pub(super) fn parse_manifest(bytes: &[u8]) -> Result<NativeManifest, String> {
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err("The native manifest exceeds 512 KiB.".into());
    }
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "Native package manifests must use UTF-8 encoding.".to_owned())?;
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let value = JsonValueSeed { depth: 0 }
        .deserialize(&mut deserializer)
        .map_err(|error| format!("Invalid native manifest JSON: {error}"))?;
    deserializer
        .end()
        .map_err(|error| format!("Invalid native manifest JSON: {error}"))?;
    let root = value
        .as_object()
        .ok_or("The native manifest must be a JSON object.")?;
    let id = required_string(root, "id")?;
    validate_id(&id)?;
    let version = required_string(root, "version")?;
    validate_version(&version)?;
    if alias_value(root, &["type"]).is_some_and(|kind| {
        [
            "Library",
            "OpenACAILibrary",
            "OpenACAI.Library",
            "OpenACAI-Library",
        ]
        .iter()
        .any(|alias| kind.eq_ignore_ascii_case(alias))
    }) {
        return Err("Library-only packages cannot be installed as executable native mods.".into());
    }
    let name = alias_value(root, &["name"]).unwrap_or(&id).to_owned();
    validate_name(&name)?;
    let assembly = alias_value(root, &["assembly", "entryAssembly", "mainAssembly", "dll"])
        .ok_or("The native manifest must explicitly declare a relative DLL path.")?;
    let assembly = validate_path(assembly)?;
    if !assembly.to_ascii_lowercase().ends_with(".dll") {
        return Err("The declared native assembly path must end in .dll.".into());
    }
    let entry_type = alias_value(
        root,
        &[
            "entryType",
            "entry_type",
            "mainType",
            "pluginType",
            "typeName",
            "class",
            "entrypoint",
            "main",
            "plugin",
        ],
    )
    .ok_or("The native manifest must explicitly declare an entrypoint type.")?
    .to_owned();
    let required = parse_dependencies(root.get("modstokedependencies"), &id)?;
    Ok(NativeManifest {
        id,
        name,
        version,
        assembly,
        entry_type,
        required,
    })
}

fn alias_value<'a>(root: &'a Map<String, Value>, aliases: &[&str]) -> Option<&'a str> {
    aliases
        .iter()
        .filter_map(|alias| root.get(&alias.to_ascii_lowercase())?.as_str())
        .find(|value| !value.trim().is_empty())
}

fn required_string(root: &Map<String, Value>, field: &str) -> Result<String, String> {
    root.get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("The native manifest requires a string {field}."))
}

pub(super) fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 96
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
    {
        return Err(
            "Native IDs must contain 1–96 ASCII letters, digits, dots, underscores, or hyphens."
                .into(),
        );
    }
    Ok(())
}

pub(super) fn validate_package_folder(folder: &str) -> Result<(), String> {
    validate_id(folder).map_err(|_| {
        "The native package folder must contain 1–96 ASCII letters, digits, dots, underscores, or hyphens.".into()
    })
}

pub(super) fn validate_version(version: &str) -> Result<(), String> {
    if version.is_empty()
        || version.trim() != version
        || version.encode_utf16().count() > 256
        || version.chars().any(char::is_control)
    {
        return Err("Native versions must contain 1–256 UTF-16 units without surrounding whitespace or control characters.".into());
    }
    Ok(())
}

fn parse_dependencies(
    value: Option<&Value>,
    own_id: &str,
) -> Result<Vec<NativeDependency>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let declaration = value
        .as_object()
        .ok_or("modstokeDependencies must be an object.")?;
    if declaration
        .keys()
        .any(|key| key != "schemaversion" && key != "required")
    {
        return Err("Unknown field in modstokeDependencies.".into());
    }
    if declaration.get("schemaversion").and_then(Value::as_u64) != Some(1) {
        return Err("modstokeDependencies.schemaVersion must be the integer 1.".into());
    }
    let requirements = declaration
        .get("required")
        .and_then(Value::as_array)
        .ok_or("modstokeDependencies.required must be an array.")?;
    if requirements.len() > 64 {
        return Err("At most 64 required native dependencies may be declared.".into());
    }
    let mut ids = HashSet::new();
    let mut result = Vec::new();
    for value in requirements {
        let dependency = value
            .as_object()
            .ok_or("Each native dependency must be an object.")?;
        if dependency.keys().any(|key| key != "id" && key != "version") {
            return Err("Unknown field in a native dependency.".into());
        }
        let id = required_string(dependency, "id")?;
        validate_id(&id)?;
        let version = required_string(dependency, "version")?;
        validate_version(&version)?;
        if id.eq_ignore_ascii_case(own_id) || !ids.insert(id.to_ascii_lowercase()) {
            return Err(format!(
                "Duplicate or self-referencing native dependency: {id}"
            ));
        }
        result.push(NativeDependency { id, version });
    }
    Ok(result)
}

// Inspect raw JSON maps recursively so duplicate keys cannot disappear into Value.
struct JsonValueSeed {
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for JsonValueSeed {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for JsonValueSeed {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(
            "JSON with at most 16 container levels and unique property names ignoring case",
        )
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("Invalid JSON number"))
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }
    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }
    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        if self.depth >= MAX_JSON_DEPTH {
            return Err(de::Error::custom(
                "Native manifest JSON exceeds 16 levels of nesting",
            ));
        }
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(JsonValueSeed {
            depth: self.depth + 1,
        })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut object: A) -> Result<Value, A::Error> {
        if self.depth >= MAX_JSON_DEPTH {
            return Err(de::Error::custom(
                "Native manifest JSON exceeds 16 levels of nesting",
            ));
        }
        let mut values = Map::new();
        let mut names = HashSet::new();
        while let Some(key) = object.next_key::<String>()? {
            if !names.insert(key.to_uppercase()) || values.contains_key(&key.to_lowercase()) {
                return Err(de::Error::custom(format!(
                    "Duplicate native manifest property: {key}"
                )));
            }
            let value = object.next_value_seed(JsonValueSeed {
                depth: self.depth + 1,
            })?;
            values.insert(key.to_lowercase(), value);
        }
        Ok(Value::Object(values))
    }
}
