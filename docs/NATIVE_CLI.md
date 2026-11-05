# modstoke-native (planned)

`modstoke-native` is a planned command-line tool that installs, removes and queues mods in a dedicated-server or game folder without the Manager's window. Server hosting runs it from its host agent. It is built from the Manager's own code, so it applies the same rules: archive validation, the native package policy, Modstoke and OpenACAI folder resolution, staging with rollback, Vortex ownership and the installation profile.

This page is the agreed contract. The tool has not been built yet.

Builds: Linux x86_64 for hosting containers, and Windows x64, signed like the Manager.

## Invocation

```
modstoke-native <command> --root <server-root> [options]
```

| Option | Meaning |
|---|---|
| `--root` | Folder with `SonsOfTheForestDS.exe` (or `SonsOfTheForest.exe`) |
| `--server-stopped` | The host agent states the server is stopped; overrides host-status.json |
| `--requested-by <label>` | Actor label, up to 64 printable characters, recorded on queued requests |
| `--request-id <id>` | Echoed as `requestId`, up to 64 printable characters |
| `--kind native\|redloader` | Overrides the kind detected from the archive |
| `--progress ndjson` | Progress lines on stderr |

stdout carries exactly one JSON object, UTF-8 on one line. stderr is for logs and, with `--progress ndjson`, one `{"phase":"verify|extract|stage|commit|rollback","done":n,"total":n,"file":"optional"}` line per step.

Exit codes: `0` ok, `1` internal error, `2` refused, `3` server running or state unknown, `4` busy.

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
| `busy` | Another installer holds `.openacai-native-update.lock` |
| `hash-mismatch` | The archive's SHA-256 differs from `--sha256` |
| `invalid-archive` | Layout, manifest, ids, limits or paths are invalid; `violations` lists them |
| `policy-violation` | Forbidden content such as game files, executables, Loader files or protected paths; `violations` lists them |
| `unsupported-kind` | A BepInEx plugin, which is listed but not installed in version 1 |
| `missing-dependency` | A required mod is not installed or queued |
| `loader-too-old` | The mod needs a newer Loader than the one installed |
| `conflict` | The mod conflicts with an installed or queued one |
| `not-installed` | No installed mod with that id |
| `not-owned` | Manual or Vortex-managed files, which are never touched |
| `no-history` | No kept archive for that rollback |
| `pending-unfinished` | An interrupted change must be applied or recovered first |
| `queue-full` | 64 requests are already queued |
| `io` | File system error, with `path` |

`violations` is `[{"path", "rule", "message"}]`. Every per-file problem is reported at once. A structural problem, such as a missing manifest, ends the scan early and is reported the same way.

## Server state

- The server counts as running while `host-status.json` in the active runtime folder has `updatedAt` (ISO UTC, `yyyy-MM-ddTHH:mm:ss.fffZ`) less than 15 seconds old. Loader 0.4.2 and later rewrite it every 2 seconds and stop at shutdown.
- The active runtime folder is `Modstoke/Runtime`, or `OpenACAI/Runtime` for an old Loader.
- Its `pid` is a Wine process id and is ignored.
- Without the file, `--server-stopped` decides.
- The lock file only stops two installers from running at once, because Wine file locks are not visible to native Linux processes.

## Kinds

| kind | Archive layout | Install |
|---|---|---|
| `native` | `Modstoke/Mods/<Folder>/`, or `OpenACAI/Mods/<Folder>/`, which installs into the active Mods folder | Yes |
| `redloader` | `Mods/` and/or `Libs/`, with exactly one `manifest.json` 3 to 6 levels deep | Yes |
| `bepinex` | `BepInEx/plugins/` | Listed only |

## Protected paths

- `Modstoke/UserData` and `OpenACAI/UserData` are never removed.
- `Modstoke.Virginia/assets/` under either Mods folder is private content, not part of the Virginia package. Nothing under it is written, replaced, deleted or listed.
  - A plan path under it is refused as `policy-violation`, whatever the ownership records say.
  - Removing Virginia leaves its folder in place while `assets/` remains.
- The Manager enforces the same rules.

## Commands

### inventory

Read-only, so it runs any time. It lists every mod in the installation and the queue.

```json
"result": {
  "packages": [
    { "id": "com.openacai.sotf.admin", "name": "Modstoke Admin", "version": "0.5.221", "author": "OpenACAI",
      "kind": "native", "owner": "manager", "enabled": true, "clientRequirement": "server-only",
      "folder": "Modstoke.Admin", "sizeOnDisk": 2183168,
      "manifestPath": "Modstoke/Mods/Modstoke.Admin/openacai.mod.json",
      "assemblyPath": "Modstoke/Mods/Modstoke.Admin/Modstoke.Admin.dll",
      "assemblySha256": "<64 lowercase hex>",
      "installedAt": null, "source": null,
      "history": [] }
  ],
  "notices": [],
  "pending": []
}
```

