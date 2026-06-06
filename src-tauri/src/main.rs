// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::fs::File;
use std::io::Read;
use zip::read::ZipArchive;

use regex::Regex;
use serde::Serialize;
use std::io::{BufRead, BufReader};
use winreg::{enums::*, RegKey};

use reqwest::header::{HeaderMap, HeaderValue};
use sha2::{Digest, Sha256};
use std::{error::Error, path::Path, path::PathBuf, ptr::null_mut};
use windows::{
    core,
    Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, VS_FIXEDFILEINFO,
    },
};

const SOTF_APP_ID: &str = "1326470";
const NEXUS_API_BASE: &str = "https://api.nexusmods.com";
const NEXUS_GAME_DOMAIN: &str = "sonsoftheforest";
const NEXUS_CREDENTIAL_SERVICE: &str = "OpenACAI Mod Manager";
const NEXUS_CREDENTIAL_ACCOUNT: &str = "nexusmods-api-key";
const NEXUS_USER_AGENT: &str =
    "OpenACAI-Mod-Manager/0.2.0 (SonsOfTheForest; Windows; SignedPublisher=OpenACAI Inc)";
const NEXUS_APPLICATION_NAME: &str = "OpenACAI Mod Manager";
const NEXUS_APPLICATION_VERSION: &str = "0.2.0";

#[derive(Serialize)]
struct LoaderZipInspection {
    is_valid: bool,
    has_manifest: bool,
    has_bridge: bool,
    has_bepinex_core: bool,
    has_doorstop: bool,
    has_private_admin_tool: bool,
    entries: usize,
    errors: Vec<String>,
}

#[derive(Serialize)]
struct NexusUser {
    user_id: Option<u64>,
    name: Option<String>,
    profile_url: Option<String>,
    membership_tier: Option<String>,
    is_premium: Option<bool>,
    is_supporter: Option<bool>,
}

#[derive(Serialize)]
struct NexusRateLimit {
    hourly_limit: Option<String>,
    hourly_remaining: Option<String>,
    hourly_reset: Option<String>,
    daily_limit: Option<String>,
    daily_remaining: Option<String>,
    daily_reset: Option<String>,
}

#[derive(Serialize)]
struct NexusSession {
    is_connected: bool,
    user: Option<NexusUser>,
    rate_limit: Option<NexusRateLimit>,
    error: Option<String>,
}

#[derive(Serialize)]
struct NexusModsResponse {
    mods: serde_json::Value,
    rate_limit: NexusRateLimit,
}

#[derive(Serialize)]
struct NexusGameInfoResponse {
    game: serde_json::Value,
    rate_limit: NexusRateLimit,
}

#[derive(Serialize)]
struct NexusModFilesResponse {
    files: serde_json::Value,
    rate_limit: NexusRateLimit,
}

#[derive(Serialize)]
struct NexusModDetailsResponse {
    details: serde_json::Value,
    rate_limit: NexusRateLimit,
}

#[derive(Serialize)]
struct NexusModDependenciesResponse {
    dependencies: serde_json::Value,
    rate_limit: NexusRateLimit,
}

#[derive(Serialize)]
struct NexusModChangelogsResponse {
    changelogs: serde_json::Value,
    rate_limit: NexusRateLimit,
}

#[derive(Serialize)]
struct NexusEndorsementsResponse {
    endorsements: serde_json::Value,
    rate_limit: NexusRateLimit,
}

#[derive(Serialize)]
struct NexusTrackedModsResponse {
    tracked_mods: serde_json::Value,
    rate_limit: NexusRateLimit,
}

#[derive(Serialize)]
struct NexusActionResponse {
    result: serde_json::Value,
    rate_limit: NexusRateLimit,
}

#[tauri::command]
fn is_dotnet10_installed() -> Result<bool, String> {
    is_dotnet_major_installed("10.")
}

#[tauri::command]
fn is_dotnet11_installed() -> Result<bool, String> {
    is_dotnet_major_installed("11.")
}

fn is_dotnet_major_installed(version_prefix: &str) -> Result<bool, String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hklm.open_subkey_with_flags(
        r"SOFTWARE\WOW6432Node\dotnet\Setup\InstalledVersions\x64\sharedfx\Microsoft.NETCore.App", 
        KEY_READ
    ).map_err(|op| op.to_string())?;

    let contains_runtime = key
        .enum_values()
        .filter_map(Result::ok)
        .any(|(name, _)| name.starts_with(version_prefix));

    Ok(contains_runtime)
}

