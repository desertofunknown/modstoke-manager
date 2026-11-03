use reqwest::header::{HeaderMap, HeaderValue};
use serde::Serialize;

const NEXUS_API_BASE: &str = "https://api.nexusmods.com";
const NEXUS_GRAPHQL_ENDPOINT: &str = "https://api.nexusmods.com/v2/graphql";
const NEXUS_GAME_DOMAIN: &str = "sonsoftheforest";
pub(crate) const NEXUS_USER_AGENT: &str = concat!(
    "Modstoke-Manager/",
    env!("CARGO_PKG_VERSION"),
    " (SonsOfTheForest; Windows; Publisher=OpenACAI)"
);
pub(crate) const NEXUS_APPLICATION_NAME: &str = "Modstoke";
pub(crate) const NEXUS_APPLICATION_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Serialize)]
pub struct NexusUser {
    user_id: Option<u64>,
    name: Option<String>,
    profile_url: Option<String>,
    membership_tier: Option<String>,
    is_premium: Option<bool>,
    is_supporter: Option<bool>,
}

#[derive(Serialize)]
pub struct NexusRateLimit {
    hourly_limit: Option<String>,
    hourly_remaining: Option<String>,
    hourly_reset: Option<String>,
    daily_limit: Option<String>,
    daily_remaining: Option<String>,
    daily_reset: Option<String>,
}

#[derive(Serialize)]
pub struct NexusSession {
    is_connected: bool,
    user: Option<NexusUser>,
    rate_limit: Option<NexusRateLimit>,
    error: Option<String>,
}

impl NexusSession {
    fn connected(user: NexusUser, rate_limit: NexusRateLimit) -> Self {
        Self {
            is_connected: true,
            user: Some(user),
            rate_limit: Some(rate_limit),
            error: None,
        }
    }

    fn disconnected(error: Option<String>) -> Self {
        Self {
            is_connected: false,
            user: None,
            rate_limit: None,
            error,
        }
    }
}

#[derive(Serialize)]
pub struct NexusModsResponse {
    mods: serde_json::Value,
    rate_limit: NexusRateLimit,
}

#[derive(Serialize)]
pub struct NexusGraphqlModsResponse {
    mods: serde_json::Value,
    total_count: Option<u64>,
    nodes_count: Option<u64>,
    rate_limit: NexusRateLimit,
}

#[derive(Serialize)]
pub struct NexusGameInfoResponse {
    game: serde_json::Value,
    rate_limit: NexusRateLimit,
}

#[derive(Serialize)]
pub struct NexusModFilesResponse {
    files: serde_json::Value,
    rate_limit: NexusRateLimit,
}

#[derive(Serialize)]
pub struct NexusModDetailsResponse {
    details: serde_json::Value,
    rate_limit: NexusRateLimit,
}

#[derive(Serialize)]
pub struct NexusModDependenciesResponse {
    dependencies: serde_json::Value,
    rate_limit: NexusRateLimit,
}

#[derive(Serialize)]
pub struct NexusModChangelogsResponse {
    changelogs: serde_json::Value,
    rate_limit: NexusRateLimit,
}

#[derive(Serialize)]
pub struct NexusEndorsementsResponse {
    endorsements: serde_json::Value,
    rate_limit: NexusRateLimit,
}

#[derive(Serialize)]
pub struct NexusTrackedModsResponse {
    tracked_mods: serde_json::Value,
    rate_limit: NexusRateLimit,
}

#[derive(Serialize)]
pub struct NexusActionResponse {
    result: serde_json::Value,
    rate_limit: NexusRateLimit,
}

#[tauri::command]
pub async fn nexus_get_session() -> Result<NexusSession, String> {
    crate::nexus_auth::forget_legacy_api_key();
    let token = match crate::nexus_auth::access_token().await {
        Ok(Some(token)) => token,
        Ok(None) => return Ok(NexusSession::disconnected(None)),
        Err(error) => return Ok(NexusSession::disconnected(Some(error))),
    };

    Ok(match validate_nexus_session(&token).await {
        Ok((user, rate_limit)) => NexusSession::connected(user, rate_limit),
        Err(error) => NexusSession::disconnected(Some(error)),
    })
}

