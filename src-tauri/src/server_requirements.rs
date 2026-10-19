use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const MAX_DOCUMENT_BYTES: usize = 64 * 1024;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServerRequirements {
    pub format: String,
    pub schema_version: u32,
    pub name: String,
    pub packages: Vec<RequirementPackage>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequirementPackage {
    pub loader: String,
    pub id: String,
    pub version: String,
}

fn validate_text(value: &str, label: &str, max_chars: usize) -> Result<(), String> {
    if value.is_empty()
        || value.trim() != value
        || value.chars().count() > max_chars
        || value.chars().any(char::is_control)
    {
        return Err(format!(
            "{label} must contain 1 to {max_chars} characters, without control characters or leading or trailing whitespace."
        ));
    }
    Ok(())
}

/// Parses a portable preparation list; it does not establish runtime compatibility.
#[tauri::command]
pub fn parse_server_requirements(text: String) -> Result<ServerRequirements, String> {
    if text.len() > MAX_DOCUMENT_BYTES {
        return Err("Server requirements must be at most 64 KiB of UTF-8 JSON.".into());
    }
    let document: ServerRequirements = serde_json::from_str(&text)
        .map_err(|error| format!("Invalid server requirements JSON: {error}"))?;
    if document.format != "modstoke.server-requirements" || document.schema_version != 1 {
        return Err("Expected modstoke.server-requirements with schemaVersion 1.".into());
    }
    validate_text(&document.name, "Name", 120)?;
    if document.packages.is_empty() || document.packages.len() > 128 {
        return Err("Server requirements must list between 1 and 128 packages.".into());
    }
    let mut ids = HashSet::new();
    for package in &document.packages {
        if package.loader != "modstoke" {
            return Err("Only the modstoke native package loader is supported.".into());
        }
        validate_text(&package.id, "Package ID", 256)?;
        validate_text(&package.version, "Package version", 128)?;
        if !ids.insert(package.id.to_lowercase()) {
            return Err(format!("Duplicate package ID (case-insensitive): {}", package.id));
        }
    }
    let portable_json = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("Could not format server requirements: {error}"))?;
    if portable_json.len() + 1 > MAX_DOCUMENT_BYTES {
        return Err("The formatted requirements exceed 64 KiB. Use a smaller package list.".into());
    }
    Ok(document)
}
