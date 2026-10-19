<script lang="ts">
    import { onDestroy } from "svelte";
    import { get } from "svelte/store";
    import { invoke } from "@tauri-apps/api/core";
    import { open } from "@tauri-apps/plugin-shell";
    import { processing, processName } from "./store";
    import { selectedInstallation, type InspectedInstallation } from "./installationProfiles";
    import { scanInstalledInventory, describeInstallSource, type InstalledInventoryEntry } from "./modInventory";
    import type { ProfileState } from "./modProfile";
    import NativePackages from "./NativePackages.svelte";
    import PackRecovery from "./PackRecovery.svelte";

    export let installation: InspectedInstallation;
    let entries: InstalledInventoryEntry[] = [];
    let profileState: ProfileState | null = null;
    let inventoryError = "";
    let profileError = "";
    let busy = false;
    let activeKey = "";
    let loadedKey = "";
    let generation = 0;
    let destroyed = false;
    let recoveryRefresh = 0;
    let nativeRefresh = 0;

    function keyOf(value: InspectedInstallation | null): string {
        return value ? JSON.stringify([value.target.id, value.target.kind, value.target.executable, value.root, value.available]) : "";
    }

    function reset(key: string) {
        if (key === activeKey) return;
        activeKey = key;
        loadedKey = "";
        generation++;
        entries = [];
        profileState = null;
        inventoryError = "";
        profileError = "";
    }

    $: key = keyOf(installation);
    $: reset(key);
    $: available = installation.available && !!installation.root && installation.target.kind === "server"
        && keyOf($selectedInstallation) === key;
    $: if (key && loadedKey !== key && available && !busy && !$processing) void refresh();
    $: pendingPlan = profileState?.profile?.pendingPlan;
    $: completed = pendingPlan?.steps.filter(step => step.status === "applied").length ?? 0;

    async function refresh() {
        if (destroyed || busy || get(processing) || !installation.available || !installation.root || installation.target.kind !== "server") return;
        const captured = installation;
        const requestKey = keyOf(captured);
        if (keyOf(get(selectedInstallation)) !== requestKey) return;
        const request = ++generation;
        loadedKey = requestKey;
        busy = true;
        processing.set(true);
        processName.set("Reading server inventory and recovery state...");
        entries = [];
        profileState = null;
        inventoryError = "";
        profileError = "";
        try {
            // Recovery can change installed files. Read inventory only after it finishes.
            try {
                const profile = await invoke<ProfileState>("read_mod_profile", { gameRoot: captured.root });
                if (isCurrent()) profileState = profile;
            } catch (error) {
                if (isCurrent()) profileError = String(error);
            }
            if (!isCurrent()) return;
            try {
                const inventory = await scanInstalledInventory(captured.root!);
                if (isCurrent()) entries = inventory;
            } catch (error) {
                if (isCurrent()) inventoryError = String(error);
            }
        } finally {
            busy = false;
            processing.set(false);
            processName.set("");
        }

        function isCurrent() {
            return !destroyed && request === generation && keyOf(installation) === requestKey
                && keyOf(get(selectedInstallation)) === requestKey;
        }
    }

    async function openFolder() {
        if (destroyed || busy || get(processing) || !available || !installation.root) return;
        const requestKey = keyOf(installation);
        if (keyOf(get(selectedInstallation)) !== requestKey) return;
        try { await open(installation.root); }
        catch (error) {
            if (!destroyed && keyOf(installation) === requestKey && keyOf(get(selectedInstallation)) === requestKey) inventoryError = String(error);
        }
    }

    onDestroy(() => { destroyed = true; generation++; });
</script>

<div class="server-packages">
<PackRecovery {installation} refreshToken={recoveryRefresh} on:changed={() => { loadedKey = ""; nativeRefresh++; }} />