#[tauri::command]
pub async fn nexus_fetch_sotf_mods(view: String) -> Result<NexusModsResponse, String> {
    let key = require_nexus_token().await?;
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
        // Trending is the default feed.
        _ => {
            format!("{NEXUS_API_BASE}/v1/games/{NEXUS_GAME_DOMAIN}/mods/trending.json")
        }
    };
    let (mods, rate_limit) = nexus_json(nexus_client(&key)?.get(url), "Nexus request").await?;
    Ok(NexusModsResponse { mods, rate_limit })
}

#[tauri::command]
pub async fn nexus_fetch_sotf_mods_graphql(
    offset: u64,
    count: u64,
) -> Result<NexusGraphqlModsResponse, String> {
    let key = require_nexus_token().await?;
    let query = r#"
        query OpenAcaiSotfMods($filter: ModsFilter, $sort: [ModsSort!], $offset: Int, $count: Int) {
            mods(filter: $filter, sort: $sort, offset: $offset, count: $count) {
                nodesCount
                totalCount
                nodes {
                    modId
                    name
                    summary
                    author
                    category
                    createdAt
                    updatedAt
                    downloads
                    endorsements
                    pictureUrl
                    thumbnailUrl
                    thumbnailLargeUrl
                    version
                    supportsVortex
                    status
                    modCategory {
                        id
                        name
                    }
                    uploader {
                        name
                    }
                }
            }
        }
    "#;
    let page_count = count.clamp(1, 80);
    let payload = serde_json::json!({
        "query": query,
        "variables": {
            "filter": {
                "gameDomainName": [
                    {
                        "value": NEXUS_GAME_DOMAIN,
                        "op": "EQUALS"
                    }
                ]
            },
            "sort": [
                {
                    "updatedAt": {
                        "direction": "DESC"
                    }
                }
            ],
            "offset": offset,
            "count": page_count
        }
    });

    let (mut value, rate_limit) = nexus_json(
        nexus_client(&key)?
            .post(NEXUS_GRAPHQL_ENDPOINT)
            .json(&payload),
        "Nexus catalog sync request",
    )
    .await?;

    if value
        .get("errors")
        .and_then(|errors| errors.as_array())
        .is_some_and(|errors| !errors.is_empty())
    {
        return Err("Nexus could not return the catalog for this account. Try again later or check your connection in Settings.".to_string());
    }

    let mods_page = value
        .get_mut("data")
        .and_then(|data| data.get_mut("mods"))
        .ok_or_else(|| "Nexus catalog sync did not return a mods page.".to_string())?;
    let total_count = mods_page.get("totalCount").and_then(|count| count.as_u64());
    let nodes_count = mods_page.get("nodesCount").and_then(|count| count.as_u64());
    let mods = mods_page
        .get_mut("nodes")
        .filter(|nodes| nodes.is_array())
        .map(serde_json::Value::take)
        .ok_or_else(|| "Nexus catalog sync did not return a valid mods list.".to_string())?;

    Ok(NexusGraphqlModsResponse {
        mods,
        total_count,
        nodes_count,
        rate_limit,
    })
}

#[tauri::command]
pub async fn nexus_fetch_sotf_game_info() -> Result<NexusGameInfoResponse, String> {
    let key = require_nexus_token().await?;
    let url = format!("{NEXUS_API_BASE}/v1/games/{NEXUS_GAME_DOMAIN}.json");
    let (game, rate_limit) =
        nexus_json(nexus_client(&key)?.get(url), "Nexus game metadata request").await?;
    Ok(NexusGameInfoResponse { game, rate_limit })
}

#[tauri::command]
pub async fn nexus_fetch_mod_details(mod_id: u64) -> Result<NexusModDetailsResponse, String> {
    let key = require_nexus_token().await?;
    let url = format!("{NEXUS_API_BASE}/v1/games/{NEXUS_GAME_DOMAIN}/mods/{mod_id}.json");
    let (details, rate_limit) =
        nexus_json(nexus_client(&key)?.get(url), "Nexus mod details request").await?;
    Ok(NexusModDetailsResponse {
        details,
        rate_limit,
    })
}

