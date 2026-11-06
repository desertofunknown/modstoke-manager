use crate::admin_paths::{FOLDER, LOCK_FILE, NODES_FILE, POLICY_FILE, STATUS_FILE};
use crate::hashing::sha256_hex;
use crate::installation_profiles::{self, required_nullable};
use crate::{native_root, safe_fs};
use serde::{Deserialize, Serialize};
use serde_json::ser::{CharEscape, Formatter, PrettyFormatter};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};
use tauri::Manager;

// Modstoke Admin keeps its data next to settings.ini in the plugin folder, on a listen host and
// on a dedicated server alike. Admin owns permission-nodes.json and permissions-status.json.
const ADMIN_MANIFEST: &str = "Mods/Modstoke.Admin/openacai.mod.json";
const FORMAT: &str = "modstoke.admin.permissions";
const STATUS_FORMAT: &str = "modstoke.admin.permissions-status";
const MAX_POLICY_BYTES: usize = 64 * 1024;
const MAX_NODES_BYTES: usize = 1024 * 1024;
const MAX_STATUS_BYTES: usize = 64 * 1024;
const BACKUP_FOLDER: &str = "admin-permission-backups";
const MAX_BACKUPS: usize = 32;
const RESERVED_ROLES: [&str; 4] = ["host", "none", "banned", "everyone"];
const MAX_RANK: u32 = 999;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Policy {
    format: String,
    schema_version: u32,
    defaults: Defaults,
    game_role_map: GameRoleMap,
    roles: Vec<Role>,
    assignments: Vec<Assignment>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Defaults {
    remote_operators: bool,
    remote_minimum_rank: u32,
    #[serde(deserialize_with = "required_nullable")]
    unassigned_role: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct GameRoleMap {
    #[serde(deserialize_with = "required_nullable")]
    owner: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    admin: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    trusted: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    guest: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Role {
    id: String,
    name: String,
    rank: u32,
    inherits: Vec<String>,
    allow: Vec<String>,
    deny: Vec<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Assignment {
    steam_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    allow: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    deny: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_utc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    added_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    added_utc: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionNode {
    node: String,
    #[serde(default)]
    group: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    sensitive: bool,
    #[serde(default)]
    host_only: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum NodeCatalog {
    List(Vec<PermissionNode>),
    Wrapped { nodes: Vec<PermissionNode> },
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminStatus {
    #[serde(skip_serializing)]
    format: String,
    #[serde(skip_serializing)]
    schema_version: u32,
    sha256: Option<String>,
    valid: bool,
    error: Option<String>,
    #[serde(default)]
    warnings: Vec<String>,
    active_sha256: Option<String>,
    checked_utc: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsState {
    installation_id: String,
    folder: String,
    path: String,
    state: &'static str,
    revision: Option<String>,
    policy: Option<Policy>,
    problems: Vec<String>,
    warnings: Vec<String>,
    nodes: Vec<PermissionNode>,
    nodes_problem: Option<String>,
    status: Option<AdminStatus>,
    status_problem: Option<String>,
    admin_version: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyCheck {
    problems: Vec<String>,
    warnings: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsSave {
    changed: bool,
    revision: String,
    backup_path: Option<String>,
    state: PermissionsState,
}

#[tauri::command]
pub async fn read_admin_permissions(executable: String, installation_id: String) -> Result<PermissionsState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = require_installation(&executable, &installation_id)?;
        Ok(snapshot(Path::new(&root), &installation_id))
    })
    .await
    .map_err(|error| format!("Could not read staff permissions: {error}"))?
}

#[tauri::command]
pub async fn check_admin_permissions(executable: String, installation_id: String, policy: Policy) -> Result<PolicyCheck, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = require_installation(&executable, &installation_id)?;
        let (nodes, _) = read_nodes(&Path::new(&root).join(FOLDER));
        let (mut problems, warnings) = check(&policy, nodes.as_deref());
        if problems.is_empty() && canonical(&policy)?.len() > MAX_POLICY_BYTES {
            problems.push("The permissions file would be larger than 64 KiB. Remove some roles, rules or assignments.".into());
        }
        Ok(PolicyCheck { problems, warnings })
    })
    .await
    .map_err(|error| format!("Could not check staff permissions: {error}"))?
}

#[tauri::command]
pub async fn write_admin_permissions(
    app: tauri::AppHandle,
    executable: String,
    installation_id: String,
    expected_revision: String,
    policy: Policy,
) -> Result<PermissionsSave, String> {
    let backups = app.path().app_config_dir()
        .map_err(|error| format!("Could not find Manager's settings folder: {error}"))?
        .join(BACKUP_FOLDER);
    tauri::async_runtime::spawn_blocking(move || {
        write_policy(&backups, &executable, &installation_id, &expected_revision, &policy)
    })
    .await
    .map_err(|error| format!("Could not save staff permissions: {error}"))?
}

fn require_installation(executable: &str, installation_id: &str) -> Result<String, String> {
    let selected = installation_profiles::require_selected(executable, installation_id, None,
        "The installation is unavailable.",
        "The installation no longer matches its saved identity. Select it again before changing staff permissions.")?;
    selected.root.ok_or_else(|| "The installation has no resolved folder.".into())
}

// Short-lived and matched immediately, so the large Parsed variant isn't boxed.
#[allow(clippy::large_enum_variant)]
enum Stored {
    Missing,
    Unreadable(String),
    Unsupported(String),
    Parsed { revision: String, policy: Policy },
}

fn read_policy(folder: &Path) -> Stored {
    let bytes = match safe_fs::read_bounded(folder, Path::new(POLICY_FILE), MAX_POLICY_BYTES) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return Stored::Missing,
        Err(problem) => return Stored::Unreadable(problem),
    };
    let revision = sha256_hex(&bytes);
    let body = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes);
    // Look at the format first, so a newer file gets a version message rather than a field error.
    let header: serde_json::Value = match serde_json::from_slice(body) {
        Ok(value) => value,
        Err(error) => return Stored::Unreadable(format!("permissions.json is not valid JSON: {error}.")),
    };
    if header.get("format").and_then(|value| value.as_str()) != Some(FORMAT) {
        return Stored::Unreadable("permissions.json is not a Modstoke Admin permissions file.".into());
    }
    match header.get("schemaVersion").and_then(|value| value.as_u64()) {
        Some(1) => {}
        Some(version) if version > 1 => return Stored::Unsupported(format!(
            "permissions.json uses schema version {version}, which this Manager does not know. Update Manager to edit it; the file was left as it is.")),
        _ => return Stored::Unreadable("permissions.json has no supported schemaVersion.".into()),
    }
    match serde_json::from_slice(body) {
        Ok(policy) => Stored::Parsed { revision, policy },
        Err(error) => Stored::Unreadable(format!("permissions.json does not match the permissions format: {error}.")),
    }
}

fn read_nodes(folder: &Path) -> (Option<Vec<PermissionNode>>, Option<String>) {
    let bytes = match safe_fs::read_bounded(folder, Path::new(NODES_FILE), MAX_NODES_BYTES) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return (None, None),
        Err(problem) => return (None, Some(problem)),
    };
    match serde_json::from_slice::<NodeCatalog>(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes)) {
        Ok(NodeCatalog::List(nodes) | NodeCatalog::Wrapped { nodes }) => (Some(nodes), None),
        Err(error) => (None, Some(format!("permission-nodes.json could not be read: {error}."))),
    }
}

