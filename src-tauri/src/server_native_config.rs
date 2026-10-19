use crate::mod_update;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::value::RawValue;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub(crate) const CONFIG_FILE: &str = "dedicatedserver.cfg";
pub(crate) const MAX_CONFIG_BYTES: usize = 64 * 1024;

pub(crate) struct NativeConfig {
    pub(crate) revision: String,
    pub(crate) summary: ConfigSummary,
    original_bytes: Vec<u8>,
}

impl NativeConfig {
    pub(crate) fn original_bytes(&self) -> &[u8] { &self.original_bytes }

    pub(crate) fn with_max_players(&self, max_players: u32) -> Result<Self, String> {
        let changed = self.with_property("MaxPlayers", &max_players.to_string())?;
        if changed.summary.max_players != max_players {
            return Err("The updated native capacity could not be verified.".into());
        }
        Ok(changed)
    }

    pub(crate) fn with_server_name(&self, server_name: &str) -> Result<Self, String> {
        let value = serde_json::to_string(server_name)
            .map_err(|_| "The server name could not be encoded as JSON.")?;
        let changed = self.with_property("ServerName", &value)?;
        if changed.summary.server_name != server_name {
            return Err("The updated native server name could not be verified.".into());
        }
        Ok(changed)
    }

    // The strict native parser has already validated the complete document. Borrowed
    // RawValues locate its original value bytes without normalizing other JSON content.
    fn with_property(&self, name: &str, value: &str) -> Result<Self, String> {
        let bytes = self.original_bytes();
        let text = std::str::from_utf8(bytes)
            .map_err(|_| "Could not locate the native configuration value.")?;
        let body = text.strip_prefix('\u{feff}').unwrap_or(text);
        let properties: HashMap<String, &RawValue> = serde_json::from_str(body)
            .map_err(|_| "Could not locate the native configuration value.")?;
        let (range, replacement) = if let Some(original) = properties.iter()
            .find_map(|(key, original)| key.eq_ignore_ascii_case(name).then_some(*original)) {
            let raw = original.get().as_bytes();
            let start = (raw.as_ptr() as usize).checked_sub(bytes.as_ptr() as usize)
                .ok_or("Could not locate the native configuration value.")?;
            let end = start.checked_add(raw.len())
                .ok_or("Could not locate the native configuration value.")?;
            if bytes.get(start..end) != Some(raw) {
                return Err("Could not locate the native configuration value.".into());
            }
            (start..end, value.to_owned())
        } else {
            // Only a validated root object's closing delimiter and JSON whitespace
            // are inspected here. All original whitespace remains in the output.
            let object = body.trim_end_matches([' ', '\t', '\r', '\n']);
            let before_close = object.strip_suffix('}')
                .ok_or("Could not locate the end of the native configuration.")?;
            let insertion = text.len() - body.len()
                + before_close.trim_end_matches([' ', '\t', '\r', '\n']).len();
            let separator = if properties.is_empty() { "" } else { "," };
            (insertion..insertion, format!("{separator}\"{name}\":{value}"))
        };
        let length = bytes.len() - range.len() + replacement.len();
        if length > MAX_CONFIG_BYTES {
            return Err("The settings change would make dedicatedserver.cfg exceed the 64 KiB limit.".into());
        }
        let mut changed = Vec::with_capacity(length);
        changed.extend_from_slice(&bytes[..range.start]);
        changed.extend_from_slice(replacement.as_bytes());
        changed.extend_from_slice(&bytes[range.end..]);
        parse(changed)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConfigSummary {
    pub(crate) server_name: String,
    pub(crate) save_mode: String,
    pub(crate) save_slot: u32,
    pub(crate) max_players: u32,
}

struct ConfigProperties(HashMap<String, Value>);

impl<'de> Deserialize<'de> for ConfigProperties {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct PropertiesVisitor;

        impl<'de> Visitor<'de> for PropertiesVisitor {
            type Value = ConfigProperties;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a JSON object with unique top-level property names")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
                let mut properties = HashMap::new();
                while let Some(key) = map.next_key::<String>()? {
                    if !key.is_ascii() {
                        return Err(de::Error::custom("native top-level property names must be ASCII"));
                    }
                    let key = key.to_ascii_lowercase();
                    if properties.contains_key(&key) {
                        // Do not include arbitrary config keys or values in diagnostics.
                        return Err(de::Error::custom("duplicate top-level property"));
                    }
                    properties.insert(key, map.next_value::<Value>()?);
                }
                Ok(ConfigProperties(properties))
            }
        }

        deserializer.deserialize_map(PropertiesVisitor)
    }
}

