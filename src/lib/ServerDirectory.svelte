<script lang="ts">
    import { onMount } from "svelte";
    import { selectedInstallation } from "./installationProfiles";
    import { processing, processName, processProgress } from "./store";
    import { readNativeInventory, type NativeInventory } from "./nativeInventory";
    import { compareServerRequirements } from "./serverRequirements";
    import {
        listCommunityServers, getCommunityServer, getCommunityHistory, joinCommunityServer,
        communityRequirements, knownCount, seenAt, type CommunityServer, type CommunityHistory,
    } from "./serverDirectory";

    export let requestedServerId: string | null = null;
    export let requestRevision = 0;

    let mounted = false;
    let servers: CommunityServer[] = [];
    let generatedAt: string | null = null;
    let listBusy = false;
    let listError = "";
    let search = "";
    let onlyOnline = false;
    let selectedId: string | null = null;
    let selected: CommunityServer | null = null;
    let history: CommunityHistory | null = null;
    let detailBusy = false;
    let detailError = "";
    let historyError = "";
    let detailGeneration = 0;
    let handledRevision = -1;
    let inventory: NativeInventory | null = null;
    let inventoryBusy = false;
    let inventoryError = "";
    let inventoryGeneration = 0;
    let previousInstallation = "";
    let joinBusy = false;
    let joinMessage = "";
    let joinError = "";

    $: query = search.trim().toLowerCase();
    $: visibleServers = servers.filter(server => (!onlyOnline || server.online)
        && (!query || `${server.name} ${server.hostLabel} ${server.region}`.toLowerCase().includes(query)));
    $: onlineCount = servers.filter(server => server.online).length;
    $: installation = $selectedInstallation;
    $: playerRoot = installation?.available && installation.target.kind === "player" ? installation.root : null;
    $: installationKey = JSON.stringify([installation?.target.id, playerRoot, $processing]);
    $: if (mounted && previousInstallation !== installationKey) {
        previousInstallation = installationKey;
        void refreshInventory();
    }
    $: comparisons = selected && inventory ? compareServerRequirements(communityRequirements(selected), inventory) : [];
    $: requirementsReady = !!selected && (selected.requiredMods.length === 0
        || (!!inventory && comparisons.length === selected.requiredMods.length && comparisons.every(row => row.status === "match")));
    $: canJoin = !!selected?.online && !!selected.join && !!playerRoot && requirementsReady
        && !detailBusy && !inventoryBusy && !joinBusy && !$processing;
    $: if (mounted && !joinBusy && requestedServerId && handledRevision !== requestRevision) {
        handledRevision = requestRevision;
        void openServer(requestedServerId);
    }

    async function refreshInventory() {
        const generation = ++inventoryGeneration;
        inventory = null;
        inventoryError = "";
        inventoryBusy = false;
        if (!playerRoot || $processing) return;
        inventoryBusy = true;
        try {
            const result = await readNativeInventory(playerRoot);
            if (mounted && generation === inventoryGeneration) inventory = result;
        } catch (error) {
            if (mounted && generation === inventoryGeneration) inventoryError = String(error);
        } finally {
            if (mounted && generation === inventoryGeneration) inventoryBusy = false;
        }
    }

    async function refresh() {
        if (listBusy || joinBusy) return;
        listBusy = true;
        listError = "";
        try {
            const result = await listCommunityServers();
            if (!mounted) return;
            servers = [...result.servers].sort((a, b) => Number(b.online) - Number(a.online) || a.name.localeCompare(b.name));
            generatedAt = result.generatedAt;
        } catch (error) { if (mounted) listError = String(error); }
        finally { if (mounted) listBusy = false; }
        if (mounted && selectedId && !detailBusy) void openServer(selectedId, true);
    }

    async function openServer(id: string, keepSelection = false) {
        if (joinBusy) return;
        const generation = ++detailGeneration;
        selectedId = id;
        detailBusy = true;
        detailError = "";
        historyError = "";
        joinError = "";
        if (!keepSelection) { selected = null; history = null; joinMessage = ""; }
        const results = await Promise.allSettled([getCommunityServer(id), getCommunityHistory(id)]);
        if (!mounted || generation !== detailGeneration) return;
        const [detailsResult, historyResult] = results;
        if (detailsResult.status === "fulfilled") selected = detailsResult.value;
        else { selected = null; detailError = String(detailsResult.reason); }
        if (historyResult.status === "fulfilled") history = historyResult.value;
        else { history = null; historyError = String(historyResult.reason); }
        detailBusy = false;
    }

    async function join() {
        if (!canJoin || !selected || !playerRoot) return;
        joinBusy = true;
        joinMessage = "";
        joinError = "";
        processName.set("Checking server requirements and opening Steam…");
        processProgress.set(0);
        processing.set(true);
        try {
            const result = await joinCommunityServer(selected, playerRoot);
            if (mounted) joinMessage = result;
        } catch (error) { if (mounted) joinError = String(error); }
        finally { joinBusy = false; processing.set(false); processName.set(""); processProgress.set(0); }
    }

    function setting(value: boolean | null): string { return value === null ? "Unknown" : value ? "Yes" : "No"; }

    onMount(() => {
        mounted = true;
        void refresh();
        const interval = window.setInterval(() => { if (document.visibilityState === "visible") void refresh(); }, 60_000);
        const visible = () => { if (document.visibilityState === "visible") void refresh(); };
        document.addEventListener("visibilitychange", visible);
        return () => {
            mounted = false;
            detailGeneration++;
            inventoryGeneration++;
            window.clearInterval(interval);
            document.removeEventListener("visibilitychange", visible);
        };
    });
