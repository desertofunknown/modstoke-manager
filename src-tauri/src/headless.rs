//! What modstoke-native runs: native package changes for a game or dedicated-server folder,
//! without the Manager's window. Every command answers with one JSON envelope (docs/NATIVE_CLI.md).
use crate::hashing::sha256_hex;
use crate::mod_profile::native_cli::{self, Violation};
use crate::safe_fs::hash_file;
use crate::{mod_update, native_inventory, native_root, safe_fs};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SIDECAR: &str = "modstoke-native-installs.json";
const STATUS_FILE: &str = "host-status.json";
const FRESH: Duration = Duration::from_secs(15);
const MAX_QUEUED: usize = 64;
const MAX_REQUEST_BYTES: u64 = 64 * 1024;
const KEPT_ARCHIVES: usize = 3;

pub enum Command {
    Inventory,
    Inspect { zip: String, sha256: String },
    Install { zip: String, sha256: String },
    Remove { id: String },
    Enable { id: String },
    Disable { id: String },
    Rollback { id: String, to: Option<String> },
    Queue(Box<Command>),
    Apply,
    Cancel { request: Option<String> },
    Recover,
    Status,
}

impl Command {
    pub fn name(&self) -> &'static str {
        match self {
            Command::Inventory => "inventory",
            Command::Inspect { .. } => "inspect",
            Command::Install { .. } => "install",
            Command::Remove { .. } => "remove",
            Command::Enable { .. } => "enable",
            Command::Disable { .. } => "disable",
            Command::Rollback { .. } => "rollback",
            Command::Queue(_) => "queue",
            Command::Apply => "apply",
            Command::Cancel { .. } => "cancel",
            Command::Recover => "recover",
            Command::Status => "status",
        }
    }
}

pub struct Options<'a> {
    pub root: String,
    pub tool_version: &'static str,
    pub server_stopped: bool,
    pub requested_by: Option<String>,
    pub request_id: Option<String>,
    pub source: Option<Value>,
    pub kind: Option<String>,
    pub progress: Option<&'a mut dyn FnMut(Value)>,
}

pub struct Outcome {
    pub envelope: Value,
    pub exit_code: i32,
}

struct Failure {
    code: &'static str,
    message: String,
    path: Option<String>,
    violations: Vec<Violation>,
}

impl Failure {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Failure { code, message: message.into(), path: None, violations: Vec::new() }
    }

    // Errors from the shared code are plain text; the agent needs a code it can branch on.
    fn from_message(message: String) -> Self {
        let message = host_text(message);
        let lower = message.to_ascii_lowercase();
        let code = if message.contains("Could not lock native mod updates") {
            "busy"
        } else if message.starts_with("The server is running") {
            "server-running"
        } else if message.contains("has not reported this server's state") {
            "server-state-unknown"
        } else if message == native_cli::PENDING_PLAN || ["unfinished", "pack recovery", "recovery workspace", "recovery conflict", "retained"].iter().any(|text| lower.contains(text)) {
            "pending-unfinished"
        } else if message.starts_with("No mod with ID") {
            "not-installed"
        } else if message.contains("requires native mod") {
            "missing-dependency"
        } else if message.contains("is required by") || message.contains("is disabled. Enable it") {
            "conflict"
        } else if ["installed from a mod catalog", "not owned by Manager", "managed by Vortex", "without Manager ownership", "unowned"].iter().any(|text| message.contains(text)) {
            "not-owned"
        } else if ["private content", "staff permissions", "Loader package cannot", "Invalid native mod path", "reserved update path"].iter().any(|text| message.contains(text)) {
            "policy-violation"
        } else if ["Unsafe path in zip entry", "conflicting capitalization", "spells its Mods folder", "duplicate paths", "Native payload", "native archive", "native ZIP", "linked or special file"].iter().any(|text| message.contains(text)) {
            "invalid-archive"
        } else if message.starts_with("Update failed. Previous files were restored") || lower.contains("os error") {
            "io"
        } else {
            "conflict"
        };
        Failure::new(code, message)
    }

    // An archive the shared inspection refused; its message is the one violation.
    fn archive(message: String) -> Self {
        let failure = Failure::from_message(message);
        if failure.code != "conflict" {
            return failure;
        }
        Failure {
            violations: vec![Violation { path: String::new(), rule: "archive", message: failure.message.clone() }],
            ..Failure::new("invalid-archive", failure.message)
        }
    }

    fn exit_code(&self) -> i32 {
        exit_code(self.code)
    }

    fn json(&self) -> Value {
        let mut error = json!({ "code": self.code, "message": self.message });
        if let Some(path) = &self.path {
            error["path"] = json!(path);
        }
        if !self.violations.is_empty() {
            error["violations"] = violations_json(&self.violations);
        }
        error
    }
}

fn exit_code(code: &str) -> i32 {
    match code {
        "server-running" | "server-state-unknown" => 3,
        "busy" => 4,
        "internal" => 1,
        _ => 2,
    }
}

// Every problem an archive has, under the code of the most basic one.
fn refusal(violations: Vec<Violation>) -> Failure {
    let has = |rules: &[&str]| violations.iter().any(|violation| rules.contains(&violation.rule));
    let code = if has(&["archive", "path"]) {
        "invalid-archive"
    } else if has(&["protected-content", "admin-data", "policy"]) {
        "policy-violation"
    } else if has(&["ownership", "vortex"]) {
        "not-owned"
    } else if has(&["dependency"]) {
        "missing-dependency"
    } else if has(&["pending", "moved"]) {
        "pending-unfinished"
    } else {
        "conflict"
    };
    Failure { violations, ..Failure::new(code, "The archive cannot be installed into this folder.") }
}

// The Manager's messages point to its Main page; a hosted server recovers with recover or apply.
fn host_text(message: String) -> String {
    if message.contains("Main page") {
        "An earlier change left its recovery data unfinished. Run recover, or apply the queue, before changing mods.".to_owned()
            + message.split_once(". ").map(|(_, rest)| format!(" ({rest})")).unwrap_or_default().as_str()
    } else {
        message
    }
}

type Done = Result<Value, Failure>;

struct Context<'a, 'b> {
    root: PathBuf,
    options: &'a mut Options<'b>,
}

impl Context<'_, '_> {
    fn progress(&mut self, phase: &str, done: usize, total: usize, file: Option<&str>) {
        if let Some(progress) = self.options.progress.as_mut() {
            let mut line = json!({ "phase": phase, "done": done, "total": total });
            if let Some(file) = file {
                line["file"] = json!(file);
            }
            progress(line);
        }
    }

    fn require_stopped(&self) -> Result<(), Failure> {
        require_stopped(&self.root, self.options.server_stopped)
    }

    // The same check again just before files change, for the shared transaction's callback.
    fn stopped_check(&self) -> impl Fn() -> Result<(), String> {
        let root = self.root.clone();
        let assumed = self.options.server_stopped;
        move || require_stopped(&root, assumed).map_err(|failure| failure.message)
    }
}

fn require_stopped(root: &Path, assumed: bool) -> Result<(), Failure> {
    if assumed {
        return Ok(());
    }
    match server_state(root).0 {
        "stopped" => Ok(()),
        "running" => Err(Failure::new("server-running", "The server is running. Queue the change, or apply it after the server stops.")),
        _ => Err(Failure::new("server-state-unknown", "The Loader has not reported this server's state. Pass --server-stopped once the server is stopped.")),
    }
}

pub fn run(command: Command, mut options: Options) -> Outcome {
    let started = Instant::now();
    let name = command.name();
    let request_id = options.request_id.clone();
    let tool_version = options.tool_version;
    let root = native_cli::server_root(&options.root);
    let (envelope_root, mods_folder, state, result) = match root {
        Err(message) => (options.root.clone(), Value::Null, "unknown", Err(Failure::new("not-a-server-root", message))),
        Ok(root) => {
            let mods_folder = json!(format!("{}/Mods", native_root::resolve_native_root(&root, "Mods")));
            let state = server_state(&root).0;
            let shown = display_of(&root);
            let mut context = Context { root, options: &mut options };
            let result = dispatch(&mut context, command);
            (shown, mods_folder, if context.options.server_stopped { "stopped" } else { state }, result)
        }
    };
    let (ok, result, error, exit_code) = match result {
        Ok(value) => {
            // apply reports a failed request inside its result and still exits with its code.
            match value.get("failed").filter(|failed| !failed.is_null()).cloned() {
                Some(failed) => {
                    let code = failed["error"]["code"].as_str().unwrap_or("conflict").to_owned();
                    (false, value, failed["error"].clone(), exit_code(&code))
                }
                None => (true, value, Value::Null, 0),
            }
        }
        Err(failure) => (false, Value::Null, failure.json(), failure.exit_code()),
    };
    Outcome {
        envelope: json!({
            "schema": 1,
            "tool": "modstoke-native",
            "toolVersion": tool_version,
            "command": name,
            "requestId": request_id,
            "ok": ok,
            "durationMs": started.elapsed().as_millis() as u64,
            "root": envelope_root,
            "modsFolder": mods_folder,
            "serverState": state,
            "result": result,
            "error": error,
        }),
        exit_code,
    }
}

// The envelope for arguments the tool could not understand, before any folder is read.
pub fn invalid_arguments(command: &str, message: &str, tool_version: &'static str, request_id: Option<String>) -> Outcome {
    Outcome {
        envelope: json!({
            "schema": 1,
            "tool": "modstoke-native",
            "toolVersion": tool_version,
            "command": command,
            "requestId": request_id,
            "ok": false,
            "durationMs": 0,
            "root": null,
            "modsFolder": null,
            "serverState": "unknown",
            "result": null,
            "error": { "code": "invalid-arguments", "message": message },
        }),
        exit_code: 2,
    }
}

// Before a change: a profile written for this folder before it was moved or restored elsewhere is
// pointed at it, an interrupted transaction is finished or restored, and RedLoader records it left
// half written are settled. Read-only commands never write; they read a moved folder as it is.
fn prepare_change(root: &Path) -> Result<Vec<String>, Failure> {
    rehome(root)?;
    mod_update::recover_pending_updates(root).map_err(Failure::from_message)?;
    let settled = settle_redloader(root)?;
    if native_cli::has_pending_plan(root) {
        return Err(Failure::new("pending-unfinished", native_cli::PENDING_PLAN));
    }
    Ok(settled)
}

fn rehome(root: &Path) -> Result<(), Failure> {
    native_cli::rehome_locked(root).map(|_| ()).map_err(moved_failure)
}

// A folder the profile cannot follow: pending-unfinished when recovering in the old folder clears
// it, conflict otherwise.
fn moved_failure((pending, message): (bool, String)) -> Failure {
    Failure::new(if pending { "pending-unfinished" } else { "conflict" }, message)
}

fn dispatch(context: &mut Context, command: Command) -> Done {
    match command {
        Command::Inventory => inventory(context),
        Command::Status => status(context),
        Command::Inspect { zip, sha256 } => {
            let zip = canonical_zip(&zip)?;
            inspect(context, &zip, &sha256)
        }
        Command::Queue(change) => {
            let change = match *change {
                Command::Install { zip, sha256 } => Command::Install { zip: canonical_zip(&zip)?, sha256 },
                change => change,
            };
            let _lock = lock(&context.root)?;
            queue(context, change)
        }
        Command::Cancel { request } => {
            let _lock = lock(&context.root)?;
            cancel(context, request.as_deref())
        }
        change @ (Command::Install { .. } | Command::Remove { .. } | Command::Enable { .. } | Command::Disable { .. } | Command::Rollback { .. }) => {
            let change = match change {
                Command::Install { zip, sha256 } => Command::Install { zip: canonical_zip(&zip)?, sha256 },
                change => change,
            };
            context.require_stopped()?;
            let _lock = lock(&context.root)?;
            prepare_change(&context.root)?;
            change_locked(context, change)
        }
        Command::Apply => {
            context.require_stopped()?;
            let _lock = lock(&context.root)?;
            rehome(&context.root)?;
            apply_locked(context)
        }
        Command::Recover => {
            context.require_stopped()?;
            let _lock = lock(&context.root)?;
            rehome(&context.root)?;
            Ok(json!({ "recovered": recover_locked(context)? }))
        }
    }
}