#[tauri::command]
pub async fn nexus_fetch_mod_files(mod_id: u64) -> Result<NexusModFilesResponse, String> {
    let key = require_nexus_token().await?;
    let url = format!("{NEXUS_API_BASE}/v1/games/{NEXUS_GAME_DOMAIN}/mods/{mod_id}/files.json");
    let (mut value, rate_limit) =
        nexus_json(nexus_client(&key)?.get(url), "Nexus mod files request").await?;
    let files = value
        .get_mut("files")
        .map(serde_json::Value::take)
        .unwrap_or(value);
    Ok(NexusModFilesResponse { files, rate_limit })
}

#[tauri::command]
pub async fn nexus_fetch_file_dependencies(
    file_id: u64,
    nexus_file_id: Option<String>,
) -> Result<NexusModDependenciesResponse, String> {
    let key = require_nexus_token().await?;
    let client = nexus_client(&key)?;
    let supplied_version_id = nexus_file_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());
    let mut failures = Vec::new();

    if let Some(version_id) = supplied_version_id {
        match nexus_request_dependencies_for_version(&client, version_id).await {
            Ok(result) => return Ok(result),
            Err(error) => failures.push(error),
        }
    }

    match nexus_resolve_v3_mod_file_version_id(&client, file_id).await {
        Ok(version_id) if Some(version_id.as_str()) != supplied_version_id => {
            match nexus_request_dependencies_for_version(&client, &version_id).await {
                Ok(result) => return Ok(result),
                Err(error) => failures.push(error),
            }
        }
        Ok(_) => {}
        Err(error) => failures.push(error),
    }

    Err(format!(
        "Could not read Nexus dependencies for game file {file_id}. {}",
        failures.join("; ")
    ))
}

async fn nexus_request_dependencies_for_version(
    client: &reqwest::Client,
    version_id: &str,
) -> Result<NexusModDependenciesResponse, String> {
    let mut first_empty_result = None;
    let mut failures = Vec::new();

    for route in ["ranges/materialized", "ranges"] {
        match nexus_request_dependency_route(client, version_id, route).await {
            Ok((mut value, rate_limit)) => {
                let has_rows = nexus_dependency_payload_has_rows(&value);
                let result = NexusModDependenciesResponse {
                    dependencies: value
                        .get_mut("dependencies")
                        .map(serde_json::Value::take)
                        .unwrap_or(value),
                    rate_limit,
                };
                if has_rows {
                    return Ok(result);
                }
                first_empty_result.get_or_insert(result);
            }
            Err(error) => failures.push(error),
        }
    }

    first_empty_result.ok_or_else(|| failures.join("; "))
}

fn nexus_dependency_payload_has_rows(value: &serde_json::Value) -> bool {
    nexus_dependency_container_has_rows(value, 0)
}

fn nexus_dependency_container_has_rows(value: &serde_json::Value, depth: usize) -> bool {
    if depth > 4 {
        return false;
    }

    if value.as_array().is_some_and(|rows| !rows.is_empty()) {
        return true;
    }

    let Some(object) = value.as_object() else {
        return false;
    };

    const ROW_KEYS: &[&str] = &[
        "dependencies",
        "dependency_definitions",
        "dependencyDefinitions",
        "dependency_definition",
        "dependencyDefinition",
        "dependency_ranges",
        "dependencyRanges",
        "range_definitions",
        "rangeDefinitions",
        "candidate_groups",
        "candidateGroups",
        "ranges",
        "items",
        "results",
        "nodes",
        "edges",
    ];
    const WRAPPER_KEYS: &[&str] = &[
        "data",
        "result",
        "payload",
        "response",
        "file_dependencies",
        "fileDependencies",
        "mod_file_dependencies",
        "modFileDependencies",
        "mod_file_dependency_response",
        "modFileDependencyResponse",
        "modFileDependencyRangesResponse",
        "modFileDependencyMaterializedResponse",
        "mod_file_dependency_ranges_response",
        "mod_file_dependency_materialized_response",
    ];

    ROW_KEYS
        .iter()
        .chain(WRAPPER_KEYS.iter())
        .any(|key| {
            object
                .get(*key)
                .is_some_and(|child| nexus_dependency_container_has_rows(child, depth + 1))
        })
}