#[tauri::command]
async fn nexus_save_api_key(api_key: String) -> Result<NexusSession, String> {
    let cleaned = api_key.trim().to_string();
    if cleaned.len() < 16 {
        return Err("Nexus API key is too short.".to_string());
    }

    let (user, rate_limit) = validate_nexus_key(&cleaned).await?;
    nexus_credential_entry()?
        .set_password(&cleaned)
        .map_err(|e| e.to_string())?;

    Ok(NexusSession {
        is_connected: true,
        user: Some(user),
        rate_limit: Some(rate_limit),
        error: None,
    })
}

#[tauri::command]
async fn nexus_get_session() -> Result<NexusSession, String> {
    let key = match read_nexus_api_key() {
        Ok(Some(key)) => key,
        Ok(None) => {
            return Ok(NexusSession {
                is_connected: false,
                user: None,
                rate_limit: None,
                error: None,
            });
        }
        Err(error) => return Err(error),
    };

    match validate_nexus_key(&key).await {
        Ok((user, rate_limit)) => Ok(NexusSession {
            is_connected: true,
            user: Some(user),
            rate_limit: Some(rate_limit),
            error: None,
        }),
        Err(error) => Ok(NexusSession {
            is_connected: false,
            user: None,
            rate_limit: None,
            error: Some(error),
        }),
    }
}