// Everything a crash can leave, native and RedLoader, so the folder can change again.
fn recover_locked(context: &mut Context) -> Result<Vec<String>, Failure> {
    let stopped = context.stopped_check();
    let mut recovered = native_cli::recover_locked(&context.root, &stopped).map_err(Failure::from_message)?;
    recovered.extend(settle_redloader(&context.root)?);
    Ok(recovered)
}

fn lock(root: &Path) -> Result<fs::File, Failure> {
    mod_update::lock_updates(root).map_err(|message| Failure::new("busy", message))
}

// Links in the archive path the host gives are resolved before its folder is checked.
fn canonical_zip(zip: &str) -> Result<String, Failure> {
    let path = fs::canonicalize(zip).map_err(|error| Failure { path: Some(zip.to_owned()), ..Failure::new("io", format!("Cannot read {zip}: {error}")) })?;
    Ok(safe_fs::strip_verbatim_prefix(&path.to_string_lossy()))
}

fn change_locked(context: &mut Context, change: Command) -> Done {
    match change {
        Command::Install { zip, sha256 } => install_locked(context, Path::new(&zip), &sha256, None),
        Command::Remove { id } => remove_locked(context, &id),
        Command::Enable { id } => set_enabled_locked(context, &id, true),
        Command::Disable { id } => set_enabled_locked(context, &id, false),
        Command::Rollback { id, to } => rollback_locked(context, &id, to.as_deref()),
        _ => Err(Failure::new("internal", "Unsupported change.")),
    }
}

// ---- install, remove, enable, rollback ----

// Checks the archive's hash and kind before anything reads its contents.
fn verify_archive(context: &mut Context, zip: &Path, sha256: &str) -> Result<(String, &'static str), Failure> {
    let expected = sha256.to_ascii_lowercase();
    if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Failure::new("invalid-arguments", "--sha256 must be 64 hexadecimal characters."));
    }
    context.progress("verify", 0, 1, None);
    let actual = hash_file(zip).map_err(|message| Failure { path: Some(display_of(zip)), ..Failure::new("io", message) })?;
    context.progress("verify", 1, 1, None);
    if actual != expected {
        return Err(Failure::new("hash-mismatch", format!("The archive's SHA-256 is {actual}, not {expected}.")));
    }
    match detect_kind(zip, context.options.kind.as_deref())? {
        kind @ ("native" | "redloader") => Ok((expected, kind)),
        kind => Err(Failure::new("unsupported-kind", format!("{kind} packages are listed but not installed by this version. Install them with the Manager or by hand."))),
    }
}

// Everything inspect reports for a native archive, plus an ID a RedLoader mod already uses.
fn native_inspection(root: &Path, zip: &Path, sidecar: &Sidecar) -> Result<native_cli::Inspection, Failure> {
    let mut inspection = native_cli::inspect(root, zip).map_err(Failure::archive)?;
    if let Some((pending, message)) = native_cli::rehome_blocker(root) {
        inspection.violations.push(Violation { path: String::new(), rule: if pending { "moved" } else { "plan" }, message });
        inspection.action = None;
    }
    if sidecar.kind(&inspection.archive.id) == Some("redloader") {
        inspection.violations.push(Violation { path: String::new(), rule: "plan", message: format!("A RedLoader mod named {} is installed. Remove it before installing the native package.", inspection.archive.id) });
        inspection.action = None;
    }
    Ok(inspection)
}

fn install_locked(context: &mut Context, zip: &Path, sha256: &str, source: Option<Value>) -> Done {
    let (expected, kind) = verify_archive(context, zip, sha256)?;
    if kind == "redloader" {
        return install_redloader_locked(context, zip, &expected, source);
    }
    let root = context.root.clone();
    let mut sidecar = Sidecar::read(&root)?;
    let inspection = native_inspection(&root, zip, &sidecar)?;
    if inspection.violations.iter().any(|violation| violation.rule == "pending") {
        return Err(Failure::new("pending-unfinished", native_cli::PENDING_PLAN));
    }
    if !inspection.violations.is_empty() {
        return Err(refusal(inspection.violations));
    }
    // Configuration the player already has stays; the install leaves it alone.
    let mut kept: Vec<String> = inspection.archive.files.iter()
        .filter(|file| mod_update::is_native_user_file(Path::new(&file.path)) && safe_fs::validate_target(&root, Path::new(&file.path)).unwrap_or(false))
        .map(|file| file.path.clone())
        .collect();
    let assets = format!("{}/assets", inspection.archive.package_path);
    if native_root::is_protected_content(Path::new(&assets)) && root.join(&assets).is_dir() {
        kept.push(format!("{assets}/"));
    }
    let planned_before = native_cli::has_pending_plan(&root);
    context.progress("stage", 0, 1, None);
    let installed = {
        let stopped = context.stopped_check();
        match native_cli::install_locked(&root, zip, &expected, &stopped) {
            Ok(installed) => installed,
            // An install that fails after recording its plan would block every later change. Undo
            // it with the recovery point the plan began, so the folder is as it was.
            Err(message) if !planned_before && native_cli::has_pending_plan(&root) => {
                let failure = Failure::from_message(message);
                return Err(match native_cli::recover_locked(&root, &stopped) {
                    // A server that started meanwhile keeps its code; the next apply rolls back.
                    Err(_) if failure.code.starts_with("server-") => failure,
                    Err(recovery) => Failure::new("pending-unfinished", format!("{} Rolling back the unfinished install also failed: {recovery}", failure.message)),
                    Ok(_) => failure,
                });
            }
            Err(message) => return Err(Failure::from_message(message)),
        }
    };
    context.progress("commit", 1, 1, None);
    let source = source.or_else(|| context.options.source.clone());
    let archive = &installed.archive;
    let entry = sidecar.entry(&archive.id);
    entry.insert("id".into(), json!(archive.id));
    entry.insert("installedAt".into(), json!(now_iso()));
    entry.insert("source".into(), source.unwrap_or(Value::Null));
    entry.insert("requestId".into(), json!(context.options.request_id));
    entry.insert("disabled".into(), json!(false));
    keep_history(&root, &mut sidecar, &archive.id, &archive.version, &archive.sha256, zip);
    let mut result = json!({
        "modId": archive.id,
        "version": archive.version,
        "action": if installed.action == "update" { "updated" } else { "installed" },
        "previousVersion": installed.previous_version,
        "filesWritten": installed.written,
        "filesRemoved": installed.removed,
        "keptPaths": kept,
    });
    record(&sidecar, &root, &mut result);
    Ok(result)
}

fn remove_locked(context: &mut Context, id: &str) -> Done {
    owned(&context.root, id)?;
    let mut sidecar = Sidecar::read(&context.root)?;
    if sidecar.kind(id) == Some("redloader") {
        return remove_redloader_locked(context, id, sidecar);
    }
    context.progress("stage", 0, 1, None);
    let removal = {
        let stopped = context.stopped_check();
        native_cli::remove_locked(&context.root, id, &stopped).map_err(Failure::from_message)?
    };
    context.progress("commit", 1, 1, None);
    sidecar.forget(&context.root, &removal.id);
    let mut result = json!({ "modId": removal.id, "version": removal.version, "filesRemoved": removal.removed, "keptPaths": removal.kept });
    if !removal.notices.is_empty() {
        result["notices"] = json!(removal.notices.into_iter().map(host_text).collect::<Vec<_>>());
    }
    record(&sidecar, &context.root, &mut result);
    Ok(result)
}

fn set_enabled_locked(context: &mut Context, id: &str, enabled: bool) -> Done {
    owned(&context.root, id)?;
    let mut sidecar = Sidecar::read(&context.root)?;
    let changed = if sidecar.kind(id) == Some("redloader") {
        set_redloader_enabled(&context.root, &sidecar, id, enabled)?
    } else {
        native_cli::set_enabled_locked(&context.root, id, enabled).map_err(Failure::from_message)?
    };
    sidecar.entry(id).insert("disabled".into(), json!(!enabled));
    let mut result = json!({ "modId": id, "enabled": enabled, "changed": changed });
    record(&sidecar, &context.root, &mut result);
    Ok(result)
}

// Once a change is made, a sidecar that cannot be written is reported, not turned into a failure.
fn record(sidecar: &Sidecar, root: &Path, result: &mut Value) {
    if let Err(failure) = sidecar.write(root) {
        let notice = json!(format!("The change was made, but {SIDECAR} could not be updated: {}", failure.message));
        match result["notices"].as_array_mut() {
            Some(notices) => notices.push(notice),
            None => result["notices"] = json!([notice]),
        }
    }
}

// The kept archive a rollback reinstalls: the named version, or the newest one older than the
// installed version.
fn rollback_target(sidecar: &Sidecar, id: &str, installed: &str, to: Option<&str>) -> Result<Value, Failure> {
    let history = sidecar.history(id);
    let target = match to {
        Some(version) => history.into_iter().find(|entry| entry["version"] == version),
        None => history.into_iter()
            .filter(|entry| entry["version"].as_str().is_some_and(|version| compare_versions(version, installed) == Ordering::Less))
            .max_by(|left, right| compare_versions(left["version"].as_str().unwrap_or_default(), right["version"].as_str().unwrap_or_default())),
    };
    target.ok_or_else(|| Failure::new("no-history", match to {
        Some(version) => format!("No kept archive of {id} {version} is available."),
        None => format!("No kept archive of {id} older than {installed} is available."),
    }))
}

fn rollback_locked(context: &mut Context, id: &str, to: Option<&str>) -> Done {
    let installed = owned(&context.root, id)?;
    let sidecar = Sidecar::read(&context.root)?;
    let target = rollback_target(&sidecar, id, &installed.version, to)?;
    let sha256 = target["sha256"].as_str().unwrap_or_default().to_owned();
    let archive = kept_archive(&context.root, target["archive"].as_str(), "native-history/", &sha256)?;
    let source = sidecar.get(id).and_then(|entry| entry.get("source")).cloned();
    install_locked(context, &archive, &sha256, source)
}

// Archives modstoke-native kept itself, named only in the forms it writes, so an edited request or
// sidecar cannot point anywhere else.
fn kept_archive(root: &Path, relative: Option<&str>, folder: &str, sha256: &str) -> Result<PathBuf, Failure> {
    let relative = relative.unwrap_or_default();
    let valid = relative.strip_prefix(folder).is_some_and(|rest| {
        let parts: Vec<&str> = rest.split('/').collect();
        parts.iter().all(|part| !part.is_empty() && *part != "." && *part != ".." && !part.contains(['\\', ':']))
            && parts.last().is_some_and(|name| name.ends_with(&format!("{sha256}.zip")))
            && parts.len() <= 2
    }) && sha256.len() == 64 && sha256.bytes().all(|byte| byte.is_ascii_hexdigit());
    if !valid {
        return Err(Failure::new("invalid-arguments", format!("{relative} is not an archive modstoke-native kept.")));
    }
    Ok(runtime_dir(root).join(relative))
}

// Whether an installed mod is enabled on disk: Some(false) when only its disabled copy is there,
// None when neither is.
fn enabled_on_disk(root: &Path, sidecar: &Sidecar, package: &native_cli::OwnedPackage) -> Option<bool> {
    if sidecar.kind(&package.id) == Some("redloader") {
        if package.package_path.is_empty() {
            return Some(true);
        }
        let exists = |path: String| fs::symlink_metadata(root.join(path)).is_ok();
        let disabled = exists(format!("{}.disabled", package.package_path));
        return if disabled && (sidecar.disabled(&package.id) || !exists(package.package_path.clone())) {
            Some(false)
        } else if exists(package.package_path.clone()) {
            Some(true)
        } else {
            None
        };
    }
    native_cli::folder_state(root, &package.package_path)
}

// ---- inspect ----

