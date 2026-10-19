<script lang="ts">
    import { onDestroy } from "svelte";
    import { get } from "svelte/store";
    import { open as chooseFolder } from "@tauri-apps/plugin-dialog";
    import { open } from "@tauri-apps/plugin-shell";
    import LucidePlay from "~icons/lucide/play";
    import LucideRefreshCw from "~icons/lucide/refresh-cw";
    import LucideFolderOpen from "~icons/lucide/folder-open";
    import { processing, processName } from "./store";
    import { selectedInstallation, type InspectedInstallation } from "./installationProfiles";
    import { readServerLaunch, writeServerLaunch, writeServerCapacity, writeServerIdentity, launchDedicatedServer, type ServerLaunchState } from "./serverLaunch";

    export let installation: InspectedInstallation;

    let busy = false;
    let ownsOperation = false;
    let destroyed = false;
    let generation = 0;
    let activeKey = "";
    let state: ServerLaunchState | null = null;
    let error = "";
    let notice = "";
    let observedAt = "";
    let nameChoice = "";
    let capacityChoice = 0;
    let experimentalAccepted = false;
    let capacityReview: { key: string; revision: string; configRevision: string; dataDirectory: string; from: number; to: number } | null = null;
    let nameReview: { key: string; revision: string; configRevision: string; configPath: string; from: string; to: string } | null = null;

    function clearNameReview() { nameReview = null; }

    function clearCapacityReview() {
        capacityReview = null;
        experimentalAccepted = false;
    }

    function clearReviews() {
        clearNameReview();
        clearCapacityReview();
    }

    function nameProblem(value: string): string {
        const name = value.trim();
        if (!name) return "Enter a server name.";
        if (Array.from(name).length > 256) return "Use no more than 256 characters.";
        if (/[\u0000-\u001f\u007f-\u009f]/u.test(value)) return "Use a name without control characters.";
        return "";
    }

    function observe(result: ServerLaunchState) {
        state = result;
        observedAt = new Date().toLocaleTimeString();
        nameChoice = result.config?.serverName ?? "";
        capacityChoice = result.config && [8, 16, 32].includes(result.config.maxPlayers) ? result.config.maxPlayers : 0;
        clearReviews();
    }

    function keyOf(value: InspectedInstallation | null): string {
        return value ? JSON.stringify([value.target.id, value.target.kind, value.target.executable, value.root, value.available]) : "";
    }

    function reset(key: string) {
        if (key === activeKey) return;
        activeKey = key;
        generation++;
        state = null;
        observedAt = "";
        nameChoice = "";
        capacityChoice = 0;
        clearReviews();
        error = "";
        notice = "";
    }

    const unsubscribe = processing.subscribe(value => {
        if (!value || ownsOperation) return;
        generation++;
        state = null;
        observedAt = "";
        clearReviews();
        error = "";
        notice = "Read status again after the current Manager operation finishes.";
    });
    const unsubscribeSelection = selectedInstallation.subscribe(value => {
        if (keyOf(value) === activeKey) return;
        generation++;
        state = null;
        observedAt = "";
        clearReviews();
    });

    $: reset(keyOf(installation));
    $: available = installation.available && !!installation.root && installation.target.kind === "server"
        && keyOf($selectedInstallation) === keyOf(installation);
    $: canEditConfiguration = available && !!state?.config && !!state?.dataDirectory
        && state.revision !== null && state.configRevision !== null && state.canSave && state.processState === "stopped";
    $: nameError = nameProblem(nameChoice);
    $: processLabel = !state ? "Not checked" : state.processState === "running" ? "Running"
        : state.processState === "stopped" ? "Stopped" : state.processState === "other" ? "Another process observed" : "Unknown";
    $: problems = state ? [...new Set([state.configProblem, state.processProblem, state.saveProblem, state.launchProblem]
        .filter((problem): problem is string => !!problem))] : [];

    function canStart(): boolean {
        return !destroyed && !busy && !get(processing) && installation.available && !!installation.root
            && installation.target.kind === "server" && keyOf(get(selectedInstallation)) === keyOf(installation);
    }

    function current(request: number, key: string): boolean {
        return !destroyed && request === generation && keyOf(installation) === key
            && keyOf(get(selectedInstallation)) === key && (ownsOperation || !get(processing));
    }

    function beginOperation(label: string) {
        clearReviews();
        busy = true;
        ownsOperation = true;
        error = "";
        notice = "";
        processName.set(label);
        processing.set(true);
    }

    function endOperation() {
        processName.set("");
        processing.set(false);
        ownsOperation = false;
        busy = false;
    }

    async function readStatus() {
        if (!canStart()) return;
        const target = installation.target;
        const key = keyOf(installation);
        const request = ++generation;
        busy = true;
        state = null;
        observedAt = "";
        clearReviews();
        error = "";
        notice = "";
        try {
            const result = await readServerLaunch(target.executable, target.id);
            if (current(request, key)) observe(result);
        } catch (failure) {
            if (current(request, key)) error = `Could not read server status: ${failure}`;
        } finally { busy = false; }
    }

    async function selectDataFolder() {
        if (!canStart() || !state?.canSave || state.processState !== "stopped") return;
        const target = installation.target;
        const revision = state.revision;
        const key = keyOf(installation);
        const request = ++generation;
        let writing = false;
        beginOperation("Selecting dedicated-server data folder...");
        try {
            const folder = await chooseFolder({ directory: true, multiple: false, title: "Choose existing dedicated-server data folder" });
            if (!current(request, key) || typeof folder !== "string") return;
            writing = true;
            processName.set("Saving dedicated-server data folder...");
            const result = await writeServerLaunch(target.executable, target.id, revision, folder);
            if (current(request, key)) {
                observe(result);
                notice = "Data folder selection saved in Manager. Server configuration and saves were not changed.";
            }
        } catch (failure) {
            if (current(request, key)) {
                if (writing) state = null;
                error = writing
                    ? `Save could not be confirmed: ${failure}. It may already have been saved. Read status again before continuing.`
                    : `Could not select a folder: ${failure}`;
            }
        } finally { endOperation(); }
    }

    async function startServer() {
        if (!canStart() || !state?.canLaunch || state.revision === null || state.configRevision === null) return;
        const target = installation.target;
        const revision = state.revision;
        const configRevision = state.configRevision;
        const key = keyOf(installation);
        const request = ++generation;
        beginOperation("Starting dedicated server...");
        try {
            const result = await launchDedicatedServer(target.executable, target.id, revision, configRevision);
            if (current(request, key)) {
                observe(result);
                notice = "Start request returned. The process observation is shown above; multiplayer readiness has not been checked.";
            }
        } catch (failure) {
            if (current(request, key)) {
                state = null;
                error = `Start could not be confirmed: ${failure}. The process may already have started. Read status again before continuing.`;
            }
        } finally { endOperation(); }
    }

    async function openDataFolder() {
        if (!canStart() || !state?.dataDirectory) return;
        const folder = state.dataDirectory;
        const key = keyOf(installation);
        const request = ++generation;
        beginOperation("Opening dedicated-server data folder...");
        try { await open(folder); }
        catch (failure) {
            if (current(request, key)) error = `Could not open data folder: ${failure}`;
        } finally { endOperation(); }
    }

    function reviewCapacity() {
        if (!canStart() || !canEditConfiguration || !state?.config || !state.dataDirectory
            || state.revision === null || state.configRevision === null || ![8, 16, 32].includes(capacityChoice)
            || capacityChoice === state.config.maxPlayers) return;
        clearReviews();
        error = "";
        notice = "";
        capacityReview = {
            key: keyOf(installation), revision: state.revision, configRevision: state.configRevision,
            dataDirectory: state.dataDirectory, from: state.config.maxPlayers, to: capacityChoice
        };
    }

    async function saveCapacity() {
        const review = capacityReview;
        if (!canStart() || !canEditConfiguration || !review || review.key !== keyOf(installation)
            || review.revision !== state?.revision || review.configRevision !== state?.configRevision
            || review.to !== capacityChoice || (review.to > 8 && !experimentalAccepted)) return;
        const target = installation.target;
        const acknowledged = experimentalAccepted;
        const key = keyOf(installation);
        const request = ++generation;
        beginOperation("Saving dedicated-server player limit...");
        state = null;
        try {
            const result = await writeServerCapacity(target.executable, target.id, review.revision,
                review.configRevision, review.to, acknowledged);
            if (current(request, key)) {
                notice = `${result.changed ? "Saved" : "Already configured for"} a player limit of ${result.maxPlayers}.`
                    + (result.backupPath ? ` Original configuration backup: ${result.backupPath}.` : "")
                    + " Read status again before continuing. The server was not started.";
            }
        } catch (failure) {
            if (current(request, key)) {
                error = `Capacity save could not be confirmed: ${failure}. Read status again before continuing; the configuration may already have changed.`;
            }
        } finally { endOperation(); }
    }

    function reviewName() {
        const name = nameChoice.trim();
        if (!canStart() || !canEditConfiguration || !state?.config || !state.dataDirectory
            || state.revision === null || state.configRevision === null || nameProblem(nameChoice)
            || name === state.config.serverName) return;
        clearReviews();
        error = "";
        notice = "";
        nameReview = {
            key: keyOf(installation), revision: state.revision, configRevision: state.configRevision,
            configPath: state.configPath ?? `${state.dataDirectory}\\dedicatedserver.cfg`,
            from: state.config.serverName, to: name
        };
    }

    async function saveName() {
        const review = nameReview;
        if (!canStart() || !canEditConfiguration || !review || review.key !== keyOf(installation)
            || review.revision !== state?.revision || review.configRevision !== state?.configRevision
            || review.to !== nameChoice.trim() || nameProblem(nameChoice)) return;
        const target = installation.target;
        const key = keyOf(installation);
        const request = ++generation;
        beginOperation("Saving dedicated-server name...");
        state = null;
        try {
            const result = await writeServerIdentity(target.executable, target.id, review.revision,
                review.configRevision, review.to);
            if (current(request, key)) {
                notice = `${result.changed ? "Saved server name" : "Server name is already"} “${review.to}”.`
                    + (result.backupPath ? ` Original configuration backup: ${result.backupPath}.` : "")
                    + " Read status again before continuing. The server was not started.";
            }
        } catch (failure) {
            if (current(request, key)) {
                error = `Server name save could not be confirmed: ${failure}. Read status again before continuing; the configuration may already have changed.`;
            }
        } finally { endOperation(); }
    }

    onDestroy(() => { destroyed = true; generation++; unsubscribe(); unsubscribeSelection(); });
