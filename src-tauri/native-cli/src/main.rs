//! modstoke-native: installs, removes and queues Modstoke mods in a Sons of the Forest game or
//! dedicated-server folder without the Manager's window. See docs/NATIVE_CLI.md.
use modstoke_manager::headless::{self, Command, Options, Outcome};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::Write;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const USAGE: &str = "\
modstoke-native <command> --root <server-root> [options]

Commands:
  inventory                          List installed mods and queued changes
  status                             Server state and queued changes
  inspect --zip <path> --sha256 <hex> Check an archive without changing anything
  install --zip <path> --sha256 <hex> Install or update a mod (server stopped)
  remove --id <mod-id>               Remove a mod (server stopped)
  enable --id <mod-id>               Enable a disabled mod (server stopped)
  disable --id <mod-id>              Disable a mod without removing it (server stopped)
  rollback --id <mod-id> [--to <v>]  Reinstall a kept earlier version (server stopped)
  queue <install|remove|enable|disable|rollback> ...  Queue a change for the next apply
  apply                              Apply queued changes in order (server stopped)
  cancel --request <id> | --all      Delete queued changes
  recover                            Roll back a change a crash interrupted (server stopped)

Options:
  --server-stopped        The host agent states the server is stopped
  --requested-by <label>  Actor label recorded on queued changes
  --request-id <id>       Echoed in the reply as requestId
  --source <type:value>   store:<id> or upload:<label>, kept with installed mods
  --kind native|redloader Overrides the kind detected from the archive
  --progress ndjson       Progress lines on stderr

stdout carries one JSON object. Exit codes: 0 ok, 1 internal error, 2 refused,
3 server running or state unknown, 4 busy.";

const VALUE_FLAGS: [&str; 11] = ["--root", "--zip", "--sha256", "--id", "--to", "--request", "--requested-by", "--request-id", "--source", "--kind", "--progress"];
const SWITCHES: [&str; 2] = ["--server-stopped", "--all"];

struct Parsed {
    command: Command,
    root: String,
    server_stopped: bool,
    requested_by: Option<String>,
    request_id: Option<String>,
    source: Option<Value>,
    kind: Option<String>,
    progress: bool,
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match arguments.first().map(String::as_str) {
        None | Some("--help" | "-h" | "help") => {
            println!("{USAGE}");
            return;
        }
        Some("--version") => {
            println!("modstoke-native {VERSION}");
            return;
        }
        _ => {}
    }
    let outcome = match parse(&arguments) {
        Err((command, message, request_id)) => headless::invalid_arguments(&command, &message, VERSION, request_id),
        Ok(parsed) => {
            let mut print_progress = |line: Value| {
                let mut stderr = std::io::stderr().lock();
                let _ = writeln!(stderr, "{line}");
            };
            let options = Options {
                root: parsed.root,
                tool_version: VERSION,
                server_stopped: parsed.server_stopped,
                requested_by: parsed.requested_by,
                request_id: parsed.request_id,
                source: parsed.source,
                kind: parsed.kind,
                progress: if parsed.progress { Some(&mut print_progress) } else { None },
            };
            headless::run(parsed.command, options)
        }
    };
    finish(outcome);
}

fn finish(outcome: Outcome) {
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "{}", outcome.envelope);
    let _ = stdout.flush();
    std::process::exit(outcome.exit_code);
}

fn printable(value: &str) -> bool {
    value.chars().count() <= 64 && !value.chars().any(char::is_control)
}

