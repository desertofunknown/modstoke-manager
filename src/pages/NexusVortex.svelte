<script lang="ts">
    import { onDestroy, onMount } from "svelte";
    import SvgSpinnersBlocksWave from "~icons/svg-spinners/blocks-wave";
    import {
        clearNexusApiKey,
        fetchNexusSotfMods,
        getNexusModDownloadUrl,
        getNexusModPageUrl,
        getNexusSession,
        NEXUS_CACHE_TTL_MINUTES,
        saveNexusApiKey,
        type NexusMod,
        type NexusSession,
        type NexusView
    } from "../lib/nexus";
    import {
        describeInstallSource,
        findMatchingInstall,
        readVortexDeployment,
        scanInstalledInventory,
        setInventoryEntryEnabled,
        type InstalledInventoryEntry
    } from "../lib/modInventory";
    import { getDirectoryPath, isPathValid } from "../lib/store";
    import * as dialog from "@tauri-apps/plugin-dialog"
    import * as shell from "@tauri-apps/plugin-shell"

    let session: NexusSession = { is_connected: false };
    let apiKey = "";
    let mods: NexusMod[] = [];
    let inventory: InstalledInventoryEntry[] = [];
    let selectedView: NexusView = "trending";
    let nexusSearchTerm = "";
    let selectedNexusCategory = "all";
    let selectedInstallFilter = "all";
    let isLoading = false;
    let status = "";
    let vortexStagingPath: string | null = null;
    let ssoSocket: WebSocket | null = null;
    let ssoTimeout: number | null = null;

    const NEXUS_SSO_URL = "wss://sso.nexusmods.com";
    const NEXUS_SSO_APPLICATION_SLUG = "openacai-mod-manager";
    const NEXUS_SSO_PROTOCOL = 2;
    const NEXUS_SSO_UUID_KEY = "openacai-nexus-sso-request-id";
    const NEXUS_SSO_TOKEN_KEY = "openacai-nexus-sso-connection-token";
    const NEXUS_MANUAL_REFRESH_COOLDOWN_MS = 60_000;
    let nextManualRefreshAt = 0;

    $: nexusCategories = Array.from(new Set(mods.map(mod => mod.category_name).filter(Boolean) as string[])).sort();
    $: visibleNexusMods = mods.filter(matchesNexusFilters);

    onMount(async () => {
        await refreshInventory();
        await refreshSession();

        if (session.is_connected) {
            await loadMods();
        }
    });

    onDestroy(() => {
        cleanupSso();
    });

    async function refreshSession() {
        try {
            session = await getNexusSession();
        } catch (error) {
            const message = `${error}`;
            session = {
                is_connected: false,
                error: isTauriBridgeError(message) ? undefined : message
            };
        }
    }

    function isTauriBridgeError(message: string): boolean {
        return message.includes("__TAURI_IPC__") || message.includes("reading 'invoke'");
    }

    async function refreshInventory() {
        if (!$isPathValid) {
            return;
        }

        inventory = await scanInstalledInventory();
        const vortex = await readVortexDeployment(await getDirectoryPath());
        vortexStagingPath = vortex.stagingPath;
    }

    async function connectWithNexus() {
        cleanupSso();
        isLoading = true;
        status = "Opening Nexus login...";

        const requestId = getOrCreateSsoRequestId();
        const token = localStorage.getItem(NEXUS_SSO_TOKEN_KEY);

        try {
            ssoSocket = new WebSocket(NEXUS_SSO_URL);
            ssoSocket.onopen = async () => {
                ssoSocket?.send(JSON.stringify({
                    id: requestId,
                    token,
                    protocol: NEXUS_SSO_PROTOCOL
                }));

                await shell.open(`https://www.nexusmods.com/sso?id=${encodeURIComponent(requestId)}&application=${encodeURIComponent(NEXUS_SSO_APPLICATION_SLUG)}`);
                status = "Approve the Nexus connection in your browser.";
            };

            ssoSocket.onmessage = async (event) => {
                await handleSsoMessage(event.data);
            };

            ssoSocket.onerror = async () => {
                await finishSsoWithError("Nexus login failed before approval. The OpenACAI Mod Manager app slug must be registered with Nexus Mods before browser SSO can complete. Use Advanced manual token until Nexus approves the slug.");
            };

            ssoSocket.onclose = () => {
                if (isLoading && status.includes("Approve")) {
                    status = "Nexus login closed before approval.";
                    isLoading = false;
                }
            };

            ssoTimeout = window.setTimeout(async () => {
                await finishSsoWithError("Nexus login timed out. Try Login with Nexus again.");
            }, 90000);
        } catch (error) {
            await finishSsoWithError(`${error}`);
        }
    }

    async function connectWithManualKey() {
        if (!apiKey.trim()) {
            await dialog.message("Paste a Nexus Mods API key before using the advanced manual connection.", {
                title: "Nexus account",
                kind: "info"
            });
            return;
        }

        isLoading = true;
        status = "Validating Nexus account...";

        try {
            session = await saveNexusApiKey(apiKey);
            apiKey = "";
            await loadMods();
        } catch (error) {
            await dialog.message(`${error}`, {
                title: "Nexus account error",
                kind: "error"
            });
        } finally {
            status = "";
            isLoading = false;
        }
    }

    function getOrCreateSsoRequestId(): string {
        const requestId = crypto.randomUUID();
        localStorage.setItem(NEXUS_SSO_UUID_KEY, requestId);
        return requestId;
    }

    async function handleSsoMessage(rawMessage: string) {
        let payload: any;
        try {
            payload = JSON.parse(rawMessage);
        } catch {
            return;
        }

        if (!payload.success) {
            await finishSsoWithError(payload.error ?? "Nexus login was not approved.");
            return;
        }

        if (payload.data?.connection_token) {
            localStorage.setItem(NEXUS_SSO_TOKEN_KEY, payload.data.connection_token);
        }

        if (!payload.data?.api_key) {
            return;
        }

        status = "Validating approved Nexus session...";

        try {
            session = await saveNexusApiKey(payload.data.api_key);
            await loadMods();
        } catch (error) {
            await dialog.message(`${error}`, {
                title: "Nexus account error",
                kind: "error"
            });
        } finally {
            cleanupSso();
            status = "";
            isLoading = false;
        }
    }

    async function finishSsoWithError(message: string) {
        cleanupSso();
        status = "";
        isLoading = false;
        await dialog.message(message, {
            title: "Nexus login",
            kind: "error"
        });
    }

    function cleanupSso() {
        if (ssoTimeout !== null) {
            window.clearTimeout(ssoTimeout);
            ssoTimeout = null;
        }

        if (ssoSocket) {
            ssoSocket.onopen = null;
            ssoSocket.onmessage = null;
            ssoSocket.onerror = null;
            ssoSocket.onclose = null;

            if (ssoSocket.readyState === WebSocket.OPEN || ssoSocket.readyState === WebSocket.CONNECTING) {
                ssoSocket.close();
            }

            ssoSocket = null;
        }
    }

    async function disconnect() {
        cleanupSso();
        await clearNexusApiKey();
        session = { is_connected: false };
        mods = [];
    }

    async function loadMods(view: NexusView = selectedView, forceRefresh = false) {
        selectedView = view;
        if (!session.is_connected) {
            return;
        }

        if (forceRefresh) {
            const now = Date.now();
            if (now < nextManualRefreshAt) {
                const seconds = Math.ceil((nextManualRefreshAt - now) / 1000);
                status = `Nexus refresh available in ${seconds}s.`;
                return;
            }

            nextManualRefreshAt = now + NEXUS_MANUAL_REFRESH_COOLDOWN_MS;
        }

        isLoading = true;
        status = forceRefresh ? "Refreshing Nexus mods..." : "Loading Nexus mods...";

        try {
            await refreshInventory();
            const response = await fetchNexusSotfMods(selectedView, { force: forceRefresh });
            mods = response.mods;
            session = {
                ...session,
                rate_limit: response.rate_limit
            };
        } catch (error) {
            await dialog.message(`${error}`, {
                title: "Nexus Mods error",
                kind: "error"
            });
        } finally {
            status = "";
            isLoading = false;
        }
    }

    async function openApiKeys() {
        await shell.open("https://www.nexusmods.com/users/myaccount?tab=api%20access");
    }

    async function openNexusGame() {
        await shell.open("https://www.nexusmods.com/games/sonsoftheforest");
    }

    async function openVortexStaging() {
        if (vortexStagingPath) {
            await shell.open(vortexStagingPath);
        }
    }

    async function openModPage(mod: NexusMod) {
        await shell.open(getNexusModPageUrl(mod));
    }

    async function openDownloadPage(mod: NexusMod) {
        await shell.open(getNexusModDownloadUrl(mod));
    }

    async function toggleMatchedMod(match: InstalledInventoryEntry, event: Event) {
        const checked = (event.currentTarget as HTMLInputElement).checked;
        try {
            await setInventoryEntryEnabled(match, checked);
            await refreshInventory();
        } catch (error) {
            await dialog.message(`${error}`, {
                title: "Mod state",
                kind: "info"
            });
        }
    }

    function installedMatch(mod: NexusMod): InstalledInventoryEntry | null {
        return findMatchingInstall(inventory, mod.name, mod.mod_id, [
            mod.author ?? "",
            mod.uploaded_by ?? ""
        ]);
    }

    function matchesNexusFilters(mod: NexusMod): boolean {
        const search = nexusSearchTerm.trim().toLowerCase();
        const match = installedMatch(mod);

        if (search && ![
            mod.name,
            mod.summary ?? "",
            mod.author ?? "",
            mod.uploaded_by ?? ""
        ].some(value => value.toLowerCase().includes(search))) {
            return false;
        }

        if (selectedNexusCategory !== "all" && mod.category_name !== selectedNexusCategory) {
            return false;
        }

        if (selectedInstallFilter === "installed" && !match) {
            return false;
        }

        if (selectedInstallFilter === "missing" && match) {
            return false;
        }

        if (selectedInstallFilter === "vortex" && match?.installSource !== "vortex") {
            return false;
        }

        if (selectedInstallFilter === "native" && match?.installSource !== "native") {
            return false;
        }

        if (selectedInstallFilter === "manual" && match?.installSource !== "manual") {
            return false;
        }

        return true;
    }

    function viewLabel(view: NexusView): string {
        switch (view) {
            case "latest_added":
                return "Latest";
            case "latest_updated":
                return "Updated";
            default:
                return "Trending";
        }
    }

    function formatTimestamp(value?: number, fallback?: string): string {
        if (value) {
            return new Date(value * 1000).toLocaleDateString();
        }

        return fallback ? new Date(fallback).toLocaleDateString() : "-";
    }