fn inspect(context: &mut Context, zip: &str, sha256: &str) -> Done {
    let zip = Path::new(zip);
    let root = context.root.clone();
    if verify_archive(context, zip, sha256)?.1 == "redloader" {
        return Ok(inspect_redloader(&root, zip, &HashSet::new())?.0);
    }
    let sidecar = Sidecar::read(&root)?;
    Ok(inspection_json(&native_inspection(&root, zip, &sidecar)?))
}

fn inspection_json(inspection: &native_cli::Inspection) -> Value {
    let archive = &inspection.archive;
    json!({
        "manifest": {
            "id": archive.id,
            "name": archive.name,
            "version": archive.version,
            "author": archive.extras.author,
            "description": archive.extras.description,
            "kind": "native",
            "clientRequirement": archive.extras.client,
            "dependencies": archive.required.iter().map(|(id, version)| json!({ "id": id, "version": version })).collect::<Vec<_>>(),
            "minimumLoader": null,
        },
        "archiveSha256": archive.sha256,
        "size": archive.size,
        "packagePath": archive.package_path,
        "manifestPath": archive.manifest_path,
        "assemblyPath": archive.assembly_path,
        "files": archive.files.iter().map(|file| json!({ "path": file.path, "size": file.size, "sha256": file.sha256, "target": file.path })).collect::<Vec<_>>(),
        "action": inspection.action,
        "replaces": inspection.previous_version.as_ref().map(|version| json!({ "modId": archive.id, "previousVersion": version, "owner": inspection.previous_owner })),
        "violations": violations_json(&inspection.violations),
    })
}

fn violations_json(violations: &[Violation]) -> Value {
    json!(violations.iter().map(|violation| json!({ "path": violation.path, "rule": violation.rule, "message": violation.message })).collect::<Vec<_>>())
}

// The archive's layout decides its kind unless --kind names one.
fn detect_kind(zip: &Path, forced: Option<&str>) -> Result<&'static str, Failure> {
    if let Some(kind) = forced {
        return match kind {
            "native" => Ok("native"),
            "redloader" => Ok("redloader"),
            _ => Err(Failure::new("invalid-arguments", "--kind must be native or redloader.")),
        };
    }
    let file = fs::File::open(zip).map_err(|error| Failure::new("io", error.to_string()))?;
    let archive = zip::ZipArchive::new(file).map_err(|error| Failure::new("invalid-archive", format!("Not a readable ZIP archive: {error}")))?;
    let mut kinds = HashSet::new();
    for name in archive.file_names() {
        let mut parts = name.split(['/', '\\']).filter(|part| !part.is_empty());
        let first = parts.next().unwrap_or_default();
        let second = parts.next().unwrap_or_default();
        if native_root::is_root_name(first) && second.eq_ignore_ascii_case("Mods") {
            kinds.insert("native");
        } else if first.eq_ignore_ascii_case("Mods") || first.eq_ignore_ascii_case("Libs") {
            kinds.insert("redloader");
        } else if first.eq_ignore_ascii_case("BepInEx") {
            kinds.insert("bepinex");
        }
    }
    match kinds.len() {
        1 => Ok(kinds.into_iter().next().expect("one kind")),
        0 => Err(Failure::new("invalid-archive", "The archive has no Modstoke/Mods, Mods, Libs or BepInEx folder.")),
        _ => Err(Failure::new("invalid-archive", "The archive mixes package layouts. Use one of Modstoke/Mods, Mods and Libs, or BepInEx.")),
    }
}

// ---- queue ----

fn pending_dir(root: &Path) -> PathBuf {
    runtime_dir(root).join("pending")
}

fn read_requests(root: &Path) -> Vec<(PathBuf, Value)> {
    let Ok(entries) = fs::read_dir(pending_dir(root)) else { return Vec::new() };
    let mut requests: Vec<(PathBuf, Value)> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|extension| extension == "json"))
        .filter_map(|path| {
            let mut bytes = Vec::new();
            fs::File::open(&path).ok()?.take(MAX_REQUEST_BYTES + 1).read_to_end(&mut bytes).ok()?;
            let value: Value = serde_json::from_slice(&bytes).ok()?;
            (bytes.len() as u64 <= MAX_REQUEST_BYTES && value["kind"] == "modstoke.pending-native-change").then_some((path, value))
        })
        .collect();
    requests.sort_by(|left, right| left.0.file_name().cmp(&right.0.file_name()));
    requests
}

fn same_mod(request: &Value, id: &str) -> bool {
    request["modId"].as_str().is_some_and(|queued| queued.eq_ignore_ascii_case(id))
}

// Checks what the direct command would, without the server stopped, against the state the queue
// leads to, and writes the request. Runs under the update lock, so apply never sees a request
// change underneath it.
fn queue(context: &mut Context, change: Command) -> Done {
    let root = context.root.clone();
    if let Some(blocker) = native_cli::rehome_blocker(&root) {
        return Err(moved_failure(blocker));
    }
    if let Some(problem) = native_cli::profile_problem(&root) {
        return Err(Failure::from_message(problem));
    }
    let queued = read_requests(&root);
    let sidecar = Sidecar::read(&root)?;
    let mut dependency_problems = Vec::new();
    let (action, mod_id, version, archive, kind, requires) = match &change {
        Command::Install { zip, sha256 } => {
            let zip = Path::new(zip);
            let (expected, kind) = verify_archive(context, zip, sha256)?;
            let (id, version, violations, requires) = if kind == "redloader" {
                let freed: HashSet<String> = queued.iter().filter(|(_, request)| request["action"] == "remove")
                    .filter_map(|(_, request)| request["modId"].as_str().map(str::to_ascii_lowercase)).collect();
                let (_, identity, mut violations) = inspect_redloader(&root, zip, &freed)?;
                violations.extend(queued_claims(&root, &queued, &identity));
                (identity.id, identity.version, violations, Vec::new())
            } else {
                let inspection = native_inspection(&root, zip, &sidecar)?;
                let requires = inspection.archive.required.clone();
                (inspection.archive.id, inspection.archive.version, inspection.violations, requires)
            };
            // Dependencies are judged against the queue below; an unfinished plan is apply's to recover.
            let (dependencies, problems): (Vec<_>, Vec<_>) = violations.into_iter()
                .filter(|violation| violation.rule != "pending")
                .partition(|violation| violation.rule == "dependency" && native_cli::own_dependency(&violation.message, &id));
            if !problems.is_empty() {
                return Err(refusal(problems));
            }
            dependency_problems = dependencies;
            ("install", id, version, Some((zip.to_path_buf(), expected)), Some(kind), requires)
        }
        Command::Remove { id } => {
            let installed = owned(&root, id)?;
            ("remove", installed.id, installed.version, None, None, Vec::new())
        }
        Command::Enable { id } | Command::Disable { id } => {
            let installed = owned(&root, id)?;
            let enable = matches!(change, Command::Enable { .. });
            if sidecar.kind(id) == Some("redloader") && installed.package_path.is_empty() {
                return Err(Failure::new("conflict", format!("{id} has no Mods/<name>.dll to enable or disable.")));
            }
            if enabled_on_disk(&root, &sidecar, &installed).is_none() {
                return Err(Failure::new("conflict", format!("{id}'s files are missing, so it cannot be {}.", if enable { "enabled" } else { "disabled" })));
            }
            (if enable { "enable" } else { "disable" }, installed.id, installed.version, None, None, Vec::new())
        }
        Command::Rollback { id, to } => {
            let installed = owned(&root, id)?;
            if enabled_on_disk(&root, &sidecar, &installed) == Some(false) {
                return Err(Failure::new("conflict", format!("{id} is disabled. Enable it before rolling it back.")));
            }
            let target = rollback_target(&sidecar, id, &installed.version, to.as_deref())?;
            let sha256 = target["sha256"].as_str().unwrap_or_default().to_owned();
            let requires = if sidecar.kind(id) == Some("redloader") {
                Vec::new()
            } else {
                let archive = kept_archive(&root, target["archive"].as_str(), "native-history/", &sha256)?;
                native_cli::archive_requirements(&archive).map_err(Failure::archive)?.2
            };
            ("rollback", installed.id, target["version"].as_str().unwrap_or_default().to_owned(), None, None, requires)
        }
        _ => return Err(Failure::new("invalid-arguments", "Only install, remove, enable, disable and rollback can be queued.")),
    };
    let replaced: Vec<&(PathBuf, Value)> = queued.iter().filter(|(_, request)| same_mod(request, &mod_id)).collect();
    let base: Vec<&(PathBuf, Value)> = queued.iter().filter(|(_, request)| !same_mod(request, &mod_id)).collect();
    if base.len() >= MAX_QUEUED {
        return Err(Failure::new("queue-full", format!("{MAX_QUEUED} changes are already queued. Apply or cancel some first.")));
    }
    let queued_utc = now_iso();
    let id = sha256_hex(format!("{queued_utc}{}{mod_id}{}", std::process::id(), queued.len()).as_bytes())[..16].to_owned();
    let mut request = json!({
        "schema": 1,
        "kind": "modstoke.pending-native-change",
        "id": id,
        "action": action,
        "modId": mod_id,
        "version": version,
        "archiveSha256": null,
        "archivePath": null,
        "queuedUtc": queued_utc,
        "requestedBy": context.options.requested_by,
        "packageKind": kind,
        "source": if action == "install" { context.options.source.clone() } else { None },
        "requestId": context.options.request_id,
        "requires": requires.iter().map(|(id, version)| json!({ "id": id, "version": version })).collect::<Vec<_>>(),
    });
    // A replacing request takes the earliest replaced one's place when the queue still works with
    // it there, so mods queued after it that depend on it still come later; otherwise it goes last.
    let earliest = replaced.first().and_then(|(path, _)| path.file_name()?.to_str()?.split('-').next().map(str::to_owned));
    let mut slots: Vec<String> = earliest.into_iter().collect();
    slots.push(compact(&queued_utc));
    let state = installed_state(&root)?;
    let order = |slot: &str| {
        let name = format!("{slot}-{id}.json");
        let mut order: Vec<&Value> = base.iter().filter(|(path, _)| path.file_name().is_some_and(|file| *file.to_string_lossy() < *name)).map(|(_, request)| request).collect();
        order.push(&request);
        order.extend(base.iter().filter(|(path, _)| path.file_name().is_some_and(|file| *file.to_string_lossy() > *name)).map(|(_, request)| request));
        order
    };
    // Judged only when the queue as it stands works; a queue that already fails is not this request's doing.
    let slot = if simulate(state.clone(), &queued.iter().map(|(_, request)| request).collect::<Vec<_>>()).is_ok() {
        let mut last = None;
        let mut chosen = None;
        for slot in &slots {
            match simulate(state.clone(), &order(slot)) {
                Ok(()) => {
                    chosen = Some(slot.clone());
                    break;
                }
                Err(failure) => last = Some(failure),
            }
        }
        match chosen {
            Some(slot) => slot,
            None => return Err(last.unwrap_or_else(|| Failure::new("conflict", "The change does not fit the queue."))),
        }
    } else {
        // The queue already fails without this request, so only the direct checks can be applied,
        // and the request is played forward only as far as its own place.
        if let Some(problem) = dependency_problems.iter().find(|violation| !dependency_queued(&violation.message, &queued)) {
            return Err(refusal(vec![Violation { path: problem.path.clone(), rule: problem.rule, message: problem.message.clone() }]));
        }
        if slots.len() > 1 {
            let early = order(&slots[0]);
            let upto = early.iter().position(|queued| std::ptr::eq(*queued, &request)).map_or(early.len(), |index| index + 1);
            if simulate(state.clone(), &early[..upto]).is_err() {
                slots.remove(0);
            }
        }
        if action == "remove" && sidecar.kind(&mod_id) != Some("redloader") {
            let queued_dependents = queued.iter()
                .filter(|(_, request)| matches!(request["action"].as_str(), Some("install" | "rollback"))
                    && request["requires"].as_array().into_iter().flatten().any(|dependency| dependency["id"].as_str().is_some_and(|needed| needed.eq_ignore_ascii_case(&mod_id))))
                .filter_map(|(_, request)| request["modId"].as_str().map(str::to_owned));
            let dependents: Vec<String> = native_cli::dependents(&root, &mod_id).map_err(Failure::from_message)?.into_iter()
                .chain(queued_dependents)
                .filter(|dependent| !queued.iter().any(|(_, request)| request["action"] == "remove" && same_mod(request, dependent)))
                .collect();
            if !dependents.is_empty() {
                return Err(Failure::new("conflict", format!("{mod_id} is required by {}. Remove those first.", dependents.join(", "))));
            }
        }
        slots.remove(0)
    };
    let dir = pending_dir(&root);
    fs::create_dir_all(dir.join("archives")).map_err(|error| Failure::new("io", error.to_string()))?;
    if let Some((zip, sha256)) = archive {
        let relative = format!("pending/archives/{sha256}.zip");
        let destination = runtime_dir(&root).join(&relative);
        if hash_file(&destination).ok().as_deref() != Some(sha256.as_str()) {
            copy_file(&zip, &destination)?;
        }
        request["archiveSha256"] = json!(sha256);
        request["archivePath"] = json!(relative);
    }
    // The new request appears only once the ones it replaces are gone, so a failure part way
    // never leaves both queued.
    let name = format!("{slot}-{id}.json");
    let staged = dir.join(format!(".{name}.tmp"));
    write_json(&staged, &request)?;
    let mut superseded = Vec::new();
    for (path, replaced) in &replaced {
        if let Err(error) = fs::remove_file(path) {
            let _ = fs::remove_file(&staged);
            return Err(Failure { path: Some(display_of(path)), ..Failure::new("io", format!("{error}. Nothing was queued; {} replaced requests were removed.", superseded.len())) });
        }
        superseded.push(replaced["id"].clone());
    }
    safe_fs::durable_rename(&staged, &dir.join(&name), false).map_err(|message| Failure::new("io", message))?;
    remove_unused_archives(&root);
    let position = read_requests(&root).iter().position(|(path, _)| path.file_name().is_some_and(|file| file == name.as_str())).map_or(0, |index| index + 1);
    Ok(json!({ "request": request, "position": position, "superseded": superseded }))
}