fn read_status(folder: &Path) -> (Option<AdminStatus>, Option<String>) {
    let bytes = match safe_fs::read_bounded(folder, Path::new(STATUS_FILE), MAX_STATUS_BYTES) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return (None, None),
        Err(problem) => return (None, Some(problem)),
    };
    match serde_json::from_slice::<AdminStatus>(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes)) {
        Ok(status) if status.format == STATUS_FORMAT && status.schema_version == 1 => (Some(status), None),
        Ok(_) => (None, Some("permissions-status.json has a format this Manager does not know.".into())),
        Err(error) => (None, Some(format!("permissions-status.json could not be read: {error}."))),
    }
}

fn installed_admin(root: &Path) -> Option<String> {
    let manifest = format!("{}/{ADMIN_MANIFEST}", native_root::resolve_native_root(root, "Mods"));
    let bytes = safe_fs::read_bounded(root, Path::new(&manifest), 64 * 1024).ok()??;
    let manifest: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    manifest.get("version")?.as_str().map(str::to_owned)
}

fn snapshot(root: &Path, installation_id: &str) -> PermissionsState {
    let folder = root.join(FOLDER);
    let mut state = PermissionsState {
        installation_id: installation_id.into(),
        folder: folder.to_string_lossy().into_owned(),
        path: folder.join(POLICY_FILE).to_string_lossy().into_owned(),
        state: "no-folder",
        revision: None,
        policy: None,
        problems: Vec::new(),
        warnings: Vec::new(),
        nodes: Vec::new(),
        nodes_problem: None,
        status: None,
        status_problem: None,
        admin_version: installed_admin(root),
    };
    match safe_fs::validate_directory(&folder) {
        Ok(()) => {}
        Err(_) if !folder.exists() => return state,
        Err(problem) => {
            state.state = "unreadable";
            state.problems.push(problem);
            return state;
        }
    }
    let (nodes, nodes_problem) = read_nodes(&folder);
    (state.status, state.status_problem) = read_status(&folder);
    state.nodes_problem = nodes_problem;
    match read_policy(&folder) {
        Stored::Missing => state.state = "missing",
        Stored::Unreadable(problem) => {
            state.state = "unreadable";
            state.problems.push(problem);
        }
        Stored::Unsupported(problem) => {
            state.state = "unsupported";
            state.problems.push(problem);
        }
        Stored::Parsed { revision, policy } => {
            (state.problems, state.warnings) = check(&policy, nodes.as_deref());
            state.state = if state.problems.is_empty() { "valid" } else { "invalid" };
            state.revision = Some(revision);
            state.policy = Some(policy);
        }
    }
    state.nodes = nodes.unwrap_or_default();
    state
}

