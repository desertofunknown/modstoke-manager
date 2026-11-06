# modstoke-native

`modstoke-native` is a command-line tool that installs, removes and queues mods in a dedicated-server or game folder without the Manager's window. Server hosting runs it from its host agent. It is built from the Manager's own code, so it applies the same rules: archive validation, the native package policy, Modstoke and OpenACAI folder resolution, staging with rollback, Vortex ownership and the installation profile.

Version 0.1.0 is published on modstoke.com:

| Build | SHA-256 |
|---|---|
| [Windows x64](https://modstoke.com/downloads/modstoke-native/0.1.0/modstoke-native-0.1.0-windows-x64.exe), signed by OpenACAI Inc | `7c1478ad843df72f6e45bfc7fa1693e81ab09d8ba21804b2442948f3b6fe649d` |
| [Linux x64](https://modstoke.com/downloads/modstoke-native/0.1.0/modstoke-native-0.1.0-linux-x64), static, no dependencies | `efe6877dbb83d52d578b1204c827b2bbbab32f95601215d0a19d358ffaed6a48` |

[SHA256SUMS.txt](https://modstoke.com/downloads/modstoke-native/0.1.0/SHA256SUMS.txt) lists both, for `sha256sum -c`. Mark the Linux file executable after downloading it.

Its source is `src-tauri/native-cli`. The shared code is the Manager's library built without the `app` feature, so it needs no Tauri and no C compiler:

```
cd src-tauri
cargo build -p modstoke-native --release
# Static Linux build for hosting containers, linked with the linker that ships with Rust:
rustup target add x86_64-unknown-linux-musl
CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=rust-lld cargo build -p modstoke-native --release --target x86_64-unknown-linux-musl
```

Windows builds are signed like the Manager before they are published. The tool reads ZIP entries stored or compressed with Deflate; unlike the Manager, it does not read bzip2-compressed entries.

## Invocation

```
modstoke-native <command> --root <server-root> [options]
```

| Option | Meaning |
|---|---|
| `--root` | Folder with `SonsOfTheForestDS.exe` (or `SonsOfTheForest.exe`). Links in this path and in `--zip` are resolved; links inside the folder are refused |
| `--server-stopped` | The host agent states the server is stopped; overrides host-status.json |
| `--requested-by <label>` | Actor label, up to 64 printable characters, recorded on queued requests |
| `--request-id <id>` | Echoed as `requestId`, up to 64 printable characters |
| `--kind native\|redloader` | Overrides the kind detected from the archive |
| `--source <type:value>` | `store:<id>` or `upload:<label>`, kept with the installed mod as `source` |
| `--progress ndjson` | Progress lines on stderr |

stdout carries exactly one JSON object, UTF-8 on one line. stderr is for logs and, with `--progress ndjson`, one `{"phase":"verify|stage|commit|apply","done":n,"total":n,"file":"optional"}` line per step. `apply` lines count queued requests and name each request's mod.

Exit codes: `0` ok, `1` internal error, `2` refused, `3` server running or state unknown, `4` busy.

`--help`, `-h`, `help` and `--version` print plain text, and only as the first argument; so does running the tool with no arguments. Errors in the arguments still give an envelope, with `command` set to the first word that is not an option and `requestId` only when it is valid.

## Envelope

```json
{
  "schema": 1,
  "tool": "modstoke-native",
  "toolVersion": "0.1.0",
  "command": "install",
  "requestId": "panel-4821",
  "ok": true,
  "durationMs": 1840,
  "root": "/srv/servers/abc/game",
  "modsFolder": "Modstoke/Mods",
  "serverState": "stopped",
  "result": { },
  "error": null
}
```

On failure `ok` is `false`, `result` is `null`, and `error` is `{"code", "message", "path"?, "violations"?}`. `message` is plain English for people; programs branch on `code`.

| code | Meaning |
|---|---|
| `invalid-arguments` | Bad or missing options |
| `not-a-server-root` | No game or dedicated-server executable in `--root` |
| `server-running` | host-status.json is fresh and `--server-stopped` was not given |
| `server-state-unknown` | No host-status.json and no `--server-stopped` |
| `busy` | Another installer, or an `apply`, holds `.openacai-native-update.lock` |
| `hash-mismatch` | The archive's SHA-256 differs from `--sha256` |
| `invalid-archive` | Layout, manifest, ids, limits or paths are invalid; `violations` lists them |
| `policy-violation` | Forbidden content such as game files, executables, Loader files or protected paths; `violations` lists them |
| `unsupported-kind` | A BepInEx plugin, which is listed but not installed in version 1 |
| `missing-dependency` | A required mod is not installed or queued at the exact version it names |
| `conflict` | The mod conflicts with an installed or queued one, or another mod requires it |
| `not-installed` | No installed mod with that id |
| `not-queued` | No queued request with that id (`cancel`) |
| `not-owned` | Manual or Vortex-managed files, which are never touched |
| `no-history` | No kept archive for that rollback |
| `pending-unfinished` | An interrupted change must be recovered first, with `recover` or `apply` |
| `queue-full` | 64 requests are already queued |
| `io` | File system error, with `path` when one is known |
| `internal` | A fault in the tool itself (exit `1`) |

A change that was made but whose sidecar record could not be written, or whose recovery data could not be cleaned up afterwards, still succeeds; its `result` then has `notices`, a list of plain-English lines. Leftover recovery data is finished by the next `recover` or `apply`, and until then native installs answer `pending-unfinished`.

A native mod whose `minLoaderVersion` is above the installed Loader's version (from `Modstoke/Runtime/openacai-loader.manifest.json`), or cannot be read, also gets a notice from `inspect`, `install` and `queue`, and from `inventory` while it is not disabled. It is not refused, since the Loader may be updated alongside it, but the Loader will not load it until then. A minimum without a pre-release ignores the Loader's own pre-release tag, so `0.4.2` accepts Loader `0.4.2-net11-preview`; a minimum with one uses SemVer order. `+build` metadata is ignored on both, a blank value means no minimum, and a value that is not a string, has a `v` prefix or an empty pre-release is unreadable. With no Loader installed, or a Loader version that cannot be read, only an unreadable minimum is reported.

`violations` is `[{"path", "rule", "message"}]`. Every per-file problem is reported at once. A structural problem, such as a missing manifest, ends the scan early and is reported the same way.

## Server state

- The server counts as running while `host-status.json` in the active runtime folder has `updatedAt` (ISO UTC, `yyyy-MM-ddTHH:mm:ss.fffZ`) less than 15 seconds old. Loader 0.4.2 and later rewrite it every 2 seconds and stop at shutdown.
- The active runtime folder is `Modstoke/Runtime`, or `OpenACAI/Runtime` for an old Loader.
- Its `pid` is a Wine process id and is ignored.
- Without the file, `--server-stopped` decides.
- An `updatedAt` more than 15 seconds in the future, or one that is not a real date and time, counts as unknown.
- The lock file only stops two installers from running at once, because Wine file locks are not visible to native Linux processes.

## Kinds

| kind | Archive layout | Install |
|---|---|---|
| `native` | `Modstoke/Mods/<Folder>/`, or `OpenACAI/Mods/<Folder>/`, which installs into the active Mods folder. `Mods` is spelled exactly | Yes |
| `redloader` | Only `Mods/` and `Libs/`, spelled exactly, with exactly one `manifest.json` 3 to 6 levels deep | Yes; the files it installs are recorded with their hashes in `modstoke-native-installs.json`, and it never replaces a file it did not install, one another RedLoader mod owns, or one that changed since it installed it. A path that differs only in letter case from one already in the folder, or from another path in the same archive, is refused on every system, and so is an update while `Mods/<name>.dll.disabled` exists |
| `bepinex` | `BepInEx/plugins/` | Listed only |

Entry names written with backslashes, as some Windows tools write them, are read as folders on every platform. Spelling rules matter because Linux folders are case-sensitive: `mods/` beside `Mods/` would be a second folder the game never reads.

Neither kind may install programs or scripts (`.exe`, `.com`, `.bat`, `.cmd`, `.ps1`, `.vbs`, `.scr`, `.msi`, `.sh`); they are refused as `policy-violation`.

## Protected paths

- `Modstoke/UserData` and `OpenACAI/UserData` are never removed.
- `Modstoke.Virginia/assets/` under either Mods folder is private content, not part of the Virginia package. Nothing under it is written, replaced, deleted or listed.
  - A plan path under it is refused as `policy-violation`, whatever the ownership records say.
  - Removing Virginia leaves its folder in place while `assets/` remains.
- The Manager enforces the same rules.

## Records

- `modstoke-profile.lock.json` is the Manager's ownership record for native mods; this tool reads and writes it the same way.
- `modstoke-native-installs.json` holds what the profile has no place for: `installedAt`, `source`, the request id, kept archives, and the files each RedLoader mod owns. While a RedLoader install commits, its new record waits under `pending` beside the old one, and the next change, `recover` or `apply` keeps it if the new files are in place or drops it if the old ones are back, judging by the files whose two versions differ: earlier contents back mean it was undone, new contents in place mean it was not. A file the mod changed or deleted afterwards does not block it. If the file exists but cannot be read, or a RedLoader record lacks a file's path or hash, every change is refused as `conflict` until it is restored or fixed; `inventory` still lists mods and reports it in `notices`.
- Read-only commands (`inventory`, `status`, `inspect`) never write either file.

### Moved folders

The profile names the folder it was written for. When the server folder has been moved or restored to another path:
- Read-only commands read it as if it named the new folder, and change nothing.
- The first command that changes mods (`install`, `remove`, `enable`, `disable`, `rollback`, `apply` or `recover`) points it at the new folder. That includes a folder prepared on the other operating system and uploaded, and a moved folder with a link left at its old path. The Manager's recovery point cannot follow, because its backups are tied to the old path, so it is released and its backups are deleted.
- It is never re-pointed when the old path still leads to the same folder by another route, such as a network path or a second mount; that is refused as `conflict`, naming the recorded path to pass as `--root`. On Windows an old path that still exists is treated this way too, since a copy cannot be told apart from the same folder. Linux under WSL reaching a folder the Manager recorded by its Windows path is refused the same way.
- An unfinished install, or a rollback that stopped part way, can only be finished in the folder it started in, so changes answer `pending-unfinished` until the folder is moved back and `recover` runs there. If a link was left at the old path, the link has to be replaced by the folder itself first.
- `inspect` reports any of these refusals as a violation, `queue` refuses with the same code, and `inventory` lists the reason in `notices`.

## Commands

### inventory

Read-only, so it runs any time. It lists every mod in the installation and the queue.

```json
"result": {
  "packages": [
    { "id": "com.openacai.sotf.admin", "name": "Modstoke Admin", "version": "0.5.221", "author": "OpenACAI",
      "kind": "native", "owner": "manager", "enabled": true, "clientRequirement": "server",
      "folder": "Modstoke.Admin", "sizeOnDisk": 2183168,
      "manifestPath": "Modstoke/Mods/Modstoke.Admin/openacai.mod.json",
      "assemblyPath": "Modstoke/Mods/Modstoke.Admin/Modstoke.Admin.dll",
      "assemblySha256": "<64 lowercase hex>",
      "installedAt": null, "source": null, "state": "candidate",
      "history": [] }
  ],
  "notices": [],
  "pending": []
}
```

Every row has the same fields, whatever its `kind`; those that don't apply are `null`. `state` is the Loader's view of the package. For native rows it is `candidate` (complete and loadable), `disabled`, `library`, `metadata-only`, `incomplete` or `ambiguous`; for RedLoader and BepInEx rows it is `enabled` or `disabled`. `enabled` only says whether the package is renamed to be skipped; only a native `candidate` is loaded. A RedLoader row's `sizeOnDisk` counts the files the tool recorded for it, under `Mods/` and `Libs/` alike. RedLoader mods this tool installed are listed even without a `Mods/<name>.dll`, for example libraries under `Libs/`. Manifests under Virginia's `assets/` are never listed.

`owner` is one of:
- `manager`: installed by the Manager or by this tool, so it can be updated and removed;
- `manual`: files with no record, never touched;
- `vortex`: deployed by Vortex, never touched;
- `unknown`: the profile could not be read, which `notices` explains.

`clientRequirement` is the manifest's `client` value, `required`, `optional` or `server`, and `unknown` when the manifest has none. `installedAt`, `source` (`{"type": "store", "id": "..."}` or `{"type": "upload", "label": "..."}`) and the request id are kept in `modstoke-native-installs.json` next to the installation profile. They are not in the profile itself, so that Manager 0.3.0 can still read the profile. Mods installed by the Manager show them as `null`.

### inspect --zip <path> --sha256 <hex>

Read-only, so it runs any time. It runs every check that `install` would, and changes nothing.

```json
"result": {
  "manifest": { "id": "virginia", "name": "Virginia", "version": "0.1.2", "author": "OpenACAI", "description": null,
                "kind": "native", "clientRequirement": "required", "dependencies": [], "minimumLoader": null },
  "archiveSha256": "<64 lowercase hex>", "size": 1048576,
  "packagePath": "Modstoke/Mods/Modstoke.Virginia",
  "manifestPath": "Modstoke/Mods/Modstoke.Virginia/openacai.mod.json",
  "assemblyPath": "Modstoke/Mods/Modstoke.Virginia/Virginia.dll",
  "files": [ { "path": "Modstoke/Mods/Modstoke.Virginia/Virginia.dll", "size": 40960, "sha256": "<64 lowercase hex>", "target": "Modstoke/Mods/Modstoke.Virginia/Virginia.dll" } ],
  "action": "update",
  "replaces": { "modId": "virginia", "previousVersion": "0.1.1", "owner": "manager" },
  "violations": []
}
```

Manifest fields are filled as far as the mod's manifest declares them:
- `author` comes from `author`, `creator`, `createdBy`, `created_by` or `owner`.
- `description` is the manifest's `description`, which the Loader shows in its `mods` list.
- `minimumLoader` is the manifest's `minLoaderVersion` (`major.minor.patch` with an optional `-pre-release`), or `null`. Dependencies are exact versions.

`action` is `install` or `update`, or `null` when there is any violation, an unfinished install (rule `pending`) included. RedLoader results have `packagePath` and `manifestPath` set to `null`.

### install --zip <path> --sha256 <hex>

Needs the server stopped. It runs every check `inspect` runs, then installs or updates one mod.
- A disabled mod must be enabled before it is updated.
- If the install fails after it recorded its plan, it is rolled back with the Manager's recovery point before the command returns, so the folder is as it was. The exception is a server that starts while the install runs: the install stops as `server-running`, and the next `apply` or `recover` rolls the plan back.
- With an unfinished native install recorded, even for the same archive, every change, RedLoader ones included, answers `pending-unfinished` until `recover` or `apply` runs.
- `keptPaths` lists configuration files the player already had, which the update left alone, and Virginia's `assets/` folder.

```json
"result": { "modId": "virginia", "version": "0.1.2", "action": "updated", "previousVersion": "0.1.1",
            "filesWritten": 14, "filesRemoved": 1, "keptPaths": ["Modstoke/Mods/Modstoke.Virginia/assets/"] }
```

Each install keeps its archive in `Modstoke/Runtime/native-history/<modid>/<version>-<sha256>.zip`, where `<modid>` is the mod ID in lower case; inventory `history` entries give each one's version and hash. The three newest are kept: the installed one and the two before it. A mod the Manager installed has no kept archive until this tool installs or updates it, and neither has an install that a crash interrupted.

### remove --id <modId>

Needs the server stopped. It removes only the files recorded for that mod.
- Another installed mod that requires it, or a recorded file that changed, stops the removal before anything changes.
- A disabled mod is removed where it is, and stays disabled if the removal fails.
- The record is dropped only once the files are gone. After a crash in between, removing the mod again finishes the job.

```json
"result": { "modId": "virginia", "version": "0.1.2", "filesRemoved": 14, "keptPaths": [] }
```

### enable --id / disable --id

Needs the server stopped. `result` is `{ "modId", "enabled", "changed" }`, with `changed` false when the mod already was in that state.
- A disabled native mod is its folder renamed to `<Folder>.disabled`.
- A disabled RedLoader mod has `.disabled` added to its `Mods/<name>.dll`. A RedLoader mod without one cannot be disabled.
- The records keep the enabled location; the sidecar notes the mod as disabled.
- The Loader lists such mods but never loads them. That covers native and RedLoader package or assembly paths ending in `.disabled` or `.pending`, or under a `_Disabled` folder, and BepInEx plugin paths with a segment ending in `.disabled` or named `_Disabled`.
- `.pending` marks unapplied staging only and is never used to disable a mod.

### rollback --id <modId> [--to <version>]

Needs the server stopped. It reinstalls a kept archive through the normal install path. By default that's the kept version with the highest version number below the installed one, so rolling back again goes further back; `--to` picks any kept version.

### queue

```
queue install --zip <path> --sha256 <hex>
queue remove|enable|disable --id <modId>
queue rollback --id <modId> [--to <version>]
```

Runs while the server runs or is stopped. It takes the update lock, so it answers `busy` while an `apply` runs.
- It checks everything the direct command would check without the server stopped, against the state the queue will have reached by then: it plays the queued requests forward from the installed mods, so a dependency must be installed and enabled, or queued earlier, at its exact version; a removal must not leave a mod installed or queued earlier that requires it, even a disabled one; and a RedLoader install may not ship a file an earlier queued RedLoader install ships, put a file where one installs a folder or the reverse, or spell such a folder differently. Files of a mod with an earlier queued removal are free to take. For enable, disable and rollback it checks that the mod's files are there and, for a rollback, that it is enabled.
- For an install it copies the archive to `Modstoke/Runtime/pending/archives/<sha256>.zip` and writes a request. `--kind`, `--source` and `--request-id` are kept with the request and used when it is applied.
- A new request for a mod replaces that mod's queued requests. It takes the earliest one's place in the order when the queue still works with it there, and goes last otherwise; when neither works it is refused. A queue that already fails without it is not held against it.
- The new request appears only once the requests it replaces are deleted, so a failure part way never leaves both queued.

```json
"result": { "request": { }, "position": 2, "superseded": ["8f3c0d1e9a7b4c21"] }
```

### apply

Needs the server stopped. It first runs `recover`, then applies the queued requests in file-name order.
- Each request is one transaction with staging and rollback, and is deleted once applied.
- It stops at the first failure; that request and everything after it stay queued.
- Rerunning is safe. After a crash, the next `apply` first resumes or restores the interrupted change.
- When a request fails, the envelope has `ok: false` and the failed request's `error`, with its `violations`, and still carries `result`. The exit code follows that error's code.
- A queued install uses the kind, source and request id it was queued with, not those given to `apply`.
- A queued removal whose mod is already gone, as after a crash between the removal and the deletion of its request, counts as applied, with `outcome` `absent`.
- Each `applied` entry has the request's `id`, `action`, `modId` and `version`. Installs and rollbacks add `outcome` (`installed` or `updated`), `filesWritten`, `filesRemoved` and `keptPaths`; removals add `filesRemoved` and `keptPaths`; enable and disable add `changed`.

```json
"result": {
  "recovered": [],
  "applied": [ { "id": "8f3c0d1e9a7b4c21", "action": "install", "modId": "virginia", "version": "0.1.2",
                 "outcome": "updated", "filesWritten": 14, "filesRemoved": 1, "keptPaths": [] } ],
  "failed": null,
  "remaining": 0
}
```

### cancel --request <id> | --all

Deletes queued requests, and archives no other request uses. It never touches installed files. `result` is `{ "cancelled": [ids], "remaining": n }`; an unknown `--request` answers `not-queued`.

### recover

Needs the server stopped. It finishes or restores a file change a crash interrupted, deletes one a crash stopped before it changed anything along with archive copies a killed install left, rolls back an install whose plan was recorded but not finished, using the Manager's recovery point, and finishes cleaning up an earlier recovery point. `result` is `{ "recovered": ["what it did", ...] }`, empty when there was nothing to do. Direct commands finish or restore an interrupted file change on their own, but answer `pending-unfinished` for an unfinished install until `recover` or `apply` runs.

### status

Read-only. Returns `{ "serverState", "hostStatusUpdatedAt", "modsFolder", "pending": [] }`.

## Pending requests

- Each request is a file, `Modstoke/Runtime/pending/<queuedUtc as yyyyMMddTHHmmssfffZ>-<id>.json`, so file-name order is apply order. A request that took a replaced request's place keeps that request's time in its file name.
- There are at most 64 files of up to 64 KiB each.
- The Loader reports them as `pendingMods` in host-status.json and never applies them.

```json
{
  "schema": 1,
  "kind": "modstoke.pending-native-change",
  "id": "8f3c0d1e9a7b4c21",
  "action": "install",
  "modId": "virginia",
  "version": "0.1.2",
  "archiveSha256": "<64 lowercase hex>",
  "archivePath": "pending/archives/<64 lowercase hex>.zip",
  "queuedUtc": "2026-11-05T14:02:11.482Z",
  "requestedBy": "alex (panel)",
  "packageKind": "native",
  "source": { "type": "store", "id": "virginia" },
  "requestId": "panel-4821",
  "requires": [ { "id": "sonsaxlib", "version": "1.2.0" } ]
}
```

`action` is `install`, `remove`, `enable`, `disable` or `rollback`. Only `install` carries `archiveSha256`, `archivePath`, `packageKind` and `source`. `requires` lists the exact versions a native install or rollback needs, and is empty otherwise. For the others they are `null`, and `version` is the installed version, or the target version for a rollback.

## Hosting flow

- **Running server:** queue the change. The panel shows `pendingMods`. On restart: stop, then take a world backup if enabled, then `apply`, then start.
- **Requirements file:** when a dedicated server becomes ready, Loader 0.4.2 and later regenerate `Modstoke/Runtime/server-requirements.json` from the installed mods; `requirements generate` does it on demand. The tool never writes it. Admin's own required-mods file is separate and the tool doesn't change it.
- **Stopped server:** run the command directly with `--server-stopped`.
