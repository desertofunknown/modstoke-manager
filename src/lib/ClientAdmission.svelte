<script lang="ts">
    import { onDestroy } from "svelte";
    import { get } from "svelte/store";
    import { processing, processName } from "./store";
    import { selectedInstallation, type InspectedInstallation } from "./installationProfiles";
    import { readClientAdmission, writeClientAdmission, type ClientAdmissionState } from "./clientAdmission";

    export let installation: InspectedInstallation | null;

    let expanded = false;
    let busy = false;
    let ownsWrite = false;
    let destroyed = false;
    let generation = 0;
    let activeKey = "";
    let state: ClientAdmissionState | null = null;
    let error = "";
    let notice = "";

    function keyOf(value: InspectedInstallation | null): string {
        return value ? JSON.stringify([value.target.id, value.target.kind, value.target.executable, value.root, value.available]) : "";
    }

    function reset(key: string) {
        if (key === activeKey) return;
        activeKey = key;
        generation++;
        state = null;
        error = "";
        notice = "";
    }

    // Subscribe synchronously so even an operation completed within one UI update invalidates the read.
    const unsubscribe = processing.subscribe(value => {
        if (!value || ownsWrite) return;
        generation++;
        state = null;
        error = "";
        notice = "Read the saved setting again after the current Manager operation finishes.";
    });

    $: reset(keyOf(installation));
    $: available = !!installation?.available && !!installation.root && installation.target.kind === "player";
    $: savedEnabled = state?.state === "missing" ? false : state?.enabled;
    $: status = busy ? "Working…" : state?.state === "unreadable" ? "Setting unreadable"
        : state?.state === "missing" ? "Default: off" : state?.enabled === true ? "Saved: enabled"
        : state?.enabled === false ? "Saved: disabled" : "Not read";

    function canStart(): boolean {
        return !destroyed && !busy && !get(processing) && !!installation?.available && !!installation.root
            && installation.target.kind === "player" && keyOf(get(selectedInstallation)) === keyOf(installation);
    }

    function current(request: number, key: string): boolean {
        return !destroyed && request === generation && keyOf(installation) === key
            && keyOf(get(selectedInstallation)) === key && (ownsWrite || !get(processing));
    }

    async function readSetting() {
        if (!canStart() || !installation) return;
        const target = installation.target;
        const key = keyOf(installation);
        const request = ++generation;
        busy = true;
        state = null;
        error = "";
        notice = "";
        try {
            const result = await readClientAdmission(target.executable, target.id);
            if (current(request, key)) state = result;
        } catch (failure) {
            if (current(request, key)) error = `Could not read the saved setting: ${failure}`;
        } finally { busy = false; }
    }

    async function saveSetting(enabled: boolean) {
        if (!canStart() || !installation || !state?.canWrite || !state.clientAvailable
            || state.state === "unreadable" || savedEnabled === enabled) return;
        const target = installation.target;
        const revision = state.revision;
        const key = keyOf(installation);
        const request = ++generation;
        busy = true;
        ownsWrite = true;
        error = "";
        notice = "";
        processing.set(true);
        processName.set("Saving managed client participation preference...");
        try {
            const result = await writeClientAdmission(target.executable, target.id, revision, enabled);
            if (current(request, key)) {
                state = result;
                if (result.state !== "unreadable" && result.enabled === enabled) {
                    notice = `Managed client participation saved as ${enabled ? "enabled" : "disabled"} for the next game start.`;
                } else {
                    state = null;
                    error = "The saved preference could not be confirmed after writing. Read the setting again before making another change.";
                }
            }
        } catch (failure) {
            if (current(request, key)) {
                state = null;
                error = `Save could not be confirmed: ${failure}. Read the saved setting again before making another change; it may already have been saved.`;
            }
        } finally {
            processName.set("");
            processing.set(false);
            ownsWrite = false;
            busy = false;
        }
    }

    onDestroy(() => { destroyed = true; generation++; unsubscribe(); });
</script>