</script>

<section class="directory" aria-label="Community server browser">
    <header class="directory-header">
        <div><span class="eyebrow">Community</span><h1>Server browser</h1><p>Find a place to play. Review the server and its required mods before joining.</p></div>
        <button disabled={listBusy || joinBusy} on:click={refresh}>{listBusy ? "Refreshing…" : "Refresh"}</button>
    </header>
    <div class="directory-summary"><span>{#if generatedAt}<strong>{onlineCount}</strong> online · {servers.length} listed{#if listError} · last successful snapshot{/if}{:else}{listBusy ? "Loading directory…" : "Directory unavailable"}{/if}</span><span>{#if generatedAt}Updated {seenAt(generatedAt)} · {/if}Refreshes every minute while visible</span></div>
    {#if listError}<div class="message error" role="alert">{listError}{#if generatedAt}<br />Showing the last successful list. Open a server to check its current details.{/if}</div>{/if}
    <div class="filters">
        <label class="search">Search servers<input type="search" bind:value={search} placeholder="Server, host or region" /></label>
        <label class="online-filter"><input type="checkbox" bind:checked={onlyOnline} /> Online only</label>
    </div>
    <div class="directory-layout">
        <div class="server-list" aria-label="Servers">
            {#if !generatedAt && listBusy}<div class="empty" role="status">Loading community servers…</div>
            {:else if visibleServers.length === 0}<div class="empty">{servers.length ? "No servers match these filters." : listError ? "The directory is unavailable. Try Refresh." : "No community servers are listed yet."}</div>
            {:else}
                {#each visibleServers as server (server.id)}
                    <button class="server-row" class:selected={selectedId === server.id} aria-pressed={selectedId === server.id} disabled={joinBusy} on:click={() => openServer(server.id)}>
                        <span class="server-title"><strong>{server.name}</strong><span class="status" class:online={server.online}>{server.online ? "Online" : "Offline"}</span></span>
                        <span>{server.hostLabel} · {server.region || "Region unknown"}</span>
                        <span class="server-footer"><span>{server.kind === "dedicated" ? "Dedicated" : "Player hosted"}</span><span>{knownCount(server.playersCurrent)} / {knownCount(server.playersMax)} players</span></span>
                    </button>
                {/each}
            {/if}
        </div>
        <div class="server-detail" aria-label="Server details" aria-busy={detailBusy}>
            {#if detailError}<div class="message error" role="alert">{detailError}</div><button on:click={() => selectedId && openServer(selectedId)}>Retry details</button>{/if}
            {#if detailBusy}<p role="status" class="muted">Refreshing server details…</p>{/if}
            {#if selected}
                <div class="detail-heading"><div><span class="eyebrow">{selected.kind === "dedicated" ? "Dedicated server" : "Player hosted"}</span><h2>{selected.name}</h2><p>Hosted by {selected.hostLabel}</p></div><span class="status" class:online={selected.online}>{selected.online ? "Online" : "Offline"}</span></div>
                {#if selected.description}<p class="description">{selected.description}</p>{/if}
                <dl class="facts">
                    <div><dt>Players</dt><dd>{knownCount(selected.playersCurrent)} / {knownCount(selected.playersMax)}</dd></div>
                    <div><dt>Region</dt><dd>{selected.region || "Unknown"}</dd></div>
                    <div><dt>Game build</dt><dd>{selected.gameBuild || "Unknown"}</dd></div>
                    <div><dt>Difficulty</dt><dd>{selected.settings.difficulty || "Unknown"}</dd></div>
                    <div><dt>PvP</dt><dd>{setting(selected.settings.pvp)}</dd></div>
                    <div><dt>Friendly fire</dt><dd>{setting(selected.settings.friendlyFire)}</dd></div>
                    <div><dt>Password required</dt><dd>{setting(selected.settings.passwordRequired)}</dd></div>
                    <div><dt>Host median latency</dt><dd>{selected.hostMedianLatencyMs === null ? "Unknown" : `${knownCount(selected.hostMedianLatencyMs)} ms`}</dd></div>
                </dl>
                <p class="caption">Latency is observed by the host for connected players. Your connection may differ.<br />Last reported {seenAt(selected.lastSeen)}</p>
                <div class="detail-section"><h3>Server components</h3>{#if selected.components.length}<ul>{#each selected.components as item}<li><strong>{item.id}</strong> <span>{item.version}</span></li>{/each}</ul>{:else}<p class="muted">No components reported.</p>{/if}</div>
                <div class="detail-section">
                    <div class="section-heading"><h3>Required mods <span>({selected.requiredMods.length})</span></h3><button class="small" disabled={!playerRoot || inventoryBusy || $processing} on:click={refreshInventory}>{inventoryBusy ? "Reading…" : "Recheck files"}</button></div>
                    <p class="caption">{playerRoot ? `Comparing files in ${installation?.target.name}.` : "Select a player installation in Overview to compare your installed mods."} Matching files do not prove runtime compatibility.</p>
                    {#if inventoryError}<div class="message error" role="alert">{inventoryError}</div>{/if}
                    {#if selected.requiredMods.length === 0}<p class="muted">The host reports no required mods.</p>
                    {:else}<ul class="requirements">{#each selected.requiredMods as item, index}<li><div><strong>{item.id}</strong><span>{item.version}</span></div><span class:match={comparisons[index]?.status === "match"}>{comparisons[index]?.status === "match" ? "Files match" : comparisons[index]?.status.replaceAll("-", " ") ?? "Not checked"}</span>{#if comparisons[index]}<p>{comparisons[index].detail}</p>{/if}</li>{/each}</ul>{/if}
                    {#if inventory?.notices.length}<details><summary>Inventory notices ({inventory.notices.length})</summary><ul>{#each inventory.notices as notice}<li>{notice}</li>{/each}</ul></details>{/if}
                </div>
                <div class="join-panel">
                    {#if selected.join}<p class="caption">{selected.join.kind === "lobby" ? `Steam lobby ${selected.join.lobbyId}` : `Game address ${selected.join.address}:${selected.join.port}`}</p>{/if}
                    <button class="join-button" disabled={!canJoin} on:click={join}>{joinBusy ? "Opening Steam…" : "Join server"}</button>
                    <p class="caption">{!selected.online ? "This server is offline." : !selected.join ? "The host has not provided a join target." : !playerRoot ? "Select an available player installation to join." : !requirementsReady ? "Required mod files must match before joining. Review Installed mods to resolve missing or different versions." : "Opens through Steam after checking the latest server details. Continue in the game's connection prompt."}</p>
                    {#if joinError}<div class="message error" role="alert">{joinError}</div>{/if}
                    {#if joinMessage}<div class="message" role="status">{joinMessage}</div>{/if}
                </div>
                <div class="detail-section"><h3>Activity history</h3><p class="caption">Hourly aggregates from the last 30 days. Unknown observations stay unknown.</p>
                    {#if historyError}<p class="message error">{historyError}</p>
                    {:else if history?.samples.length}<div class="history-scroll"><table><thead><tr><th>Time</th><th>Players</th><th>Host latency</th></tr></thead><tbody>{#each [...history.samples].reverse() as sample}<tr><td>{seenAt(sample.at)}</td><td>{knownCount(sample.players)}</td><td>{sample.latencyMs === null ? "Unknown" : `${knownCount(sample.latencyMs)} ms`}</td></tr>{/each}</tbody></table></div>
                    {:else}<p class="muted">No activity history is available yet.</p>{/if}
                </div>
            {:else if !detailBusy && !detailError}<div class="empty detail-empty"><h2>Choose a server</h2><p>See its current status, required mods and recent activity here.</p></div>{/if}
        </div>
    </div>
</section>

<style>
    .directory { width: 100%; min-width: 0; }
    .directory-header, .directory-summary, .filters, .server-title, .server-footer, .detail-heading, .section-heading { display: flex; align-items: center; justify-content: space-between; gap: 14px; }
    .directory-header { align-items: flex-start; margin-bottom: 22px; }
    h1 { margin: 5px 0 8px; font-size: 28px; letter-spacing: -0.7px; }
    h2 { margin: 5px 0; font-size: 21px; overflow-wrap: anywhere; }
    h3 { margin: 0 0 10px; font-size: 14px; } h3 span { color: var(--text-secondary); font-weight: 400; }
    p { margin: 0; } .directory-header p, .detail-heading p, .muted { color: var(--text-secondary); }
    .eyebrow { color: var(--accent); font-size: 10px; letter-spacing: 1.4px; text-transform: uppercase; }
    .directory-summary { padding: 13px 0; border-top: 1px solid var(--border-subtle); border-bottom: 1px solid var(--border-subtle); color: var(--text-secondary); font-size: 11px; flex-wrap: wrap; }
    .directory-summary strong { color: #a8d8b7; }
    .filters { justify-content: flex-start; margin: 20px 0; }
    .search { flex: 1; max-width: 460px; display: grid; gap: 6px; color: var(--text-secondary); font-size: 11px; }
    .search input { width: 100%; box-sizing: border-box; }
    .online-filter { display: flex; align-items: center; gap: 8px; margin-top: 20px; font-size: 12px; }
    .online-filter input { float: none; flex-shrink: 0; margin: 0; }
    .directory-layout { display: grid; grid-template-columns: minmax(220px, 0.8fr) minmax(330px, 1.5fr); gap: 20px; align-items: start; }
    .server-list { display: grid; gap: 8px; max-height: 72vh; overflow-y: auto; padding: 3px; }
    .server-row { display: grid; gap: 9px; width: 100%; text-align: left; padding: 16px; border: 1px solid var(--border-subtle); background: var(--surface-panel); box-shadow: none; }
    .server-row.selected { border-color: var(--accent); background: var(--accent-soft); }
    .server-row > span:not(.server-title), .server-footer { color: var(--text-secondary); font-size: 11px; }
    .server-title { align-items: flex-start; } .server-title strong { overflow-wrap: anywhere; min-width: 0; font-size: 13px; }
    .status { border-radius: 20px; padding: 3px 8px; background: #323640; color: #b9c0cb; font-size: 10px; white-space: nowrap; }
    .status.online { background: #213c2c; color: #a8d8b7; }
    .server-footer { flex-wrap: wrap; gap: 6px; }
    .server-detail { padding: 24px; min-width: 0; border: 1px solid var(--border-subtle); border-radius: 12px; background: var(--surface-panel); overflow-wrap: anywhere; }
    .detail-heading { align-items: flex-start; } .description { margin-top: 18px; white-space: pre-wrap; }
    .facts { display: grid; grid-template-columns: 1fr 1fr; gap: 15px; margin: 24px 0 18px; }
    .facts dt { color: var(--text-secondary); font-size: 11px; } .facts dd { margin: 3px 0 0; font-size: 13px; }
    .caption { font-size: 11px; color: var(--text-secondary); line-height: 1.65; }
    .detail-section { border-top: 1px solid var(--border-subtle); margin-top: 23px; padding-top: 20px; }
    ul { list-style: none; padding: 0; margin: 10px 0 0; display: grid; gap: 8px; font-size: 12px; }
    li span { color: var(--text-secondary); margin-left: 7px; }
    .requirements li { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 6px; padding: 10px 0; border-bottom: 1px solid var(--border-subtle); }
    .requirements li p { width: 100%; color: var(--text-secondary); font-size: 11px; }
    .requirements .match { color: #a8d8b7; } .section-heading h3 { margin: 0; }
    .small { font-size: 11px; min-height: 30px; padding: 5px 9px; } .section-heading { margin-bottom: 10px; }
    details { margin-top: 12px; color: var(--text-secondary); font-size: 11px; } summary { cursor: pointer; }
    .join-panel { margin-top: 22px; padding: 16px; border-radius: 8px; background: var(--surface-base); display: grid; gap: 11px; }
    .join-button { color: #171a21; background: var(--accent); border-color: var(--accent); font-weight: 600; justify-self: start; }
    .join-button:enabled:hover { background: #f5c689; color: #171a21; border-color: #f5c689; }
    .message { margin: 12px 0; padding: 12px; background: var(--surface-raised); border-radius: 7px; font-size: 12px; overflow-wrap: anywhere; }
    .error { color: #efadb3; border: 1px solid #693b44; }
    .empty { padding: 28px 18px; color: var(--text-secondary); line-height: 1.7; text-align: center; } .detail-empty { padding: 65px 20px; }
    .history-scroll { max-height: 260px; overflow: auto; margin-top: 12px; }
    table { border-collapse: collapse; width: 100%; font-size: 11px; text-align: left; }
    th, td { padding: 9px 6px; border-bottom: 1px solid var(--border-subtle); } th { position: sticky; top: 0; background: var(--surface-panel); color: var(--text-secondary); }
    @media (max-width: 1100px) { .directory-layout { grid-template-columns: minmax(190px, 0.8fr) minmax(290px, 1.3fr); gap: 12px; } .server-detail { padding: 18px; } .server-row { padding: 12px; } }
    @media (max-width: 850px) { .directory-layout { grid-template-columns: 1fr; } .server-list { max-height: 300px; } .directory-header { flex-wrap: wrap; } }
</style>