// Installed native mods by lower-case ID: the ID, its version, what it requires, and whether it is
// enabled.
type State = HashMap<String, (String, String, Vec<(String, String)>, bool)>;

fn installed_state(root: &Path) -> Result<State, Failure> {
    Ok(native_cli::installed_requirements(root).map_err(Failure::from_message)?.into_iter()
        .map(|((id, version, requires), enabled)| (id.to_ascii_lowercase(), (id, version, requires, enabled)))
        .collect())
}

// Files and folders earlier queued RedLoader installs will have put in place by the time this one
// runs: the same file, a file where the other needs a folder or the reverse, or a folder spelled
// another way.
fn queued_claims(root: &Path, queued: &[(PathBuf, Value)], identity: &RedLoaderIdentity) -> Vec<Violation> {
    let files: HashSet<String> = identity.files.iter().map(|(path, _, _)| key(path)).collect();
    let folders_of = |path: &str| -> Vec<String> {
        let parts: Vec<&str> = path.split('/').collect();
        (1..parts.len()).map(|end| parts[..end].join("/")).collect()
    };
    let mine: HashMap<String, String> = identity.files.iter().flat_map(|(path, _, _)| folders_of(path)).map(|folder| (key(&folder), folder)).collect();
    let mut violations = Vec::new();
    for (_, request) in queued {
        if request["action"] != "install" || request["packageKind"] != "redloader" || same_mod(request, &identity.id) {
            continue;
        }
        let sha256 = request["archiveSha256"].as_str().unwrap_or_default();
        let Ok(archive) = kept_archive(root, request["archivePath"].as_str(), "pending/archives/", sha256) else { continue };
        let Ok(other) = redloader_identity(&archive) else { continue };
        for (path, _, _) in other.files.iter().filter(|(path, _, _)| files.contains(&key(path)) || mine.contains_key(&key(path))) {
            violations.push(Violation { path: path.clone(), rule: "ownership", message: format!("{path} will belong to the RedLoader mod {}, queued earlier.", other.id) });
        }
        let theirs: HashMap<String, String> = other.files.iter().flat_map(|(path, _, _)| folders_of(path)).map(|folder| (key(&folder), folder)).collect();
        for (path, _, _) in &identity.files {
            if theirs.contains_key(&key(path)) {
                violations.push(Violation { path: path.clone(), rule: "ownership", message: format!("{path} is a folder the RedLoader mod {}, queued earlier, installs into.", other.id) });
            }
            for folder in folders_of(path) {
                if let Some(spelled) = theirs.get(&key(&folder)).filter(|spelled| **spelled != folder) {
                    violations.push(Violation { path: path.clone(), rule: "path", message: format!("{path} spells the folder {spelled}, which {} queued earlier installs, as {folder}.", other.id) });
                    break;
                }
            }
        }
    }
    violations
}