<details class="client-admission" bind:open={expanded}>
    <summary><strong>Managed client participation</strong><span>{status}</span></summary>
    {#if expanded}
        <div class="content">
            <p>Enable this saved preference only for managed servers. Disable it before ordinary vanilla joining or hosting. Changes apply at the next game start.</p>
            <p class="note">This setting does not confirm that Client is loaded, connected or admitted to a server.</p>
            <p class="note">Close the game and any dedicated server before saving. Process status is a snapshot, not a launch lock.</p>
            <div class="heading-row">
                <p>{installation?.target.name ?? "No player installation selected"}</p>
                <button type="button" disabled={busy || $processing || !available} on:click={readSetting}>Read setting</button>
            </div>
            {#if !available}<p class="note">Choose an available player installation to read its saved preference.</p>{/if}
            {#if busy}<p role="status">{ownsWrite ? "Saving preference…" : "Reading saved preference and Client files…"}</p>{/if}
            {#if error}<p class="error" role="alert">{error}</p>{/if}
            {#if notice}<p role="status">{notice}</p>{/if}
            {#if state}
                <section aria-label="Saved admission preference">
                    <h3>{status}</h3>
                    {#if state.state === "missing"}<p class="note">No configuration file exists. Managed client participation defaults to off.</p>{/if}
                    {#if state.configProblem}<p class="error">{state.configProblem}</p>{/if}
                    <p class="path"><strong>Configuration:</strong> {state.configPath}</p>
                    <p><strong>Client files:</strong> {state.clientAvailable ? "Available" : "Unavailable"}{state.clientVersion ? ` · Version ${state.clientVersion}` : " · Version not established"}</p>
                    {#if state.clientProblem}<p class="note">{state.clientProblem}</p>{/if}
                    {#if !state.clientAvailable}<p class="note">Valid existing Client package files are required to enable or disable this preference.</p>{/if}
                    <p><strong>Game / dedicated server process:</strong> {state.processState === "stopped" ? "Stopped" : state.processState === "running" ? "Running" : "Unknown"} at the last read.</p>
                    {#if state.processProblem}<p class="note">{state.processProblem}</p>{/if}
                    {#if state.writeProblem}<p class="note">{state.writeProblem}</p>{/if}
                    {#if state.canWrite && state.clientAvailable && state.state !== "unreadable"}
                        <div class="actions">
                            <button type="button" disabled={busy || $processing || savedEnabled === true} on:click={() => saveSetting(true)}>Enable for next start</button>
                            <button type="button" disabled={busy || $processing || savedEnabled === false} on:click={() => saveSetting(false)}>Disable for next start</button>
                        </div>
                    {/if}
                </section>
            {:else if !busy && !error && !notice}
                <p class="note">Read the saved setting to see the preference, Client files and game process status.</p>
            {/if}
        </div>
    {/if}
</details>

<style>
    .client-admission { box-sizing: border-box; flex-shrink: 0; width: 100%; min-width: 0; margin: 0 auto; border: 1px solid var(--border-subtle); border-radius: 10px; background: var(--surface-panel); color: var(--text-primary); text-align: left; }
    summary { padding: 16px; cursor: pointer; font-size: 13px; line-height: 1.6; overflow-wrap: anywhere; }
    summary span { margin-left: .8em; color: var(--text-secondary); font-size: 13px; display: inline-block; }
    .content { padding: 0 16px 16px; }
    p { margin: .55em 0; font-size: 13px; line-height: 1.5; overflow-wrap: anywhere; }
    .note { color: var(--text-secondary); }
    .heading-row, .actions { display: flex; flex-wrap: wrap; align-items: center; gap: .6em; margin: .8em 0; }
    .heading-row { justify-content: space-between; border-top: 1px solid var(--border-subtle); padding-top: .5em; }
    .heading-row p { min-width: 0; }
    button { margin: 0; padding: 8px 12px; font-size: 13px; max-width: 100%; white-space: normal; overflow-wrap: anywhere; border-radius: 8px; }
    :disabled { opacity: .55; }
    .error { color: #ffb4ac; }
    h3 { margin: .7em 0; font-size: 14px; }
    .path { user-select: text; }
    section { min-width: 0; padding: 14px; margin-top: 12px; border: 1px solid var(--border-subtle); border-radius: 8px; background: var(--surface-base); }
    section h3 { margin-top: 0; }
</style>
