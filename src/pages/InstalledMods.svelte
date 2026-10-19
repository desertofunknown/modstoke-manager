<script lang="ts">
    import { createEventDispatcher, onDestroy, onMount } from "svelte";
    import { get } from "svelte/store";
    import { dirname } from "@tauri-apps/api/path";
    import { confirm } from "@tauri-apps/plugin-dialog";
    import { open } from "@tauri-apps/plugin-shell";
    import LucideSearch from "~icons/lucide/search";
    import LucideRefreshCw from "~icons/lucide/refresh-cw";
    import LucideFolderOpen from "~icons/lucide/folder-open";
    import LucideX from "~icons/lucide/x";
    import NativePackages from "../lib/NativePackages.svelte";
    import PathSelector from "../lib/PathSelector.svelte";
    import { selectedInstallation } from "../lib/installationProfiles";
    import { nativeInstallationKey } from "../lib/nativePackage";
    import { ModDatabase } from "../lib/mods";
    import { gameExePath, isPathValid, processing, processName, processProgress } from "../lib/store";
    import {
        assertCurrentGameRoot, describeInstallSource, scanInstalledInventory, setInventoryEntryEnabled,
        type InstalledInventoryEntry, type InstallSource, type LoaderType
    } from "../lib/modInventory";

    type InventoryTab = "mods" | "native";
    type SortColumn = "name" | "enabled" | "version" | "loaderType" | "installSource";
    type SelectionContext = { identity: string; root: string; executable: string; generation: number; name: string };
    type OperationNotice = { message: string; details: string[]; warning: boolean };

    const dispatch = createEventDispatcher<{ navigate: { id: "mods" | "main" }; changed: void }>();
    const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });
    let tab: InventoryTab = "mods";
    let entries: InstalledInventoryEntry[] = [];
    let searchTerm = "";
    let stateFilter: "all" | "enabled" | "disabled" = "all";
    let sourceFilter: "all" | InstallSource = "all";
    let loaderFilter: "all" | LoaderType = "all";
    let sortColumn: SortColumn = "name";
    let sortAscending = true;
    let selectedKeys = new Set<string>();
    let detailKey = "";
    let currentIdentity = "";
    let gameRoot = "";
    let generation = 0;
    let scanRequest = 0;
    let mounted = false;
    let destroyed = false;
    let scanning = false;
    let scanned = false;
    let actionBusy = false;
    let folderBusy = false;
    let scanError = "";
    let folderError = "";
    let notice: OperationNotice | null = null;

    const unsubscribe = selectedInstallation.subscribe(installation => {
        const identity = nativeInstallationKey(installation);
        if (identity === currentIdentity) return;
        currentIdentity = identity;
        gameRoot = installation?.available && installation.target.kind === "player" ? installation.root ?? "" : "";
        generation++;
        scanRequest++;
        entries = [];
        selectedKeys = new Set();
        detailKey = "";
        scanning = false;
        scanned = false;
        scanError = "";
        folderError = "";
        notice = null;
    });

    $: available = !!gameRoot && !!$selectedInstallation?.available
        && $selectedInstallation.target.kind === "player" && $isPathValid
        && $gameExePath === $selectedInstallation.target.executable;
    $: if (mounted && tab === "mods" && available && !scanned && !scanning && !actionBusy && !$processing) void refresh();
    $: query = searchTerm.trim().toLocaleLowerCase();
    $: visibleEntries = entries.filter(entry =>
        (stateFilter === "all" || entry.enabled === (stateFilter === "enabled"))
        && (sourceFilter === "all" || entry.installSource === sourceFilter)
        && (loaderFilter === "all" || entry.loaderType === loaderFilter)
        && (!query || [entry.name, entry.id, entry.author, entry.version, loaderLabel(entry.loaderType),
            sourceLabel(entry), entry.vortexPackage].some(value => value?.toLocaleLowerCase().includes(query)))
    ).sort((a, b) => compareEntries(a, b, sortColumn, sortAscending));
    $: enabledCount = entries.filter(entry => entry.enabled).length;
    $: vortexCount = entries.filter(entry => entry.installSource === "vortex").length;
    $: selectedEntries = entries.filter(entry => selectedKeys.has(entryKey(entry)));
    $: selectableVisible = visibleEntries.filter(canChangeEntry);
    $: allVisibleSelected = selectableVisible.length > 0 && selectableVisible.every(entry => selectedKeys.has(entryKey(entry)));
    $: someVisibleSelected = !allVisibleSelected && selectableVisible.some(entry => selectedKeys.has(entryKey(entry)));
    $: selectedHiddenCount = selectedEntries.filter(entry => !visibleEntries.includes(entry)).length;
    $: enableCount = selectedEntries.filter(entry => canChangeEntry(entry) && !entry.enabled).length;
    $: disableCount = selectedEntries.filter(entry => canChangeEntry(entry) && entry.enabled).length;
    $: detailEntry = entries.find(entry => entryKey(entry) === detailKey) ?? null;
    $: busy = scanning || actionBusy || $processing;
    $: hasFilters = !!searchTerm || stateFilter !== "all" || sourceFilter !== "all" || loaderFilter !== "all";

    function normalizedPath(value: string) {
        return value.replace(/\\/g, "/").replace(/\/+$/g, "").toLocaleLowerCase();
    }

    function entryKey(entry: InstalledInventoryEntry) {
        const location = entry.manifestPath ?? entry.assemblyPath ?? entry.packagePath ?? entry.id;
        return `${entry.loaderType}:${entry.id}:${normalizedPath(location)}`;
    }

    function operationKey(entry: InstalledInventoryEntry) {
        return entryKey(entry).replace(/\/_disabled(?=\/)/g, "").replace(/\.disabled$/, ".dll");
    }

    function loaderLabel(loader: LoaderType) {
        return loader === "bepinex-plugin" ? "BepInEx" : loader === "redloader-library" ? "RedLoader library" : "RedLoader";
    }

    function sourceLabel(entry: InstalledInventoryEntry) {
        return entry.installSource === "native" ? `${describeInstallSource(entry)} / local`
            : entry.installSource === "vortex" ? "Vortex" : describeInstallSource(entry);
    }

    function canChangeEntry(entry: InstalledInventoryEntry) {
        return entry.installSource !== "vortex" && !!(entry.assemblyPath || entry.packagePath);
    }

    function actionReason(entry: InstalledInventoryEntry) {
        return entry.installSource === "vortex" ? "Managed by Vortex. Change its deployment in Vortex."
            : !canChangeEntry(entry) ? "No assembly or package path is available for this entry." : "";
    }

    function compareEntries(a: InstalledInventoryEntry, b: InstalledInventoryEntry, column: SortColumn, ascending: boolean) {
        const value = (entry: InstalledInventoryEntry) => column === "enabled" ? (entry.enabled ? "Enabled" : "Disabled")
            : column === "loaderType" ? loaderLabel(entry.loaderType)
            : column === "installSource" ? sourceLabel(entry) : entry[column] ?? "";
        const primary = collator.compare(value(a), value(b));
        return (ascending ? primary : -primary) || collator.compare(a.name, b.name) || collator.compare(entryKey(a), entryKey(b));
    }

    function sortBy(column: SortColumn) {
        sortAscending = sortColumn === column ? !sortAscending : true;
        sortColumn = column;
    }

    function sortState(column: SortColumn, active: SortColumn, ascending: boolean): "ascending" | "descending" | "none" {
        return column === active ? ascending ? "ascending" : "descending" : "none";
    }

    function sortMarker(column: SortColumn, active: SortColumn, ascending: boolean) {
        return column === active ? ascending ? "↑" : "↓" : "";
    }

    function context(): SelectionContext | null {
        const installation = get(selectedInstallation);
        if (destroyed || !installation?.available || installation.target.kind !== "player" || !installation.root
            || !get(isPathValid) || get(gameExePath) !== installation.target.executable) return null;
        return { identity: nativeInstallationKey(installation), root: installation.root,
            executable: installation.target.executable, name: installation.target.name, generation };
    }

    function isCurrent(request: SelectionContext) {
        const installation = get(selectedInstallation);
        return !destroyed && request.generation === generation && request.identity === nativeInstallationKey(installation)
            && request.root === gameRoot && installation?.available && installation.target.kind === "player"
            && get(isPathValid) && request.executable === get(gameExePath);
    }

    async function assertCurrent(request: SelectionContext) {
        if (!isCurrent(request)) throw new Error("The selected installation changed. Refresh its installed mods before continuing.");
        await assertCurrentGameRoot(request.root);
        if (!isCurrent(request)) throw new Error("The selected installation changed. Refresh its installed mods before continuing.");
    }

    function publishInventory(result: InstalledInventoryEntry[]) {
        const selected = entries.filter(entry => selectedKeys.has(entryKey(entry))).map(operationKey);
        const detail = entries.find(entry => entryKey(entry) === detailKey);
        const selectedMatches = selected.flatMap(key => {
            const matches = result.filter(entry => canChangeEntry(entry) && operationKey(entry) === key);
            return matches.length === 1 ? [entryKey(matches[0])] : [];
        });
        const detailMatches = detail ? result.filter(entry => operationKey(entry) === operationKey(detail)) : [];
        entries = result;
        selectedKeys = new Set(selectedMatches);
        detailKey = detailMatches.length === 1 ? entryKey(detailMatches[0]) : "";
        scanned = true;
    }

    async function refresh() {
        if (destroyed || scanning || actionBusy || get(processing)) return;
        const request = context();
        if (!request) return;
        const token = ++scanRequest;
        scanning = true;
        scanned = true;
        scanError = "";
        folderError = "";
        try {
            await assertCurrent(request);
            const result = await scanInstalledInventory(request.root);
            if (isCurrent(request) && token === scanRequest) publishInventory(result);
        } catch (failure) {
            if (isCurrent(request) && token === scanRequest) {
                entries = [];
                selectedKeys = new Set();
                detailKey = "";
                scanError = String(failure);
            }
        } finally {
            if (token === scanRequest) scanning = false;
        }
    }

    function selectEntry(entry: InstalledInventoryEntry, selected: boolean) {
        if (busy || !canChangeEntry(entry)) return;
        const next = new Set(selectedKeys);
        if (selected) next.add(entryKey(entry));
        else next.delete(entryKey(entry));
        selectedKeys = next;
    }

    function selectVisible(selected: boolean) {
        if (busy) return;
        const next = new Set(selectedKeys);
        for (const entry of selectableVisible) {
            if (selected) next.add(entryKey(entry));
            else next.delete(entryKey(entry));
        }
        selectedKeys = next;
    }

    function clearFilters() {
        searchTerm = "";
        stateFilter = "all";
        sourceFilter = "all";
        loaderFilter = "all";
    }

    function selectTab(next: InventoryTab) {
        if (actionBusy || get(processing) || next === tab) return;
        tab = next;
        if (next === "mods") scanned = false;
    }

    function inventoryChanged() {
        ModDatabase.invalidateInstalledMods();
        if (!destroyed) dispatch("changed");
    }

    function nativePackagesChanged() {
        scanned = false;
        inventoryChanged();
    }

    async function changeEntries(candidates: InstalledInventoryEntry[], shouldEnable: boolean) {
        if (destroyed || scanning || actionBusy || get(processing) || scanError) return;
        const request = context();
        const targets = candidates.filter(entry => entries.includes(entry) && canChangeEntry(entry) && entry.enabled !== shouldEnable)
            .map(entry => ({ key: operationKey(entry), name: entry.name }));
        if (!request || !targets.length) return;
        const action = shouldEnable ? "Enable" : "Disable";
        const desiredState = shouldEnable ? "enabled" : "disabled";
        let ownsProcessing = false;
        let verifiedInventory: InstalledInventoryEntry[] | null = null;
        const failures: string[] = [];
        let attempted = 0;
        actionBusy = true;
        notice = null;
        folderError = "";
        try {
            await assertCurrent(request);
            if (get(processing)) return;
            const restoresFolder = shouldEnable && candidates.some(entry => entry.loaderType === "bepinex-plugin"
                && normalizedPath(entry.packagePath ?? "").includes("/_disabled/"));
            const accepted = await confirm(
                `${action} ${targets.length} of ${candidates.length} selected ${candidates.length === 1 ? "entry" : "entries"} in “${request.name}”?\n\n${request.root}\n\n`
                + (targets.length === 1 ? targets[0].name : targets.map(entry => entry.name).slice(0, 8).join("\n")
                    + (targets.length > 8 ? `\n…and ${targets.length - 8} more` : ""))
                + (restoresFolder ? "\n\nRestoring a disabled BepInEx package folder can also restore the other assemblies in that folder." : "")
                + "\n\nChanges apply the next time the game starts.",
                { title: `${action} installed mods`, kind: "warning", okLabel: action, cancelLabel: "Cancel" }
            );
            if (!accepted) return;
            await assertCurrent(request);
            if (get(processing)) return;
            ownsProcessing = true;
            processing.set(true);
            processName.set("Checking installed mod files…");
            processProgress.set(0);
            verifiedInventory = await scanInstalledInventory(request.root);
            await assertCurrent(request);
            for (const [index, target] of targets.entries()) {
                await assertCurrent(request);
                const matches = verifiedInventory.filter(entry => operationKey(entry) === target.key);
                const entry = matches.length === 1 ? matches[0] : null;
                if (!entry) {
                    failures.push(`${target.name}: ${matches.length ? "more than one matching entry was found" : "the entry was no longer found"}. No change was attempted.`);
                } else if (!canChangeEntry(entry)) {
                    failures.push(`${target.name}: ${actionReason(entry)} No change was attempted.`);
                } else if (entry.enabled !== shouldEnable) {
                    processName.set(`${shouldEnable ? "Enabling" : "Disabling"} ${entry.name} (${index + 1}/${targets.length})…`);
                    attempted++;
                    verifiedInventory = null;
                    try {
                        // The API mutates its argument. Only a subsequent disk scan can update this view.
                        await setInventoryEntryEnabled({ ...entry }, shouldEnable);
                    } catch (failure) {
                        failures.push(`${target.name}: ${String(failure)}`);
                    }
                    await assertCurrent(request);
                    processName.set(`Verifying installed mod files (${index + 1}/${targets.length})…`);
                    // Shared package folders can move together; use new paths for every later item.
                    verifiedInventory = await scanInstalledInventory(request.root);
                    await assertCurrent(request);
                }
                processProgress.set(Math.round(((index + 1) / targets.length) * 100));
            }
            const confirmed = targets.filter(target => {
                const matches = verifiedInventory!.filter(entry => operationKey(entry) === target.key);
                return matches.length === 1 && matches[0].enabled === shouldEnable;
            }).length;
            for (const target of targets) {
                const matches = verifiedInventory.filter(entry => operationKey(entry) === target.key);
                if (matches.length === 1 && matches[0].enabled !== shouldEnable) {
                    failures.push(`${target.name}: still ${matches[0].enabled ? "enabled" : "disabled"} after scanning the files again.`);
                } else if (!matches.length && !failures.some(failure => failure.startsWith(`${target.name}:`))) {
                    failures.push(`${target.name}: no longer found after scanning the files again.`);
                }
            }
            publishInventory(verifiedInventory);
            notice = {
                message: `${confirmed} of ${targets.length} requested ${targets.length === 1 ? "entry is" : "entries are"} ${desiredState} in ${request.name}, verified from files.`
                    + (confirmed < targets.length ? ` ${targets.length - confirmed} ${targets.length - confirmed === 1 ? "entry was" : "entries were"} unchanged or could not be confirmed.` : ""),
                details: failures,
                warning: confirmed < targets.length || failures.length > 0
            };
        } catch (failure) {
            if (isCurrent(request)) {
                if (verifiedInventory) publishInventory(verifiedInventory);
                else if (attempted) {
                    entries = [];
                    selectedKeys = new Set();
                    detailKey = "";
                    scanError = "The latest file state could not be read. Refresh before making another change.";
                }
                notice = { message: attempted ? "The operation stopped before all changes could be verified. Refresh the inventory to check the current files."
                    : "No changes were made. The operation could not start.", details: [...failures, String(failure)], warning: true };
            }
        } finally {
            actionBusy = false;
            if (ownsProcessing) {
                processName.set("");
                processProgress.set(0);
                processing.set(false);
            }
            if (attempted) inventoryChanged();
        }
    }

    async function openPackageFolder(entry: InstalledInventoryEntry) {
        if (busy || folderBusy || !entries.includes(entry)) return;
        const request = context();
        const location = entry.packagePath ?? entry.assemblyPath;
        if (!request || !location) return;
        folderBusy = true;
        folderError = "";
        try {
            const folder = location === entry.assemblyPath ? await dirname(location) : location;
            await assertCurrent(request);
            await open(folder);
        } catch (failure) {
            if (isCurrent(request)) folderError = String(failure);
        } finally {
            folderBusy = false;
        }
    }

    onMount(() => { mounted = true; });
    onDestroy(() => { destroyed = true; generation++; scanRequest++; unsubscribe(); });