fn valid_role_id(id: &str) -> bool {
    let mut characters = id.chars();
    characters.next().is_some_and(|first| first.is_ascii_lowercase())
        && id.len() <= 32
        && characters.all(|character| character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-')
}

// Segments of [a-z0-9][a-z0-9-]{0,31} joined by dots, with an optional trailing ".*", or a bare "*".
fn valid_pattern(pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let exact = pattern.strip_suffix(".*").unwrap_or(pattern);
    exact.split('.').all(|segment| {
        (1..=32).contains(&segment.len())
            && segment.starts_with(|character: char| character.is_ascii_lowercase() || character.is_ascii_digit())
            && segment.chars().all(|character| character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-')
    })
}

// The same individual-account range Admin accepts, with no trimming or cleaning.
fn valid_steam_id(value: &str) -> bool {
    value.len() == 17
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && value.parse::<u64>().is_ok_and(|id| (76561197960265728..=76561202255233023).contains(&id))
}

// UTC in the form yyyy-MM-ddTHH:mm:ssZ.
fn valid_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 20 || bytes[4] != b'-' || bytes[7] != b'-' || bytes[10] != b'T'
        || bytes[13] != b':' || bytes[16] != b':' || bytes[19] != b'Z' {
        return false;
    }
    let number = |range: std::ops::Range<usize>| -> Option<u32> {
        let text = &value[range];
        text.bytes().all(|byte| byte.is_ascii_digit()).then(|| text.parse().ok()).flatten()
    };
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) =
        (number(0..4), number(5..7), number(8..10), number(11..13), number(14..16), number(17..19)) else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    year >= 1 && (1..=days).contains(&day) && hour < 24 && minute < 60 && second < 60
}

// Text limits count code points, as Admin does.
fn printable(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max && !value.chars().any(char::is_control)
}