<section class="server-installation" aria-labelledby="server-inventory-heading">
    <div class="heading-row">
        <div>
            <h2 id="server-inventory-heading">Installed server files</h2>
            <p class="note">Legacy packages in Mods, Libs and BepInEx/plugins.</p>
        </div>
        <button type="button" disabled={busy || $processing || !available} on:click={refresh}>
            {busy ? "Refreshing…" : "Refresh"}
        </button>
    </div>
    <div class="folder-row">
        <p class="root">{installation.root ?? "Installation folder unavailable"}</p>
        <button type="button" disabled={busy || $processing || !available} on:click={openFolder}>Open server folder</button>
    </div>
    <p class="note">Native Modstoke packages are managed below. Listed files do not establish server compatibility.</p>
    <p class="note">Reading package status may recover an interrupted Manager package transaction.</p>
    {#if !available}
        <p class="error" role="status">{installation.problem ?? "This server installation is unavailable."}</p>
    {:else}
        <div class="profile-state" aria-live="polite">
            {#if busy}
                <p>Reading inventory and recovery state…</p>
            {:else if profileError}
                <p class="error" role="alert">Profile / recovery: {profileError}</p>
            {:else if profileState}
                <p>Profile lock file: {profileState.profile ? "present" : "not found"}.
                    {#if profileState.profile}{profileState.profile.packages.length} recorded packages.{/if}</p>
                {#if pendingPlan}
                    <p>Pending plan: {completed} completed steps; {pendingPlan.steps.length - completed} remaining steps.</p>
                {:else}
                    <p>No pending plan recorded.</p>
                {/if}
            {/if}
        </div>
        {#if inventoryError}
            <p class="error" role="alert">Inventory: {inventoryError}</p>
        {:else if !busy && loadedKey === key}
            <p class="inventory-count">{entries.length} installed entries in covered locations</p>
            {#if entries.length}
                <!-- svelte-ignore a11y_no_noninteractive_tabindex (Keyboard users need to focus and scroll this inventory.) -->
                <div class="inventory-scroll" tabindex="0" role="region" aria-label="Server installed inventory">
                    <table>
                        <thead><tr><th>Package</th><th>Location</th><th>Observed state</th><th>Source</th></tr></thead>
                        <tbody>
                            {#each entries as entry}
                                <tr>
                                    <td><strong>{entry.name}</strong>{#if entry.version}<span class="version">{entry.version}</span>{/if}</td>
                                    <td>{entry.expectedLocation}</td>
                                    <td>{entry.enabled ? "Enabled" : "Disabled"}</td>
                                    <td>{describeInstallSource(entry)}</td>
                                </tr>
                            {/each}
                        </tbody>
                    </table>
                </div>
            {:else}
                <p class="note">No packages found in the covered locations.</p>
            {/if}
        {/if}
    {/if}
</section>

{#if available && installation.root}
    {#key nativeRefresh}
        <NativePackages gameRoot={installation.root} on:changed={() => { loadedKey = ""; recoveryRefresh++; }} />
    {/key}
{/if}
</div>

<style>
    .server-packages { display: flex; flex-direction: column; gap: 16px; min-width: 0; }
    .server-installation { min-width: 0; padding: 22px; background: var(--surface-panel); border: 1px solid var(--border-subtle); border-radius: 10px; }
    .heading-row, .folder-row { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 14px; }
    .heading-row { margin-bottom: 16px; }
    .heading-row .note { margin-bottom: 0; }
    h2 { margin: 0; font-size: 18px; }
    button { flex-shrink: 0; }
    p { margin: 8px 0; font-size: 13px; line-height: 1.6; }
    .folder-row { padding: 12px 14px; margin-bottom: 16px; background: var(--surface-base); border: 1px solid var(--border-subtle); border-radius: 7px; }
    .root { flex: 1 1 260px; min-width: 0; margin: 0; font-size: 12px; overflow-wrap: anywhere; user-select: text; }
    .note { color: var(--text-secondary); font-size: 12px; }
    .profile-state { border-top: 1px solid var(--border-subtle); border-bottom: 1px solid var(--border-subtle); margin-top: 16px; padding: 6px 0; }
    .profile-state p { color: var(--text-secondary); font-size: 12px; }
    .error { color: #ffb4ac; overflow-wrap: anywhere; }
    .profile-state .error { color: #ffb4ac; }
    .inventory-count { color: var(--text-primary); }
    .inventory-scroll { max-height: 36vh; overflow: auto; }
    table { width: 100%; border-collapse: collapse; font-size: 12px; text-align: left; }
    th { background: var(--surface-raised); position: sticky; top: 0; color: var(--text-secondary); font-weight: 500; }
    th, td { padding: 10px 12px; border-bottom: 1px solid var(--border-subtle); vertical-align: top; overflow-wrap: anywhere; }
    .version { display: block; color: var(--text-secondary); }
    .server-packages :global(.pack-recovery) { width: 100%; margin: 0; background: var(--surface-panel); border-color: var(--border-subtle); border-radius: 10px; }
    .server-packages :global(.native-packages), .server-packages :global(.native-install), .server-packages :global(.native-pack) { background: var(--surface-panel); border-color: var(--border-subtle); border-radius: 10px; padding: 22px; }
    @media (max-width: 760px) {
        .server-installation, .server-packages :global(.native-packages), .server-packages :global(.native-install), .server-packages :global(.native-pack) { padding: 18px; }
    }
</style>