pub(crate) fn read(data_directory: &Path) -> Result<NativeConfig, String> {
    if !mod_update::validate_target(data_directory, Path::new(CONFIG_FILE))? {
        return Err("The server data folder has no dedicatedserver.cfg. Create the native server configuration before launching.".into());
    }
    let file = File::open(data_directory.join(CONFIG_FILE))
        .map_err(|error| format!("Could not read dedicatedserver.cfg: {error}"))?;
    let metadata = file.metadata()
        .map_err(|error| format!("Could not inspect dedicatedserver.cfg: {error}"))?;
    if !metadata.is_file() {
        return Err("dedicatedserver.cfg must be an ordinary file.".into());
    }
    if metadata.len() > MAX_CONFIG_BYTES as u64 {
        return Err("dedicatedserver.cfg exceeds the 64 KiB limit.".into());
    }
    let mut bytes = Vec::new();
    file.take((MAX_CONFIG_BYTES + 1) as u64).read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read dedicatedserver.cfg: {error}"))?;
    parse(bytes)
}

pub(crate) fn parse(bytes: Vec<u8>) -> Result<NativeConfig, String> {
    if bytes.len() > MAX_CONFIG_BYTES {
        return Err("dedicatedserver.cfg exceeds the 64 KiB limit.".into());
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| "dedicatedserver.cfg must contain valid UTF-8 JSON.")?;
    let body = text.strip_prefix('\u{feff}').unwrap_or(text);
    let ConfigProperties(properties) = serde_json::from_str::<ConfigProperties>(body)
        .map_err(|_| "dedicatedserver.cfg must be a JSON object with ASCII top-level property names, without comments, trailing commas, or duplicate properties (including case aliases).")?;

    let save_mode = match properties.get("savemode") {
        None => "Continue",
        Some(Value::String(value)) if value.eq_ignore_ascii_case("Continue") => "Continue",
        Some(Value::String(value)) if value.eq_ignore_ascii_case("New") => "New",
        _ => return Err("dedicatedserver.cfg SaveMode must be Continue or New.".into()),
    };
    let server_name = match properties.get("servername") {
        None => "Sons of the Forest Server",
        Some(Value::String(value))
            if value.chars().count() <= 256 && !value.chars().any(char::is_control) => value,
        _ => return Err("dedicatedserver.cfg ServerName must be a string of at most 256 characters without control characters.".into()),
    };

    Ok(NativeConfig {
        // Hash the original bytes, including any BOM and formatting.
        revision: format!("{:x}", Sha256::digest(&bytes)),
        summary: ConfigSummary {
            server_name: server_name.to_owned(),
            save_mode: save_mode.to_owned(),
            save_slot: positive_integer(&properties, "saveslot", "SaveSlot", 1)?,
            max_players: positive_integer(&properties, "maxplayers", "MaxPlayers", 8)?,
        },
        original_bytes: bytes,
    })
}

fn positive_integer(
    properties: &HashMap<String, Value>,
    key: &str,
    label: &str,
    default: u32,
) -> Result<u32, String> {
    let Some(value) = properties.get(key) else { return Ok(default); };
    value.as_u64()
        .and_then(|number| u32::try_from(number).ok())
        .filter(|number| *number > 0)
        .ok_or_else(|| format!("dedicatedserver.cfg {label} must be a positive 32-bit integer."))
}