fn check(policy: &Policy, nodes: Option<&[PermissionNode]>) -> (Vec<String>, Vec<String>) {
    let mut problems = Vec::new();
    let mut warnings = Vec::new();
    if policy.format != FORMAT {
        problems.push(format!("format must be \"{FORMAT}\"."));
    }
    if policy.schema_version != 1 {
        problems.push("schemaVersion must be 1.".into());
    }
    if policy.defaults.remote_minimum_rank > MAX_RANK {
        problems.push("The lowest rank for remote F7 must be from 0 to 999.".into());
    }
    match policy.roles.len() {
        0 => problems.push("There are no roles. Admin needs at least one.".into()),
        count if count > 32 => problems.push(format!("There are {count} roles; the limit is 32.")),
        _ => {}
    }

    let catalog: Option<HashMap<&str, &PermissionNode>> =
        nodes.map(|nodes| nodes.iter().map(|node| (node.node.as_str(), node)).collect());
    let mut check_rules = |owner: &str, allow: &[String], deny: &[String], exact_allow: bool, limit: usize,
        problems: &mut Vec<String>| {
        for (list, rules) in [("allow", allow), ("deny", deny)] {
            if rules.len() > limit {
                problems.push(format!("{owner} has {} {list} entries; the limit is {limit}.", rules.len()));
            }
            let mut listed = HashSet::new();
            for rule in rules {
                if !listed.insert(rule) {
                    problems.push(format!("{owner}: {rule} is in the {list} list more than once."));
                }
                if !valid_pattern(rule) {
                    problems.push(format!("{owner}: \"{rule}\" is not a permission node or pattern."));
                    continue;
                }
                let wildcard = rule.ends_with('*');
                if list == "allow" && exact_allow && wildcard {
                    problems.push(format!("{owner}: extra allow entries must name exact nodes, not \"{rule}\"."));
                    continue;
                }
                let Some(catalog) = &catalog else { continue };
                if wildcard {
                    continue;
                }
                match catalog.get(rule.as_str()) {
                    None if list == "allow" => warnings.push(format!("{owner}: Admin does not know {rule}, so it ignores this entry.")),
                    None => warnings.push(format!("{owner}: Admin does not know {rule}. The deny entry is kept but matches nothing yet.")),
                    Some(node) if node.host_only && list == "allow" =>
                        warnings.push(format!("{owner}: {rule} is a host-only tool and can't be granted.")),
                    Some(_) => {}
                }
            }
        }
    };

    let mut ranks: HashMap<u32, &str> = HashMap::new();
    let roles: HashMap<&str, &Role> = policy.roles.iter().map(|role| (role.id.as_str(), role)).collect();
    let mut seen = HashSet::new();
    for role in &policy.roles {
        let id = role.id.as_str();
        if !valid_role_id(id) {
            problems.push(format!("Role id \"{id}\" must start with a lowercase letter and use only lowercase letters, digits and hyphens, up to 32 characters."));
        } else if RESERVED_ROLES.contains(&id) {
            problems.push(format!("Role id \"{id}\" is reserved by Admin."));
        }
        if !seen.insert(id) {
            problems.push(format!("Role id \"{id}\" is used by more than one role."));
        }
        if !printable(&role.name, 48) || role.name.trim() != role.name {
            problems.push(format!("Role {id}: the name must be 1 to 48 printable characters, without spaces at either end."));
        }
        if role.rank > MAX_RANK {
            problems.push(format!("Role {id}: the rank must be from 0 to 999."));
        } else if let Some(other) = ranks.insert(role.rank, id) {
            problems.push(format!("Roles {other} and {id} both have rank {}. Each role needs its own rank.", role.rank));
        }
        if role.inherits.len() > 8 {
            problems.push(format!("Role {id} inherits {} roles; the limit is 8.", role.inherits.len()));
        }
        let mut inherited = HashSet::new();
        for parent in &role.inherits {
            if !inherited.insert(parent) {
                problems.push(format!("Role {id} inherits {parent} more than once."));
            }
            match roles.get(parent.as_str()) {
                None => problems.push(format!("Role {id} inherits {parent}, which does not exist.")),
                // A strictly lower rank also rules out cycles.
                Some(other) if other.rank >= role.rank =>
                    problems.push(format!("Role {id} can only inherit roles ranked below it, and {parent} has rank {}.", other.rank)),
                Some(_) => {}
            }
        }
        check_rules(&format!("Role {id}"), &role.allow, &role.deny, false, 128, &mut problems);
    }

    let mapped = [
        ("unassigned players", &policy.defaults.unassigned_role),
        ("the game's Owner role", &policy.game_role_map.owner),
        ("the game's Admin role", &policy.game_role_map.admin),
        ("the game's Trusted role", &policy.game_role_map.trusted),
        ("the game's Guest role", &policy.game_role_map.guest),
    ];
    for (label, role) in mapped {
        if let Some(role) = role {
            if !roles.contains_key(role.as_str()) {
                problems.push(format!("The role for {label}, {role}, does not exist."));
            }
        }
    }
    // Game roles map from the top down, and a lower game role never gets a higher-ranked role.
    let order = [
        ("Owner", &policy.game_role_map.owner),
        ("Admin", &policy.game_role_map.admin),
        ("Trusted", &policy.game_role_map.trusted),
        ("Guest", &policy.game_role_map.guest),
    ];
    for pair in order.windows(2) {
        let [(above_label, above), (label, role)] = pair else { continue };
        let Some(role) = role else { continue };
        match above {
            None => problems.push(format!("The game's {label} role can only be mapped when the game's {above_label} role is mapped too.")),
            Some(above) => if let (Some(lower), Some(upper)) = (roles.get(role.as_str()), roles.get(above.as_str())) {
                if lower.rank > upper.rank {
                    problems.push(format!("The game's {label} role maps to {role} (rank {}), which ranks above {above} (rank {}) for the game's {above_label} role.", lower.rank, upper.rank));
                }
            },
        }
    }

    if policy.assignments.len() > 512 {
        problems.push(format!("There are {} assignments; the limit is 512.", policy.assignments.len()));
    }
    let mut players = HashSet::new();
    for assignment in &policy.assignments {
        let steam_id = assignment.steam_id.as_str();
        if !valid_steam_id(steam_id) {
            problems.push(format!("\"{steam_id}\" is not a SteamID64. Use the 17-digit number from the player's Steam profile."));
        } else if !players.insert(steam_id) {
            problems.push(format!("{steam_id} has more than one assignment."));
        }
        match &assignment.role {
            Some(role) if !roles.contains_key(role.as_str()) =>
                problems.push(format!("{steam_id} is assigned {role}, which does not exist.")),
            None if assignment.allow.is_empty() && assignment.deny.is_empty() =>
                problems.push(format!("{steam_id} needs a role or at least one extra allow or deny entry.")),
            _ => {}
        }
        check_rules(steam_id, &assignment.allow, &assignment.deny, true, 16, &mut problems);
        if assignment.note.as_deref().is_some_and(|note| note.chars().count() > 120) {
            problems.push(format!("{steam_id}: the note must be up to 120 characters."));
        }
        if assignment.added_by.as_deref().is_some_and(|added_by| !added_by.is_empty() && !printable(added_by, 64)) {
            problems.push(format!("{steam_id}: addedBy must be up to 64 printable characters."));
        }
        for (label, value) in [("expiry", &assignment.expires_utc), ("added time", &assignment.added_utc)] {
            if value.as_deref().is_some_and(|value| !valid_timestamp(value)) {
                problems.push(format!("{steam_id}: the {label} must be a UTC time like 2026-12-31T00:00:00Z."));
            }
        }
    }
    (problems, warnings)
}

