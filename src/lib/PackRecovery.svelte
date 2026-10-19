<script lang="ts">
    import { createEventDispatcher, onDestroy } from "svelte";
    import { get } from "svelte/store";
    import { confirm } from "@tauri-apps/plugin-dialog";
    import { invoke } from "@tauri-apps/api/core";
    import { processing, processName, processProgress } from "./store";
    import { selectedInstallation, type InspectedInstallation } from "./installationProfiles";
    import { readModProfile, type ProfileState } from "./modProfile";
    import { ModDatabase } from "./mods";

    export let installation: InspectedInstallation;
    export let refreshToken = 0;
    const dispatch = createEventDispatcher<{ changed: void }>();

    let state: ProfileState | null = null;
    let error = "";
    let notice = "";
    let busy = false;
    let expanded = false;
    let activeKey = "";
    let loadedKey = "";
    let generation = 0;
    let destroyed = false;

    function keyOf(value: InspectedInstallation | null, token = refreshToken): string {
        return value ? JSON.stringify([value.target.id, value.target.kind, value.target.executable, value.root, value.available, token]) : "";
    }

    function reset(key: string) {
        if (key === activeKey) return;
        activeKey = key;
        loadedKey = "";
        generation++;
        state = null;
        error = "";
        notice = "";
        expanded = false;
    }

    $: key = keyOf(installation, refreshToken);
    $: reset(key);
    $: if (key && loadedKey !== key && !busy && !$processing) void refresh();
    $: checkpoint = state?.profile?.rollback;
    $: pendingPlan = state?.profile?.pendingPlan;
    $: completed = checkpoint?.receipts.length ?? 0;
    $: restored = checkpoint?.receipts.filter(receipt => receipt.restored).length ?? 0;
    $: status = busy ? "Working…" : error ? "Needs attention" : checkpoint?.phase === "discarding" ? "Cleanup interrupted"
        : checkpoint?.phase === "restoring" ? "Rollback interrupted" : checkpoint ? (pendingPlan ? "Install incomplete" : "Recovery available")
        : pendingPlan ? "Older install incomplete" : state ? "No recovery point" : "Waiting…";

    function canStart() {
        return !destroyed && !busy && !get(processing) && installation.available && installation.root
            && keyOf(get(selectedInstallation)) === keyOf(installation);
    }

    function current(request: number, requestKey: string) {
        return !destroyed && request === generation && keyOf(installation) === requestKey
            && keyOf(get(selectedInstallation)) === requestKey;
    }

    function begin(message: string) {
        busy = true;
        processing.set(true);
        processName.set(message);
        processProgress.set(0);
        error = "";
        notice = "";
    }

    function finish() {
        busy = false;
        processName.set("");
        processProgress.set(0);
        processing.set(false);
    }

    function readRecovery(root: string): Promise<ProfileState> {
        return installation.target.kind === "player" ? readModProfile(root) : invoke("read_mod_profile", { gameRoot: root });
    }

    async function refresh() {
        if (!canStart()) return;
        const root = installation.root!;
        const requestKey = keyOf(installation);
        const request = ++generation;
        loadedKey = requestKey;
        begin("Reading pack recovery status...");
        state = null;
        try {
            const result = await readRecovery(root);
            if (current(request, requestKey)) {
                state = result;
                if (result.profile?.rollback || result.profile?.pendingPlan) expanded = true;
            }
        } catch (failure) {
            if (current(request, requestKey)) { error = String(failure); expanded = true; }
        } finally { finish(); }
    }

    async function change(action: "rollback" | "discard") {
        if (!canStart() || !state?.profile?.rollback) return;
        const captured = state;
        const recovery = captured.profile!.rollback!;
        if (action === "rollback" && recovery.phase === "discarding") return;
        if (action === "discard" && recovery.phase !== "discarding" && (captured.profile!.pendingPlan || recovery.phase === "restoring")) return;
        const root = installation.root!;
        const requestKey = keyOf(installation);
        const request = ++generation;
        begin(action === "rollback" ? "Rolling back the latest pack install..." : "Removing retained pack recovery data...");
        let attempted = false;
        try {
            const approved = recovery.phase === "discarding" || await confirm(action === "rollback"
                ? `Roll back the latest install for ${recovery.rootName}? Manager will restore the package files changed by this plan and the previous profile records. If configuration or other files have changed since installation, conflicts stop rollback rather than overwrite those edits. Resolve any reported conflicts before resuming.`
                : `Keep the installed changes for ${recovery.rootName} and remove their retained recovery data? This recovery point will no longer be available.`,
                { title: action === "rollback" ? "Roll back pack install" : "Keep installed changes", kind: "warning" });
            if (!approved || !current(request, requestKey)) return;
            attempted = true;
            const result = installation.target.kind === "player"
                ? await ModDatabase.changeProfileRecovery(root, captured, action)
                : await invoke<ProfileState>(action === "rollback" ? "rollback_mod_profile" : "discard_mod_profile_rollback",
                    { gameRoot: root, expectedRevision: captured.revision, planId: recovery.planId });
            if (current(request, requestKey)) {
                state = result;
                notice = action === "rollback" ? "Package rollback completed. Previous profile records restored." : "Recovery data removed.";
            }
        } catch (failure) {
            if (!current(request, requestKey)) return;
            error = String(failure);
            // A partial rollback changes the profile revision. Always reload before retrying.
            state = null;
            try {
                const result = await readRecovery(root);
                if (current(request, requestKey)) state = result;
            } catch (reloadFailure) {
                if (current(request, requestKey)) error += ` Recovery status could not be reloaded: ${reloadFailure}`;
            }
        } finally {
            if (attempted && current(request, requestKey)) dispatch("changed");
            finish();
        }
    }

    onDestroy(() => { destroyed = true; generation++; });
