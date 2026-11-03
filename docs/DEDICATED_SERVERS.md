# Dedicated servers

Modstoke Manager, by OpenACAI, works with a Sons of the Forest dedicated server installed on this PC. After you add and select a server under **Dedicated servers**, it has six tabs: **Server** (status, start, stop and restart, server name, player limit, log files and data folder), **Game settings**, **Staff** (Modstoke Admin permissions), **Console** (game log, Loader log and Loader console), **Packages** (native packages and recovery) and **World backups**. Manager does not install the dedicated server, open firewall ports or set up port forwarding.

## Prepare the server

Install and configure the game's dedicated server through its normal setup.
Follow the [dedicated-server configuration guide](https://steamcommunity.com/sharedfiles/filedetails/?id=2992700419).
Keep the server installation separate from the player installation.

Open **Dedicated servers** in Manager, add `SonsOfTheForestDS.exe`, select that installation and choose
**Read status**. **Choose data folder** selects an existing folder
containing `dedicatedserver.cfg`. This saves a Manager preference; it does not
move files or change the native configuration, owner list or saves.

Manager shows the server name, save mode, save slot and configured player limit.
The player limit is configuration information, not a qualified capacity claim.
Passwords and the rest of the configuration are not displayed.

## Change the server name

Read status, enter a name under **Server name**, then review and save the change.
Names must contain 1–256 characters without control characters. Manager trims
surrounding spaces. The server and player game must be stopped. The change keeps
all other configuration values and uses the same original-file backup and revision
checks as a player-limit change. Read status again after saving.

## Change the player limit

Close the player game and stop the dedicated server, then read status. Under
**Player limit**, choose 8, 16 or 32, select **Review limit**,
and confirm the change. Limits above 8 require an explicit acknowledgement:
the unmodified server checks `MaxPlayers` at startup, accepts 1 to 8, and will
not start with 16 or 32. Those limits need a Modstoke Loader build that supports
them on dedicated servers and remain experimental.
Saving a number does not install Loader, start the server or establish that the
chosen population works in multiplayer. Qualification proceeds at 8, 16 and 32
players, including admission, rosters, reconnects, saves and replication cost.

Manager changes only the `MaxPlayers` value, adding the property if it is absent.
It preserves other configuration bytes, including unknown properties, formatting
and the UTF-8 byte-order mark. An existing limit outside these choices remains
unchanged until you explicitly select a new value. The selected installation,
data folder and original configuration must still match the reviewed observation.
Pending package or world recovery blocks the change.

Before replacing the configuration, Manager saves and verifies an exact copy in
`.modstoke-config-backups` inside the data folder. The filename contains the original
configuration's SHA-256 hash. Up to 64 entries are retained without automatic pruning;
an identical existing backup can be reused. These are complete native configuration
files and can contain passwords, so keep them private. To restore one manually, stop
the game and server, preserve the current file, and copy the chosen backup to
`dedicatedserver.cfg`. Read status again afterward.

The write replaces one file after its backup has been saved. If Manager closes or
reports an uncertain result, read the current configuration before trying again;
it may already contain the new limit. Manager does not retry or restore automatically.
External editors and processes can still act independently of Manager's locks.

## Start and observe

Use **Start server** after reading the configuration summary. Manager
starts the selected executable in its installation folder, passing the chosen
folder with the game's `-userdatapath` argument. It does not invoke the player
Steam launch, append custom arguments or run a batch file.

Launch requires `SaveMode` to be `Continue`, including the native default when
the field is absent. `New` overwrites the selected save slot on each native
server start, so Manager blocks that mode instead of changing it. A missing,
unsupported or unreadable configuration also blocks launch. Manager accepts
UTF-8 JSON up to 64 KiB with ASCII top-level property names, and rejects duplicate
top-level properties, comments and trailing commas. Configure or correct the native file outside Manager, then
read it again.

A changed data-folder preference or configuration requires another read before
launch. An unfinished Manager package plan, update transaction or pack recovery
also blocks launch. The launch action never performs package recovery itself.

Status reports the process observed at the last refresh. A running process does
not establish that startup checks passed, clients can join, mods loaded or the
world is ready. Manager records the exit code of the process it started; this
is not confirmation that a save completed. Refresh status to observe changes.

## Game settings

The **Game settings** tab edits the server's game mode, autosave interval,
world settings, custom-game settings, frame rates and network options in
`dedicatedserver.cfg`. It offers only the settings and values the dedicated
server reads, and **Game default** removes a setting so the game's own value
applies. Review the list of changes, then save. Saving uses the same backup and
checks as the other settings, needs the server and game closed, and applies the
next time the server starts.

Game mode and the custom-game settings shape a new world only. They are used
when SaveMode is New, or when SaveMode is Continue and the save slot is still
empty. An existing world keeps its own settings. Tree regrowth and structure
damage apply on every start. Entries the server does not read, and values
Manager does not recognize, are kept unless you choose a new value.

## Stop and restart

**Stop** sends Ctrl+C to the server's own console window, the shutdown the
native server expects, and status shows **Stopping** until the process exits.
Endnight's patch notes say the dedicated server saves before it shuts down;
Manager does not confirm that a save happened.

**Restart** stops the server the same way and starts it again once it has
exited, using the data folder and configuration in place when you chose
Restart. If they changed or another launch check fails, the server stays
stopped and status shows why. The restart continues if you leave the page.

**Force stop** ends the process immediately after you confirm it. The server
cannot save, so progress since its last save is lost. A forced stop cancels a
pending restart.

Manager can stop only a server it started. Closing Manager leaves the server
running. After Manager restarts, an existing process is reported as external;
ownership is not inferred from its process ID. Stop an external server with
Ctrl+C in its own console window.

## Console

The **Console** tab follows the server's log files. **Game log** reads the
newest `sotf_log*.txt` in the data folder's `logs` folder. The native server
writes it only while **Log files** is on under Server settings. That setting
changes `LogFilesEnabled` in `dedicatedserver.cfg` with the same backup and
checks as the other settings, and applies the next time the server starts.
**Loader log** reads `BepInEx\LogOutput.log` in the server installation when
Modstoke Loader is installed there.

Lines can trail the server's own window by a few seconds, and the view keeps
the latest 2,000 lines. These two views only read files.

**Loader console** connects to Modstoke Loader in the running server. It shows
the Loader's live output and sends its commands: type a command or pick one
from the list, add arguments if it takes them, and Run. Commands that ask for
confirmation wait until you confirm. A command is only queued when it is sent,
so its result appears later under **Recent results**, which can also include
results of commands sent from another Loader console view. Commands typed in
the server's own window print their results there and in the Loader log
instead. Admin treats commands from this console as coming from the host. The
console needs the server to be running with Modstoke Loader, and connects only
to the server process started from this installation. When the server stops,
its last output stays until it starts again. The Loaders page has the same
console for a running game.

## Staff permissions

The **Staff** tab edits Modstoke Admin's `permissions.json` in the server
installation's `BepInEx\plugins\OpenACAIAdminTool` folder. A player
installation has the same editor under **Multiplayer**, for games you host.
Admin 0.5.216 or later creates the file on its first start and moves any
operators from `settings.ini` into it; until then there is nothing to edit.

- **Access** turns remote F7 staff tools on or off, sets the lowest rank that
  may use them, and can give every player a floor role. The game's own Owner,
  Admin, Trusted and Guest roles map onto Admin roles here.
- **Roles** have a name, an id, a rank from 0 to 999, the lower-ranked roles
  they inherit, and allow and deny lists. A pattern like `moderate.*` or `*`
  also covers nodes a later Admin version adds, but never the sensitive nodes,
  which have to be named exactly. Deny always beats allow, and staff can only
  act on players ranked below them. From Admin 0.5.217, staff with
  `permissions.edit` can also edit roles in F7, but only roles ranked below
  their own that no game role or the role for every player uses, and only with
  nodes their own lasting role holds.
- **Staff** gives a SteamID64 a role, with optional extra nodes, a note and an
  expiry time. An expired entry counts for nothing. While a player's game role
  is Guest, for example after a quarantine, their role and extra allow entries
  are set aside, but their extra denies still apply. Staff with
  `permissions.assign` can also give roles in F7, but only roles ranked below
  their own whose nodes they hold, to players ranked below them, and never to
  themselves.

Manager checks the file against Admin's rules and suggests node names from the
`permission-nodes.json` list Admin writes. Review the changes, then save.
Saving works while the game or server runs: Admin picks up the file within
about 5 seconds, and Manager then shows whether Admin is using it or rejected
it. If the file changed since it was read, for example in F7, nothing is saved
and you read it again. Each save first keeps a copy of the previous file in
Manager's settings folder.

## World backup and recovery

Close the player game and stop the dedicated server through its normal controls
before using **World backups** in the server workspace. Read its status, choose the existing native save folder
inside the configured data folder, then read status again. The folder must
contain `SaveData.zip` or the loose `SaveData.json`. Manager does not infer the
dedicated server's save directory from `SaveSlot`; select the actual world you
intend to preserve.

**Create complete backup** copies every file and directory in that folder,
including loose fallback files. Modstoke's current world records are stored
inside the native save and travel with it. Manager checks the complete file
inventory and contents before publishing the backup. This preserves the bytes
present while stopped; it does not establish that the game's last save completed
successfully or that the world will load.

Choose a backup, use **Review restore**, and confirm the selected installation
and save folder. Restore stages the complete saved folder and retains an undo
backup of the current one before replacing it. Backups are bound to the same
installation, data folder and selected save folder. Server configuration,
installed mods and other world folders are outside the restore.

If Manager stops during a restore, use **Read pending recovery** after reopening
it. This also works when the save folder is temporarily absent between directory
moves. **Review finish** continues the recorded restore; **Review undo** returns
the previous folder. Each requires confirmation. Changed or unrecognized files
halt recovery and remain preserved. Manager blocks server launch and data-folder
changes while a restore is pending; it never retries recovery automatically.

Backups and retained restore work are local to `.modstoke-world-backups` in the
configured data folder. A snapshot supports up to 4,096 files and 4,096 directories,
4 GiB total, 1 GiB per file and 16 directory levels. Manager retains at most 64
backups and 64 workspaces of each kind, without automatic pruning. Restoring needs
additional disk space for the staged world, undo backup and displaced folder.
Keep copies outside the server's disk for protection against disk failure.

The current restore implementation requires Windows. These backups include only
the selected native folder. Older mods may keep sidecar records elsewhere; those
need separate recovery and must not be assumed to match the restored world.
External tools can still edit files or start a server independently of Manager's
locks. Backup, recovery after an interruption and loading a restored world in the game have not finished testing yet.

## Package files and recovery

On the **Packages** tab, use **Select ZIP…** under **Install native mod ZIP** to inspect a local native mod
archive for the selected server installation. Review its identity, version,
installation root, destination and required dependencies before choosing
**Install package**, **Update package** or **Resume package**. Close the game and
dedicated server first. Required dependencies must already be installed at their
exact declared versions; updates apply only to packages previously installed by
Manager. Select the same ZIP to resume an interrupted install, or use
**Pack recovery** to restore its previous files and profile records. See
[native package installation](NATIVE_PACKAGES.md) for the archive format and
configuration preservation rules.

Native inventory is a filesystem observation, not proof that a package loaded or
supports the dedicated server. Legacy catalog installation, toggling and removal
remain unavailable for server profiles. Dedicated-server launch and native package
installation, update and recovery have not finished testing yet.

While Manager owns a running server, it retains locks for the installation and
selected data folder. It releases them when it observes the child exit, or when
Manager exits. Reading status or requesting another operation observes that
exit. All Manager operations that take a server installation's update lock also
check for running dedicated-server processes. Unknown process status blocks
changes. These checks use the standard executable name and are snapshots;
external tools can still launch or modify the server independently.

Manager currently allows one dedicated-server process at a time. A detected
server in another installation also blocks launch and data-folder changes.
The player game may remain open. Server inventory can still be inspected while
recovery is blocked. Refresh its inventory after shutdown to recover recognized
interrupted Manager updates. Conflicting files or unfinished plans need their
reported recovery steps resolved before launch.

Data-folder preferences use separate schema-1 files under `server-launch` in
Manager's existing app configuration folder. Installation registry and package
lockfile formats remain unchanged. Re-adding a forgotten installation reuses
its saved data-folder choice; forgetting an installation never deletes data.
To send commands to a running server, use **Loader console** on the **Console** tab. The server needs Modstoke Loader, and the commands offered depend on the Loader and mods installed there.