// Plays requests forward from what is installed, as apply would, and reports the first one that
// would fail: a dependency not enabled at its exact version, a version another enabled mod does
// not accept, an update of a disabled mod, or a removal another mod, even a disabled one, still
// needs. RedLoader mods have no dependencies.
fn simulate(mut state: State, requests: &[&Value]) -> Result<(), Failure> {
    for request in requests {
        let id = request["modId"].as_str().unwrap_or_default();
        let key = id.to_ascii_lowercase();
        let action = request["action"].as_str().unwrap_or_default();
        let native = request["packageKind"] != "redloader" && (action == "install" || state.contains_key(&key));
        match action {
            "install" | "rollback" if native => {
                if state.get(&key).is_some_and(|(_, _, _, enabled)| !enabled) {
                    return Err(Failure::new("conflict", format!("{id} is disabled at that point in the queue. Enable it before updating it.")));
                }
                let version = request["version"].as_str().unwrap_or_default().to_owned();
                let requires: Vec<(String, String)> = request["requires"].as_array().into_iter().flatten()
                    .filter_map(|dependency| Some((dependency["id"].as_str()?.to_owned(), dependency["version"].as_str()?.to_owned())))
                    .collect();
                for (dependency, wanted) in &requires {
                    if state.get(&dependency.to_ascii_lowercase()).filter(|(_, _, _, enabled)| *enabled).map(|(_, version, _, _)| version) != Some(wanted) {
                        return Err(Failure::new("missing-dependency", format!("{id} requires native mod {dependency} at exactly {wanted}, which the queue would not have installed and enabled by then.")));
                    }
                }
                if let Some((other, _, _, _)) = state.values().find(|(other, _, needs, enabled)| *enabled && other.to_ascii_lowercase() != key
                    && needs.iter().any(|(needed, wanted)| needed.eq_ignore_ascii_case(id) && *wanted != version)) {
                    return Err(Failure::new("conflict", format!("{other} requires a different version of {id} at that point in the queue.")));
                }
                // The planner also checks what every other enabled mod requires.
                for (other, _, needs, _) in state.values().filter(|(other, _, _, enabled)| *enabled && other.to_ascii_lowercase() != key) {
                    for (needed, wanted) in needs.iter().filter(|(needed, _)| !needed.eq_ignore_ascii_case(id)) {
                        if state.get(&needed.to_ascii_lowercase()).filter(|(_, _, _, enabled)| *enabled).map(|(_, version, _, _)| version) != Some(wanted) {
                            return Err(Failure::new("missing-dependency", format!("{other} requires native mod {needed} at exactly {wanted}, which would not be installed and enabled when {id} is installed.")));
                        }
                    }
                }
                state.insert(key, (id.to_owned(), version, requires, true));
            }
            "remove" if state.contains_key(&key) => {
                let dependents: Vec<String> = state.values()
                    .filter(|(other, _, needs, _)| other.to_ascii_lowercase() != key && needs.iter().any(|(needed, _)| needed.eq_ignore_ascii_case(id)))
                    .map(|(other, _, _, _)| other.clone())
                    .collect();
                if !dependents.is_empty() {
                    return Err(Failure::new("conflict", format!("{id} is required by {} at that point in the queue. Remove those first.", dependents.join(", "))));
                }
                state.remove(&key);
            }
            "enable" | "disable" => {
                if let Some(entry) = state.get_mut(&key) {
                    entry.3 = action == "enable";
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn dependency_queued(message: &str, queued: &[(PathBuf, Value)]) -> bool {
    // "<id> requires native mod <dependency> at exactly <version>."
    let Some(rest) = message.split("requires native mod ").nth(1) else { return false };
    let mut words = rest.split(" at exactly ");
    let (Some(id), Some(version)) = (words.next(), words.next()) else { return false };
    let version = version.split(". ").next().unwrap_or_default().trim_end_matches('.');
    queued.iter().any(|(_, request)| request["action"] == "install" && same_mod(request, id) && request["version"] == version)
}

fn owned(root: &Path, id: &str) -> Result<native_cli::OwnedPackage, Failure> {
    let sidecar = Sidecar::read(root)?;
    if sidecar.kind(id) == Some("redloader") {
        let entry = sidecar.get(id).expect("recorded");
        return Ok(native_cli::OwnedPackage {
            id: entry.get("id").and_then(Value::as_str).unwrap_or(id).to_owned(),
            version: entry.get("version").and_then(Value::as_str).unwrap_or_default().to_owned(),
            package_path: entry.get("assembly").and_then(Value::as_str).unwrap_or_default().to_owned(),
        });
    }
    native_cli::owned_packages(root).map_err(Failure::from_message)?
        .into_iter().find(|package| package.id.eq_ignore_ascii_case(id))
        .ok_or_else(|| {
            let known = native_inventory::read_inventory(&root.to_string_lossy()).ok()
                .is_some_and(|inventory| inventory.packages.iter().any(|entry| entry.id.eq_ignore_ascii_case(id)));
            if known {
                Failure::new("not-owned", format!("{id} was not installed by the Manager or modstoke-native, so its files are left alone."))
            } else {
                Failure::new("not-installed", format!("No mod with ID {id} is installed."))
            }
        })
}

fn apply_locked(context: &mut Context) -> Done {
    let root = context.root.clone();
    // Whatever a crash interrupted is finished or rolled back first; its request is still queued.
    let recovered = recover_locked(context)?;
    let requests = read_requests(&root);
    let total = requests.len();
    let mut applied = Vec::new();
    let mut failed = Value::Null;
    for (index, (path, request)) in requests.iter().enumerate() {
        context.progress("apply", index, total, request["modId"].as_str());
        let id = request["modId"].as_str().unwrap_or_default().to_owned();
        // A queued install keeps the kind, source and request id it was queued with.
        let saved = (
            std::mem::replace(&mut context.options.kind, request["packageKind"].as_str().map(str::to_owned)),
            std::mem::replace(&mut context.options.source, Some(request["source"].clone()).filter(Value::is_object)),
            std::mem::replace(&mut context.options.request_id, request["requestId"].as_str().map(str::to_owned)),
        );
        let result = match request["action"].as_str().unwrap_or_default() {
            "install" => {
                let sha256 = request["archiveSha256"].as_str().unwrap_or_default().to_owned();
                kept_archive(&root, request["archivePath"].as_str(), "pending/archives/", &sha256)
                    .and_then(|archive| install_locked(context, &archive, &sha256, None))
            }
            "remove" => match remove_locked(context, &id) {
                Err(failure) if failure.code == "not-installed" => Ok(json!({ "action": "absent", "filesRemoved": 0, "keptPaths": [] })),
                result => result,
            },
            "enable" => set_enabled_locked(context, &id, true),
            "disable" => set_enabled_locked(context, &id, false),
            "rollback" => {
                let version = request["version"].as_str().map(str::to_owned);
                rollback_locked(context, &id, version.as_deref())
            }
            other => Err(Failure::new("invalid-arguments", format!("Unknown queued action {other}."))),
        };
        (context.options.kind, context.options.source, context.options.request_id) = saved;
        match result {
            Ok(outcome) => {
                let _ = fs::remove_file(path);
                let mut entry = json!({ "id": request["id"], "action": request["action"], "modId": id, "version": request["version"] });
                if let (Some(entry), Some(outcome)) = (entry.as_object_mut(), outcome.as_object()) {
                    for field in ["action", "filesWritten", "filesRemoved", "keptPaths", "changed"] {
                        if let Some(value) = outcome.get(field) {
                            entry.insert(if field == "action" { "outcome".into() } else { field.into() }, value.clone());
                        }
                    }
                }
                applied.push(entry);
            }
            Err(failure) => {
                failed = json!({ "id": request["id"], "error": failure.json() });
                break;
            }
        }
    }
    remove_unused_archives(&root);
    context.progress("apply", applied.len(), total, None);
    let remaining = read_requests(&root).len();
    Ok(json!({ "recovered": recovered, "applied": applied, "failed": failed, "remaining": remaining }))
}

fn cancel(context: &mut Context, request: Option<&str>) -> Done {
    let mut cancelled = Vec::new();
    for (path, queued) in read_requests(&context.root) {
        if request.is_none_or(|id| queued["id"] == id) {
            fs::remove_file(&path).map_err(|error| Failure { path: Some(display_of(&path)), ..Failure::new("io", error.to_string()) })?;
            cancelled.push(queued["id"].clone());
        }
    }
    if request.is_some() && cancelled.is_empty() {
        return Err(Failure::new("not-queued", "No queued request has that ID."));
    }
    remove_unused_archives(&context.root);
    Ok(json!({ "cancelled": cancelled, "remaining": read_requests(&context.root).len() }))
}

fn remove_unused_archives(root: &Path) {
    let used: HashSet<String> = read_requests(root).iter()
        .filter_map(|(_, request)| request["archivePath"].as_str().map(|path| path.rsplit('/').next().unwrap_or_default().to_owned()))
        .collect();
    let Ok(entries) = fs::read_dir(pending_dir(root).join("archives")) else { return };
    for entry in entries.flatten() {
        if !used.contains(&entry.file_name().to_string_lossy().into_owned()) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

// ---- RedLoader mods ----
// RedLoader mods have no profile record. modstoke-native keeps the files it installed for each one
// in the sidecar, with their hashes, and only ever replaces or removes those. While an install
// commits, its new record waits under `pending`, and the next locked command settles it against
// the files on disk: kept if the install's files are in place, dropped if the old ones are back.

struct RedLoaderIdentity {
    id: String,
    version: String,
    name: Option<String>,
    author: Option<String>,
    assembly: Option<String>,
    // Path, size and SHA-256 of every file.
    files: Vec<(String, u64, String)>,
}

struct OwnedFile {
    path: String,
    sha256: String,
    // What the file held before an install that has not been settled yet replaced it.
    previous: Option<String>,
}

fn key(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

// Exactly one manifest.json 3 to 6 levels deep under Mods or Libs names the mod, as the Manager
// requires of catalog packages. Its main assembly is Mods/<folder>.dll beside the manifest folder.
fn redloader_identity(zip: &Path) -> Result<RedLoaderIdentity, Failure> {
    mod_update::check_archive(zip).map_err(Failure::archive)?;
    let file = fs::File::open(zip).map_err(|error| Failure::new("io", error.to_string()))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| Failure::new("invalid-archive", format!("Not a readable ZIP archive: {error}")))?;
    let mut files = Vec::new();
    let mut manifests = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| Failure::new("invalid-archive", error.to_string()))?;
        if entry.name().ends_with(['/', '\\']) {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        let parts: Vec<String> = name.split('/').filter(|part| !part.is_empty() && *part != ".").map(str::to_owned).collect();
        let mut hasher = Sha256::new();
        std::io::copy(&mut entry, &mut hasher).map_err(|error| Failure::new("invalid-archive", format!("{name} cannot be read: {error}")))?;
        files.push((parts.join("/"), entry.size(), format!("{:x}", hasher.finalize())));
        if parts.last().is_some_and(|last| last.eq_ignore_ascii_case("manifest.json"))
            && (3..=6).contains(&parts.len())
            && (parts[0] == "Mods" || parts[0] == "Libs")
        {
            manifests.push((parts.join("/"), parts[1].clone(), index));
        }
    }
    let mut identities = Vec::new();
    for (path, folder, index) in manifests {
        let mut bytes = Vec::new();
        archive.by_index(index).map_err(|error| Failure::new("invalid-archive", error.to_string()))?
            .take(256 * 1024 + 1).read_to_end(&mut bytes).map_err(|error| Failure::new("io", error.to_string()))?;
        if bytes.len() > 256 * 1024 {
            return Err(Failure::new("invalid-archive", format!("{path} is too large.")));
        }
        let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
        let Ok(manifest) = serde_json::from_slice::<Value>(bytes) else { continue };
        if let (Some(id), Some(version)) = (manifest["id"].as_str(), manifest["version"].as_str()) {
            identities.push((id.to_owned(), version.to_owned(), manifest["name"].as_str().map(str::to_owned), native_cli::extras(bytes).author, folder));
        }
    }
    if identities.len() != 1 {
        return Err(Failure::new("invalid-archive", "A RedLoader mod must contain exactly one manifest.json with an id and version, 3 to 6 folders deep under Mods or Libs."));
    }
    let (id, version, name, author, folder) = identities.remove(0);
    if id.is_empty() || id.len() > 96 || !id.chars().all(|character| character.is_ascii_alphanumeric() || "._-".contains(character)) {
        return Err(Failure::new("invalid-archive", "The mod ID must be 1 to 96 letters, digits, dots, underscores or hyphens."));
    }
    let assembly = format!("Mods/{folder}.dll");
    let assembly = files.iter().any(|(path, _, _)| key(path) == key(&assembly)).then_some(assembly);
    Ok(RedLoaderIdentity { id, version, name, author, assembly, files })
}

fn owned_files(files: Option<&Value>) -> Vec<OwnedFile> {
    files.and_then(Value::as_array).into_iter().flatten()
        .filter_map(|file| Some(OwnedFile {
            path: file["path"].as_str()?.to_owned(),
            sha256: file["sha256"].as_str()?.to_owned(),
            previous: file["previousSha256"].as_str().map(str::to_owned),
        }))
        .collect()
}

fn redloader_files(sidecar: &Sidecar, id: &str) -> Vec<OwnedFile> {
    owned_files(sidecar.get(id).and_then(|entry| entry.get("files")))
}

fn owned_files_json(files: &[OwnedFile]) -> Value {
    json!(files.iter().map(|file| {
        let mut value = json!({ "path": file.path, "sha256": file.sha256 });
        if let Some(previous) = &file.previous {
            value["previousSha256"] = json!(previous);
        }
        value
    }).collect::<Vec<_>>())
}

// The spelling a path, or one of its folders, already has on disk when it differs from `relative`
// only in letter case. On a case-sensitive file system that would be a second copy beside it.
fn other_spelling(root: &Path, relative: &str) -> Option<String> {
    let mut current = root.to_path_buf();
    let mut actual = Vec::new();
    let mut differs = false;
    for part in relative.split('/') {
        let found = fs::read_dir(&current).ok().into_iter().flatten().flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .find(|name| name.eq_ignore_ascii_case(part));
        match found {
            Some(name) => {
                differs |= name != part;
                current = current.join(&name);
                actual.push(name);
            }
            None => break,
        }
    }
    differs.then(|| actual.join("/"))
}

// `freed` names mods queued for removal before this install, whose files it may take.
fn inspect_redloader(root: &Path, zip: &Path, freed: &HashSet<String>) -> Result<(Value, RedLoaderIdentity, Vec<Violation>), Failure> {
    let identity = redloader_identity(zip)?;
    let sidecar = Sidecar::read(root)?;
    // What this mod owns, and what an install of it not yet settled would own.
    let mut recorded: HashMap<String, Vec<String>> = HashMap::new();
    let pending = owned_files(sidecar.pending.get(&identity.id.to_ascii_lowercase()).and_then(|record| record.get("files")));
    for file in redloader_files(&sidecar, &identity.id).into_iter().chain(pending) {
        recorded.entry(key(&file.path)).or_default().push(file.sha256);
    }
    // Files other RedLoader records own, and the disabled name of their assembly.
    let mut claimed: HashMap<String, String> = HashMap::new();
    for entry in sidecar.packages.values().filter(|entry| entry["kind"] == "redloader"
        && !entry["id"].as_str().is_some_and(|id| id.eq_ignore_ascii_case(&identity.id) || freed.contains(&id.to_ascii_lowercase()))) {
        let other = entry["id"].as_str().unwrap_or_default().to_owned();
        for file in owned_files(entry.get("files")) {
            claimed.insert(key(&file.path), other.clone());
        }
        if let Some(assembly) = entry["assembly"].as_str() {
            claimed.insert(key(&format!("{assembly}.disabled")), other.clone());
        }
    }
    let mut violations = Vec::new();
    let mut violation = |path: &str, rule: &'static str, message: String| violations.push(Violation { path: path.to_owned(), rule, message });
    if let Some((pending, message)) = native_cli::rehome_blocker(root) {
        violation("", if pending { "moved" } else { "plan" }, message);
    }
    if native_cli::has_pending_plan(root) {
        violation("", "pending", native_cli::PENDING_PLAN.into());
    }
    // Folders spelled two ways in one archive would be two folders on Linux.
    let mut spellings: HashMap<String, String> = HashMap::new();
    for (path, _, _) in &identity.files {
        let parts: Vec<&str> = path.split('/').collect();
        for end in 1..parts.len() {
            let folder = parts[..end].join("/");
            if let Some(first) = spellings.insert(key(&folder), folder.clone()).filter(|first| *first != folder) {
                violation(path, "path", format!("{path} spells the folder {first} as {folder}."));
                break;
            }
        }
    }
    for (path, _, _) in &identity.files {
        let relative = Path::new(path);
        let first = path.split('/').next().unwrap_or_default();
        // An existing file where the archive needs a folder.
        let blocked = relative.ancestors().skip(1).filter(|parent| !parent.as_os_str().is_empty())
            .find(|parent| fs::symlink_metadata(root.join(parent)).is_ok_and(|metadata| !metadata.is_dir()));
        if mod_update::validate_mod_path(relative).is_err() || path.contains(':') {
            violation(path, "path", format!("{path} is not a safe path inside the game folder."));
        } else if first != "Mods" && first != "Libs" {
            let message = if first.eq_ignore_ascii_case("Mods") || first.eq_ignore_ascii_case("Libs") {
                format!("{path} spells its top folder differently. Use Mods/ and Libs/ exactly.")
            } else {
                format!("{path} is outside Mods/ and Libs/, where RedLoader packages install.")
            };
            violation(path, "policy", message);
        } else if native_cli::is_executable(path) {
            violation(path, "policy", format!("{path} is a program, which mod packages may not install."));
        } else if native_root::is_protected_content(relative) || native_root::is_player_data(relative) {
            violation(path, "protected-content", format!("{path} belongs to the player, not to a mod package."));
        } else if crate::admin_paths::is_admin_data(&mod_update::path_key(relative)) {
            violation(path, "admin-data", format!("{path} belongs to Modstoke Admin's staff permissions."));
        } else if let Err(message) = mod_update::ensure_unmanaged(root, relative) {
            violation(path, "vortex", message);
        } else if let Some(parent) = blocked {
            violation(path, "ownership", format!("{path} needs a folder where {} is a file.", parent.to_string_lossy().replace('\\', "/")));
        } else if let Some(other) = claimed.get(&key(path)) {
            violation(path, "ownership", format!("{path} belongs to the RedLoader mod {other}."));
        } else if let Some(actual) = other_spelling(root, path) {
            violation(path, "path", format!("{path} differs only in letter case from {actual}, already in the game folder."));
        } else if fs::symlink_metadata(root.join(relative)).is_ok() {
            match recorded.get(&key(path)) {
                None => violation(path, "ownership", format!("{path} already exists and was not installed with this mod. It would not be replaced.")),
                Some(installed) if !safe_fs::target_hash(root, relative).ok().flatten().is_some_and(|current| installed.contains(&current)) => {
                    violation(path, "plan", format!("{path} changed since this mod installed it, and the update would replace it."));
                }
                Some(_) => {}
            }
        }
    }
    if native_cli::owned_packages(root).map_err(Failure::from_message)?.iter().any(|package| package.id.eq_ignore_ascii_case(&identity.id)) {
        violation("", "plan", format!("A native package named {} is installed.", identity.id));
    }
    let previous = sidecar.get(&identity.id).filter(|entry| entry.get("kind").and_then(Value::as_str) == Some("redloader"));
    // A disabled copy of the assembly means the mod is disabled, whatever the record says.
    let assembly = previous.and_then(|entry| entry.get("assembly")).and_then(Value::as_str).map(str::to_owned).or_else(|| identity.assembly.clone());
    if assembly.is_some_and(|assembly| fs::symlink_metadata(root.join(format!("{assembly}.disabled"))).is_ok()) {
        violation("", "plan", format!("{} is disabled. Enable it before updating it.", identity.id));
    }
    let inspection = json!({
        "manifest": {
            "id": identity.id,
            "name": identity.name,
            "version": identity.version,
            "author": identity.author,
            "description": null,
            "kind": "redloader",
            "clientRequirement": "unknown",
            "dependencies": [],
            "minimumLoader": null,
        },
        "archiveSha256": hash_file(zip).ok(),
        "size": fs::metadata(zip).map(|metadata| metadata.len()).ok(),
        "packagePath": null,
        "manifestPath": null,
        "assemblyPath": identity.assembly,
        "files": identity.files.iter().map(|(path, size, sha256)| json!({ "path": path, "size": size, "sha256": sha256, "target": path })).collect::<Vec<_>>(),
        "action": if violations.is_empty() { json!(if previous.is_some() { "update" } else { "install" }) } else { Value::Null },
        "replaces": previous.map(|entry| json!({ "modId": identity.id, "previousVersion": entry.get("version"), "owner": "manager" })),
        "violations": violations_json(&violations),
    });
    Ok((inspection, identity, violations))
}

// A private copy of the archive, so the bytes checked are the bytes installed.
fn snapshot(root: &Path, zip: &Path, expected: &str) -> Result<tempfile::TempPath, Failure> {
    let copy = tempfile::Builder::new().prefix(".modstoke-native-").tempfile_in(root).map_err(|error| Failure::new("io", error.to_string()))?;
    let copy = copy.into_temp_path();
    fs::copy(zip, &copy).map_err(|error| Failure::new("io", error.to_string()))?;
    if hash_file(&copy).map_err(|message| Failure::new("io", message))? != expected {
        return Err(Failure::new("hash-mismatch", "The archive changed while it was being copied."));
    }
    Ok(copy)
}

fn install_redloader_locked(context: &mut Context, zip: &Path, expected: &str, source: Option<Value>) -> Done {
    let root = context.root.clone();
    let copy = snapshot(&root, zip, expected)?;
    let (_, identity, violations) = inspect_redloader(&root, &copy, &HashSet::new())?;
    if !violations.is_empty() {
        return Err(refusal(violations));
    }
    let sidecar = Sidecar::read(&root)?;
    let previous_version = sidecar.get(&identity.id).and_then(|entry| entry.get("version")).and_then(Value::as_str).map(str::to_owned);
    let mut recorded: HashMap<String, Vec<String>> = HashMap::new();
    for file in redloader_files(&sidecar, &identity.id) {
        recorded.entry(key(&file.path)).or_default().push(file.sha256);
    }
    let entry = json!({
        "id": identity.id,
        "kind": "redloader",
        "version": identity.version,
        "assembly": identity.assembly,
        "installedAt": now_iso(),
        "source": source.or_else(|| context.options.source.clone()),
        "requestId": context.options.request_id,
        "disabled": false,
    });
    let mut written = 0;
    let mut staged = false;
    context.progress("stage", 0, 1, None);
    let result = {
        let stopped = context.stopped_check();
        mod_update::replace_archive_locked(copy.to_string_lossy().into_owned(), root.to_string_lossy().into_owned(), None, &mut |targets, _| {
            stopped()?;
            let mut changed = Vec::new();
            for target in targets {
                let path = target.relative_path.to_string_lossy().replace('\\', "/");
                if let Some(current) = &target.original_sha256 {
                    match recorded.get(&key(&path)) {
                        None => return Err(format!("{path} already exists and was not installed with this mod. Nothing was changed.")),
                        Some(installed) if !installed.contains(current) => changed.push(path),
                        Some(_) => {}
                    }
                }
            }
            if !changed.is_empty() {
                return Err(format!("{} changed since this mod installed it, and the update would replace it. Nothing was changed.", changed.join(", ")));
            }
            let files: Vec<OwnedFile> = targets.iter().filter_map(|target| Some(OwnedFile {
                path: target.relative_path.to_string_lossy().replace('\\', "/"),
                sha256: target.replacement_sha256.clone()?,
                previous: target.original_sha256.clone(),
            })).collect();
            written = files.len();
            // The new record waits beside the old one until the files are settled, so a crash at
            // any point leaves both what was and what was coming.
            let mut pending = sidecar.clone();
            pending.pending.insert(identity.id.to_ascii_lowercase(), json!({ "entry": entry, "files": owned_files_json(&files) }));
            pending.write(&root).map_err(|failure| failure.message)?;
            staged = true;
            Ok(())
        })
    };
    let committed = match &result {
        Ok(()) => true,
        Err(message) => message.starts_with("The update was applied") || message.starts_with("The update was committed"),
    };
    let restored = result.as_ref().err().is_some_and(|message| message.starts_with("Update failed. Previous files were restored"));
    let mut notices = Vec::new();
    if staged && (committed || restored) {
        let mut settled = Sidecar::read(&root)?;
        if committed {
            finalize_redloader(&root, &mut settled, &identity.id);
            keep_history(&root, &mut settled, &identity.id, &identity.version, expected, &copy);
        } else {
            settled.pending.remove(&identity.id.to_ascii_lowercase());
        }
        if let Err(failure) = settled.write(&root) {
            notices.push(format!("{SIDECAR} could not be updated: {}", failure.message));
        }
    }
    // Any other failure kept the transaction for recovery; the next locked command settles the record.
    match result {
        Err(message) if !committed => return Err(Failure::from_message(message)),
        Err(message) => notices.push(message),
        Ok(()) => {}
    }
    context.progress("commit", 1, 1, None);
    let mut result = json!({
        "modId": identity.id,
        "version": identity.version,
        "action": if previous_version.is_some() { "updated" } else { "installed" },
        "previousVersion": previous_version,
        "filesWritten": written,
        "filesRemoved": 0,
        "keptPaths": [],
    });
    if !notices.is_empty() {
        result["notices"] = json!(notices);
    }
    Ok(result)
}

// Makes a settled install's pending record the mod's record. Files an earlier version installed
// stay owned while they remain on disk.
fn finalize_redloader(root: &Path, sidecar: &mut Sidecar, id: &str) {
    let Some(record) = sidecar.pending.remove(&id.to_ascii_lowercase()) else { return };
    let mut files: Vec<OwnedFile> = owned_files(record.get("files")).into_iter().map(|file| OwnedFile { previous: None, ..file }).collect();
    let new: HashSet<String> = files.iter().map(|file| key(&file.path)).collect();
    for old in redloader_files(sidecar, id) {
        if !new.contains(&key(&old.path)) && fs::symlink_metadata(root.join(&old.path)).is_ok() {
            files.push(OwnedFile { previous: None, ..old });
        }
    }
    let history = sidecar.history(id);
    let entry = sidecar.entry(id);
    entry.clear();
    if let Some(fields) = record["entry"].as_object() {
        entry.extend(fields.clone());
    }
    entry.insert("files".into(), owned_files_json(&files));
    if !history.is_empty() {
        entry.insert("history".into(), json!(history));
    }
}

// Settles RedLoader records an interrupted install left pending, once its transaction has been
// finished or restored: kept when its files are in place, dropped when the old ones are back.
fn settle_redloader(root: &Path) -> Result<Vec<String>, Failure> {
    let mut sidecar = Sidecar::read(root)?;
    if sidecar.pending.is_empty() {
        return Ok(Vec::new());
    }
    let mut done = Vec::new();
    for pending_key in sidecar.pending.keys().cloned().collect::<Vec<_>>() {
        let record = sidecar.pending[&pending_key].clone();
        let id = record["entry"]["id"].as_str().unwrap_or(&pending_key).to_owned();
        let files = owned_files(record.get("files"));
        let current = |path: &str| safe_fs::target_hash(root, Path::new(path)).ok().flatten();
        // Only files whose new and earlier contents differ tell the outcome apart; one that matches
        // neither changed afterwards, as files of a running mod may.
        let telling: Vec<(&OwnedFile, Option<String>)> = files.iter()
            .filter(|file| file.previous.as_deref() != Some(file.sha256.as_str()))
            .map(|file| (file, current(&file.path)))
            .collect();
        let new = telling.iter().any(|(file, now)| now.as_deref() == Some(file.sha256.as_str()));
        let old = telling.iter().any(|(file, now)| file.previous.is_some() && *now == file.previous);
        let gone = telling.iter().all(|(_, now)| now.is_none());
        if new && old {
            return Err(Failure::new("pending-unfinished", format!("An interrupted RedLoader install of {id} left some files at the earlier version and some at the new one. Check its files under Mods/ and Libs/ by hand.")));
        }
        if new || (!old && !gone) || telling.is_empty() {
            finalize_redloader(root, &mut sidecar, &id);
            done.push(format!("Recorded the RedLoader install of {id}, whose files were in place when it was interrupted."));
        } else {
            sidecar.pending.remove(&pending_key);
            done.push(format!("Dropped the interrupted RedLoader install of {id}; its earlier files are back."));
        }
    }
    sidecar.write(root)?;
    Ok(done)
}

fn remove_redloader_locked(context: &mut Context, id: &str, mut sidecar: Sidecar) -> Done {
    let root = context.root.clone();
    let assembly = sidecar.get(id).and_then(|entry| entry.get("assembly")).and_then(Value::as_str).map(key);
    let disabled = owned(&root, id).ok().is_some_and(|installed| enabled_on_disk(&root, &sidecar, &installed) == Some(false));
    let files = redloader_files(&sidecar, id);
    let version = sidecar.get(id).and_then(|entry| entry.get("version")).cloned().unwrap_or(Value::Null);
    let recorded_id = sidecar.get(id).and_then(|entry| entry.get("id")).and_then(Value::as_str).unwrap_or(id).to_owned();
    // A disabled mod's assembly is removed where it is.
    let paths: Vec<(PathBuf, String)> = files.iter().map(|file| {
        let path = if disabled && assembly.as_deref() == Some(key(&file.path).as_str()) { format!("{}.disabled", file.path) } else { file.path.clone() };
        (PathBuf::from(path), file.sha256.clone())
    }).collect();
    let mut removed = 0;
    let mut notices = Vec::new();
    context.progress("stage", 0, 1, None);
    {
        let stopped = context.stopped_check();
        let result = mod_update::remove_files_locked(&root, paths.clone(), &mut |targets, _| {
            stopped()?;
            removed = targets.len();
            Ok(())
        });
        match result {
            Ok(()) => {}
            Err(message) if message.starts_with("The update was applied") || message.starts_with("The update was committed") => notices.push(message),
            Err(message) => return Err(Failure::from_message(message)),
        }
    }
    context.progress("commit", 1, 1, None);
    // Folders the mod's files leave empty go too, deepest first; Mods and Libs themselves stay.
    let mut folders: Vec<PathBuf> = paths.iter().filter_map(|(path, _)| path.parent().map(Path::to_path_buf)).collect();
    folders.sort_by_key(|folder| std::cmp::Reverse(folder.components().count()));
    folders.dedup();
    for folder in folders {
        let mut current = Some(folder.as_path());
        while let Some(folder) = current.filter(|folder| folder.components().count() > 1) {
            if native_root::is_protected_content(folder) || fs::remove_dir(root.join(folder)).is_err() {
                break;
            }
            current = folder.parent();
        }
    }
    sidecar.forget(&root, id);
    let mut result = json!({ "modId": recorded_id, "version": version, "filesRemoved": removed, "keptPaths": [] });
    if !notices.is_empty() {
        result["notices"] = json!(notices);
    }
    record(&sidecar, &root, &mut result);
    Ok(result)
}

// A disabled RedLoader mod is its main assembly renamed to <name>.dll.disabled.
fn set_redloader_enabled(root: &Path, sidecar: &Sidecar, id: &str, enabled: bool) -> Result<bool, Failure> {
    let assembly = sidecar.get(id).and_then(|entry| entry.get("assembly")).and_then(Value::as_str)
        .ok_or_else(|| Failure::new("conflict", format!("{id} has no Mods/<name>.dll to enable or disable.")))?;
    let active = root.join(assembly);
    let disabled = root.join(format!("{assembly}.disabled"));
    let (from, to) = if enabled { (&disabled, &active) } else { (&active, &disabled) };
    if !from.exists() && to.exists() {
        return Ok(false);
    }
    if !from.exists() || to.exists() {
        return Err(Failure::new("conflict", format!("Cannot {} {id}: {} is missing or already present.", if enabled { "enable" } else { "disable" }, assembly)));
    }
    fs::rename(from, to).map_err(|error| Failure { path: Some(assembly.to_owned()), ..Failure::new("io", error.to_string()) })?;
    Ok(true)
}

// ---- inventory and status ----

fn status(context: &mut Context) -> Done {
    let (state, updated_at) = server_state(&context.root);
    Ok(json!({
        "serverState": if context.options.server_stopped { "stopped" } else { state },
        "hostStatusUpdatedAt": updated_at,
        "modsFolder": format!("{}/Mods", native_root::resolve_native_root(&context.root, "Mods")),
        "pending": read_requests(&context.root).into_iter().map(|(_, request)| request).collect::<Vec<_>>(),
    }))
}

fn history_json(sidecar: &Sidecar, id: &str) -> Value {
    json!(sidecar.history(id).into_iter().map(|entry| json!({ "version": entry["version"], "sha256": entry["sha256"], "installedAt": entry["installedAt"] })).collect::<Vec<_>>())
}

fn inventory(context: &mut Context) -> Done {
    let root = &context.root;
    let inventory = native_inventory::read_inventory(&root.to_string_lossy()).map_err(Failure::from_message)?;
    let mut notices = inventory.notices.clone();
    notices.extend(inventory.legacy_notices.iter().cloned());
    // Without readable ownership records every mod is listed, with owner unknown.
    let owned = native_cli::owned_packages(root).map_err(|message| notices.push(message)).ok();
    if let Some((_, message)) = native_cli::rehome_blocker(root) {
        notices.push(message);
    }
    // Listing still works with an unreadable sidecar; changes refuse until it is fixed.
    let sidecar = Sidecar::read(root).unwrap_or_else(|failure| {
        notices.push(failure.message);
        Sidecar::default()
    });
    let vortex = mod_update::vortex_paths(root);
    let mut packages = Vec::new();
    for entry in &inventory.packages {
        let manifest = relative(root, &entry.manifest_path);
        if native_root::is_protected_content(Path::new(&manifest)) {
            continue;
        }
        let package = relative(root, &entry.package_path);
        let assembly = entry.assembly_path.as_deref().map(|path| relative(root, path));
        let enabled_path = package.trim_end_matches(".disabled");
        let record = owned.iter().flatten().find(|record| record.id.eq_ignore_ascii_case(&entry.id) && record.package_path.eq_ignore_ascii_case(enabled_path));
        let owner = if owned.is_none() {
            "unknown"
        } else if record.is_some() {
            "manager"
        } else if vortex.iter().any(|path| path.starts_with(&mod_update::vortex_key(Path::new(&package)))) {
            "vortex"
        } else {
            "manual"
        };
        let extras = fs::read(root.join(&manifest)).ok().map(|bytes| native_cli::extras(&bytes));
        let side = sidecar.get(&entry.id);
        packages.push(json!({
            "id": entry.id,
            "name": entry.name,
            "version": entry.version,
            "author": entry.author,
            "kind": "native",
            "owner": owner,
            "enabled": entry.state != "disabled",
            "state": entry.state,
            "clientRequirement": extras.map(|extras| extras.client).unwrap_or_else(|| "unknown".into()),
            "folder": package.rsplit('/').next().unwrap_or_default(),
            "sizeOnDisk": size_on_disk(root, Path::new(&package)),
            "manifestPath": manifest,
            "assemblyPath": assembly,
            "assemblySha256": assembly.as_ref().and_then(|path| safe_fs::target_hash(root, Path::new(path)).ok().flatten()),
            "installedAt": side.and_then(|side| side.get("installedAt")).cloned().unwrap_or(Value::Null),
            "source": side.and_then(|side| side.get("source")).cloned().unwrap_or(Value::Null),
            "history": history_json(&sidecar, &entry.id),
        }));
    }
    packages.extend(other_kinds(root, &vortex, &sidecar));
    Ok(json!({ "packages": packages, "notices": notices, "pending": read_requests(root).into_iter().map(|(_, request)| request).collect::<Vec<_>>() }))
}

// RedLoader mods and BepInEx plugins, listed read-only. The Loader and Admin folders are not mods.
fn other_kinds(root: &Path, vortex: &HashSet<String>, sidecar: &Sidecar) -> Vec<Value> {
    let mut rows = Vec::new();
    let recorded: Vec<(String, String)> = sidecar.packages.values()
        .filter(|entry| entry["kind"] == "redloader")
        .filter_map(|entry| Some((entry["assembly"].as_str().map(key).unwrap_or_default(), entry["id"].as_str()?.to_owned())))
        .collect();
    let mut listed = HashSet::new();
    let owner = |relative: &str| {
        let disabled = relative.strip_suffix(".disabled").unwrap_or(relative);
        if recorded.iter().any(|(assembly, _)| !assembly.is_empty() && *assembly == key(disabled)) {
            "manager"
        } else if vortex.contains(&mod_update::vortex_key(Path::new(relative))) {
            "vortex"
        } else {
            "manual"
        }
    };
    let row = |id: &str, name: &str, kind: &str, owner: &str, enabled: bool, folder: Option<String>, assembly: Option<String>, manifest: &Value, manifest_path: Option<String>, side: Option<&Map<String, Value>>| {
        // A RedLoader mod this tool installed is the files it recorded, under Mods/ and Libs/ alike.
        let size = match side.filter(|_| kind == "redloader") {
            Some(_) => redloader_files(sidecar, id).iter()
                .map(|file| size_on_disk(root, Path::new(&file.path)).max(size_on_disk(root, Path::new(&format!("{}.disabled", file.path)))))
                .sum::<u64>(),
            None => folder.iter().chain(assembly.iter()).map(|path| size_on_disk(root, Path::new(path))).sum::<u64>(),
        };
        json!({
            "id": id,
            "name": name,
            "version": manifest["version"],
            "author": manifest["author"],
            "kind": kind,
            "owner": owner,
            "enabled": enabled,
            "state": if enabled { "enabled" } else { "disabled" },
            "clientRequirement": "unknown",
            "folder": folder.as_deref().and_then(|folder| folder.rsplit('/').next()),
            "sizeOnDisk": size,
            "manifestPath": manifest_path,
            "assemblyPath": assembly,
            "assemblySha256": assembly.as_deref().and_then(|path| safe_fs::target_hash(root, Path::new(path)).ok().flatten()),
            "installedAt": side.and_then(|side| side.get("installedAt")).cloned().unwrap_or(Value::Null),
            "source": side.and_then(|side| side.get("source")).cloned().unwrap_or(Value::Null),
            "history": if kind == "redloader" { history_json(sidecar, id) } else { json!([]) },
        })
    };
    if let Ok(entries) = fs::read_dir(root.join("Mods")) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let lower = name.to_ascii_lowercase();
            if !entry.path().is_file() || !(lower.ends_with(".dll") || lower.ends_with(".dll.disabled")) {
                continue;
            }
            let stem = name.split(".dll").next().unwrap_or_default().to_owned();
            let manifest_path = format!("Mods/{stem}/manifest.json");
            let manifest = fs::read(root.join(&manifest_path)).ok()
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok()).unwrap_or(Value::Null);
            let relative = format!("Mods/{name}");
            let side_id = recorded.iter().find(|(assembly, _)| *assembly == key(relative.strip_suffix(".disabled").unwrap_or(&relative))).map(|(_, id)| id.clone());
            let id = side_id.clone().unwrap_or_else(|| manifest["id"].as_str().unwrap_or(&stem).to_owned());
            listed.insert(id.to_ascii_lowercase());
            let folder = root.join("Mods").join(&stem).is_dir().then(|| format!("Mods/{stem}"));
            let manifest_path = manifest.is_object().then_some(manifest_path);
            rows.push(row(&id, manifest["name"].as_str().unwrap_or(&stem), "redloader", owner(&relative), !lower.ends_with(".disabled"),
                folder, Some(relative), &manifest, manifest_path, side_id.as_deref().and_then(|id| sidecar.get(id))));
        }
    }
    // Mods this tool installed with no Mods/<name>.dll, such as libraries under Libs/.
    for (_, id) in &recorded {
        if listed.contains(&id.to_ascii_lowercase()) {
            continue;
        }
        let side = sidecar.get(id);
        let manifest = json!({ "version": side.and_then(|side| side.get("version")), "author": null });
        rows.push(row(id, id, "redloader", "manager", true, None, None, &manifest, None, side));
    }
    if let Ok(entries) = fs::read_dir(root.join("BepInEx").join("plugins")) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if ["OpenACAILoader", "OpenACAIAdminTool"].iter().any(|own| name.eq_ignore_ascii_case(own)) {
                continue;
            }
            let lower = name.to_ascii_lowercase();
            let relative = format!("BepInEx/plugins/{name}");
            let directory = entry.path().is_dir();
            if directory || lower.ends_with(".dll") || lower.ends_with(".dll.disabled") {
                let stem = name.split(".dll").next().unwrap_or(&name);
                let enabled = !lower.ends_with(".disabled") && lower != "_disabled";
                let (folder, assembly) = if directory { (Some(relative.clone()), None) } else { (None, Some(relative.clone())) };
                rows.push(row(stem, stem, "bepinex", owner(&relative), enabled, folder, assembly, &Value::Null, None, None));
            }
        }
    }
    rows
}

fn size_on_disk(root: &Path, relative: &Path) -> u64 {
    if native_root::is_protected_content(relative) {
        return 0;
    }
    let path = root.join(relative);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() => metadata.len(),
        Ok(metadata) if metadata.is_dir() => fs::read_dir(&path).map(|entries| entries.flatten()
            .map(|entry| size_on_disk(root, &relative.join(entry.file_name()))).sum()).unwrap_or(0),
        _ => 0,
    }
}

