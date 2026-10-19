<script lang="ts">
    import { onDestroy } from "svelte";
    import { get } from "svelte/store";
    import { save } from "@tauri-apps/plugin-dialog";
    import { writeTextFile } from "@tauri-apps/plugin-fs";
    import { processing } from "./store";
    import type { InspectedInstallation } from "./installationProfiles";
    import { readNativeInventory, type NativeInventory } from "./nativeInventory";
    import {
        parseServerRequirements, createServerRequirements, compareServerRequirements,
        selectableNativePackages, type ServerRequirements
    } from "./serverRequirements";

    export let installation: InspectedInstallation | null;

    let expanded = false;
    let busy = false;
    let destroyed = false;
    let generation = 0;
    let currentKey = "";
    let inventory: NativeInventory | null = null;
    let document: ServerRequirements | null = null;
    let selectedPaths: string[] = [];
    let listName = "Community server";
    let pastedJson = "";
    let showPaste = false;
    let showExport = false;
    let error = "";
    let notice = "";

    function installationKey(value: InspectedInstallation | null): string {
        return JSON.stringify([value?.target.id, value?.target.kind, value?.target.executable, value?.root, value?.available]);
    }

    function resetInventory(key: string) {
        if (key === currentKey) return;
        currentKey = key;
        generation++;
        inventory = null;
        selectedPaths = [];
        error = "";
        notice = "";
    }

    $: resetInventory(installationKey(installation));
    $: if ($processing) {
        generation++;
        inventory = null;
        selectedPaths = [];
    }
    $: available = !!installation?.available && !!installation.root;
    $: candidates = inventory ? selectableNativePackages(inventory) : [];
    $: comparisons = document && inventory ? compareServerRequirements(document, inventory) : [];
    $: matching = comparisons.filter(row => row.status === "match").length;
    $: exportJson = document ? JSON.stringify(document, null, 2) + "\n" : "";

    async function readPackages() {
        if (destroyed || busy || get(processing) || !installation?.available || !installation.root) return;
        const root = installation.root;
        const key = installationKey(installation);
        const request = ++generation;
        busy = true;
        error = "";
        notice = "";
        inventory = null;
        selectedPaths = [];
        try {
            const result = await readNativeInventory(root);
            if (isCurrent(request, key)) inventory = result;
        } catch (failure) {
            if (isCurrent(request, key)) error = String(failure);
        } finally { busy = false; }
    }

    function isCurrent(request: number, key: string): boolean {
        return !destroyed && request === generation && key === installationKey(installation) && !get(processing);
    }

    async function importText(text: string) {
        const parsed = await parseServerRequirements(text);
        if (destroyed) return;
        document = parsed;
        showExport = false;
        notice = `Loaded ${parsed.packages.length} requirement${parsed.packages.length === 1 ? "" : "s"}. ${inventory ? "Comparison uses the last inventory read." : "Read this installation to compare its files."}`;
    }

    async function importFile(event: Event) {
        const input = event.currentTarget as HTMLInputElement;
        const file = input.files?.[0];
        input.value = "";
        if (!file || destroyed || busy || get(processing)) return;
        busy = true;
        error = "";
        notice = "";
        try {
            if (file.size > 65536) throw new Error("Requirements files must be 64 KiB or smaller.");
            await importText(await file.text());
        } catch (failure) { if (!destroyed) error = String(failure); }
        finally { busy = false; }
    }

    async function importPaste() {
        if (destroyed || busy || get(processing)) return;
        busy = true;
        error = "";
        notice = "";
        try { await importText(pastedJson); }
        catch (failure) { if (!destroyed) error = String(failure); }
        finally { busy = false; }
    }

    async function useSelection() {
        if (destroyed || busy || get(processing) || !inventory) return;
        const request = generation;
        const key = installationKey(installation);
        const packages = candidates.filter(entry => selectedPaths.includes(entry.manifestPath)).map(entry => ({
            loader: "modstoke" as const, id: entry.id, version: entry.version!
        }));
        busy = true;
        error = "";
        notice = "";
        try {
            const parsed = await createServerRequirements(listName, packages);
            if (isCurrent(request, key)) {
                document = parsed;
                showExport = false;
                notice = "List created from your selection. Save the JSON to share it.";
            }
        } catch (failure) { if (isCurrent(request, key)) error = String(failure); }
        finally { busy = false; }
    }

    async function saveDocument() {
        if (destroyed || busy || get(processing) || !document) return;
        const text = exportJson;
        busy = true;
        error = "";
        notice = "";
        try {
            const path = await save({
                title: "Save server requirements", defaultPath: "modstoke-server-requirements.json",
                filters: [{ name: "Requirements JSON", extensions: ["json"] }]
            });
            if (!path || destroyed) return;
            await writeTextFile(path, text);
            if (!destroyed) notice = "Requirements saved. This file does not activate a server admission policy.";
        } catch (failure) { if (!destroyed) error = String(failure); }
        finally { busy = false; }
    }

    function statusLabel(status: string): string {
        switch (status) {
            case "match": return "Files match";
            case "missing": return "Missing";
            case "version-mismatch": return "Different version";
            case "disabled": return "Disabled";
            case "incomplete": return "Incomplete";
            case "ambiguous": return "Conflicting entries";
            default: return "Not established";
        }
    }

    onDestroy(() => { destroyed = true; generation++; });