`owner` is one of:
- `manager`: installed by the Manager or by this tool, so it can be updated and removed;
- `manual`: files with no record, never touched;
- `vortex`: deployed by Vortex, never touched.

`installedAt`, `source` (`{"type": "store", "id": "..."}` or `{"type": "upload", "label": "..."}`) and the request id are kept in `modstoke-native-installs.json` next to the installation profile. They are not in the profile itself, so that Manager 0.3.0 can still read the profile. Mods installed by the Manager show them as `null`.

### inspect --zip <path> --sha256 <hex>

Read-only, so it runs any time. It runs every check that `install` would, and changes nothing.

```json
"result": {
  "manifest": { "id": "virginia", "name": "Virginia", "version": "0.1.2", "author": "OpenACAI", "description": "...",
                "kind": "native", "clientRequirement": "all-players", "dependencies": [], "minimumLoader": "0.4.2" },
  "files": [ { "path": "Modstoke/Mods/Modstoke.Virginia/Virginia.dll", "size": 40960, "target": "Modstoke/Mods/Modstoke.Virginia/Virginia.dll" } ],
  "replaces": { "modId": "virginia", "previousVersion": "0.1.1", "owner": "manager" },
  "violations": []
}
```

Manifest fields are filled as far as the mod's manifest declares them.

### install --zip <path> --sha256 <hex>

Needs the server stopped. It checks dependencies and the Loader version, then installs or updates one mod.

```json
"result": { "modId": "virginia", "version": "0.1.2", "action": "updated", "previousVersion": "0.1.1",
            "filesWritten": 14, "filesRemoved": 1, "keptPaths": ["Modstoke/Mods/Modstoke.Virginia/assets/"] }
```

An update keeps the replaced archive in `Modstoke/Runtime/native-history/<modId>/<version>-<sha256>.zip`. The last two versions of each mod are kept.

### remove --id <modId>

Needs the server stopped. It removes only the files recorded for that mod.

```json
"result": { "modId": "virginia", "version": "0.1.2", "filesRemoved": 14, "keptPaths": [] }
```

### enable --id / disable --id

Needs the server stopped.
- A disabled native mod is its folder renamed to `<Folder>.disabled`.
- A disabled RedLoader mod has `.disabled` added to its files.
- The records follow the rename.
- The Loader lists such mods but never loads them. That covers native and RedLoader package or assembly paths ending in `.disabled` or `.pending`, or under a `_Disabled` folder, and BepInEx plugin paths with a segment ending in `.disabled` or named `_Disabled`.
- `.pending` marks unapplied staging only and is never used to disable a mod.

### rollback --id <modId> [--to <version>]

Needs the server stopped. It reinstalls a kept archive through the normal install path. By default that's the newest kept version older than the installed one.

### queue

```
queue install --zip <path> --sha256 <hex>
queue remove|enable|disable --id <modId>
queue rollback --id <modId> [--to <version>]
```

Runs while the server runs or is stopped.
- It checks everything the direct command would check without the server stopped.
- For an install it copies the archive to `Modstoke/Runtime/pending/archives/<sha256>.zip` and writes a request.
- A new request for a mod replaces that mod's queued requests.

```json
"result": { "request": { }, "position": 2, "superseded": ["8f3c0d1e9a7b4c21"] }
```

### apply

Needs the server stopped. It applies the queued requests in file-name order.
- Each request is one transaction with staging and rollback, and is deleted once applied.
- It stops at the first failure; that request and everything after it stay queued.
- Rerunning is safe. After a crash, the next `apply` first resumes or restores the interrupted change.

```json
"result": {
  "applied": [ { "id": "8f3c0d1e9a7b4c21", "action": "install", "modId": "virginia", "version": "0.1.2",
                 "outcome": "updated", "filesWritten": 14, "filesRemoved": 1, "keptPaths": [] } ],
  "failed": null,
  "remaining": 0
}
```

### cancel --request <id> | --all

Deletes queued requests, and archives no other request uses. It never touches installed files.

### status

Read-only. Returns `{ "serverState", "hostStatusUpdatedAt", "modsFolder", "pending": [] }`.

## Pending requests

- Each request is a file, `Modstoke/Runtime/pending/<queuedUtc as yyyyMMddTHHmmssfffZ>-<id>.json`, so file-name order is apply order.
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
  "requestedBy": "alex (panel)"
}
```

`action` is `install`, `remove`, `enable`, `disable` or `rollback`. Only `install` carries `archiveSha256` and `archivePath`. For the others they are `null`, and `version` is the installed version, or the target version for a rollback.

## Hosting flow

- **Running server:** queue the change. The panel shows `pendingMods`. On restart: stop, then take a world backup if enabled, then `apply`, then start.
- **Requirements file:** when a dedicated server becomes ready, Loader 0.4.2 and later regenerate `Modstoke/Runtime/server-requirements.json` from the installed mods; `requirements generate` does it on demand. The tool never writes it. Admin's own required-mods file is separate and the tool doesn't change it.
- **Stopped server:** run the command directly with `--server-stopped`.