// Admin writes with System.Text.Json's default encoder: two-space indent, LF, and
// HTML-sensitive and non-ASCII characters as uppercase \u escapes. Matching it byte for byte
// keeps the hash both tools compare stable for the same policy.
struct AdminFormatter(PrettyFormatter<'static>);

impl Formatter for AdminFormatter {
    fn begin_array<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.begin_array(writer)
    }
    fn end_array<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.end_array(writer)
    }
    fn begin_array_value<W: ?Sized + Write>(&mut self, writer: &mut W, first: bool) -> io::Result<()> {
        self.0.begin_array_value(writer, first)
    }
    fn end_array_value<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.end_array_value(writer)
    }
    fn begin_object<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.begin_object(writer)
    }
    fn end_object<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.end_object(writer)
    }
    fn begin_object_key<W: ?Sized + Write>(&mut self, writer: &mut W, first: bool) -> io::Result<()> {
        self.0.begin_object_key(writer, first)
    }
    fn begin_object_value<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.begin_object_value(writer)
    }
    fn end_object_value<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.end_object_value(writer)
    }

    fn write_string_fragment<W: ?Sized + Write>(&mut self, writer: &mut W, fragment: &str) -> io::Result<()> {
        let mut start = 0;
        for (index, character) in fragment.char_indices() {
            if matches!(character, ' '..='~') && !matches!(character, '<' | '>' | '&' | '\'' | '+' | '`') {
                continue;
            }
            writer.write_all(&fragment.as_bytes()[start..index])?;
            for unit in character.encode_utf16(&mut [0; 2]) {
                write!(writer, "\\u{unit:04X}")?;
            }
            start = index + character.len_utf8();
        }
        writer.write_all(&fragment.as_bytes()[start..])
    }

    fn write_char_escape<W: ?Sized + Write>(&mut self, writer: &mut W, escape: CharEscape) -> io::Result<()> {
        writer.write_all(match escape {
            CharEscape::Quote => b"\\u0022",
            CharEscape::ReverseSolidus => b"\\\\",
            CharEscape::Solidus => b"/",
            CharEscape::Backspace => b"\\b",
            CharEscape::FormFeed => b"\\f",
            CharEscape::LineFeed => b"\\n",
            CharEscape::CarriageReturn => b"\\r",
            CharEscape::Tab => b"\\t",
            CharEscape::AsciiControl(byte) => return write!(writer, "\\u{byte:04X}"),
        })
    }
}