</script>

<details class="requirements" bind:open={expanded}>
    <summary><strong>Server requirements</strong><span>{document ? `${document.packages.length} exact version${document.packages.length === 1 ? "" : "s"}` : "Prepare a shared mod list"}</span></summary>
    {#if expanded}
        <div class="content">
            <p class="scope">Share the native Modstoke packages your community needs. Choose each required package explicitly, or import a list to compare with a player or server installation.</p>
            <p class="note">This prepares a list; it does not enforce admission, download mods or confirm successful loading. RedLoader and BepInEx requirements are not supported in this format yet.</p>
            <div class="import-row">
                <label class="file-input">Import JSON<input type="file" accept=".json,application/json" disabled={busy || $processing} on:change={importFile} /></label>
                <button type="button" disabled={busy || $processing} aria-expanded={showPaste} on:click={() => showPaste = !showPaste}>{showPaste ? "Hide paste field" : "Paste JSON"}</button>
            </div>
            {#if showPaste}
                <label class="field">Requirements JSON<textarea bind:value={pastedJson} maxlength="65536" rows="6" spellcheck="false" disabled={busy || $processing}></textarea></label>
                <button type="button" disabled={busy || $processing || !pastedJson.trim()} on:click={importPaste}>Read pasted list</button>
            {/if}
            {#if error}<p class="error" role="alert">{error}</p>{/if}
            {#if notice}<p role="status">{notice}</p>{/if}
            <div class="installation-row">
                <p>{installation?.target.name ?? "No installation selected"}{#if installation}{` · ${installation.target.kind === "server" ? "Dedicated server" : "Player"}`}{/if}</p>
                <button type="button" disabled={busy || $processing || !available} on:click={readPackages}>{busy ? "Working…" : "Read installation"}</button>
            </div>
            {#if !available}<p class="note">Choose an available installation above to compare files or select packages.</p>{/if}
            {#if inventory?.notices.length}
                <div class="scan-notes"><strong>Inventory is incomplete</strong><ul>{#each inventory.notices as note}<li>{note}</li>{/each}</ul></div>
            {/if}
            {#if document}
                <section class="list-result" aria-label="Requirements comparison">
                    <h3>{document.name}</h3>
                    <p class="note">{#if inventory}{matching} of {document.packages.length} requirements match the observed files. Versions must match exactly.{:else}Read the selected installation to compare these requirements.{/if}</p>
                    <ul class="requirements-list">
                        {#each document.packages as requirement, index (requirement.id)}
                            <li>
                                <div class="requirement-heading"><strong>{requirement.id}</strong><span class:match={comparisons[index]?.status === "match"}>{comparisons[index] ? statusLabel(comparisons[index].status) : "Not compared"}</span></div>
                                <p class="version">Required: {requirement.version}</p>
                                {#if comparisons[index]?.observedVersion}<p class="version">Observed: {comparisons[index].observedVersion}</p>{/if}
                                {#if comparisons[index]}<p class="note">{comparisons[index].detail}</p>{/if}
                            </li>
                        {/each}
                    </ul>
                    <div class="actions">
                        <button type="button" disabled={busy || $processing} on:click={saveDocument}>Save JSON</button>
                        <button type="button" disabled={busy || $processing} aria-expanded={showExport} on:click={() => showExport = !showExport}>{showExport ? "Hide JSON" : "Show JSON"}</button>
                    </div>
                    <p class="note">Save this list before leaving Main. Unsaved lists are kept only in this view.</p>
                    {#if showExport}<label class="field">Portable requirements<textarea readonly value={exportJson} rows="9" spellcheck="false"></textarea></label>{/if}
                </section>
            {/if}
            {#if inventory}
                <details class="create-list">
                    <summary>Create a list from this installation</summary>
                    {#if candidates.length}
                        <p class="note">Select only packages that participating clients should have. Installation alone does not make a package a server requirement.</p>
                        <label class="field">List name<input type="text" bind:value={listName} maxlength="120" disabled={busy || $processing} /></label>
                        <div class="candidates">
                            {#each candidates as entry (entry.manifestPath)}
                                <label><input type="checkbox" bind:group={selectedPaths} value={entry.manifestPath} disabled={busy || $processing} /><span><strong>{entry.name}</strong><small>{entry.id} · {entry.version}{entry.packageType === "library" ? " · Library" : ""}</small></span></label>
                            {/each}
                        </div>
                        <button type="button" disabled={busy || $processing || !selectedPaths.length || selectedPaths.length > 128 || !listName.trim()} on:click={useSelection}>Use {selectedPaths.length} selected package{selectedPaths.length === 1 ? "" : "s"}</button>
                        {#if document}<p class="note">Using this selection replaces the list shown above. Previously saved files stay unchanged.</p>{/if}
                    {:else}<p>No unambiguous native packages with a version are available for selection.</p>{/if}
                </details>
            {/if}
        </div>
    {/if}
</details>

<style>
    .requirements { box-sizing: border-box; flex-shrink: 0; width: 100%; min-width: 0; margin: 0 auto; border: 1px solid var(--border-subtle); border-radius: 10px; background: var(--surface-panel); color: var(--text-primary); text-align: left; }
    summary { padding: 16px; cursor: pointer; font-size: 13px; line-height: 1.6; overflow-wrap: anywhere; }
    summary span { margin-left: .8em; color: var(--text-secondary); font-size: 13px; display: inline-block; }
    .content { padding: 0 16px 16px; }
    p { margin: .55em 0; font-size: 13px; line-height: 1.5; overflow-wrap: anywhere; }
    .note { color: var(--text-secondary); }
    .import-row, .actions, .installation-row { display: flex; flex-wrap: wrap; align-items: center; gap: .6em; margin: .8em 0; }
    .installation-row { justify-content: space-between; border-top: 1px solid var(--border-subtle); padding-top: .5em; }
    button { margin: 0; padding: 8px 12px; font-size: 13px; max-width: 100%; white-space: normal; overflow-wrap: anywhere; border-radius: 8px; }
    :disabled { opacity: .55; }
    .file-input { font-size: 13px; display: flex; flex-wrap: wrap; align-items: center; gap: .6em; min-width: 0; }
    .file-input input { width: 230px; max-width: 100%; font: inherit; margin: 0; padding: .4em; }
    .field { display: flex; flex-direction: column; gap: .4em; margin: .8em 0; font-size: 13px; font-weight: 500; }
    .field input, textarea { box-sizing: border-box; width: 100%; min-width: 0; margin: 0; padding: 9px 12px; background: var(--surface-base); color: var(--text-primary); border: 1px solid var(--border-subtle); border-radius: 8px; }
    textarea { resize: vertical; font: 12px/1.6 monospace; user-select: text; }
    .error { color: #ffb4ac; }
    .scan-notes { padding: 14px; background: var(--accent-soft); border: 1px solid color-mix(in srgb, var(--accent) 35%, transparent); font-size: 13px; border-radius: 8px; }
    ul { padding-left: 1.2em; line-height: 1.5; overflow-wrap: anywhere; }
    h3 { margin: .7em 0; font-size: 14px; overflow-wrap: anywhere; }
    .requirements-list { list-style: none; padding: 0; max-height: 360px; overflow: auto; }
    .requirements-list li { margin: .5em 0; padding: 14px; border: 1px solid var(--border-subtle); background: var(--surface-base); border-radius: 8px; }
    .requirement-heading { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: baseline; gap: .5em; font-size: 13px; }
    .requirement-heading strong { min-width: 0; overflow-wrap: anywhere; }
    .requirement-heading span { color: var(--accent); font-size: 13px; }
    .requirement-heading .match { color: #a6cbb4; }
    .version { user-select: text; }
    .create-list { margin-top: .8em; border-top: 1px solid var(--border-subtle); }
    .create-list summary { padding-left: 0; }
    .candidates { max-height: 260px; overflow: auto; margin: .8em 0; }
    .candidates label { display: flex; align-items: flex-start; gap: .6em; padding: .6em; margin-bottom: .3em; background: var(--surface-base); font-size: 13px; border-radius: 8px; }
    .candidates input { flex-shrink: 0; width: 18px; margin: .25em 0 0; height: 18px; float: none; padding: 0; }
    .candidates span { min-width: 0; overflow-wrap: anywhere; }
    .candidates small { display: block; color: var(--text-secondary); margin-top: .25em; font-size: 12px; }
    .installation-row p { min-width: 0; }
</style>