fn relative(root: &Path, display: &str) -> String {
    let root_text = display_of(root);
    let text = safe_fs::strip_verbatim_prefix(display);
    text.strip_prefix(&root_text)
        .map(|rest| rest.trim_start_matches(['/', '\\']).to_owned())
        .unwrap_or(text)
        .replace('\\', "/")
}

fn display_of(path: &Path) -> String {
    safe_fs::strip_verbatim_prefix(&path.to_string_lossy())
}

// ---- server state ----

fn runtime_dir(root: &Path) -> PathBuf {
    root.join(native_root::resolve_native_root(root, "Runtime")).join("Runtime")
}

// Loader 0.4.2 and later rewrite host-status.json every two seconds while the server runs. A time
// further ahead than that is a clock problem, not proof the server runs.
fn server_state(root: &Path) -> (&'static str, Option<String>) {
    let Ok(text) = fs::read_to_string(runtime_dir(root).join(STATUS_FILE)) else { return ("unknown", None) };
    let Some(updated_at) = serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')).ok().and_then(|value| value["updatedAt"].as_str().map(str::to_owned)) else {
        return ("unknown", None);
    };
    let Some(updated) = parse_iso(&updated_at) else { return ("unknown", Some(updated_at)) };
    let state = match SystemTime::now().duration_since(updated) {
        Ok(age) if age < FRESH => "running",
        Ok(_) => "stopped",
        Err(ahead) if ahead.duration() < FRESH => "running",
        Err(_) => "unknown",
    };
    (state, Some(updated_at))
}