</script>

<div class="column nexus-page">
    <section class="account-panel">
        <div class="account-copy">
            <span class="panel-title">Vortex / Nexus Mods</span>
            {#if session.is_connected}
                <span class="panel-subtitle">Connected as {session.user?.name ?? "Nexus user"}</span>
            {:else}
                <span class="panel-subtitle">Login through Nexus to inspect Nexus-hosted Sons Of The Forest mods and Vortex deployments. Browser SSO requires Nexus app approval; manual tokens are for development testing only.</span>
            {/if}
        </div>

        {#if session.is_connected}
            <div class="account-actions">
                <button on:click={openNexusGame}>Open Nexus</button>
                {#if vortexStagingPath}
                    <button on:click={openVortexStaging}>Open Vortex Staging</button>
                {/if}
                <button class="uninstall" on:click={disconnect}>Disconnect</button>
            </div>
        {:else}
            <div class="connect-column">
                <div class="connect-row">
                    <button class="install login-button" disabled={isLoading} on:click={connectWithNexus}>Login with Nexus</button>
                    <button on:click={openApiKeys}>API Access</button>
                </div>
                <details class="manual-key">
                    <summary>Advanced manual token for testing</summary>
                    <div class="connect-row manual-row">
                        <input class="generic-input key-input" bind:value={apiKey} type="password" placeholder="Nexus API key" />
                        <button on:click={connectWithManualKey}>Save Token</button>
                    </div>
                    <span class="manual-warning">Public releases should use the Nexus-approved application slug and SSO flow.</span>
                </details>
            </div>
        {/if}
    </section>

    {#if status}
        <div class="notice live-status">{status}</div>
    {/if}

    {#if session.error}
        <div class="notice warning">{session.error}</div>
    {/if}

    {#if session.rate_limit}
        <div class="rate-row">
            <span>Hourly {session.rate_limit.hourly_remaining ?? "-"} / {session.rate_limit.hourly_limit ?? "-"}</span>
            <span>Daily {session.rate_limit.daily_remaining ?? "-"} / {session.rate_limit.daily_limit ?? "-"}</span>
        </div>
    {/if}

    {#if session.is_connected}
        <div class="row-center controls">
            <button class="btn-left cat-btn" class:cat-btn-selected={selectedView === "trending"} on:click={() => loadMods("trending")}>{viewLabel("trending")}</button>
            <button class="cat-btn middle-btn" class:cat-btn-selected={selectedView === "latest_added"} on:click={() => loadMods("latest_added")}>{viewLabel("latest_added")}</button>
            <button class="btn-right cat-btn" class:cat-btn-selected={selectedView === "latest_updated"} on:click={() => loadMods("latest_updated")}>{viewLabel("latest_updated")}</button>
            <button class="cat-btn refresh-btn" disabled={isLoading} on:click={() => loadMods(selectedView, true)}>Refresh</button>
        </div>

        <div class="notice api-note">
            Nexus requests are cached locally for {NEXUS_CACHE_TTL_MINUTES} minutes. Refresh only when you need current Nexus data.
        </div>

        <div class="nexus-filter-row">
            <input class="generic-input key-input" bind:value={nexusSearchTerm} placeholder="Search Nexus" />
            <select bind:value={selectedNexusCategory}>
                <option value="all">All categories</option>
                {#each nexusCategories as category}
                    <option value={category}>{category}</option>
                {/each}
            </select>
            <select bind:value={selectedInstallFilter}>
                <option value="all">All installs</option>
                <option value="installed">Installed</option>
                <option value="missing">Not installed</option>
                <option value="vortex">Vortex</option>
                <option value="native">OpenACAI store</option>
                <option value="manual">Manual</option>
            </select>
        </div>

        <div class="nexus-scroller">
            {#each visibleNexusMods as mod}
                {@const match = installedMatch(mod)}
                <article class="nexus-card">
                    <img
                        class="nexus-img"
                        src={mod.picture_url ?? "https://placehold.co/320x180/252525/FFF?text=Nexus"}
                        alt={mod.name}
                    />
                    <div class="nexus-body">
                        <div class="card-head">
                            <span class="mod-title">{mod.name}</span>
                            <span class="source-pill" class:source-vortex={match?.installSource === "vortex"} class:source-native={match?.installSource === "native"} class:source-manual={match?.installSource === "manual"}>
                                {describeInstallSource(match)}
                            </span>
                        </div>
                        {#if match}
                            <label class="nexus-enable" class:vortex-disabled={match.installSource === "vortex"}>
                                <input
                                    type="checkbox"
                                    checked={match.enabled}
                                    disabled={match.installSource === "vortex"}
                                    on:change={(event) => toggleMatchedMod(match, event)}
                                />
                                <span>{match.enabled ? "Enabled" : "Disabled"}</span>
                            </label>
                        {/if}
                        <span class="description-content">{mod.summary ?? ""}</span>
                        <div class="facts">
                            <span>Author: <b>{mod.author ?? mod.uploaded_by ?? "-"}</b></span>
                            <span>Version: <b>{mod.version ?? "-"}</b></span>
                            <span>Updated: <b>{formatTimestamp(mod.updated_timestamp, mod.updated_time)}</b></span>
                            <span>Downloads: <b>{mod.mod_downloads ?? "-"}</b></span>
                        </div>
                        {#if match}
                            <span class="match-detail">{match.loaderType === "bepinex-plugin" ? "BepInEx plugin" : "RedLoader package"} in {match.expectedLocation}</span>
                        {/if}
                        <div class="button-row">
                            <button on:click={() => openModPage(mod)}>Open Page</button>
                            <button on:click={() => openDownloadPage(mod)}>Files</button>
                        </div>
                    </div>
                </article>
            {/each}

            {#if isLoading}
                <div class="loading-line">
                    <SvgSpinnersBlocksWave />
                    <span>{status}</span>
                </div>
            {/if}

            {#if !isLoading && visibleNexusMods.length === 0}
                <div class="notice empty-nexus">No Nexus mods match the current filters.</div>
            {/if}
        </div>
    {:else}
        <div class="notice">Nexus login is required before this section can compare Vortex packages against native installs.</div>
    {/if}
</div>

<style>
    .nexus-page {
        gap: 1em;
        height: 100%;
        justify-content: flex-start;
        min-height: 0;
    }

    .account-panel {
        align-items: center;
        background: rgba(42, 42, 42, 0.78);
        border: 1px solid rgba(255, 255, 255, 0.14);
        display: flex;
        gap: 1em;
        justify-content: space-between;
        padding: 0.9em 1em;
        text-align: left;
    }

    .account-copy {
        display: flex;
        flex-direction: column;
        min-width: 0;
    }

    .panel-title {
        color: #eefcff;
        font-size: 1.25em;
        font-weight: 700;
    }

    .panel-subtitle {
        color: #9eb0bf;
        font-size: 0.9em;
    }

    .account-actions,
    .connect-row,
    .button-row,
    .rate-row {
        align-items: center;
        display: flex;
        gap: 0.5em;
    }

    .connect-row {
        flex: 1;
        justify-content: flex-end;
    }

    .connect-column {
        align-items: flex-end;
        display: flex;
        flex-direction: column;
        gap: 0.4em;
        min-width: min(100%, 34em);
    }

    .login-button {
        min-width: 13em;
    }

    .manual-key {
        color: #8d8d8d;
        font-size: 0.82em;
        font-weight: 700;
        text-align: right;
        width: 100%;
    }

    .manual-key summary {
        cursor: pointer;
        text-transform: uppercase;
    }

    .manual-row {
        margin-top: 0.5em;
    }

    .manual-warning {
        color: #fdc66d;
        display: block;
        font-size: 0.9em;
        line-height: 1.25;
        margin-top: 0.35em;
    }

    .key-input {
        margin: 0;
        max-width: 24em;
        min-width: 12em;
        text-align: left;
        width: 100%;
    }

    .rate-row {
        color: #9eb0bf;
        font-size: 0.85em;
        justify-content: flex-end;
    }

    .notice {
        background: rgba(34, 34, 34, 0.9);
        border: 1px solid rgba(252, 252, 252, 0.08);
        color: #aab8c5;
        padding: 0.8em 1em;
        text-align: left;
    }

    .live-status {
        color: #38d68d;
        font-weight: 800;
        text-transform: uppercase;
    }

    .warning {
        color: #fdc66d;
    }

    .controls {
        justify-content: flex-end;
    }

    .api-note {
        color: #8d99a5;
        font-size: 0.78em;
        padding: 0.55em 0.8em;
    }

    .nexus-filter-row {
        display: grid;
        gap: 0.6em;
        grid-template-columns: minmax(16em, 1fr) minmax(10em, 0.6fr) minmax(10em, 0.6fr);
        width: 100%;
    }

    .nexus-filter-row .key-input {
        max-width: none;
    }

    .nexus-filter-row select {
        background: rgba(18, 18, 18, 0.92);
        border: 1px solid rgba(255, 255, 255, 0.16);
        color: #e8e8e8;
        font-family: inherit;
        font-weight: 700;
        min-height: 2.6em;
        padding: 0.45em 0.7em;
        text-transform: uppercase;
        width: 100%;
    }

    .cat-btn {
        height: 2.7em;
        margin: 0;
        padding: 0;
        width: 7em;
    }

    .refresh-btn {
        color: #c8c8c8;
        width: 8em;
    }

    .middle-btn {
        border-radius: 0;
    }

    .cat-btn-selected {
        background-color: #111;
        color: #38d68d;
    }

    .nexus-scroller {
        display: grid;
        gap: 1em;
        grid-template-columns: repeat(auto-fill, minmax(330px, 1fr));
        flex: 1 1 auto;
        min-height: 0;
        overflow-y: auto;
        padding-right: 0.4em;
    }

    .nexus-card {
        background: #121212;
        border-bottom: 2px solid #333;
        display: flex;
        flex-direction: column;
        min-height: 0;
        overflow: hidden;
    }

    .nexus-img {
        aspect-ratio: 16 / 9;
        background: #252525;
        object-fit: cover;
        width: 100%;
    }

    .nexus-body {
        display: flex;
        flex: 1;
        flex-direction: column;
        gap: 0.6em;
        padding: 0.9em;
        text-align: left;
    }

    .card-head {
        align-items: flex-start;
        display: flex;
        gap: 0.6em;
        justify-content: space-between;
    }

    .mod-title {
        color: #d8e4ec;
        font-size: 1.05em;
        font-weight: 700;
        line-height: 1.25;
        min-width: 0;
    }

    .description-content {
        color: #8d99a5;
        display: -webkit-box;
        font-size: 0.9em;
        line-clamp: 3;
        line-height: 1.35;
        margin: 0;
        min-height: 3.6em;
        overflow: hidden;
        -webkit-line-clamp: 3;
        -webkit-box-orient: vertical;
    }

    .facts {
        color: #87929d;
        display: grid;
        font-size: 0.82em;
        gap: 0.25em;
        grid-template-columns: 1fr 1fr;
    }

    .facts b {
        color: #b9c7d2;
        font-weight: 600;
    }

    .match-detail {
        color: #78d9f4;
        font-size: 0.82em;
        font-weight: 700;
    }

    .nexus-enable {
        align-items: center;
        background: rgba(18, 18, 18, 0.88);
        border: 1px solid rgba(255, 255, 255, 0.14);
        color: #62f09b;
        display: inline-flex;
        font-size: 0.78em;
        font-weight: 800;
        gap: 0.55em;
        padding: 0.38em 0.55em;
        text-transform: uppercase;
        width: max-content;
    }

    .nexus-enable input[type="checkbox"] {
        appearance: none;
        background: rgba(8, 8, 8, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.35);
        display: grid;
        float: none;
        height: 1.15em;
        margin: 0;
        padding: 0;
        place-content: center;
        transform: none;
        width: 1.15em;
    }

    .nexus-enable input[type="checkbox"]::before {
        box-shadow: inset 1em 1em #62f09b;
        content: "";
        height: 0.62em;
        transform: scale(0);
        transition: transform 120ms ease-in-out;
        width: 0.62em;
    }

    .nexus-enable input[type="checkbox"]:checked::before {
        transform: scale(1);
    }

    .nexus-enable:not(:has(input:checked)) {
        color: #fd9b9d;
    }

    .vortex-disabled {
        color: #78d9f4;
        opacity: 0.72;
    }

    .source-pill {
        background: #202832;
        border: 1px solid rgba(252, 252, 252, 0.08);
        border-radius: 6px;
        color: #bfc8d2;
        flex: 0 0 auto;
        font-size: 0.72em;
        font-weight: 700;
        line-height: 1.2;
        max-width: 12em;
        overflow: hidden;
        padding: 0.35em 0.55em;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .source-vortex {
        color: #78d9f4;
    }

    .source-native {
        color: #38d68d;
    }

    .source-manual {
        color: #fdc66d;
    }

    .button-row {
        margin-top: auto;
    }

    .button-row button {
        flex: 1;
        margin: 0;
    }

    .loading-line {
        align-items: center;
        color: #38d68d;
        display: flex;
        font-weight: 700;
        gap: 0.6em;
        justify-content: center;
        min-height: 4em;
    }

    .empty-nexus {
        grid-column: 1 / -1;
    }

    @media (max-width: 780px) {
        .account-panel,
        .connect-row,
        .account-actions {
            align-items: stretch;
            flex-direction: column;
        }

        .nexus-filter-row {
            grid-template-columns: 1fr;
        }

        .nexus-scroller {
            grid-template-columns: 1fr;
        }
    }
</style>