#[tauri::command]
fn nexus_clear_api_key() -> Result<(), String> {
    match nexus_credential_entry()?.delete_password() {
        Ok(_) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[tauri::command]
async fn nexus_fetch_sotf_mods(view: String) -> Result<NexusModsResponse, String> {
    let key =
        read_nexus_api_key()?.ok_or_else(|| "Connect a Nexus Mods account first.".to_string())?;
    let url = match view.as_str() {
        "latest_added" => {
            format!("{NEXUS_API_BASE}/v1/games/{NEXUS_GAME_DOMAIN}/mods/latest_added.json")
        }
        "latest_updated" => {
            format!("{NEXUS_API_BASE}/v1/games/{NEXUS_GAME_DOMAIN}/mods/latest_updated.json")
        }
        "updated" => {
            format!("{NEXUS_API_BASE}/v1/games/{NEXUS_GAME_DOMAIN}/mods/updated.json?period=1m")
        }
        "trending" | _ => {
            format!("{NEXUS_API_BASE}/v1/games/{NEXUS_GAME_DOMAIN}/mods/trending.json")
        }
    };
    let response = nexus_client(&key)
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let rate_limit = read_rate_limit(response.headers());

    if !response.status().is_success() {
        return Err(format!("Nexus request failed: {}", response.status()));
    }

    let mods = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;
    Ok(NexusModsResponse { mods, rate_limit })
}

#[tauri::command]
async fn nexus_fetch_sotf_game_info() -> Result<NexusGameInfoResponse, String> {
    let key =
        read_nexus_api_key()?.ok_or_else(|| "Connect a Nexus Mods account first.".to_string())?;
    let url = format!("{NEXUS_API_BASE}/v1/games/{NEXUS_GAME_DOMAIN}.json");
    let response = nexus_client(&key)
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let rate_limit = read_rate_limit(response.headers());

    if !response.status().is_success() {
        return Err(format!(
            "Nexus game metadata request failed: {}",
            response.status()
        ));
    }

    let game = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;
    Ok(NexusGameInfoResponse { game, rate_limit })
}

#[tauri::command]
async fn nexus_fetch_mod_details(mod_id: u64) -> Result<NexusModDetailsResponse, String> {
    let key =
        read_nexus_api_key()?.ok_or_else(|| "Connect a Nexus Mods account first.".to_string())?;
    let url = format!("{NEXUS_API_BASE}/v1/games/{NEXUS_GAME_DOMAIN}/mods/{mod_id}.json");
    let response = nexus_client(&key)
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let rate_limit = read_rate_limit(response.headers());

    if !response.status().is_success() {
        return Err(format!(
            "Nexus mod details request failed: {}",
            response.status()
        ));
    }

    let details = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;
    Ok(NexusModDetailsResponse {
        details,
        rate_limit,
    })
}

#[tauri::command]
async fn nexus_fetch_mod_files(mod_id: u64) -> Result<NexusModFilesResponse, String> {
    let key =
        read_nexus_api_key()?.ok_or_else(|| "Connect a Nexus Mods account first.".to_string())?;
    let url = format!("{NEXUS_API_BASE}/v1/games/{NEXUS_GAME_DOMAIN}/mods/{mod_id}/files.json");
    let response = nexus_client(&key)
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let rate_limit = read_rate_limit(response.headers());

    if !response.status().is_success() {
        return Err(format!(
            "Nexus mod files request failed: {}",
            response.status()
        ));
    }

    let value = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;
    let files = value.get("files").cloned().unwrap_or(value);
    Ok(NexusModFilesResponse { files, rate_limit })
}

#[tauri::command]
async fn nexus_fetch_file_dependencies(
    file_id: u64,
) -> Result<NexusModDependenciesResponse, String> {
    let key =
        read_nexus_api_key()?.ok_or_else(|| "Connect a Nexus Mods account first.".to_string())?;
    let client = nexus_client(&key);
    let dependency_file_id = match nexus_resolve_v3_mod_file_id(&client, file_id).await {
        Ok(Some(resolved_id)) => resolved_id,
        _ => file_id.to_string(),
    };

    let fallback_file_id = file_id.to_string();
    let dependency_result = nexus_request_materialized_dependencies_with_fallback(
        &client,
        &dependency_file_id,
        &fallback_file_id,
    )
    .await;
    let (value, rate_limit) = match dependency_result {
        Ok(result) => {
            if nexus_dependency_payload_has_rows(&result.0) {
                result
            } else {
                nexus_request_dependency_ranges_with_fallback(
                    &client,
                    &dependency_file_id,
                    &fallback_file_id,
                )
                .await
                .ok()
                .filter(|range_result| nexus_dependency_payload_has_rows(&range_result.0))
                .unwrap_or(result)
            }
        }
        Err(error) => return Err(error),
    };
    let dependencies = value.get("dependencies").cloned().unwrap_or(value);
    Ok(NexusModDependenciesResponse {
        dependencies,
        rate_limit,
    })
}

async fn nexus_request_materialized_dependencies_with_fallback(
    client: &reqwest::Client,
    primary_file_id: &str,
    fallback_file_id: &str,
) -> Result<(serde_json::Value, NexusRateLimit), String> {
    match nexus_request_materialized_dependencies(client, primary_file_id).await {
        Ok(result) => Ok(result),
        Err(error) if primary_file_id != fallback_file_id => {
            nexus_request_materialized_dependencies(client, fallback_file_id)
                .await
                .map_err(|fallback_error| {
                    format!("{error}; fallback dependency request failed: {fallback_error}")
                })
        }
        Err(error) => Err(error),
    }
}

async fn nexus_request_dependency_ranges_with_fallback(
    client: &reqwest::Client,
    primary_file_id: &str,
    fallback_file_id: &str,
) -> Result<(serde_json::Value, NexusRateLimit), String> {
    match nexus_request_dependency_ranges(client, primary_file_id).await {
        Ok(result) => Ok(result),
        Err(error) if primary_file_id != fallback_file_id => {
            nexus_request_dependency_ranges(client, fallback_file_id)
                .await
                .map_err(|fallback_error| {
                    format!("{error}; fallback dependency request failed: {fallback_error}")
                })
        }
        Err(error) => Err(error),
    }
}

fn nexus_dependency_payload_has_rows(value: &serde_json::Value) -> bool {
    let has_rows = |key: &str, source: &serde_json::Value| {
        source
            .get(key)
            .and_then(|rows| rows.as_array())
            .is_some_and(|rows| !rows.is_empty())
    };

    value.as_array().is_some_and(|rows| !rows.is_empty())
        || has_rows("dependencies", value)
        || has_rows("dependency_definitions", value)
        || value.get("data").is_some_and(|data| {
            data.as_array().is_some_and(|rows| !rows.is_empty())
                || has_rows("dependencies", data)
                || has_rows("dependency_definitions", data)
        })
}

async fn nexus_resolve_v3_mod_file_id(
    client: &reqwest::Client,
    game_scoped_file_id: u64,
) -> Result<Option<String>, String> {
    let url =
        format!("{NEXUS_API_BASE}/v3/games/{NEXUS_GAME_DOMAIN}/mod-files/{game_scoped_file_id}");
    let response = client.get(url).send().await.map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let value = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;
    let data = value.get("data").unwrap_or(&value);

    if let Some(id) = data
        .get("id")
        .and_then(|id| id.as_str())
        .filter(|id| !id.trim().is_empty())
    {
        return Ok(Some(id.to_string()));
    }

    Ok(data
        .get("id")
        .and_then(|id| id.as_u64())
        .map(|id| id.to_string()))
}

async fn nexus_request_materialized_dependencies(
    client: &reqwest::Client,
    file_id: &str,
) -> Result<(serde_json::Value, NexusRateLimit), String> {
    let url = format!("{NEXUS_API_BASE}/v3/mod-files/{file_id}/dependencies/materialized");
    let response = client.get(url).send().await.map_err(|e| e.to_string())?;
    let rate_limit = read_rate_limit(response.headers());

    if !response.status().is_success() {
        return Err(format!(
            "Nexus dependency request failed: {}",
            response.status()
        ));
    }

    let value = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;

    Ok((value, rate_limit))
}

async fn nexus_request_dependency_ranges(
    client: &reqwest::Client,
    file_id: &str,
) -> Result<(serde_json::Value, NexusRateLimit), String> {
    let url = format!("{NEXUS_API_BASE}/v3/mod-files/{file_id}/dependencies/ranges");
    let response = client.get(url).send().await.map_err(|e| e.to_string())?;
    let rate_limit = read_rate_limit(response.headers());

    if !response.status().is_success() {
        return Err(format!(
            "Nexus dependency range request failed: {}",
            response.status()
        ));
    }

    let value = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;

    Ok((value, rate_limit))
}

#[tauri::command]
async fn nexus_fetch_mod_changelogs(mod_id: u64) -> Result<NexusModChangelogsResponse, String> {
    let key =
        read_nexus_api_key()?.ok_or_else(|| "Connect a Nexus Mods account first.".to_string())?;
    let url =
        format!("{NEXUS_API_BASE}/v1/games/{NEXUS_GAME_DOMAIN}/mods/{mod_id}/changelogs.json");
    let response = nexus_client(&key)
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let rate_limit = read_rate_limit(response.headers());

    if !response.status().is_success() {
        return Err(format!(
            "Nexus changelog request failed: {}",
            response.status()
        ));
    }

    let changelogs = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;
    Ok(NexusModChangelogsResponse {
        changelogs,
        rate_limit,
    })
}

#[tauri::command]
async fn nexus_fetch_user_endorsements() -> Result<NexusEndorsementsResponse, String> {
    let key =
        read_nexus_api_key()?.ok_or_else(|| "Connect a Nexus Mods account first.".to_string())?;
    let response = nexus_client(&key)
        .get(format!("{NEXUS_API_BASE}/v1/user/endorsements.json"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let rate_limit = read_rate_limit(response.headers());

    if !response.status().is_success() {
        return Err(format!(
            "Nexus endorsements request failed: {}",
            response.status()
        ));
    }

    let endorsements = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;
    Ok(NexusEndorsementsResponse {
        endorsements,
        rate_limit,
    })
}

#[tauri::command]
async fn nexus_fetch_user_tracked_mods() -> Result<NexusTrackedModsResponse, String> {
    let key =
        read_nexus_api_key()?.ok_or_else(|| "Connect a Nexus Mods account first.".to_string())?;
    let response = nexus_client(&key)
        .get(format!("{NEXUS_API_BASE}/v1/user/tracked_mods.json"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let rate_limit = read_rate_limit(response.headers());

    if !response.status().is_success() {
        return Err(format!(
            "Nexus tracked mods request failed: {}",
            response.status()
        ));
    }

    let tracked_mods = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;
    Ok(NexusTrackedModsResponse {
        tracked_mods,
        rate_limit,
    })
}

#[tauri::command]
async fn nexus_endorse_sotf_mod(
    mod_id: u64,
    version: Option<String>,
) -> Result<NexusActionResponse, String> {
    let key =
        read_nexus_api_key()?.ok_or_else(|| "Connect a Nexus Mods account first.".to_string())?;
    let url = format!("{NEXUS_API_BASE}/v1/games/{NEXUS_GAME_DOMAIN}/mods/{mod_id}/endorse.json");
    let mut form: Vec<(&str, String)> = Vec::new();
    if let Some(version) = version
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        form.push(("version", version));
    }

    let response = nexus_client(&key)
        .post(url)
        .form(&form)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let rate_limit = read_rate_limit(response.headers());

    if !response.status().is_success() {
        return Err(format!(
            "Nexus endorse request failed: {}",
            response.status()
        ));
    }

    let result = read_json_or_empty(response).await?;
    Ok(NexusActionResponse { result, rate_limit })
}

#[tauri::command]
async fn nexus_track_sotf_mod(mod_id: u64) -> Result<NexusActionResponse, String> {
    nexus_set_sotf_tracking(mod_id, true).await
}

#[tauri::command]
async fn nexus_untrack_sotf_mod(mod_id: u64) -> Result<NexusActionResponse, String> {
    nexus_set_sotf_tracking(mod_id, false).await
}

async fn nexus_set_sotf_tracking(
    mod_id: u64,
    should_track: bool,
) -> Result<NexusActionResponse, String> {
    let key =
        read_nexus_api_key()?.ok_or_else(|| "Connect a Nexus Mods account first.".to_string())?;
    let form = vec![("mod_id", mod_id.to_string())];
    let request = nexus_client(&key)
        .request(
            if should_track {
                reqwest::Method::POST
            } else {
                reqwest::Method::DELETE
            },
            format!("{NEXUS_API_BASE}/v1/user/tracked_mods.json"),
        )
        .query(&[("domain_name", NEXUS_GAME_DOMAIN)])
        .form(&form);
    let response = request.send().await.map_err(|e| e.to_string())?;
    let rate_limit = read_rate_limit(response.headers());

    if !response.status().is_success() {
        let action = if should_track { "track" } else { "untrack" };
        return Err(format!(
            "Nexus {action} request failed: {}",
            response.status()
        ));
    }

    let result = read_json_or_empty(response).await?;
    Ok(NexusActionResponse { result, rate_limit })
}

async fn validate_nexus_key(api_key: &str) -> Result<(NexusUser, NexusRateLimit), String> {
    let response = nexus_client(api_key)
        .get(format!("{NEXUS_API_BASE}/v1/users/validate.json"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let rate_limit = read_rate_limit(response.headers());

    if !response.status().is_success() {
        return Err(format!(
            "Nexus account validation failed: {}",
            response.status()
        ));
    }

    let value = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;
    let user = NexusUser {
        user_id: value.get("user_id").and_then(|v| v.as_u64()),
        name: value
            .get("name")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string()),
        profile_url: value
            .get("profile_url")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string()),
        membership_tier: value
            .get("membership_tier")
            .or_else(|| value.get("premium_tier"))
            .or_else(|| value.get("account_type"))
            .and_then(|v| v.as_str())
            .map(|v| v.to_string()),
        is_premium: value.get("is_premium").and_then(|v| v.as_bool()),
        is_supporter: value.get("is_supporter").and_then(|v| v.as_bool()),
    };

    Ok((user, rate_limit))
}

fn nexus_client(api_key: &str) -> reqwest::Client {
    let mut headers = HeaderMap::new();
    headers.insert(
        "apikey",
        HeaderValue::from_str(api_key).unwrap_or_else(|_| HeaderValue::from_static("")),
    );
    headers.insert("User-Agent", HeaderValue::from_static(NEXUS_USER_AGENT));
    headers.insert(
        "Application-Name",
        HeaderValue::from_static(NEXUS_APPLICATION_NAME),
    );
    headers.insert(
        "Application-Version",
        HeaderValue::from_static(NEXUS_APPLICATION_VERSION),
    );

    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .expect("failed to build Nexus API client")
}

fn read_rate_limit(headers: &HeaderMap) -> NexusRateLimit {
    NexusRateLimit {
        hourly_limit: read_header(headers, "X-RL-Hourly-Limit"),
        hourly_remaining: read_header(headers, "X-RL-Hourly-Remaining"),
        hourly_reset: read_header(headers, "X-RL-Hourly-Reset"),
        daily_limit: read_header(headers, "X-RL-Daily-Limit"),
        daily_remaining: read_header(headers, "X-RL-Daily-Remaining"),
        daily_reset: read_header(headers, "X-RL-Daily-Reset"),
    }
}

fn read_header(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string())
}

async fn read_json_or_empty(response: reqwest::Response) -> Result<serde_json::Value, String> {
    let text = response.text().await.map_err(|e| e.to_string())?;
    if text.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }

    serde_json::from_str(&text).map_err(|e| e.to_string())
}

fn nexus_credential_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(NEXUS_CREDENTIAL_SERVICE, NEXUS_CREDENTIAL_ACCOUNT)
        .map_err(|e| e.to_string())
}

fn read_nexus_api_key() -> Result<Option<String>, String> {
    match nexus_credential_entry()?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

#[tauri::command]
fn get_file_version(path: String) -> Result<String, String> {
    let desc = get_file_description(path).map_err(|e| e.to_string())?;
    Ok(desc)
}

#[tauri::command]
fn sha256_file(path: String) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 64];

    loop {
        let read = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }

        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:X}", hasher.finalize()))
}

