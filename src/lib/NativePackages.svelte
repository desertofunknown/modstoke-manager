<script lang="ts">
    import { createEventDispatcher, onDestroy } from "svelte";
    import { get } from "svelte/store";
    import { open } from "@tauri-apps/plugin-shell";
    import { processing } from "./store";
    import NativePackageInstall from "./NativePackageInstall.svelte";
    import NativePackPrepare from "./NativePackPrepare.svelte";
    import { readNativeInventory, nativePackageStateLabel, type NativeInventory, type NativePackage } from "./nativeInventory";

    export let gameRoot: string;
    let singleRefresh = 0;
    let packRefresh = 0;
    export let searchTerm = "";
    const dispatch = createEventDispatcher<{ changed: void }>();

    let inventory: NativeInventory | null = null;
    let error = "";
    let folderError = "";
    let busy = false;
    let currentRoot = "";
    let requestedRoot = "";
    let generation = 0;
    let destroyed = false;

    function reset(root: string) {
        if (root === currentRoot) return;
        currentRoot = root;
        requestedRoot = "";
        generation++;
        inventory = null;
        error = "";
        folderError = "";
    }

    $: reset(gameRoot);
    $: if (gameRoot && requestedRoot !== gameRoot && !busy && !$processing) void refresh();
    $: query = searchTerm.trim().toLocaleLowerCase();
    $: visible = (inventory?.packages ?? []).filter(entry => !query || [
        entry.id, entry.name, entry.version, entry.author, entry.packageType,
        entry.manifestPath, entry.assemblyPath, nativePackageStateLabel(entry.state)
    ].some(value => value?.toLocaleLowerCase().includes(query)));
    $: attention = inventory?.packages.filter(entry => entry.state === "incomplete" || entry.state === "ambiguous").length ?? 0;

    async function refresh() {
        if (destroyed || busy || get(processing) || !gameRoot) return;
        const root = gameRoot;
        const request = ++generation;
        requestedRoot = root;
        busy = true;
        inventory = null;
        error = "";
        folderError = "";
        try {
            const result = await readNativeInventory(root);
            if (isCurrent()) inventory = result;
        } catch (failure) {
            if (isCurrent()) error = String(failure);
        } finally {
            busy = false;
        }
        function isCurrent() {
            return !destroyed && request === generation && root === gameRoot;
        }
    }

    async function openPackage(entry: NativePackage) {
        if (busy || get(processing) || !inventory?.packages.includes(entry)) return;
        const root = gameRoot;
        folderError = "";
        try { await open(entry.packagePath); }
        catch (failure) {
            if (!destroyed && root === gameRoot) folderError = String(failure);
        }
    }

    onDestroy(() => { destroyed = true; generation++; });

    function packagesChanged(source: "single" | "pack") {
        if (source === "single") packRefresh++;
        else singleRefresh++;
        requestedRoot = "";
        inventory = null;
        dispatch("changed");
    }
</script>

