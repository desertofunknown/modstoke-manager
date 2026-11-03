use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{collections::HashSet, net::Ipv4Addr, path::Path, sync::Mutex, time::Duration};
use tauri::{Emitter, Manager};

const API: &str = "https://modstoke.com/api/community/v1";

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DirectoryPackage {
    pub id: String,
    pub version: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum JoinTarget {
    Lobby { #[serde(rename = "lobbyId")] lobby_id: String },
    Dedicated { address: String, port: u16 },
}

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServerSettings {
    pub difficulty: Option<String>,
    pub pvp: Option<bool>,
    pub friendly_fire: Option<bool>,
    pub password_required: Option<bool>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommunityServer {
    pub id: String,
    pub name: String,
    pub description: String,
    pub host_label: String,
    pub region: String,
    pub kind: String,
    pub online: bool,
    pub first_seen: Option<String>,
    pub last_seen: Option<String>,
    pub players_current: Option<u32>,
    pub players_max: Option<u32>,
    pub components: Vec<DirectoryPackage>,
    pub required_mods: Vec<DirectoryPackage>,
    pub game_build: Option<String>,
    pub settings: ServerSettings,
    pub host_median_latency_ms: Option<f64>,
    pub join: Option<JoinTarget>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServerList {
    schema_version: u32,
    generated_at: String,
    servers: Vec<CommunityServer>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ServerDetail {
    schema_version: u32,
    server: CommunityServer,
}

// History may gain aggregate fields; only these bounded measurements are exposed.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySample {
    at: String,
    players: Option<f64>,
    latency_ms: Option<f64>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerHistory {
    schema_version: u32,
    server_id: String,
    samples: Vec<HistorySample>,
}

fn valid_id(id: &str) -> bool {
    id.len() == 36 && id.bytes().enumerate().all(|(i, byte)| {
        if [8, 13, 18, 23].contains(&i) { byte == b'-' } else { byte.is_ascii_hexdigit() }
    })
}

fn text(value: &str, limit: usize, required: bool) -> Result<(), String> {
    if value.chars().count() > limit || value.chars().any(char::is_control)
        || (required && value.trim().is_empty()) {
        return Err("The directory returned invalid text.".into());
    }
    Ok(())
}

fn timestamp(value: &str) -> Result<(), String> {
    text(value, 40, true)?;
    if value.len() < 20 || !value.contains('T') || !(value.ends_with('Z') || value.ends_with("+00:00")) {
        return Err("The directory returned an invalid UTC timestamp.".into());
    }
    Ok(())
}

fn measure(value: Option<f64>, maximum: f64) -> Result<(), String> {
    if value.is_some_and(|value| !value.is_finite() || value < 0.0 || value > maximum) {
        return Err("The directory returned an out-of-range measurement.".into());
    }
    Ok(())
}

fn packages(values: &[DirectoryPackage], maximum: usize) -> Result<(), String> {
    if values.len() > maximum { return Err("The directory returned too many packages.".into()); }
    let mut ids = HashSet::new();
    for package in values {
        text(&package.id, 100, true)?;
        text(&package.version, 80, true)?;
        if package.id.trim() != package.id || package.version.trim() != package.version
            || !ids.insert(package.id.to_lowercase()) {
            return Err("The directory returned ambiguous package requirements.".into());
        }
    }
    Ok(())
}

fn public_ipv4(address: &str) -> Result<Ipv4Addr, String> {
    let ip: Ipv4Addr = address.parse().map_err(|_| "The server must advertise a public IPv4 address.")?;
    let [a, b, c, _] = ip.octets();
    if ip.to_string() != address || a == 0 || a == 10 || a == 127 || a >= 224
        || (a == 100 && (64..=127).contains(&b)) || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b)) || (a == 192 && b == 168)
        || (a == 192 && b == 0 && (c == 0 || c == 2))
        || (a == 192 && b == 88 && c == 99) || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100) || (a == 203 && b == 0 && c == 113) {
        return Err("The server must advertise a public IPv4 address.".into());
    }
    Ok(ip)
}

fn validate_join(join: &JoinTarget) -> Result<(), String> {
    match join {
        JoinTarget::Lobby { lobby_id } => {
            if lobby_id.is_empty() || lobby_id.len() > 20 || !lobby_id.bytes().all(|c| c.is_ascii_digit())
                || lobby_id.starts_with('0') || lobby_id.parse::<u64>().ok().filter(|id| *id > 0).is_none() {
                return Err("The directory returned an invalid Steam lobby ID.".into());
            }
        }
        JoinTarget::Dedicated { address, port } => {
            public_ipv4(address)?;
            if *port == 0 { return Err("The directory returned an invalid game connection port.".into()); }
        }
    }
    Ok(())
}

fn validate_server(server: &CommunityServer) -> Result<(), String> {
    if !valid_id(&server.id) || !matches!(server.kind.as_str(), "dedicated" | "player-hosted") {
        return Err("The directory returned an invalid server identity.".into());
    }
    text(&server.name, 80, true)?;
    text(&server.host_label, 80, true)?;
    text(&server.description, 800, false)?;
    text(&server.region, 40, false)?;
    for value in [&server.first_seen, &server.last_seen].into_iter().flatten() { timestamp(value)?; }
    if let Some(value) = &server.game_build { text(value, 80, false)?; }
    if let Some(value) = &server.settings.difficulty { text(value, 80, false)?; }
    measure(server.players_current.map(f64::from), 10000.0)?;
    measure(server.players_max.map(f64::from), 10000.0)?;
    measure(server.host_median_latency_ms, 60000.0)?;
    packages(&server.components, 64)?;
    packages(&server.required_mods, 128)?;
    if let Some(join) = &server.join { validate_join(join)?; }
    Ok(())
}

async fn fetch<T: DeserializeOwned>(suffix: &str, limit: usize) -> Result<T, String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .user_agent(concat!("Modstoke-Manager/", env!("CARGO_PKG_VERSION")))
        .build().map_err(|_| "Could not initialize the directory connection.")?;
    let mut response = client.get(format!("{API}{suffix}")).send().await
        .map_err(|_| "Could not reach the community directory. Check your connection and try again.")?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err("This server is no longer listed in the community directory.".into());
    }
    if !response.status().is_success() {
        return Err(format!("The community directory returned HTTP {}. Try again later.", response.status().as_u16()));
    }
    if response.content_length().is_some_and(|size| size > limit as u64) {
        return Err("The community directory response is too large.".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| "Could not read the directory response.")? {
        if bytes.len() + chunk.len() > limit { return Err("The community directory response is too large.".into()); }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| "The community directory returned an unsupported response.".into())
}

#[tauri::command]
pub async fn list_community_servers() -> Result<ServerList, String> {
    let result: ServerList = fetch("/servers", 2 * 1024 * 1024).await?;
    if result.schema_version != 1 || result.servers.len() > 1000 {
        return Err("The directory version or server count is unsupported.".into());
    }
    timestamp(&result.generated_at)?;
    let mut ids = HashSet::new();
    for server in &result.servers {
        validate_server(server)?;
        if !ids.insert(server.id.to_lowercase()) { return Err("The directory returned duplicate server IDs.".into()); }
    }
    Ok(result)
}

#[tauri::command]
pub async fn get_community_server(server_id: String) -> Result<CommunityServer, String> {
    if !valid_id(&server_id) { return Err("Invalid community server ID.".into()); }
    let result: ServerDetail = fetch(&format!("/servers/{server_id}"), 64 * 1024).await?;
    if result.schema_version != 1 || !result.server.id.eq_ignore_ascii_case(&server_id) {
        return Err("The directory returned an unexpected server.".into());
    }
    validate_server(&result.server)?;
    Ok(result.server)
}

#[tauri::command]
pub async fn get_community_server_history(server_id: String) -> Result<ServerHistory, String> {
    if !valid_id(&server_id) { return Err("Invalid community server ID.".into()); }
    let result: ServerHistory = fetch(&format!("/servers/{server_id}/history"), 256 * 1024).await?;
    if result.schema_version != 1 || !result.server_id.eq_ignore_ascii_case(&server_id) || result.samples.len() > 744 {
        return Err("The directory returned unsupported server history.".into());
    }
    for sample in &result.samples {
        timestamp(&sample.at)?;
        measure(sample.players, 10000.0)?;
        measure(sample.latency_ms, 60000.0)?;
    }
    Ok(result)
}

#[derive(Default)]
pub struct PendingJoin(Mutex<Option<String>>);

// Validate the original argument, before any URL parser can normalize a path.
pub fn receive_arguments(app: &tauri::AppHandle, arguments: &[String]) {
    if arguments.len() != 2 { return; }
    let Some(id) = arguments[1].strip_prefix("modstoke://join/") else { return; };
    if !valid_id(id) { return; }
    let state = app.state::<PendingJoin>();
    if let Ok(mut pending) = state.0.lock() {
        *pending = Some(id.to_lowercase());
    }
    let _ = app.emit("community-join-pending", ());
}

#[tauri::command]
pub fn take_pending_community_join(state: tauri::State<'_, PendingJoin>) -> Result<Option<String>, String> {
    state.0.lock().map(|mut pending| pending.take()).map_err(|_| "Could not read the requested server.".into())
}

#[tauri::command]
pub async fn join_community_server(app: tauri::AppHandle, expected_server: CommunityServer, game_root: String) -> Result<String, String> {
    validate_server(&expected_server)?;
    let fresh = get_community_server(expected_server.id.clone()).await?;
    if !fresh.online { return Err("This server is offline. Refresh before trying again.".into()); }
    if fresh.name != expected_server.name || fresh.host_label != expected_server.host_label
        || fresh.join != expected_server.join || fresh.required_mods != expected_server.required_mods
        || fresh.components != expected_server.components || fresh.settings != expected_server.settings
        || fresh.game_build != expected_server.game_build {
        return Err("The server details changed. Refresh and review them before joining.".into());
    }
    let join = fresh.join.ok_or("This server has not advertised a join target.")?;
    validate_join(&join)?;
    let selected = crate::installation_profiles::read_installation_profiles(app).await?.selected
        .ok_or("Select your player installation before joining.")?;
    if !selected.available || selected.target.kind != crate::installation_profiles::TargetKind::Player
        || selected.root.as_deref() != Some(game_root.as_str()) {
        return Err("The selected player installation changed or is unavailable. Review it before joining.".into());
    }
    let steam_game = crate::steam::get_steam_path().await.ok_or("Could not verify Steam's game installation. Open Steam and try again.")?;
    tauri::async_runtime::spawn_blocking(move || {
        let root = std::fs::canonicalize(&game_root).map_err(|_| "The player installation is unavailable.")?;
        let steam_root = Path::new(&steam_game).parent().and_then(|path| std::fs::canonicalize(path).ok())
            .ok_or("Could not verify Steam's game installation.")?;
        if root != steam_root { return Err("Select Steam's Sons of the Forest installation before joining. Steam launches its registered copy.".into()); }
        crate::mod_update::validate_directory(&root)?;
        if !crate::mod_update::validate_target(&root, Path::new("SonsOfTheForest.exe"))? {
            return Err("The player executable is unavailable.".into());
        }
        let _lock = crate::mod_update::acquire_update_lock(&root)?;
        crate::mod_profile::require_launch_ready(&root)?;
        if !fresh.required_mods.is_empty() {
            let inventory = crate::native_inventory::read_inventory(&game_root)?;
            for required in &fresh.required_mods {
                let matching: Vec<_> = inventory.packages.iter().filter(|item| item.id.to_lowercase() == required.id.to_lowercase()).collect();
                if matching.len() != 1 || !matches!(matching[0].state.as_str(), "candidate" | "library")
                    || matching[0].version.as_deref() != Some(required.version.as_str()) {
                    return Err(format!("Required package {} {} is missing, different, or could not be verified. Review your installed mods before joining.", required.id, required.version));
                }
            }
        }
        let mut command = std::process::Command::new(crate::steam::steam_executable()?);
        command.args(["-applaunch", crate::steam::SOTF_APP_ID]);
        match join {
            JoinTarget::Lobby { lobby_id } => { command.args(["+connect_lobby", &lobby_id]); }
            JoinTarget::Dedicated { address, port } => { command.args(["+connect", &format!("{address}:{port}")]); }
        }
        command.spawn().map_err(|_| "Steam could not be started. Open Steam and try again.")?;
        Ok("Join request sent to Steam. Continue in the game's connection prompt.".into())
    }).await.map_err(|_| "The join request could not finish.".to_string())?
}