fn get_file_description(path: impl AsRef<Path>) -> Result<String, Box<dyn Error>> {
    let size = unsafe { GetFileVersionInfoSizeW(path.as_ref().as_os_str(), null_mut()) };
    if size == 0 {
        return Err(core::Error::from_win32().into());
    }

    let mut buffer = vec![0u8; size as usize];
    unsafe {
        GetFileVersionInfoW(
            path.as_ref().as_os_str(),
            0,
            size,
            buffer.as_mut_ptr() as *mut std::ffi::c_void,
        )
    }
    .ok()?;

    let mut ptr = null_mut();
    let mut len = 0;
    let success = unsafe {
        VerQueryValueW(
            buffer.as_ptr() as *const std::ffi::c_void,
            "\\",
            &mut ptr,
            &mut len,
        )
    }
    .as_bool();

    if !success {
        return Err("Failed to query file description".into());
    }

    let info = ptr as *const VS_FIXEDFILEINFO;
    unsafe {
        if (*info).dwSignature != 0xfeef04bd {
            return Err("Invalid fixed file info signature".into());
        }

        let description = *info;

        Ok(format!(
            "{}.{}.{}",
            description.dwFileVersionMS >> 16,
            description.dwFileVersionMS & 0xffff,
            description.dwFileVersionLS >> 16,
        ))
    }
}

