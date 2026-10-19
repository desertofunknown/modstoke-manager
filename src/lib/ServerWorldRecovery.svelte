<script lang="ts">
    import { onDestroy } from "svelte";
    import { get } from "svelte/store";
    import { open as chooseFolder } from "@tauri-apps/plugin-dialog";
    import { processing, processName } from "./store";
    import { selectedInstallation, type InspectedInstallation } from "./installationProfiles";
    import { readServerLaunch, type ServerLaunchState } from "./serverLaunch";
    import { readServerWorld, createServerWorldBackup, restoreServerWorldBackup, recoverServerWorldRestore,
        type WorldRequest, type WorldState } from "./serverWorldRecovery";

    export let installation: InspectedInstallation;

    type Review = { action: "restore" | "finish" | "undo"; binding: string; phrase: string };
    let busy = false;
    let ownsOperation = false;
    let destroyed = false;
    let generation = 0;
    let activeKey = "";
    let launch: ServerLaunchState | null = null;
    let state: WorldState | null = null;
    let saveDirectory = "";
    let folderDataDirectory = "";
    let backupId = "";
    let review: Review | null = null;
    let confirmation = "";
    let error = "";
    let notice = "";

    function keyOf(value: InspectedInstallation | null): string {
        return value ? JSON.stringify([value.target.id, value.target.kind, value.target.executable, value.root, value.available]) : "";
    }

    function clearReview() { review = null; confirmation = ""; }

    function clearObservation() {
        launch = null;
        state = null;
        backupId = "";
        clearReview();
    }

    function reset(key: string) {
        if (key === activeKey) return;
        activeKey = key;
        generation++;
        clearObservation();
        saveDirectory = "";
        folderDataDirectory = "";
        error = "";
        notice = "";
    }

    const unsubscribe = processing.subscribe(value => {
        if (!value || ownsOperation) return;
        generation++;
        clearObservation();
        error = "";
        notice = "Read status again after the current Manager operation finishes.";
    });
    const unsubscribeSelection = selectedInstallation.subscribe(value => {
        if (keyOf(value) === activeKey) return;
        generation++;
        clearObservation();
    });

    $: reset(keyOf(installation));
    $: available = installation.available && !!installation.root && installation.target.kind === "server";
    $: selectedBackup = state?.backups.find(backup => backup.id === backupId);

    function canStart(): boolean {
        return !destroyed && !busy && !get(processing) && installation.available && !!installation.root
            && installation.target.kind === "server" && keyOf(get(selectedInstallation)) === keyOf(installation);
    }

    function current(request: number, key: string): boolean {
        return !destroyed && request === generation && keyOf(installation) === key
            && keyOf(get(selectedInstallation)) === key && (ownsOperation || !get(processing));
    }

    function beginOperation(label: string) {
        busy = true;
        ownsOperation = true;
        error = "";
        notice = "";
        clearReview();
        processName.set(label);
        processing.set(true);
    }

    function endOperation() {
        processName.set("");
        processing.set(false);
        ownsOperation = false;
        busy = false;
    }

    function requestForWorld(): WorldRequest | null {
        if (!launch?.revision || !launch.configRevision || !launch.dataDirectory || launch.processState !== "stopped"
            || folderDataDirectory !== launch.dataDirectory || !saveDirectory) return null;
        return { executable: installation.target.executable, installationId: installation.target.id,
            expectedRevision: launch.revision, expectedConfigRevision: launch.configRevision, saveDirectory };
    }

    function binding(): string {
        const backup = state?.backups.find(item => item.id === backupId);
        return JSON.stringify([keyOf(installation), saveDirectory, launch?.dataDirectory, launch?.revision,
            launch?.configRevision, state?.installationId, state?.saveDirectory, state?.worldRevision,
            backup?.id, backup?.revision, state?.pending?.revision]);
    }

    async function readStatus(pendingOnly = false) {
        if (!canStart()) return;
        const target = installation.target;
        const key = keyOf(installation);
        const request = ++generation;
        clearObservation();
        beginOperation("Reading dedicated-server world status...");
        try {
            const result = await readServerLaunch(target.executable, target.id);
            if (!current(request, key)) return;
            launch = result;
            if (folderDataDirectory !== result.dataDirectory) {
                saveDirectory = "";
                folderDataDirectory = "";
            }
            const worldRequest = pendingOnly && result.revision && result.configRevision && result.dataDirectory && result.processState === "stopped"
                ? { executable: target.executable, installationId: target.id, expectedRevision: result.revision,
                    expectedConfigRevision: result.configRevision, saveDirectory: "" }
                : pendingOnly ? null : requestForWorld();
            if (worldRequest) {
                const world = await readServerWorld(worldRequest);
                if (current(request, key)) {
                    state = world;
                    saveDirectory = world.saveDirectory;
                    folderDataDirectory = result.dataDirectory!;
                }
            }
        } catch (failure) {
            if (current(request, key)) {
                state = null;
                backupId = "";
                clearReview();
                error = `Could not read world status: ${failure}. Read status again before continuing.`;
            }
        } finally { endOperation(); }
    }

    async function selectSaveFolder() {
        if (!canStart() || !launch?.dataDirectory || !launch.revision || !launch.configRevision || launch.processState !== "stopped") return;
        const dataDirectory = launch.dataDirectory;
        const key = keyOf(installation);
        const request = ++generation;
        beginOperation("Selecting dedicated-server save folder...");
        try {
            const folder = await chooseFolder({ directory: true, multiple: false, defaultPath: dataDirectory,
                title: "Choose an existing native save folder inside the data folder" });
            if (!current(request, key) || typeof folder !== "string") return;
            saveDirectory = folder;
            folderDataDirectory = dataDirectory;
            state = null;
            backupId = "";
            notice = "Read status to validate this save folder and list its backups.";
        } catch (failure) {
            if (current(request, key)) error = `Could not choose a save folder: ${failure}`;
        } finally { endOperation(); }
    }

    function prepare(action: Review["action"]) {
        if (!canStart() || !requestForWorld() || !state) return;
        const backup = state.backups.find(item => item.id === backupId);
        if (action === "restore" && (state.pending || state.problem || !state.worldRevision || !backup)) return;
        if (action !== "restore" && !state.pending) return;
        confirmation = "";
        review = { action, binding: binding(), phrase: action.toUpperCase() };
    }

    async function mutate(action: "backup" | Review["action"]) {
        if (!canStart() || !state) return;
        const worldRequest = requestForWorld();
        if (!worldRequest) return;
        const observed = state;
        const backup = observed.backups.find(item => item.id === backupId);
        if (action !== "backup" && (!review || review.action !== action || review.binding !== binding() || confirmation !== review.phrase)) {
            clearReview();
            return;
        }
        if ((action === "backup" || action === "restore") && (observed.pending || observed.problem || !observed.worldRevision)) return;
        if (action === "restore" && !backup) return;
        if ((action === "finish" || action === "undo") && !observed.pending) return;
        const key = keyOf(installation);
        const request = ++generation;
        beginOperation(action === "backup" ? "Backing up dedicated-server world..." : "Recovering dedicated-server world...");
        try {
            const result = action === "backup" ? await createServerWorldBackup(worldRequest, observed.worldRevision!)
                : action === "restore" ? await restoreServerWorldBackup(worldRequest, observed.worldRevision!, backup!.id, backup!.revision)
                : await recoverServerWorldRestore(worldRequest, observed.pending!.revision, action);
            if (current(request, key)) {
                state = result;
                backupId = "";
                notice = action === "backup" ? "Backup created." : "Recovery action returned. Review the observed status below.";
            }
        } catch (failure) {
            if (current(request, key)) {
                clearObservation();
                error = `The operation could not be confirmed: ${failure}. Files may already have changed. Read status before taking another action.`;
            }
        } finally { endOperation(); }
    }

    onDestroy(() => { destroyed = true; generation++; unsubscribe(); unsubscribeSelection(); });