<NativePackageInstall {gameRoot} refreshToken={singleRefresh} on:changed={() => packagesChanged("single")} />
<NativePackPrepare {gameRoot} refreshToken={packRefresh} on:changed={() => packagesChanged("pack")} />
<section class="native-packages" aria-label="Native packages">
    <div class="inventory-heading">
        <div>
            <h2>Native packages</h2>
            <p class="intro">Modstoke packages in OpenACAI/Mods</p>
        </div>
        <button type="button" disabled={busy || $processing || !gameRoot} on:click={refresh}>{busy ? "Reading…" : "Refresh native packages"}</button>
    </div>
    <p class="scope">Manifests and files describe load candidates. Successful loading and server compatibility are determined when the Loader starts.</p>
    {#if !gameRoot}
        <p role="status">Select an available installation to read its packages.</p>
    {:else if busy || !requestedRoot}
        <p role="status">{busy ? "Reading package manifests…" : "Waiting for the current operation…"}</p>
    {:else if error}
        <p class="error" role="alert">{error}</p>
    {:else if inventory}
        {#if inventory.notices.length}
            <div class="scan-notices" role="status">
                <strong>Scan notes</strong>
                <ul>{#each inventory.notices as notice}<li>{notice}</li>{/each}</ul>
            </div>
        {/if}
        <p class="inventory-summary">{visible.length} of {inventory.packages.length} manifest entries{#if attention} · {attention} need attention{/if}</p>
        {#if folderError}<p class="error" role="alert">{folderError}</p>{/if}
        {#if !inventory.packages.length}
            <div class="empty-state">
                <h3>No native package manifests found</h3>
                <p>{inventory.notices.length ? "Review the scan notes before treating this inventory as complete." : "This installation has no discovered manifests in OpenACAI/Mods."}</p>
                <p>RedLoader and BepInEx packages are listed separately.</p>
            </div>
        {:else if !visible.length}
            <p role="status">No native packages match this search.</p>
        {:else}
            <div class="package-list">
                {#each visible as entry (entry.manifestPath)}
                    <article class="package" class:needs-attention={entry.state === "incomplete" || entry.state === "ambiguous"}>
                        <div class="package-heading">
                            <div class="package-name">
                                <h3>{entry.name}</h3>
                                <p>{entry.packageType === "library" ? "Native library" : "Native mod"}{#if entry.version} · {entry.version}{/if}{#if entry.author} · {entry.author}{/if}</p>
                            </div>
                            <span class="state">{nativePackageStateLabel(entry.state)}</span>
                        </div>
                        <dl>
                            <dt>ID</dt><dd>{entry.id}</dd>
                            <dt>Manifest</dt><dd>{entry.manifestPath}</dd>
                            <dt>Assembly</dt><dd>{entry.assemblyPath ?? (entry.assemblyCandidates.length ? "No unambiguous selection" : "Not resolved")}</dd>
                            {#if entry.entryType}<dt>Entrypoint</dt><dd>{entry.entryType}{#if entry.entryMethod} · {entry.entryMethod}{/if}</dd>{/if}
                        </dl>
                        {#if entry.details.length}<ul class="details">{#each entry.details as detail}<li>{detail}</li>{/each}</ul>{/if}
                        {#if entry.assemblyCandidates.length > 1}
                            <details>
                                <summary>Assembly candidates ({entry.assemblyCandidates.length})</summary>
                                <ul>{#each entry.assemblyCandidates as candidate}<li>{candidate}</li>{/each}</ul>
                            </details>
                        {/if}
                        <button type="button" disabled={busy || $processing} on:click={() => openPackage(entry)}>Open package folder</button>
                    </article>
                {/each}
            </div>
        {/if}
    {/if}
</section>

<style>
    .native-packages { box-sizing: border-box; flex-shrink: 0; min-width: 0; padding: 16px; color: var(--text-primary); background: var(--surface-panel); border: 1px solid var(--border-subtle); border-radius: 10px; }
    .inventory-heading, .package-heading { display: flex; justify-content: space-between; align-items: flex-start; gap: 10px; flex-wrap: wrap; }
    h2 { margin: 0; font-size: 16px; }
    h3 { margin: 0; font-size: 14px; overflow-wrap: anywhere; }
    p { margin: .45em 0; font-size: 13px; line-height: 1.55; overflow-wrap: anywhere; }
    .intro, .scope, .package-name p { color: var(--text-secondary); }
    .scope { max-width: 80em; }
    button { margin: 0; padding: 8px 12px; flex-shrink: 0; max-width: 100%; white-space: normal; overflow-wrap: anywhere; font-size: 13px; border-radius: 8px; }
    :disabled { opacity: .55; }
    .error { color: #ffb4ac; overflow-wrap: anywhere; }
    .scan-notices { background: var(--accent-soft); border: 1px solid color-mix(in srgb, var(--accent) 35%, transparent); padding: 14px; margin-top: .7em; font-size: 13px; border-radius: 8px; }
    ul { margin: .5em 0; padding-left: 1.3em; line-height: 1.5; overflow-wrap: anywhere; }
    .inventory-summary { padding: .5em 0; border-bottom: 1px solid var(--border-subtle); }
    .empty-state { padding: 1.5em .5em; }
    .package-list { display: grid; gap: .7em; }
    .package { min-width: 0; padding: 14px; border: 1px solid var(--border-subtle); background: var(--surface-base); border-radius: 8px; }
    .package.needs-attention { border-left: 3px solid var(--accent); }
    .package-name { min-width: 0; flex: 1 1 200px; }
    .state { flex-shrink: 0; padding: .3em .5em; background: var(--surface-raised); border: 1px solid var(--border-subtle); font-size: 13px; max-width: 100%; box-sizing: border-box; overflow-wrap: anywhere; border-radius: 6px; }
    dl { display: grid; grid-template-columns: 5.5em minmax(0, 1fr); gap: .3em .7em; font-size: 13px; line-height: 1.55; }
    dt { color: var(--text-secondary); }
    dd { margin: 0; overflow-wrap: anywhere; user-select: text; }
    .details, details { font-size: 13px; }
    details { margin: .6em 0; }
    summary { cursor: pointer; padding: 6px 0; color: var(--text-primary); }
    .inventory-heading > div { min-width: 0; flex: 1 1 200px; }
    @media (max-width: 600px) { .inventory-heading, .package-heading { flex-direction: column; } .inventory-heading > div, .package-name { flex-basis: auto; } dl { grid-template-columns: 1fr; } }
</style>