#[tauri::command]
async fn get_steam_path() -> Option<String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let steam_install: PathBuf = hklm
        .open_subkey_with_flags(r"SOFTWARE\WOW6432Node\Valve\Steam", KEY_READ)
        .and_then(|key| key.get_value::<String, _>("InstallPath"))
        .or_else(|_| {
            hklm.open_subkey_with_flags(r"SOFTWARE\Valve\Steam", KEY_READ)
                .and_then(|key| key.get_value::<String, _>("InstallPath"))
        })
        .ok()?
        .into();

    let vdf = steam_install.join(r"steamapps\libraryfolders.vdf");
    if !vdf.exists() {
        return None;
    }

    let re = Regex::new(r#"\s"(?:\d|path)"\s+"(.+)""#).unwrap();
    let mut steam_paths = vec![steam_install.join(r"steamapps")];

    let file = File::open(vdf).ok()?;
    let reader = BufReader::new(file);
    for line in reader.lines() {
        if let Ok(text) = line {
            if let Some(cap) = re.captures(&text) {
                steam_paths.push(PathBuf::from(cap[1].replace(r"\\", r"\")).join(r"steamapps"));
            }
        }
    }

    let install_re = Regex::new(r#"\s"installdir"\s+"(.+)""#).unwrap();
    for path in steam_paths {
        let manifest = path.join(format!(r"appmanifest_{}.acf", SOTF_APP_ID));
        if manifest.exists() {
            let file = File::open(manifest).ok()?;
            let reader = BufReader::new(file);
            for line in reader.lines() {
                if let Ok(text) = line {
                    if let Some(cap) = install_re.captures(&text) {
                        let file_path = path
                            .join("common")
                            .join(&cap[1])
                            .join("SonsOfTheForest.exe");
                        if file_path.exists() {
                            return file_path.to_str().map(|s| s.to_string());
                        }
                    }
                }
            }
        }
    }

    None
}

fn unzip_file(source: &str, destination: &str) -> Result<(), String> {
    let reader = File::open(source).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(reader).map_err(|e| e.to_string())?;
    let destination_root = fs::canonicalize(destination).map_err(|e| e.to_string())?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let enclosed_name = file
            .enclosed_name()
            .ok_or_else(|| format!("Unsafe path in zip entry: {}", file.name()))?
            .to_owned();
        let outpath = destination_root.join(enclosed_name);

        if !outpath.starts_with(&destination_root) {
            return Err(format!("Zip entry escapes destination: {}", file.name()));
        }

        if file.is_dir() {
            if !outpath.exists() {
                fs::create_dir_all(&outpath).map_err(|e| e.to_string())?;
            }
            continue; // Skip to the next iteration since it's a directory.
        }

        if let Some(parent_dir) = outpath.parent() {
            fs::create_dir_all(parent_dir).map_err(|e| e.to_string())?;
        }

        if outpath.exists() {
            if outpath.is_file() {
                fs::remove_file(&outpath).map_err(|e| e.to_string())?;
            } else {
                return Err(format!(
                    "Refusing to overwrite non-file path: {}",
                    outpath.display()
                ));
            }
        }

        let mut outfile = File::create(&outpath).map_err(|e| e.to_string())?;
        std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn normalize_zip_entry(name: &str) -> String {
    name.replace('\\', "/")
        .trim_start_matches("./")
        .to_ascii_lowercase()
}

#[tauri::command]
fn inspect_openacai_loader_zip(source: String) -> Result<LoaderZipInspection, String> {
    let reader = File::open(source).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(reader).map_err(|e| e.to_string())?;
    let mut inspection = LoaderZipInspection {
        is_valid: false,
        has_manifest: false,
        has_bridge: false,
        has_bepinex_core: false,
        has_doorstop: false,
        has_private_admin_tool: false,
        entries: archive.len(),
        errors: Vec::new(),
    };

    for i in 0..archive.len() {
        let file = archive.by_index(i).map_err(|e| e.to_string())?;
        if file.enclosed_name().is_none() {
            inspection
                .errors
                .push(format!("Unsafe path in zip entry: {}", file.name()));
            continue;
        }

        let entry = normalize_zip_entry(file.name());
        inspection.has_manifest |=
            entry == "bepinex/plugins/openacailoader/openacai-loader.manifest.json";
        inspection.has_bridge |=
            entry == "bepinex/plugins/openacailoader/openacai.endnight.loader.core.dll";
        inspection.has_bepinex_core |= entry == "bepinex/core/bepinex.core.dll";
        inspection.has_doorstop |= entry == "winhttp.dll" || entry == "doorstop_config.ini";
        inspection.has_private_admin_tool |= entry.contains("openacaiadmintool")
            || entry.contains("anti-cheat")
            || entry.contains("anticheat");
    }

    if !inspection.has_manifest {
        inspection.errors.push(
            "Missing BepInEx/plugins/OpenACAILoader/openacai-loader.manifest.json".to_string(),
        );
    }

    if !inspection.has_bridge {
        inspection.errors.push(
            "Missing OpenACAI Endnight Loader core assembly at BepInEx/plugins/OpenACAILoader/OpenACAI.Endnight.Loader.Core.dll".to_string(),
        );
    }

    if !inspection.has_bepinex_core {
        inspection
            .errors
            .push("Missing BepInEx/core/BepInEx.Core.dll".to_string());
    }

    if !inspection.has_doorstop {
        inspection.errors.push(
            "Missing Doorstop bootstrap files (winhttp.dll or doorstop_config.ini).".to_string(),
        );
    }

    if inspection.has_private_admin_tool {
        inspection.errors.push("Package appears to contain private OpenACAI admin/anti-cheat files; public loader installs must not include them.".to_string());
    }

    inspection.is_valid = inspection.errors.is_empty();
    Ok(inspection)
}

#[tauri::command]
fn unzip_handler(source: String, destination: String) -> Result<(), String> {
    unzip_file(&source, &destination)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            unzip_handler,
            inspect_openacai_loader_zip,
            get_steam_path,
            is_dotnet10_installed,
            is_dotnet11_installed,
            get_file_version,
            nexus_save_api_key,
            nexus_get_session,
            nexus_clear_api_key,
            nexus_fetch_sotf_mods,
            nexus_fetch_sotf_game_info,
            nexus_fetch_mod_details,
            nexus_fetch_mod_files,
            nexus_fetch_file_dependencies,
            nexus_fetch_mod_changelogs,
            nexus_fetch_user_endorsements,
            nexus_fetch_user_tracked_mods,
            nexus_endorse_sotf_mod,
            nexus_track_sotf_mod,
            nexus_untrack_sotf_mod,
            sha256_file
        ])
        .plugin(tauri_plugin_upload::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