// ---- sidecar ----

// What modstoke-native knows beyond the profile: when and from where a mod came, whether it is
// disabled, which archives it kept, and which files each RedLoader mod owns, plus RedLoader
// installs not yet settled. Kept apart so Managers that read the profile strictly still can.
#[derive(Clone, Default)]
struct Sidecar {
    packages: Map<String, Value>,
    pending: Map<String, Value>,
}

impl Sidecar {
    // A sidecar that exists but cannot be read stops every change: it is the only record of which
    // RedLoader files this tool installed, and starting over empty would forget them.
    fn read(root: &Path) -> Result<Self, Failure> {
        let path = root.join(SIDECAR);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Sidecar::default()),
            Err(error) => return Err(Failure { path: Some(SIDECAR.into()), ..Failure::new("io", format!("Cannot read {SIDECAR}: {error}")) }),
        };
        let unreadable = |detail: &str| Failure {
            path: Some(SIDECAR.into()),
            ..Failure::new("conflict", format!("{SIDECAR} cannot be read ({detail}). It records the files this tool installed, so restore or fix it before changing mods."))
        };
        let value: Value = serde_json::from_slice(&bytes).map_err(|error| unreadable(&error.to_string()))?;
        let (Some(packages), true) = (value.get("packages").and_then(Value::as_object), value["kind"] == "modstoke.native-installs") else {
            return Err(unreadable("it is not a modstoke.native-installs file"));
        };
        let pending = match value.get("pending") {
            None | Some(Value::Null) => Map::new(),
            Some(Value::Object(pending)) => pending.clone(),
            Some(_) => return Err(unreadable("its pending records are not an object")),
        };
        // Every owned file needs its path and hash, or a removal would leave it behind unnoticed.
        let valid_files = |files: Option<&Value>| files.and_then(Value::as_array).is_some_and(|files| files.iter().all(|file| {
            file["path"].as_str().is_some_and(|path| !path.is_empty())
                && file["sha256"].as_str().is_some_and(|hash| hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
        }));
        for (id, entry) in packages {
            if entry["kind"] == "redloader" && !valid_files(entry.get("files")) {
                return Err(unreadable(&format!("the files recorded for {id} are incomplete")));
            }
        }
        for (id, record) in &pending {
            if !valid_files(record.get("files")) || !record["entry"].is_object() {
                return Err(unreadable(&format!("the pending record for {id} is incomplete")));
            }
        }
        Ok(Sidecar { packages: packages.clone(), pending })
    }

    fn get(&self, id: &str) -> Option<&Map<String, Value>> {
        self.packages.get(&id.to_ascii_lowercase()).and_then(Value::as_object)
    }

    fn entry(&mut self, id: &str) -> &mut Map<String, Value> {
        let entry = self.packages.entry(id.to_ascii_lowercase()).or_insert_with(|| json!({ "id": id }));
        if !entry.is_object() {
            *entry = json!({ "id": id });
        }
        entry.as_object_mut().expect("object")
    }

    fn kind(&self, id: &str) -> Option<&str> {
        self.get(id).and_then(|entry| entry.get("kind")).and_then(Value::as_str)
    }

    fn disabled(&self, id: &str) -> bool {
        self.get(id).and_then(|entry| entry.get("disabled")).and_then(Value::as_bool).unwrap_or(false)
    }

    fn history(&self, id: &str) -> Vec<Value> {
        self.get(id).and_then(|entry| entry.get("history")).and_then(Value::as_array).cloned().unwrap_or_default()
    }

    // Removing a mod also drops its kept archives.
    fn forget(&mut self, root: &Path, id: &str) {
        for entry in self.history(id) {
            if let Some(archive) = entry["archive"].as_str() {
                let _ = fs::remove_file(runtime_dir(root).join(archive));
            }
        }
        let _ = fs::remove_dir(runtime_dir(root).join("native-history").join(id.to_ascii_lowercase()));
        self.packages.remove(&id.to_ascii_lowercase());
    }

    fn write(&self, root: &Path) -> Result<(), Failure> {
        let mut document = json!({ "schema": 1, "kind": "modstoke.native-installs", "packages": self.packages });
        if !self.pending.is_empty() {
            document["pending"] = json!(self.pending);
        }
        write_json(&root.join(SIDECAR), &document).map_err(|failure| Failure { path: Some(SIDECAR.into()), ..failure })
    }
}