</script>

<div class="installed-page">
    <header class="page-heading">
        <div class="heading-copy">
            <h1>Installed mods</h1>
            <p>{available ? $selectedInstallation?.target.name : "Manage the files in a player installation."}</p>
        </div>
        <button type="button" class="browse-button" disabled={$processing || actionBusy} on:click={() => dispatch("navigate", { id: "mods" })}>Browse mods <span aria-hidden="true">↗</span></button>
    </header>

    <div class="installation-selector"><PathSelector kind="player" /></div>

    {#if !available}
        <section class="unavailable" aria-label="Select a player installation">
            <h2>Select a player installation</h2>
            <p>{$selectedInstallation?.target.kind === "server"
                ? "This workspace manages player mods. Use the selected server’s own workspace for its native packages."
                : $selectedInstallation?.problem ?? "Choose an available Sons of the Forest player installation to read its installed mods."}</p>
            <button type="button" disabled={$processing || actionBusy} on:click={() => dispatch("navigate", { id: "main" })}>View installations</button>
        </section>
    {:else}
        <div class="workspace-toolbar">
            <div class="inventory-tabs" role="group" aria-label="Installed package type">
                <button type="button" class:active={tab === "mods"} aria-pressed={tab === "mods"} disabled={$processing || actionBusy} on:click={() => selectTab("mods")}>Mods &amp; plugins{#if scanned && !scanError}<span>{entries.length}</span>{/if}</button>
                <button type="button" class:active={tab === "native"} aria-pressed={tab === "native"} disabled={$processing || actionBusy} on:click={() => selectTab("native")}>Native packages</button>
            </div>
            <label class="search-field">
                <LucideSearch aria-hidden="true" />
                <span class="sr-only">Search installed {tab === "native" ? "native packages" : "mods and plugins"}</span>
                <input type="search" bind:value={searchTerm} placeholder={tab === "native" ? "Search native packages…" : "Search installed mods…"} />
            </label>
        </div>

        {#if tab === "native"}
            <div class="native-workspace">
                {#key gameRoot}
                    <NativePackages {gameRoot} {searchTerm} on:changed={nativePackagesChanged} />
                {/key}
            </div>
        {:else}
            <div class="filter-bar">
                <label><span class="sr-only">Filter by state</span><select bind:value={stateFilter}><option value="all">All states</option><option value="enabled">Enabled</option><option value="disabled">Disabled</option></select></label>
                <label><span class="sr-only">Filter by loader</span><select bind:value={loaderFilter}><option value="all">All loaders</option><option value="redloader-mod">RedLoader mods</option><option value="redloader-library">RedLoader libraries</option><option value="bepinex-plugin">BepInEx plugins</option></select></label>
                <label><span class="sr-only">Filter by source</span><select bind:value={sourceFilter}><option value="all">All sources</option><option value="native">Modstoke / local</option><option value="manual">Manual / local</option><option value="vortex">Vortex</option></select></label>
                {#if hasFilters}<button type="button" class="clear-button" on:click={clearFilters}>Clear filters</button>{/if}
                <button type="button" class="refresh-button" disabled={busy} on:click={refresh}><LucideRefreshCw aria-hidden="true" />{scanning ? "Scanning…" : "Refresh"}</button>
            </div>

            {#if notice}
                <div class="operation-notice" class:warning={notice.warning} role={notice.warning ? "alert" : "status"}>
                    <div><p>{notice.message}</p>{#if notice.details.length}<details><summary>Details ({notice.details.length})</summary><ul>{#each notice.details as detail}<li>{detail}</li>{/each}</ul></details>{/if}</div>
                    <button type="button" aria-label="Dismiss operation result" on:click={() => { notice = null; }}><LucideX aria-hidden="true" /></button>
                </div>
            {/if}
            {#if folderError}<p class="inline-error" role="alert">Could not open the package folder: {folderError}</p>{/if}

            <section class="inventory-panel" aria-label="Installed mod inventory" aria-busy={scanning}>
                <div class="inventory-summary">
                    <p>{#if scanning}Scanning installed files…{:else if !scanned}Waiting for the current operation…{:else if scanError}Inventory unavailable{:else}<strong>{visibleEntries.length}</strong> of {entries.length} entries <span>· {enabledCount} enabled · {entries.length - enabledCount} disabled{#if vortexCount} · {vortexCount} managed by Vortex{/if}</span>{/if}</p>
                    {#if selectedEntries.length}<span class="selected-count">{selectedEntries.length} selected{#if selectedHiddenCount} · {selectedHiddenCount} hidden by filters{/if}</span>{/if}
                </div>
                <div class="inventory-body">
                    <div class="table-wrap">
                        <table>
                            <caption class="sr-only">Installed RedLoader mods, libraries and BepInEx plugins. File state does not confirm successful loading.</caption>
                            <thead>
                                <tr>
                                    <th scope="col" class="select-cell"><input type="checkbox" aria-label="Select all visible editable entries" checked={allVisibleSelected} indeterminate={someVisibleSelected} disabled={busy || !selectableVisible.length} on:change={event => selectVisible(event.currentTarget.checked)} /></th>
                                    <th scope="col" aria-sort={sortState("enabled", sortColumn, sortAscending)}><button type="button" on:click={() => sortBy("enabled")}>State <span aria-hidden="true">{sortMarker("enabled", sortColumn, sortAscending)}</span></button></th>
                                    <th scope="col" class="name-cell" aria-sort={sortState("name", sortColumn, sortAscending)}><button type="button" on:click={() => sortBy("name")}>Name <span aria-hidden="true">{sortMarker("name", sortColumn, sortAscending)}</span></button></th>
                                    <th scope="col" aria-sort={sortState("version", sortColumn, sortAscending)}><button type="button" on:click={() => sortBy("version")}>Version <span aria-hidden="true">{sortMarker("version", sortColumn, sortAscending)}</span></button></th>
                                    <th scope="col" aria-sort={sortState("loaderType", sortColumn, sortAscending)}><button type="button" on:click={() => sortBy("loaderType")}>Loader <span aria-hidden="true">{sortMarker("loaderType", sortColumn, sortAscending)}</span></button></th>
                                    <th scope="col" aria-sort={sortState("installSource", sortColumn, sortAscending)}><button type="button" on:click={() => sortBy("installSource")}>Source <span aria-hidden="true">{sortMarker("installSource", sortColumn, sortAscending)}</span></button></th>
                                    <th scope="col"><span class="sr-only">Action</span></th>
                                </tr>
                            </thead>
                            <tbody>
                                {#each visibleEntries as entry (entryKey(entry))}
                                    <tr class:selected={selectedKeys.has(entryKey(entry))} class:inspected={detailKey === entryKey(entry)}>
                                        <td class="select-cell"><input type="checkbox" aria-label={`Select ${entry.name}`} title={actionReason(entry) || `Select ${entry.name}`} checked={selectedKeys.has(entryKey(entry))} disabled={busy || !canChangeEntry(entry)} on:change={event => selectEntry(entry, event.currentTarget.checked)} /></td>
                                        <td><span class="state" class:enabled={entry.enabled}><span aria-hidden="true"></span>{entry.enabled ? "Enabled" : "Disabled"}</span></td>
                                        <td class="name-cell"><button type="button" class="entry-name" aria-label={`Details for ${entry.name}`} aria-expanded={detailKey === entryKey(entry)} aria-controls="installed-mod-details" on:click={() => { detailKey = entryKey(entry); }}>{entry.name}</button></td>
                                        <td class="version-cell">{entry.version ?? "—"}</td>
                                        <td>{loaderLabel(entry.loaderType)}</td>
                                        <td title={entry.installSource === "native" ? "Local RedLoader package; download origin is not recorded by this scan." : describeInstallSource(entry)}><span class:vortex-source={entry.installSource === "vortex"}>{sourceLabel(entry)}</span></td>
                                        <td class="action-cell">{#if entry.installSource === "vortex"}<span class="read-only" title={actionReason(entry)}>Read-only</span>{:else}<button type="button" class="row-action" disabled={busy || !canChangeEntry(entry)} title={actionReason(entry) || `${entry.enabled ? "Disable" : "Enable"} ${entry.name}`} on:click={() => changeEntries([entry], !entry.enabled)}>{entry.enabled ? "Disable" : "Enable"}</button>{/if}</td>
                                    </tr>
                                {:else}
                                    <tr><td class="empty-cell" colspan="7">
                                        {#if scanning}<p role="status">Reading Mods, Libs and BepInEx/plugins…</p>
                                        {:else if scanError}<h2>Could not read installed mods</h2><p class="inline-error" role="alert">{scanError}</p><button type="button" disabled={busy} on:click={refresh}>Try again</button>
                                        {:else if !scanned}<p role="status">The inventory will refresh when the current operation finishes.</p>
                                        {:else if entries.length}<h2>No matching entries</h2><p>Change the search or filters to see your installed mods.</p><button type="button" on:click={clearFilters}>Clear filters</button>
                                        {:else}<h2>No mods or plugins found</h2><p>This scan covers Mods, Libs and BepInEx/plugins. Check Native packages for OpenACAI/Mods.</p><button type="button" on:click={() => dispatch("navigate", { id: "mods" })}>Browse mods</button>{/if}
                                    </td></tr>
                                {/each}
                            </tbody>
                        </table>
                    </div>
                    {#if detailEntry}
                        <aside class="entry-details" id="installed-mod-details" aria-label={`Details for ${detailEntry.name}`}>
                            <div class="details-heading"><h2>{detailEntry.name}</h2><button type="button" class="icon-button" aria-label="Close mod details" on:click={() => { detailKey = ""; }}><LucideX aria-hidden="true" /></button></div>
                            <p class="detail-state">{detailEntry.enabled ? "Enabled" : "Disabled"} · {loaderLabel(detailEntry.loaderType)}</p>
                            <dl>
                                <dt>Version</dt><dd>{detailEntry.version ?? "Not recorded"}</dd>
                                <dt>Author</dt><dd>{detailEntry.author ?? "Not recorded"}</dd>
                                <dt>Source</dt><dd>{detailEntry.installSource === "native" ? "Modstoke / local · origin unverified" : describeInstallSource(detailEntry)}</dd>
                                <dt>Mod ID</dt><dd>{detailEntry.id}</dd>
                                {#if detailEntry.nexusModId}<dt>Nexus ID</dt><dd>{detailEntry.nexusModId}</dd>{/if}
                                <dt>Folder</dt><dd>{detailEntry.expectedLocation}</dd>
                                <dt>Package</dt><dd>{detailEntry.packagePath ?? "Not resolved"}</dd>
                                <dt>Assembly</dt><dd>{detailEntry.assemblyPath ?? "Not resolved"}</dd>
                                {#if detailEntry.manifestPath}<dt>Manifest</dt><dd>{detailEntry.manifestPath}</dd>{/if}
                            </dl>
                            {#if actionReason(detailEntry)}<p class="detail-note">{actionReason(detailEntry)}</p>{/if}
                            <button type="button" class="folder-button" disabled={busy || folderBusy || !(detailEntry.packagePath || detailEntry.assemblyPath)} on:click={() => detailEntry && openPackageFolder(detailEntry)}><LucideFolderOpen aria-hidden="true" />Open package folder</button>
                        </aside>
                    {/if}
                </div>
                <footer class="bulk-bar">
                    <span>{selectedEntries.length ? `${selectedEntries.length} selected` : "Select entries to manage together"}</span>
                    <div class="bulk-actions"><button type="button" disabled={busy || !enableCount} on:click={() => changeEntries(selectedEntries, true)}>Enable selected{enableCount ? ` (${enableCount})` : ""}</button><button type="button" disabled={busy || !disableCount} on:click={() => changeEntries(selectedEntries, false)}>Disable selected{disableCount ? ` (${disableCount})` : ""}</button>{#if selectedEntries.length}<button type="button" class="clear-button" disabled={busy} on:click={() => { selectedKeys = new Set(); }}>Clear</button>{/if}</div>
                </footer>
            </section>
            <p class="inventory-footnote">File state is shown here; the game confirms whether a mod loads. Vortex entries are managed in Vortex.</p>
        {/if}
    {/if}
</div>

<style>
    .installed-page { display: flex; flex: 1 1 auto; flex-direction: column; gap: 14px; min-width: 0; min-height: 0; }
    .page-heading, .workspace-toolbar, .filter-bar, .inventory-summary, .bulk-bar, .bulk-actions, .details-heading { display: flex; align-items: center; gap: 12px; }
    .page-heading { justify-content: space-between; flex-shrink: 0; }
    .heading-copy { min-width: 0; }
    h1 { margin: 0; font-size: 25px; letter-spacing: -.5px; }
    .heading-copy p { margin: 5px 0 0; color: var(--text-secondary); font-size: 12px; overflow-wrap: anywhere; }
    .installation-selector { flex-shrink: 0; min-width: 0; }
    .installation-selector :global(#path-selector) { display: grid; grid-template-columns: 110px minmax(0, 1fr); gap: 5px 12px; padding: 10px 12px; border-radius: 6px; }
    .installation-selector :global(.selector-heading) { flex-direction: column; justify-content: center; align-items: flex-start; gap: 2px; }
    .installation-selector :global(.profile-row) { flex-direction: row; align-items: center; gap: 10px; }
    .installation-selector :global(.profile-row select) { flex: 1 1 0; width: 0; height: 32px; padding: 6px 10px; font-size: 12px; }
    .installation-selector :global(.profile-actions button) { min-height: 30px; padding: 5px 8px; font-size: 11px; }
    .installation-selector :global(.path-row) { grid-column: 2; }
    .installation-selector :global(.rename-row), .installation-selector :global(.error-row), .installation-selector :global(.hint), .installation-selector :global(.installation-problem) { grid-column: 1 / -1; }
    .browse-button { display: flex; align-items: center; gap: 18px; flex-shrink: 0; color: var(--accent); background: var(--accent-soft); border-color: rgba(238, 182, 107, .2); }
    .workspace-toolbar { justify-content: space-between; border-bottom: 1px solid var(--border-subtle); gap: 20px; flex-shrink: 0; }
    .inventory-tabs { display: flex; gap: 20px; align-self: stretch; }
    .inventory-tabs button { display: flex; align-items: center; gap: 7px; padding: 10px 0 13px; border: 0; border-radius: 0; border-bottom: 2px solid transparent; background: transparent; box-shadow: none; color: var(--text-secondary); white-space: nowrap; }
    .inventory-tabs button.active { border-bottom-color: var(--accent); color: var(--text-primary); }
    .inventory-tabs button span { min-width: 16px; padding: 1px 5px; border-radius: 4px; background: var(--surface-raised); color: var(--text-secondary); font-size: 11px; text-align: center; }
    .search-field { display: flex; align-items: center; gap: 8px; flex: 0 1 290px; min-width: 170px; margin-bottom: 8px; padding: 0 10px; background: var(--surface-panel); border: 1px solid var(--border-subtle); border-radius: 6px; }
    .search-field:focus-within { border-color: var(--accent); }
    .search-field :global(svg) { width: 15px; height: 15px; flex-shrink: 0; color: var(--text-secondary); }
    .search-field input { width: 100%; min-width: 0; padding: 8px 0; background: transparent; border: 0; border-radius: 0; font-size: 12px; }
    .search-field input:focus-visible { outline-offset: 1px; }
    .filter-bar { flex-wrap: wrap; gap: 8px; flex-shrink: 0; }
    .filter-bar select { min-height: 32px; padding: 6px 27px 6px 10px; font-size: 12px; }
    .filter-bar button { min-height: 32px; font-size: 12px; }
    .refresh-button { display: flex; align-items: center; gap: 7px; margin-left: auto; }
    .refresh-button :global(svg), .folder-button :global(svg) { width: 14px; height: 14px; flex-shrink: 0; }
    .clear-button { padding: 6px 8px; background: transparent; border-color: transparent; box-shadow: none; color: var(--text-secondary); }
    .inventory-panel { display: flex; flex: 1 1 auto; flex-direction: column; min-width: 0; min-height: 210px; border: 1px solid var(--border-subtle); border-radius: 7px; overflow: hidden; background: var(--surface-panel); }
    .inventory-summary { justify-content: space-between; flex-wrap: wrap; gap: 4px 14px; padding: 10px 12px; border-bottom: 1px solid var(--border-subtle); font-size: 11px; color: var(--text-secondary); flex-shrink: 0; }
    .inventory-summary p { margin: 0; }
    .inventory-summary strong { color: var(--text-primary); font-weight: 600; }
    .selected-count { color: var(--accent); }
    .inventory-body { display: flex; flex: 1 1 auto; min-height: 0; min-width: 0; }
    .table-wrap { flex: 1 1 auto; min-width: 0; min-height: 130px; overflow: auto; }
    table { width: 100%; min-width: 740px; border-collapse: separate; border-spacing: 0; font-size: 12px; text-align: left; }
    th { position: sticky; top: 0; z-index: 1; padding: 0 10px; height: 35px; background: var(--surface-raised); border-bottom: 1px solid var(--border-subtle); color: var(--text-secondary); font-size: 11px; font-weight: 500; white-space: nowrap; }
    th button { display: inline-flex; align-items: center; gap: 5px; min-height: 32px; padding: 0; border: 0; border-radius: 2px; background: transparent; box-shadow: none; color: inherit; font-size: inherit; font-weight: inherit; }
    th button span { color: var(--accent); }
    td { height: 43px; padding: 0 10px; border-bottom: 1px solid var(--border-subtle); color: var(--text-secondary); white-space: nowrap; }
    tbody tr:last-child td { border-bottom: 0; }
    tbody tr:hover { background: rgba(255, 255, 255, .025); }
    tbody tr.selected { background: rgba(238, 182, 107, .055); }
    tbody tr.inspected { background: var(--accent-soft); }
    .select-cell { width: 34px; padding: 0 8px 0 12px; }
    .select-cell input { float: none; margin: 0; width: 15px; height: 15px; border-radius: 3px; }
    .select-cell input::before { width: 8px; height: 8px; }
    .select-cell input:indeterminate::before { transform: scale(1); height: 2px; }
    .name-cell { width: 100%; min-width: 150px; max-width: 340px; }
    .entry-name { display: block; max-width: 100%; min-height: 32px; padding: 3px 0; border: 0; border-radius: 2px; background: transparent; box-shadow: none; color: var(--text-primary); font-size: 12px; font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; text-align: left; }
    .entry-name:hover { color: var(--accent); background: transparent; }
    .version-cell { max-width: 90px; overflow: hidden; text-overflow: ellipsis; font-variant-numeric: tabular-nums; }
    .state { display: inline-flex; align-items: center; gap: 6px; font-size: 11px; }
    .state > span { width: 5px; height: 5px; border-radius: 50%; background: #6d7686; }
    .state.enabled { color: #a9cdb9; }
    .state.enabled > span { background: #85b49c; }
    .vortex-source { color: #d3b285; }
    .action-cell { text-align: right; }
    .row-action { min-height: 27px; padding: 3px 7px; border-color: transparent; background: transparent; box-shadow: none; color: var(--text-secondary); font-size: 11px; }
    .read-only { font-size: 10px; color: var(--text-secondary); }
    .empty-cell { height: 210px; padding: 26px; text-align: center; white-space: normal; }
    .empty-cell h2 { margin: 0 0 8px; font-size: 17px; }
    .empty-cell p { max-width: 510px; margin: 0 auto 15px; line-height: 1.6; }
    .entry-details { box-sizing: border-box; flex: 0 0 280px; min-width: 0; padding: 15px; overflow: auto; border-left: 1px solid var(--border-subtle); background: var(--surface-base); }
    .details-heading { align-items: flex-start; justify-content: space-between; gap: 8px; }
    .details-heading h2 { margin: 2px 0 0; font-size: 15px; overflow-wrap: anywhere; }
    .icon-button, .operation-notice > button { display: grid; place-items: center; min-height: 25px; width: 25px; padding: 4px; border: 0; background: transparent; box-shadow: none; flex-shrink: 0; }
    .icon-button :global(svg), .operation-notice > button :global(svg) { width: 14px; height: 14px; }
    .detail-state { margin: 7px 0 16px; color: var(--text-secondary); font-size: 11px; }
    dl { display: grid; grid-template-columns: 58px minmax(0, 1fr); gap: 10px 9px; margin: 0 0 18px; font-size: 11px; line-height: 1.5; }
    dt { color: var(--text-secondary); }
    dd { margin: 0; overflow-wrap: anywhere; user-select: text; }
    .detail-note { margin: 0 0 12px; color: var(--accent); font-size: 11px; line-height: 1.6; }
    .folder-button { display: inline-flex; align-items: center; gap: 7px; min-height: 32px; padding: 6px 9px; font-size: 11px; }
    .bulk-bar { justify-content: space-between; flex-wrap: wrap; gap: 6px 12px; flex-shrink: 0; min-height: 38px; padding: 7px 10px 7px 12px; border-top: 1px solid var(--border-subtle); background: var(--surface-panel); color: var(--text-secondary); font-size: 11px; }
    .bulk-actions { gap: 6px; flex-wrap: wrap; }
    .bulk-actions button { min-height: 29px; padding: 5px 8px; font-size: 11px; }
    .inventory-footnote { margin: -4px 0 0; flex-shrink: 0; color: var(--text-secondary); font-size: 11px; line-height: 1.5; }
    .operation-notice { display: flex; justify-content: space-between; align-items: flex-start; gap: 12px; padding: 9px 11px; flex-shrink: 0; border: 1px solid #385347; border-radius: 5px; background: rgba(76, 127, 99, .08); font-size: 12px; }
    .operation-notice.warning { border-color: rgba(238, 182, 107, .3); background: var(--accent-soft); }
    .operation-notice p { margin: 2px 0; }
    .operation-notice details { margin-top: 6px; }
    .operation-notice summary { cursor: pointer; color: var(--accent); }
    .operation-notice ul { max-height: 125px; margin: 8px 0 0; padding-left: 18px; overflow: auto; color: var(--text-secondary); overflow-wrap: anywhere; }
    .inline-error { margin: 0; color: #eeb2ac; font-size: 12px; overflow-wrap: anywhere; }
    .unavailable { margin-top: 20px; max-width: 600px; padding: 24px; border: 1px solid var(--border-subtle); border-radius: 7px; background: var(--surface-panel); }
    .unavailable h2 { margin: 0 0 10px; font-size: 19px; }
    .unavailable p { color: var(--text-secondary); font-size: 13px; line-height: 1.7; }
    .native-workspace { display: flex; flex-direction: column; gap: 12px; flex: 1 1 auto; min-height: 0; overflow: auto; padding-bottom: 4px; }
    .native-workspace :global(section) { color: var(--text-primary); border-color: var(--border-subtle); background: var(--surface-panel); border-radius: 7px; }
    .sr-only { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; border: 0; }
    @media (max-width: 1280px) {
        .inventory-body { flex-direction: column; overflow: auto; }
        .entry-details { flex: 0 0 auto; max-height: 190px; padding: 12px 15px; border-left: 0; border-top: 1px solid var(--border-subtle); }
        .entry-details dl { grid-template-columns: 58px minmax(0, 1fr) 58px minmax(0, 1fr); gap: 7px 10px; margin-bottom: 10px; }
        .detail-state { margin-bottom: 12px; }
    }
    @media (max-width: 900px) {
        .installed-page { gap: 11px; }
        .workspace-toolbar { gap: 12px; flex-wrap: wrap; }
        .inventory-tabs { gap: 16px; }
        .search-field { flex: 1 1 210px; }
        .filter-bar { gap: 6px; }
        .filter-bar select { max-width: 155px; }
        .inventory-summary p > span { display: block; margin-top: 3px; }
        .entry-details dl { grid-template-columns: 58px minmax(0, 1fr); }
    }
</style>