async fn nexus_resolve_v3_mod_file_version_id(
    client: &reqwest::Client,
    game_scoped_file_id: u64,
) -> Result<String, String> {
    let url = format!(
        "{NEXUS_API_BASE}/v3/games/{NEXUS_GAME_DOMAIN}/mod-file-versions/{game_scoped_file_id}"
    );
    let context = format!("Version lookup for game file {game_scoped_file_id}");
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("{context} failed: {error}"))?;

    if !response.status().is_success() {
        return Err(format!("{context} failed: {}", response.status()));
    }

    let value = response
        .json::<serde_json::Value>()
        .await
        .map_err(|error| format!("{context} returned invalid JSON: {error}"))?;
    value
        .get("data")
        .and_then(|data| data.get("id"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("{context} returned no global version ID"))
}

async fn nexus_request_dependency_route(
    client: &reqwest::Client,
    version_id: &str,
    route: &str,
) -> Result<(serde_json::Value, NexusRateLimit), String> {
    let mut url = reqwest::Url::parse(NEXUS_API_BASE).expect("Nexus API base URL is valid");
    url.path_segments_mut()
        .expect("Nexus API URL supports path segments")
        .extend(["v3", "mod-file-versions", version_id, "dependencies"])
        .extend(route.split('/'));
    let context = format!("Dependencies ({route}) for version {version_id}");
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("{context} failed: {error}"))?;
    let rate_limit = read_rate_limit(response.headers());

    if !response.status().is_success() {
        return Err(format!("{context} failed: {}", response.status()));
    }

    let value = response
        .json::<serde_json::Value>()
        .await
        .map_err(|error| format!("{context} returned invalid JSON: {error}"))?;

    Ok((value, rate_limit))
}

#[tauri::command]
pub async fn nexus_fetch_mod_changelogs(mod_id: u64) -> Result<NexusModChangelogsResponse, String> {
    let key = require_nexus_token().await?;
    let url =
        format!("{NEXUS_API_BASE}/v1/games/{NEXUS_GAME_DOMAIN}/mods/{mod_id}/changelogs.json");
    let (changelogs, rate_limit) =
        nexus_json(nexus_client(&key)?.get(url), "Nexus changelog request").await?;
    Ok(NexusModChangelogsResponse {
        changelogs,
        rate_limit,
    })
}

#[tauri::command]
pub async fn nexus_fetch_user_endorsements() -> Result<NexusEndorsementsResponse, String> {
    let key = require_nexus_token().await?;
    let (endorsements, rate_limit) = nexus_json(
        nexus_client(&key)?.get(format!("{NEXUS_API_BASE}/v1/user/endorsements.json")),
        "Nexus endorsements request",
    )
    .await?;
    Ok(NexusEndorsementsResponse {
        endorsements,
        rate_limit,
    })
}

#[tauri::command]
pub async fn nexus_fetch_user_tracked_mods() -> Result<NexusTrackedModsResponse, String> {
    let key = require_nexus_token().await?;
    let (tracked_mods, rate_limit) = nexus_json(
        nexus_client(&key)?.get(format!("{NEXUS_API_BASE}/v1/user/tracked_mods.json")),
        "Nexus tracked mods request",
    )
    .await?;
    Ok(NexusTrackedModsResponse {
        tracked_mods,
        rate_limit,
    })
}

#[tauri::command]
pub async fn nexus_endorse_sotf_mod(
    mod_id: u64,
    version: Option<String>,
) -> Result<NexusActionResponse, String> {
    let key = require_nexus_token().await?;
    let url = format!("{NEXUS_API_BASE}/v1/games/{NEXUS_GAME_DOMAIN}/mods/{mod_id}/endorse.json");
    let mut form: Vec<(&str, String)> = Vec::new();
    if let Some(version) = version
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        form.push(("version", version));
    }

    let (response, rate_limit) = nexus_send(
        nexus_client(&key)?.post(url).form(&form),
        "Nexus endorse request",
    )
    .await?;
    let result = read_json_or_empty(response).await?;
    Ok(NexusActionResponse { result, rate_limit })
}