// Keeps the installed archive, so a later update can be rolled back to it. The newest few stay.
fn keep_history(root: &Path, sidecar: &mut Sidecar, id: &str, version: &str, sha256: &str, zip: &Path) {
    let folder = format!("native-history/{}", id.to_ascii_lowercase());
    let relative = format!("{folder}/{}-{sha256}.zip", sanitize(version));
    let destination = runtime_dir(root).join(&relative);
    if fs::create_dir_all(runtime_dir(root).join(&folder)).is_err()
        || (hash_file(&destination).ok().as_deref() != Some(sha256) && copy_file(zip, &destination).is_err())
    {
        return;
    }
    let mut history: Vec<Value> = sidecar.history(id).into_iter().filter(|entry| entry["sha256"] != sha256).collect();
    history.insert(0, json!({ "version": version, "sha256": sha256, "archive": relative, "installedAt": now_iso() }));
    for old in history.split_off(KEPT_ARCHIVES.min(history.len())) {
        if let Some(path) = old["archive"].as_str() {
            let _ = fs::remove_file(runtime_dir(root).join(path));
        }
    }
    sidecar.entry(id).insert("history".into(), json!(history));
}

fn sanitize(version: &str) -> String {
    version.chars().map(|character| if character.is_ascii_alphanumeric() || "._-".contains(character) { character } else { '_' }).collect()
}

// Orders versions by their dotted numbers, then a release after its pre-releases (1.0.0-rc1 < 1.0.0).
fn compare_versions(left: &str, right: &str) -> Ordering {
    fn split(version: &str) -> (&str, Option<&str>) {
        let version = version.trim().trim_start_matches(['v', 'V']);
        let version = version.split('+').next().unwrap_or_default();
        match version.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (version, None),
        }
    }
    fn parts(text: &str) -> Vec<&str> {
        text.split('.').collect()
    }
    fn compare_parts(left: &[&str], right: &[&str]) -> Ordering {
        for index in 0..left.len().max(right.len()) {
            let (a, b) = (left.get(index).copied().unwrap_or("0"), right.get(index).copied().unwrap_or("0"));
            let order = match (a.parse::<u64>(), b.parse::<u64>()) {
                (Ok(a), Ok(b)) => a.cmp(&b),
                (Ok(_), Err(_)) => Ordering::Less,
                (Err(_), Ok(_)) => Ordering::Greater,
                (Err(_), Err(_)) => a.cmp(b),
            };
            if order != Ordering::Equal {
                return order;
            }
        }
        Ordering::Equal
    }
    let ((left_core, left_pre), (right_core, right_pre)) = (split(left), split(right));
    compare_parts(&parts(left_core), &parts(right_core)).then_with(|| match (left_pre, right_pre) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(a), Some(b)) => compare_parts(&parts(a), &parts(b)),
    })
}

// ---- files and time ----

fn write_json(path: &Path, value: &Value) -> Result<(), Failure> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| Failure::new("internal", error.to_string()))?;
    let parent = path.parent().ok_or_else(|| Failure::new("io", "No parent folder."))?;
    let temporary = tempfile::NamedTempFile::new_in(parent).map_err(|error| Failure::new("io", error.to_string()))?;
    fs::write(temporary.path(), &bytes).map_err(|error| Failure::new("io", error.to_string()))?;
    let replace = path.exists();
    let temporary = temporary.into_temp_path();
    safe_fs::durable_rename(&temporary, path, replace).map_err(|message| Failure::new("io", message))
}

fn copy_file(source: &Path, destination: &Path) -> Result<(), Failure> {
    safe_fs::copy_durable(source, destination).map_err(|message| Failure { path: Some(display_of(destination)), ..Failure::new("io", message) })
}

fn now_iso() -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let (year, month, day, hour, minute, second) = civil(now.as_secs() as i64);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{:03}Z", now.subsec_millis())
}

// 2026-11-05T14:02:11.482Z -> 20261105T140211482Z, so file names sort by time.
fn compact(iso: &str) -> String {
    iso.chars().filter(|character| !"-:.".contains(*character)).collect()
}

fn parse_iso(text: &str) -> Option<SystemTime> {
    let text = text.strip_suffix('Z')?;
    let (date, time) = text.split_once('T')?;
    let number = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit()).then(|| part.parse::<i64>().ok()).flatten();
    let mut date = date.split('-').map(number);
    let (year, month, day) = (date.next()??, date.next()??, date.next()??);
    let (clock, fraction) = time.split_once('.').unwrap_or((time, "0"));
    let mut clock = clock.split(':').map(number);
    let (hour, minute, second) = (clock.next()??, clock.next()??, clock.next()??);
    if date.next().is_some() || clock.next().is_some() || fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let seconds = days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second;
    // Out-of-range fields would roll into another time; only a real date and time is accepted.
    if civil(seconds) != (year, month, day, hour, minute, second) {
        return None;
    }
    let millis: u64 = format!("{:0<3}", &fraction[..3.min(fraction.len())]).parse().ok()?;
    UNIX_EPOCH.checked_add(Duration::from_secs(u64::try_from(seconds).ok()?) + Duration::from_millis(millis))
}

// Howard Hinnant's civil calendar conversions.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil(seconds: i64) -> (i64, i64, i64, i64, i64, i64) {
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day, rest / 3_600, rest % 3_600 / 60, rest % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes_utc_times() {
        let time = parse_iso("2026-11-05T14:02:11.482Z").unwrap();
        let seconds = time.duration_since(UNIX_EPOCH).unwrap();
        assert_eq!(civil(seconds.as_secs() as i64), (2026, 11, 5, 14, 2, 11));
        assert_eq!(seconds.subsec_millis(), 482);
        assert_eq!(compact("2026-11-05T14:02:11.482Z"), "20261105T140211482Z");
        assert!(parse_iso("2026-11-05T14:02:11Z").is_some());
        assert!(parse_iso("2026-11-05T14:02:11.4820000Z").is_some());
        for invalid in ["not a time", "2026-13-05T14:02:11.482Z", "2026-02-30T14:02:11Z", "2026-11-05T24:00:00Z", "2026-11-05T14:02:11+00:00", "2026-11-05T14:02:-1Z"] {
            assert!(parse_iso(invalid).is_none(), "{invalid}");
        }
    }

    #[test]
    fn finds_queued_dependencies() {
        let queued = vec![(PathBuf::from("a.json"), json!({ "action": "install", "modId": "SonsAxLib", "version": "1.2.0" }))];
        assert!(dependency_queued("example requires native mod SonsAxLib at exactly 1.2.0. Install a compatible dependency first; this package was not applied.", &queued));
        assert!(!dependency_queued("example requires native mod SonsAxLib at exactly 1.3.0. Install a compatible dependency first.", &queued));
    }

    #[test]
    fn orders_versions() {
        assert_eq!(compare_versions("1.10.0", "1.9.3"), Ordering::Greater);
        assert_eq!(compare_versions("2.0.0", "2.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.0.0-rc1", "1.0.0"), Ordering::Less);
        assert_eq!(compare_versions("v0.4.2", "0.4.10"), Ordering::Less);
        assert_eq!(compare_versions("0.4.1-net11-preview", "0.4.1-net10"), Ordering::Greater);
    }
}