</script>

<div class="server-launch">
    <section class="status-panel" aria-labelledby="server-launch-heading">
        <div class="heading-row">
            <div class="heading-copy">
                <p class="eyebrow">Server control</p>
                <h2 id="server-launch-heading">{state?.config?.serverName || installation.target.name}</h2>
            </div>
            <span class="process-state" class:observed={!!state} class:running={state?.processState === "running"}>{processLabel}</span>
        </div>
        <p class="status-description">
            {#if !available}Choose an available dedicated-server installation to read status.
            {:else if !state}Read status to load the configuration and observe the server process.
            {:else if state.processState === "running"}Manager-owned process observed{state.processId !== null ? ` · PID ${state.processId}` : ""}.
            {:else if state.processState === "stopped"}No dedicated-server process was observed at the last check.
            {:else if state.processState === "other"}Another or external server process was observed. Manager ownership is not established.
            {:else}The process state could not be established.{/if}
        </p>
        {#if state?.config}
            <dl class="server-facts">
                <div><dt>Player limit</dt><dd>{state.config.maxPlayers}{#if state.config.maxPlayers > 8}<span>experimental</span>{/if}</dd></div>
                <div><dt>Save slot</dt><dd>{state.config.saveSlot}</dd></div>
                <div><dt>Save mode</dt><dd>{state.config.saveMode}</dd></div>
            </dl>
        {/if}
        <div class="control-row">
            <div class="actions">
                <button class="primary-button" type="button" disabled={busy || $processing || !available || !state?.canLaunch
                    || state.revision === null || state.configRevision === null} on:click={startServer}>
                    <LucidePlay aria-hidden="true" /> Start server
                </button>
                <button type="button" disabled={busy || $processing || !available} on:click={readStatus}>
                    <LucideRefreshCw aria-hidden="true" /> {busy ? "Working…" : state ? "Refresh status" : "Read status"}
                </button>
            </div>
            {#if state && observedAt}<span class="observed-time">Observed {observedAt}</span>{/if}
        </div>
        {#if problems.length}
            <div class="status-notes" role="status">{#each problems as problem}<p>{problem}</p>{/each}</div>
        {/if}
        {#if state?.lastExit}
            <p class="note last-exit">Last owned process exit: {state.lastExit.success ? "successful" : "unsuccessful"} · Code {state.lastExit.code ?? "unavailable"}.</p>
        {/if}
    </section>

    {#if error}<p class="message error" role="alert">{error}</p>{/if}
    {#if notice}<p class="message notice" role="status">{notice}</p>{/if}

    <section class="settings-panel" aria-labelledby="server-settings-heading">
        <div class="settings-heading">
            <h2 id="server-settings-heading">Server settings</h2>
            <p class="note">Read a stopped server's configuration to edit it. Close the game and all dedicated servers before saving changes.</p>
        </div>

        <div class="setting-row">
            <label class="setting-label" for="server-name">Server name</label>
            <div class="setting-content">
                <div class="field-controls">
                    <input id="server-name" bind:value={nameChoice} on:input={clearNameReview} placeholder="Read status to load the server name"
                        disabled={busy || $processing || !canEditConfiguration} aria-describedby="server-name-help" />
                    <button type="button" disabled={busy || $processing || !canEditConfiguration || !!nameError
                        || nameChoice.trim() === state?.config?.serverName} on:click={reviewName}>Review name</button>
                </div>
                <p id="server-name-help" class="note">Displayed in the server browser. Up to 256 characters.</p>
                {#if canEditConfiguration && nameError}<p class="field-error">{nameError}</p>{/if}
                {#if nameReview}
                    <div class="change-review" role="region" aria-label="Review server name">
                        <h3>Change server name</h3>
                        <dl class="name-comparison"><div><dt>Current</dt><dd>{nameReview.from || "Empty name"}</dd></div><div><dt>New</dt><dd>{nameReview.to}</dd></div></dl>
                        <p class="path">{nameReview.configPath}</p>
                        <p>The original configuration will be backed up before saving. The server stays stopped.</p>
                        <div class="actions">
                            <button class="primary-button" type="button" disabled={busy || $processing || !canEditConfiguration} on:click={saveName}>Save server name</button>
                            <button type="button" disabled={busy || $processing} on:click={clearNameReview}>Cancel</button>
                        </div>
                    </div>
                {/if}
            </div>
        </div>

        <div class="setting-row">
            <label class="setting-label" for="server-capacity">Player limit</label>
            <div class="setting-content">
                <div class="field-controls capacity-controls">
                    <select id="server-capacity" bind:value={capacityChoice} on:change={clearCapacityReview}
                        disabled={busy || $processing || !canEditConfiguration} aria-describedby="server-capacity-help">
                        {#if capacityChoice === 0}<option value={0}>{state?.config ? "Choose a limit" : "Read status to load the limit"}</option>{/if}
                        <option value={8}>8 players</option>
                        <option value={16}>16 players — experimental</option>
                        <option value={32}>32 players — experimental</option>
                    </select>
                    <button type="button" disabled={busy || $processing || !canEditConfiguration || ![8, 16, 32].includes(capacityChoice)
                        || capacityChoice === state?.config?.maxPlayers} on:click={reviewCapacity}>Review limit</button>
                </div>
                <p id="server-capacity-help" class="note">The maximum configured player count. Other settings and world saves are preserved.</p>
                {#if capacityChoice > 8}
                    <p class="experimental-note">16 and 32 players require compatible Modstoke Loader support and have not been qualified in multiplayer. Saving a limit does not install Loader or confirm that this many players can join.</p>
                {/if}
                {#if capacityReview}
                    <div class="change-review" role="region" aria-label="Review player limit">
                        <h3>Change {capacityReview.from} → {capacityReview.to} players</h3>
                        <p class="path">{capacityReview.dataDirectory}\dedicatedserver.cfg</p>
                        <p>The original configuration will be backed up before saving. The server stays stopped.</p>
                        {#if capacityReview.to > 8}
                            <label class="acknowledgement"><input type="checkbox" bind:checked={experimentalAccepted} disabled={busy || $processing} />
                                I understand that {capacityReview.to} players is experimental and still needs multiplayer qualification.</label>
                        {/if}
                        <div class="actions">
                            <button class="primary-button" type="button" disabled={busy || $processing || !canEditConfiguration || (capacityReview.to > 8 && !experimentalAccepted)} on:click={saveCapacity}>Save player limit</button>
                            <button type="button" disabled={busy || $processing} on:click={clearCapacityReview}>Cancel</button>
                        </div>
                    </div>
                {/if}
            </div>
        </div>

        <div class="setting-row">
            <span class="setting-label" id="server-data-label">Data folder</span>
            <div class="setting-content" aria-labelledby="server-data-label">
                <p class="data-path path">{state?.dataDirectory ?? (state ? "No data folder selected" : "Read status to load the saved folder")}</p>
                <div class="actions">
                    <button type="button" disabled={busy || $processing || !available || !state?.canSave || state.processState !== "stopped"} on:click={selectDataFolder}>Choose data folder</button>
                    <button type="button" disabled={busy || $processing || !available || !state?.dataDirectory} on:click={openDataFolder}><LucideFolderOpen aria-hidden="true" /> Open folder</button>
                </div>
                <p class="note">Choose an existing folder containing dedicatedserver.cfg, initialized through the game's dedicated-server setup. This selection saves only Manager's folder preference.</p>
                {#if state?.configPath}<p class="config-path path">Configuration: {state.configPath}</p>{/if}
            </div>
        </div>
    </section>
    <p class="snapshot-note">Process status is a snapshot, not a multiplayer readiness check. Refresh to observe changes. Stop the server through its own controls; exiting Manager leaves it running. Starting here requires SaveMode to be Continue.</p>
</div>

<style>
    .server-launch { display: flex; flex-direction: column; gap: 16px; min-width: 0; }
    .status-panel, .settings-panel { min-width: 0; background: var(--surface-panel); border: 1px solid var(--border-subtle); border-radius: 10px; }
    .status-panel { padding: 22px; }
    .heading-row, .control-row { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 16px; }
    .heading-copy { min-width: 0; }
    .eyebrow { margin: 0 0 6px; color: var(--text-secondary); font-size: 11px; font-weight: 600; letter-spacing: 1px; text-transform: uppercase; }
    h2 { margin: 0; font-size: 19px; overflow-wrap: anywhere; }
    h3 { margin: 0 0 10px; font-size: 14px; }
    p { overflow-wrap: anywhere; }
    .status-description { margin: 10px 0 18px; color: var(--text-secondary); font-size: 13px; }
    .process-state { flex-shrink: 0; padding: 5px 10px; border: 1px solid var(--border-subtle); border-radius: 6px; color: var(--text-secondary); background: var(--surface-base); font-size: 12px; font-weight: 600; }
    .process-state.observed { color: var(--text-primary); background: var(--surface-raised); }
    .process-state.running { color: var(--accent); background: var(--accent-soft); border-color: rgba(238, 182, 107, .25); }
    .server-facts { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 16px; padding: 16px 0; margin: 0 0 18px; border-top: 1px solid var(--border-subtle); border-bottom: 1px solid var(--border-subtle); }
    dt { color: var(--text-secondary); font-size: 12px; }
    dd { margin: 5px 0 0; overflow-wrap: anywhere; }
    .server-facts dd { display: flex; flex-wrap: wrap; align-items: baseline; gap: 8px; font-size: 20px; font-weight: 600; }
    .server-facts dd span { color: var(--accent); font-size: 11px; font-weight: 400; }
    .actions, .field-controls { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
    button { display: inline-flex; justify-content: center; align-items: center; gap: 7px; flex-shrink: 0; }
    button :global(svg) { width: 15px; height: 15px; }
    .primary-button { color: #251c10; background: var(--accent); border-color: var(--accent); }
    .primary-button:enabled:hover { color: #251c10; background: #ffd296; border-color: #ffd296; }
    .observed-time { color: var(--text-secondary); font-size: 11px; }
    .status-notes { margin-top: 18px; padding: 12px 14px; background: var(--accent-soft); border-left: 2px solid var(--accent); border-radius: 3px; }
    .status-notes p { margin: 4px 0; color: #e6c89e; font-size: 12px; line-height: 1.6; }
    .note, .snapshot-note, .config-path { color: var(--text-secondary); font-size: 12px; line-height: 1.6; }
    .last-exit { margin: 14px 0 0; }
    .message { margin: 0; padding: 14px 16px; font-size: 13px; line-height: 1.6; border: 1px solid var(--border-subtle); border-radius: 8px; }
    .message.error { color: #ffb4ac; background: rgba(168, 61, 51, .1); max-height: 14em; overflow: auto; }
    .message.notice { color: var(--text-primary); background: var(--surface-raised); }
    .settings-heading { padding: 20px 22px; }
    .settings-heading .note { margin: 8px 0 0; }
    .setting-row { display: grid; grid-template-columns: 150px minmax(0, 1fr); gap: 20px; padding: 20px 22px; border-top: 1px solid var(--border-subtle); }
    .setting-label { padding-top: 9px; color: var(--text-primary); font-size: 13px; font-weight: 600; }
    .setting-content { min-width: 0; }
    .setting-content .note { margin: 8px 0 0; }
    .field-controls input { flex: 1 1 220px; min-width: 0; }
    .capacity-controls select { flex: 0 1 250px; min-width: 0; max-width: 100%; }
    .field-error { margin: 6px 0 0; color: #ffb4ac; font-size: 12px; }
    .experimental-note { padding: 10px 12px; margin: 12px 0 0; border-radius: 6px; background: var(--accent-soft); color: #e6c89e; font-size: 12px; }
    .change-review { margin-top: 16px; padding: 16px; border: 1px solid rgba(238, 182, 107, .3); border-radius: 8px; background: var(--surface-raised); }
    .change-review p { font-size: 12px; line-height: 1.6; }
    .change-review .actions { margin-top: 14px; }
    .name-comparison { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 16px; margin: 0 0 12px; }
    .name-comparison dd { font-size: 13px; }
    .path { user-select: text; overflow-wrap: anywhere; }
    .data-path { margin: 0 0 12px; padding: 10px 12px; border: 1px solid var(--border-subtle); border-radius: 6px; background: var(--surface-base); font-size: 12px; }
    .config-path { margin: 8px 0 0; }
    .acknowledgement { display: flex; gap: 10px; align-items: flex-start; font-size: 12px; line-height: 1.6; }
    .acknowledgement input { float: none; flex-shrink: 0; margin: 2px 0 0; }
    .snapshot-note { margin: 0 2px; }
    @media (max-width: 760px) {
        .status-panel { padding: 18px; }
        .setting-row { grid-template-columns: minmax(0, 1fr); gap: 10px; padding: 18px; }
        .setting-label { padding-top: 0; }
        .settings-heading { padding: 18px; }
        .server-facts { gap: 12px; }
        .server-facts dd { font-size: 17px; }
    }
</style>