#[tauri::command]
pub async fn nexus_track_sotf_mod(mod_id: u64) -> Result<NexusActionResponse, String> {
    nexus_set_sotf_tracking(mod_id, true).await
}

#[tauri::command]
pub async fn nexus_untrack_sotf_mod(mod_id: u64) -> Result<NexusActionResponse, String> {
    nexus_set_sotf_tracking(mod_id, false).await
}

async fn nexus_set_sotf_tracking(
    mod_id: u64,
    should_track: bool,
) -> Result<NexusActionResponse, String> {
    let key = require_nexus_token().await?;
    let form = vec![("mod_id", mod_id.to_string())];
    let request = nexus_client(&key)?
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
    let action = if should_track { "track" } else { "untrack" };
    let (response, rate_limit) = nexus_send(request, &format!("Nexus {action} request")).await?;
    let result = read_json_or_empty(response).await?;
    Ok(NexusActionResponse { result, rate_limit })
}

// Nexus checks the access token itself here, including whether the player has revoked it.
async fn validate_nexus_session(access_token: &str) -> Result<(NexusUser, NexusRateLimit), String> {
    let (value, rate_limit) = nexus_json(
        nexus_client(access_token)?.get(format!("{NEXUS_API_BASE}/v1/users/validate.json")),
        "Nexus account validation",
    )
    .await?;
    let user_id = value
        .get("user_id")
        .and_then(|value| value.as_u64())
        .filter(|id| *id > 0 && *id <= 9_007_199_254_740_991)
        .ok_or_else(|| "Nexus did not return a valid account identity.".to_string())?;
    let user = NexusUser {
        user_id: Some(user_id),
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

fn nexus_client(access_token: &str) -> Result<reqwest::Client, String> {
    let mut headers = HeaderMap::new();
    let mut authorization = HeaderValue::from_str(&format!("Bearer {access_token}"))
        .map_err(|_| "Nexus Mods returned an unusable sign-in. Sign in again.".to_string())?;
    authorization.set_sensitive(true);
    headers.insert("Authorization", authorization);
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
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Could not initialize the Nexus API connection.".to_string())
}

async fn nexus_send(
    request: reqwest::RequestBuilder,
    context: &str,
) -> Result<(reqwest::Response, NexusRateLimit), String> {
    let response = request.send().await.map_err(|e| e.to_string())?;
    let rate_limit = read_rate_limit(response.headers());

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        crate::nexus_auth::forget_access_token().await;
        return Err(format!("{context} failed: Nexus Mods did not accept the sign-in. Try again, or sign in again if this keeps happening."));
    }
    if !response.status().is_success() {
        return Err(format!("{context} failed: {}", response.status()));
    }

    Ok((response, rate_limit))
}

async fn nexus_json(
    request: reqwest::RequestBuilder,
    context: &str,
) -> Result<(serde_json::Value, NexusRateLimit), String> {
    let (response, rate_limit) = nexus_send(request, context).await?;
    let value = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;
    Ok((value, rate_limit))
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

async fn require_nexus_token() -> Result<String, String> {
    crate::nexus_auth::access_token().await?.ok_or_else(|| "Sign in with Nexus Mods first.".to_string())
}

// Hands a Nexus file link to the mod manager the player registered for nxm links, such as Vortex.
#[tauri::command]
pub fn open_nxm_link(url: String) -> Result<(), String> {
    let link = regex::Regex::new(r"^nxm://sonsoftheforest/mods/\d{1,10}/files/\d{1,12}$").unwrap();
    if !link.is_match(&url) {
        return Err("This is not a Sons of the Forest file link from Nexus Mods.".into());
    }
    crate::system::shell_execute("open", std::ffi::OsStr::new(&url))
        .map_err(|_| "No program is registered for Nexus download links. Install Vortex or another mod manager that handles them.".into())
}