</script>

<details class="pack-recovery" bind:open={expanded}>
    <summary><strong>Pack recovery</strong><span>{status}</span></summary>
    <div class="recovery-content">
        <div class="heading-row">
            <p class="installation" title={installation.root ?? ""}>{installation.target.name}</p>
            <button type="button" disabled={busy || $processing} on:click={refresh}>Refresh</button>
        </div>
        {#if error}<p class="error" role="alert">{error}</p>{/if}
        {#if notice}<p role="status">{notice}</p>{/if}
        {#if busy}
            <p role="status">{$processName}</p>
        {:else if checkpoint}
            <p class="plan-name"><strong>{checkpoint.rootName}</strong></p>
            <p>{completed} package {completed === 1 ? "receipt" : "receipts"} retained for recovery.</p>
            {#if checkpoint.phase === "discarding"}
                <p>Recovery data cleanup was interrupted. Finish cleanup to continue installing packages.</p>
                <button type="button" disabled={$processing} on:click={() => change("discard")}>Finish cleanup</button>
            {:else}
                {#if checkpoint.phase === "restoring"}
                    <p>{restored} of {completed} package changes restored. Resume rollback after resolving any reported conflicts.</p>
                {:else if pendingPlan}
                    <p>This install is incomplete. {pendingPlan.steps.some(step => step.package.source.provider === "modstoke-native-local")
                        ? "Resume it from Native packages using the same ZIP" : "Resume it through the catalog"}, or roll back its completed package changes here.</p>
                {:else}
                    <p>The latest install can be rolled back. Starting another install replaces this recovery point.</p>
                {/if}
                <p class="note">Rollback restores changed package files and previous profile records. Conflicting configuration or external edits stop rollback instead of being overwritten.</p>
                <div class="actions">
                    <button type="button" disabled={$processing} on:click={() => change("rollback")}>{checkpoint.phase === "restoring" ? "Resume rollback" : "Roll back install"}</button>
                    {#if !pendingPlan && checkpoint.phase === "available"}
                        <button type="button" disabled={$processing} on:click={() => change("discard")}>Keep changes / discard recovery</button>
                    {/if}
                </div>
            {/if}
        {:else if pendingPlan}
            <p><strong>Pending install:</strong> {pendingPlan.requestedRoots.join(", ")}</p>
            <p>This older plan has no retained rollback data. Resume it through the catalog. Rollback is available for new dependency installs.</p>
        {:else if state}
            <p>No pack recovery point is available. New dependency installs retain recovery data for the latest plan.</p>
        {/if}
    </div>
</details>

<style>
    .pack-recovery { box-sizing: border-box; flex-shrink: 0; min-width: 0; margin: 0 auto; width: 100%; color: var(--text-primary); background: var(--surface-panel); border: 1px solid var(--border-subtle); border-radius: 10px; text-align: left; }
    summary { padding: 16px; cursor: pointer; font-size: 13px; line-height: 1.6; overflow-wrap: anywhere; }
    summary span { margin-left: .8em; color: var(--text-secondary); font-size: 13px; display: inline-block; }
    .recovery-content { padding: 0 16px 16px; }
    .heading-row { display: flex; align-items: center; justify-content: space-between; gap: .7em; flex-wrap: wrap; }
    p { margin: .5em 0; font-size: 13px; line-height: 1.55; overflow-wrap: anywhere; }
    .installation, .plan-name { overflow-wrap: anywhere; min-width: 0; }
    .installation, .note { color: var(--text-secondary); }
    .actions { display: flex; flex-wrap: wrap; gap: .5em; margin-top: .7em; }
    button { margin: 0; padding: 8px 12px; font-size: 13px; max-width: 100%; white-space: normal; overflow-wrap: anywhere; border-radius: 8px; }
    .heading-row button { flex-shrink: 0; }
    .error { color: #ffb4ac; overflow-wrap: anywhere; }
    :disabled { opacity: .55; }
</style>