// On failure: the command named, the reason, and the request id when it is valid to echo.
fn parse(arguments: &[String]) -> Result<Parsed, (String, String, Option<String>)> {
    let mut words = Vec::new();
    let mut flags = HashMap::new();
    let mut switches = HashSet::new();
    let mut problem = None;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index].as_str();
        if SWITCHES.contains(&argument) {
            switches.insert(argument);
        } else if VALUE_FLAGS.contains(&argument) {
            // A value may itself start with "--", unless it is another option.
            match arguments.get(index + 1).filter(|value| !VALUE_FLAGS.contains(&value.as_str()) && !SWITCHES.contains(&value.as_str())) {
                Some(value) => {
                    if flags.insert(argument, value.clone()).is_some() {
                        problem.get_or_insert(format!("{argument} is given more than once."));
                    }
                    index += 1;
                }
                None => {
                    problem.get_or_insert(format!("{argument} needs a value."));
                }
            }
        } else if argument.starts_with("--") {
            problem.get_or_insert(format!("Unknown option {argument}."));
        } else {
            words.push(argument.to_owned());
        }
        index += 1;
    }
    let command = match words.first().map(String::as_str) {
        Some("queue") => "queue".to_owned(),
        Some(word) => word.to_owned(),
        None => String::new(),
    };
    let request_id = flags.get("--request-id").filter(|id| printable(id)).cloned();
    let fail = |message: String| (command.clone(), message, request_id.clone());
    if let Some(message) = problem {
        return Err(fail(message));
    }
    let flag = |name: &str| flags.get(name).cloned();
    let required = |name: &str| flags.get(name).cloned().ok_or_else(|| format!("{name} is required."));
    let root = required("--root").map_err(fail)?;
    if flag("--requested-by").is_some_and(|label| !printable(&label)) {
        return Err(fail("--requested-by takes up to 64 printable characters.".into()));
    }
    if flag("--request-id").is_some() && request_id.is_none() {
        return Err(fail("--request-id takes up to 64 printable characters.".into()));
    }
    let progress = match flag("--progress").as_deref() {
        None => false,
        Some("ndjson") => true,
        Some(_) => return Err(fail("--progress only supports ndjson.".into())),
    };
    let source = match flag("--source") {
        None => None,
        Some(text) => Some(match text.split_once(':') {
            Some(("store", id)) if !id.is_empty() && printable(id) => json!({ "type": "store", "id": id }),
            Some(("upload", label)) if !label.is_empty() && printable(label) => json!({ "type": "upload", "label": label }),
            _ => return Err(fail("--source takes store:<id> or upload:<label>.".into())),
        }),
    };
    let change = |name: &str| -> Result<Command, String> {
        Ok(match name {
            "install" => Command::Install { zip: required("--zip")?, sha256: required("--sha256")? },
            "remove" => Command::Remove { id: required("--id")? },
            "enable" => Command::Enable { id: required("--id")? },
            "disable" => Command::Disable { id: required("--id")? },
            "rollback" => Command::Rollback { id: required("--id")?, to: flag("--to") },
            other => return Err(format!("Unknown change {other}.")),
        })
    };
    let parsed = match words.first().map(String::as_str) {
        Some("inventory") => Ok(Command::Inventory),
        Some("status") => Ok(Command::Status),
        Some("apply") => Ok(Command::Apply),
        Some("recover") => Ok(Command::Recover),
        Some("inspect") => required("--zip").and_then(|zip| Ok(Command::Inspect { zip, sha256: required("--sha256")? })),
        Some("cancel") => match (flag("--request"), switches.contains("--all")) {
            (Some(request), false) => Ok(Command::Cancel { request: Some(request) }),
            (None, true) => Ok(Command::Cancel { request: None }),
            _ => Err("cancel takes --request <id> or --all.".into()),
        },
        Some("queue") => words.get(1).ok_or_else(|| "queue needs a change: install, remove, enable, disable or rollback.".to_owned())
            .and_then(|name| change(name)).map(|change| Command::Queue(Box::new(change))),
        Some(name @ ("install" | "remove" | "enable" | "disable" | "rollback")) => change(name),
        Some(other) => Err(format!("Unknown command {other}.")),
        None => Err("A command is required.".into()),
    };
    let command_value = parsed.map_err(fail)?;
    let expected_words = if matches!(command_value, Command::Queue(_)) { 2 } else { 1 };
    if words.len() != expected_words {
        return Err(fail(format!("Unexpected argument {}.", words[expected_words.min(words.len() - 1)])));
    }
    Ok(Parsed {
        command: command_value,
        root,
        server_stopped: switches.contains("--server-stopped"),
        requested_by: flag("--requested-by"),
        request_id,
        source,
        kind: flag("--kind"),
        progress,
    })
}