</script>

<section class="world-recovery" aria-labelledby="world-recovery-heading">
    <div class="heading-row">
        <h2 id="world-recovery-heading">World backup and recovery</h2>
        <button type="button" disabled={busy || $processing || !available} on:click={() => readStatus()}>{busy ? "Working…" : "Read / refresh world status"}</button>
    </div>
    <p class="note">Close the game and dedicated server before accessing saves. Choose the actual native save folder inside the configured data folder; the configured save slot does not identify it for you.</p>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    {#if notice}<p role="status">{notice}</p>{/if}
    {#if launch}
        <p><strong>Configured data folder:</strong> {launch.dataDirectory ?? "Not selected"}</p>
        <p><strong>Configured save slot (context only):</strong> {launch.config?.saveSlot ?? "Unavailable"}</p>
        <p><strong>Process at last observation:</strong> {launch.processState}</p>
        {#if launch.configProblem}<p class="error">{launch.configProblem}</p>{/if}
        {#if launch.processProblem}<p class="note">{launch.processProblem}</p>{/if}
        <div class="actions">
            <button type="button" disabled={busy || $processing || !available || !launch.dataDirectory || !launch.revision || !launch.configRevision || launch.processState !== "stopped"} on:click={selectSaveFolder}>Choose native save folder</button>
            <button type="button" disabled={busy || $processing || !available || !launch.dataDirectory || !launch.revision || !launch.configRevision || launch.processState !== "stopped"} on:click={() => readStatus(true)}>Read pending recovery</button>
        </div>
        <p class="note">Use Read pending recovery after an interrupted restore, including when its save folder is temporarily missing.</p>
    {:else if !busy}
        <p class="note">Read status to load the configured data folder and server configuration.</p>
    {/if}
    {#if saveDirectory}<p><strong>Chosen save folder:</strong> {saveDirectory}</p>{/if}
    {#if state}
        <!-- svelte-ignore a11y_no_noninteractive_tabindex (Keyboard users need to focus and scroll world status.) -->
        <div class="observation" tabindex="0" role="region" aria-label="Observed world and backups">
            <p><strong>Validated save folder:</strong> {state.saveDirectory}</p>
            <p><strong>Files:</strong> {state.files} · <strong>Bytes:</strong> {state.bytes.toLocaleString()}</p>
            <p><strong>Save fingerprint:</strong> {state.worldRevision ?? "Unavailable"}</p>
            {#if state.problem}<p class="error">{state.problem}</p>{/if}
            {#if state.pending}
                <p><strong>Pending restore:</strong> {state.pending.detail}</p>
                <p class="note">A pending restore blocks server launch and data-folder changes in Manager. Review finish to complete it, or undo to recover the previous folder.</p>
                <div class="actions">
                    <button type="button" disabled={busy || $processing || !available} on:click={() => prepare("finish")}>Review finish</button>
                    <button type="button" disabled={busy || $processing || !available} on:click={() => prepare("undo")}>Review undo</button>
                </div>
            {:else}
                <p><strong>Pending restore:</strong> None observed.</p>
                <button type="button" disabled={busy || $processing || !available || !!state.problem || !state.worldRevision} on:click={() => mutate("backup")}>Create complete backup</button>
            {/if}
            <p><strong>Backups:</strong> {state.backups.length}</p>
            {#if state.backups.length}
                <label for="world-backup">Backup to review</label>
                <select id="world-backup" bind:value={backupId} on:change={clearReview} disabled={busy || $processing}>
                    <option value="">Choose a backup</option>
                    {#each state.backups as backup (backup.id)}
                        <option value={backup.id}>{backup.createdUtc} · {backup.label || backup.id}</option>
                    {/each}
                </select>
                {#if selectedBackup}
                    <p><strong>Backup:</strong> {selectedBackup.id} · {selectedBackup.label}</p>
                    <p><strong>Created:</strong> {selectedBackup.createdUtc}</p>
                    <p><strong>Files:</strong> {selectedBackup.files} · <strong>Bytes:</strong> {selectedBackup.bytes.toLocaleString()}</p>
                    <p><strong>Backup fingerprint:</strong> {selectedBackup.revision}</p>
                    <button type="button" disabled={busy || $processing || !available || !!state.pending || !!state.problem || !state.worldRevision} on:click={() => prepare("restore")}>Review restore</button>
                {/if}
            {/if}
            {#if review}
                <div class="review" role="region" aria-label="Confirm world recovery">
                    <p><strong>{review.action === "restore" ? "Restore selected backup" : review.action === "finish" ? "Finish pending restore" : "Undo pending restore"}</strong></p>
                    <p><strong>Installation:</strong> {installation.target.name} ({installation.target.id})</p>
                    <p><strong>Save folder:</strong> {saveDirectory}</p>
                    {#if review.action === "restore" && selectedBackup}
                        <p><strong>Selected backup:</strong> {selectedBackup.createdUtc} · {selectedBackup.label || selectedBackup.id}</p>
                    {/if}
                    <p class="note">{review.action === "restore" ? "This replaces the complete selected save folder. The previous world is retained as an undo backup." : review.action === "finish" ? "This completes the recorded restore using its staged world." : "This returns the selected folder to the world held before the pending restore."}</p>
                    <label for="world-confirmation">Type exactly: <span class="phrase">{review.phrase}</span></label>
                    <input id="world-confirmation" type="text" maxlength="7" autocomplete="off" spellcheck="false" bind:value={confirmation} disabled={busy || $processing} />
                    <div class="actions">
                        <button type="button" disabled={busy || $processing || !available || confirmation !== review.phrase} on:click={() => review && mutate(review.action)}>Confirm {review.action}</button>
                        <button type="button" disabled={busy || $processing} on:click={clearReview}>Cancel</button>
                    </div>
                </div>
            {/if}
        </div>
    {/if}
    <p class="note">Status changes only when you request it. Modstoke Manager by OpenACAI does not stop the server or retry recovery automatically.</p>
</section>

<style>
    .world-recovery { min-width: 0; padding: 14px; background: var(--surface-panel); border: 1px solid var(--border-subtle); border-radius: 10px; box-sizing: border-box; color: var(--text-primary); }
    .heading-row, .actions { display: flex; flex-wrap: wrap; align-items: center; gap: .6em; }
    .heading-row { justify-content: space-between; }
    h2 { margin: 0; font-size: 16px; }
    button { margin: 0; padding: 8px 12px; font-size: 13px; max-width: 100%; white-space: normal; overflow-wrap: anywhere; border-radius: 8px; }
    p, label { margin: .55em 0; font-size: 13px; line-height: 1.5; overflow-wrap: anywhere; }
    label { display: block; font-weight: 500; color: var(--text-primary); }
    select, input { display: block; width: 100%; min-width: 0; box-sizing: border-box; font: inherit; padding: 9px 12px; background: var(--surface-base); border: 1px solid var(--border-subtle); border-radius: 8px; font-size: 13px; }
    p, .phrase { user-select: text; }
    .note { color: var(--text-secondary); }
    .error { color: #ffb4ac; max-height: 12em; overflow: auto; }
    .observation { max-height: 55vh; overflow: auto; border-top: 1px solid var(--border-subtle); margin: .7em 0; padding: 14px 4px; }
    .actions { margin: .8em 0; }
    .review { border: 1px solid var(--border-subtle); border-radius: 10px; padding: 14px; margin-top: .8em; background: var(--surface-raised); }
    :disabled { opacity: .55; }
    .heading-row > h2 { min-width: 0; overflow-wrap: anywhere; }
    .phrase { color: var(--accent); font-weight: 600; }
</style>