fn canonical(policy: &Policy) -> Result<Vec<u8>, String> {
    // Admin leaves an assignment's optional text out when it is empty.
    let mut policy = policy.clone();
    for assignment in &mut policy.assignments {
        for value in [&mut assignment.note, &mut assignment.added_by] {
            if value.as_deref() == Some("") {
                *value = None;
            }
        }
    }
    let mut bytes = Vec::new();
    let mut serializer = serde_json::Serializer::with_formatter(&mut bytes, AdminFormatter(PrettyFormatter::new()));
    policy.serialize(&mut serializer)
        .map_err(|error| format!("Could not prepare permissions.json: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn busy(error: &io::Error) -> bool {
    // Sharing, lock and access-denied errors while Admin or an editor holds the file briefly.
    matches!(error.raw_os_error(), Some(5 | 32 | 33))
}

// Admin holds the same lock across its own re-read, compare and replace, and only briefly.
fn lock(folder: &Path) -> Result<File, String> {
    safe_fs::validate_target(folder, Path::new(LOCK_FILE))?;
    let options = safe_fs::exclusive_lock_options();
    let mut attempt = 0;
    let file = loop {
        match options.open(folder.join(LOCK_FILE)) {
            Ok(file) => break file,
            Err(error) if busy(&error) && attempt < 20 => {
                attempt += 1;
                thread::sleep(Duration::from_millis(100));
            }
            Err(error) => return Err(format!("Could not lock permissions.json. Admin may be saving it right now; try again: {error}")),
        }
    };
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if safe_fs::is_linked(&metadata) || !metadata.is_file() {
        return Err("permissions.lock is not an ordinary file.".into());
    }
    #[cfg(not(windows))]
    file.try_lock().map_err(|error| format!("Could not lock permissions.json: {error}"))?;
    Ok(file)
}

fn require_revision(folder: &Path, expected: &str) -> Result<Vec<u8>, String> {
    let bytes = safe_fs::read_bounded(folder, Path::new(POLICY_FILE), MAX_POLICY_BYTES)?
        .ok_or("permissions.json was removed after it was read. Read it again before saving.")?;
    if sha256_hex(&bytes) != expected {
        return Err("permissions.json changed after it was read, in F7, by Admin or in another editor. Read it again before saving; nothing was written.".into());
    }
    Ok(bytes)
}

// Staged without tempfile's usual FILE_ATTRIBUTE_TEMPORARY, which the rename would carry over.
fn stage(folder: &Path, prefix: &str, bytes: &[u8]) -> io::Result<tempfile::TempPath> {
    let mut file = tempfile::Builder::new().prefix(prefix)
        .make_in(folder, |path| OpenOptions::new().read(true).write(true).create_new(true).open(path))?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    Ok(file.into_temp_path())
}

fn backup(directory: &Path, revision: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    fs::create_dir_all(directory)
        .map_err(|error| format!("Could not create the permissions backup folder: {error}"))?;
    safe_fs::validate_directory(directory)?;
    let name = format!("{revision}.{POLICY_FILE}");
    let path = directory.join(&name);
    if safe_fs::validate_target(directory, Path::new(&name))? {
        // Rotation goes by modification time, so a revision backed up again counts as recent.
        let _ = File::options().write(true).open(&path).and_then(|file| file.set_modified(SystemTime::now()));
    } else {
        let temporary = stage(directory, ".backup-", bytes)
            .map_err(|error| format!("Could not write the permissions backup: {error}"))?;
        safe_fs::durable_rename(&temporary, &path, false)?;
    }
    // Backups are Manager's own copies; keep only the newest.
    let mut entries: Vec<(SystemTime, PathBuf)> = fs::read_dir(directory)
        .map_err(|error| format!("Could not inspect the permissions backup folder: {error}"))?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(POLICY_FILE))
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .collect();
    entries.sort();
    for (_, old) in entries.iter().take(entries.len().saturating_sub(MAX_BACKUPS)) {
        if old != &path {
            let _ = fs::remove_file(old);
        }
    }
    Ok(path)
}

fn write_policy(backups: &Path, executable: &str, installation_id: &str, expected: &str, policy: &Policy)
    -> Result<PermissionsSave, String> {
    let root = require_installation(executable, installation_id)?;
    let folder = Path::new(&root).join(FOLDER);
    safe_fs::validate_directory(&folder)?;
    let (nodes, _) = read_nodes(&folder);
    let (problems, _) = check(policy, nodes.as_deref());
    if !problems.is_empty() {
        return Err(format!("The permissions have problems to fix first: {}", problems.join(" ")));
    }
    let bytes = canonical(policy)?;
    if bytes.len() > MAX_POLICY_BYTES {
        return Err("The permissions file would be larger than 64 KiB. Remove some roles, rules or assignments.".into());
    }
    let original = require_revision(&folder, expected)?;
    if original == bytes {
        return Ok(PermissionsSave {
            changed: false, revision: expected.into(), backup_path: None,
            state: snapshot(Path::new(&root), installation_id),
        });
    }
    // The backup is named by the original's hash, so it stays right even if the save is refused.
    let backup_path = backup(&backups.join(installation_id), expected, &original)?;
    let temporary = stage(&folder, ".permissions-", &bytes)
        .map_err(|error| format!("Could not write permissions.json: {error}"))?;
    {
        // Admin tries this lock once without waiting, so hold it only for the last check and the replace.
        let _lock = lock(&folder)?;
        let mut attempt = 0;
        loop {
            require_revision(&folder, expected)?;
            match safe_fs::move_durably(&temporary, &folder.join(POLICY_FILE), true) {
                Ok(()) => break,
                Err(error) if busy(&error) && attempt < 10 => {
                    attempt += 1;
                    thread::sleep(Duration::from_millis(100));
                }
                Err(error) => return Err(format!("Could not replace permissions.json; it was left as it was: {error}")),
            }
        }
    }
    let revision = sha256_hex(&bytes);
    Ok(PermissionsSave {
        changed: true, revision, backup_path: Some(backup_path.to_string_lossy().into_owned()),
        state: snapshot(Path::new(&root), installation_id),
    })
}
