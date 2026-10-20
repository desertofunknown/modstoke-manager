<script lang="ts">
    import { onDestroy, onMount, tick } from "svelte";
    import SOTFMods from "./Mods.svelte";
    import NexusVortex from "./NexusVortex.svelte";
    import NativePackages from "../lib/NativePackages.svelte";
    import { selectedInstallation } from "../lib/installationProfiles";
    import { gameExePath } from "../lib/store";
    import LucideCloudDownload from "~icons/lucide/cloud-download";
    import LucideLayoutGrid from "~icons/lucide/layout-grid";
    import LucideMaximize2 from "~icons/lucide/maximize-2";
    import LucideSearch from "~icons/lucide/search";
    import LucidePackage from "~icons/lucide/package";
    import LucideSlidersHorizontal from "~icons/lucide/sliders-horizontal";
    import LucideStore from "~icons/lucide/store";
    import LucideX from "~icons/lucide/x";

    type ModHubSource = "all" | "sotf" | "nexus" | "native";
    type SourceSummary = {
        visible: number;
        total: number;
        mode: string;
        note: string;
        activeFilters: boolean;
        attention: number;
    };

    export let isActive = false;

    const MOD_HUB_SOURCE_KEY = "openacai-mod-hub-source";
    const MOD_HUB_SEARCH_KEY = "openacai-mod-hub-search";

    let activeSource: ModHubSource = "all";
    let isMounted = false;
    let sharedSearchTerm = "";
    let searchInput: HTMLInputElement;
    let sharedSearchVersion = 0;
    let sotfSummary: SourceSummary | null = null;
    let nexusSummary: SourceSummary | null = null;
    let allStoresSotfControlsExpanded = false;
    let allStoresNexusControlsExpanded = false;
    let modHubElement: HTMLDivElement | null = null;
    let sourcePanelElement: HTMLDivElement | null = null;
    let hubLayoutObserver: ResizeObserver | null = null;
    let hubLayoutFrame: number | null = null;
    let hubWindowResizeHandler: (() => void) | null = null;

    onMount(() => {
        localStorage.removeItem(MOD_HUB_SOURCE_KEY);
        const savedSource = sessionStorage.getItem(MOD_HUB_SOURCE_KEY);
        if (savedSource === "all" || savedSource === "sotf" || savedSource === "nexus" || savedSource === "native") {
            activeSource = savedSource;
        }

        const savedSearch = localStorage.getItem(MOD_HUB_SEARCH_KEY) ?? "";
        if (savedSearch) {
            sharedSearchTerm = savedSearch;
            sharedSearchVersion += 1;
        }

        setupHubLayoutObserver();
        void measureHubLayoutAfterTick();
        isMounted = true;
    });

    onDestroy(() => {
        cleanupHubLayoutObserver();
    });

    $: {
        activeSource;
        sharedSearchTerm;
        allStoresSotfControlsExpanded;
        allStoresNexusControlsExpanded;
        void measureHubLayoutAfterTick();
    }

    function selectSource(source: ModHubSource) {
        activeSource = source;
        localStorage.removeItem(MOD_HUB_SOURCE_KEY);
        sessionStorage.setItem(MOD_HUB_SOURCE_KEY, source);
    }

    function setSharedSearchTerm(value: string) {
        sharedSearchTerm = value;
        sharedSearchVersion += 1;
        localStorage.setItem(MOD_HUB_SEARCH_KEY, value);
    }

    function handleSharedSearchInput(event: Event) {
        setSharedSearchTerm((event.currentTarget as HTMLInputElement).value);
    }

    function handleSearchShortcut(event: KeyboardEvent) {
        if ((event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase() === "f"
            && modHubElement?.getClientRects().length) {
            const hasVisibleDialog = Array.from(modHubElement.querySelectorAll<HTMLElement>('[role="dialog"]'))
                .some(dialog => dialog.getClientRects().length > 0);
            if (hasVisibleDialog) return;
            event.preventDefault();
            searchInput?.focus();
            searchInput?.select();
        }
    }

    function clearSharedSearch() {
        setSharedSearchTerm("");
    }

    function updateSourceSummary(source: "sotf" | "nexus", detail: SourceSummary) {
        const summary = normalizeSourceSummary(detail);
        if (source === "sotf") {
            sotfSummary = summary;
        } else {
            nexusSummary = summary;
        }
    }

    function normalizeSourceSummary(summary: SourceSummary): SourceSummary {
        return {
            visible: Math.max(0, Number(summary.visible) || 0),
            total: Math.max(0, Number(summary.total) || 0),
            mode: summary.mode || "Catalog",
            note: summary.note || "",
            activeFilters: Boolean(summary.activeFilters),
            attention: Math.max(0, Number(summary.attention) || 0)
        };
    }

    function sourceSummaryLine(summary: SourceSummary | null, fallback: string): string {
        if (!summary) {
            return fallback;
        }

        const countLabel = summary.total > 0
            ? `${summary.visible}/${summary.total}`
            : `${summary.visible}`;
        const status = summary.attention > 0
            ? `${summary.attention} attention`
            : summary.activeFilters
                ? "filters"
                : summary.note;
        return [countLabel, summary.mode, status].filter(Boolean).join(" · ");
    }

    function allStoresSummaryLine(left: SourceSummary | null, right: SourceSummary | null, searchTerm: string): string {
        const visible = (left?.visible ?? 0) + (right?.visible ?? 0);
        const total = (left?.total ?? 0) + (right?.total ?? 0);
        const attention = (left?.attention ?? 0) + (right?.attention ?? 0);
        if (total === 0) {
            return searchTerm.trim() ? "Shared search" : "Unified catalogs";
        }

        return attention > 0
            ? `${visible}/${total} shown · ${attention} attention`
            : `${visible}/${total} shown`;
    }

    function setupHubLayoutObserver() {
        hubWindowResizeHandler = () => scheduleHubLayoutMeasure();
        window.addEventListener("resize", hubWindowResizeHandler);

        if ("ResizeObserver" in window) {
            hubLayoutObserver = new ResizeObserver(() => scheduleHubLayoutMeasure());
            if (modHubElement) {
                hubLayoutObserver.observe(modHubElement);
            }
            if (sourcePanelElement) {
                hubLayoutObserver.observe(sourcePanelElement);
            }
        }

        scheduleHubLayoutMeasure();
    }

    function cleanupHubLayoutObserver() {
        if (hubWindowResizeHandler) {
            window.removeEventListener("resize", hubWindowResizeHandler);
            hubWindowResizeHandler = null;
        }

        hubLayoutObserver?.disconnect();
        hubLayoutObserver = null;

        if (hubLayoutFrame !== null) {
            window.cancelAnimationFrame(hubLayoutFrame);
            hubLayoutFrame = null;
        }
    }

    async function measureHubLayoutAfterTick() {
        await tick();
        if (hubLayoutObserver) {
            hubLayoutObserver.disconnect();
            if (modHubElement) {
                hubLayoutObserver.observe(modHubElement);
            }
            if (sourcePanelElement) {
                hubLayoutObserver.observe(sourcePanelElement);
            }
        }
        scheduleHubLayoutMeasure();
    }

    function scheduleHubLayoutMeasure() {
        if (hubLayoutFrame !== null) {
            return;
        }

        hubLayoutFrame = window.requestAnimationFrame(() => {
            hubLayoutFrame = null;
            measureHubLayout();
        });
    }

    function measureHubLayout() {
        const panel = sourcePanelElement;
        if (!panel || activeSource !== "all") {
            return;
        }

        const panelRect = panel.getBoundingClientRect();
        const panelStyle = getComputedStyle(panel);
        const panelGap = cssPixels(panelStyle.rowGap || panelStyle.gap);
        const panelPaddingBlock = cssPixels(panelStyle.paddingTop) + cssPixels(panelStyle.paddingBottom);
        const width = Math.max(320, panelRect.width);
        const height = Math.max(360, panelRect.height);
        const isShort = height < 720;
        const isNarrow = width < 860;
        const sectionBodies = Array.from(panel.querySelectorAll<HTMLElement>(".source-section-body"));
        const sectionChrome = sectionBodies.reduce((sum, body) => {
            const section = body.closest<HTMLElement>(".source-section");
            if (!section) {
                return sum;
            }

            return sum + Math.max(0, section.getBoundingClientRect().height - body.getBoundingClientRect().height);
        }, 0);
        const fixedHeight = panelPaddingBlock + panelGap + sectionChrome;
        const bodyBudget = Math.max(260, height - fixedHeight);
        const sotfBaseMinimum = isNarrow ? 255 : 285;
        const nexusBaseMinimum = isNarrow ? 290 : 330;
        const sotfMinimum = Math.min(sotfBaseMinimum, Math.max(isNarrow ? 138 : 156, Math.round(bodyBudget * 0.34)));
        const nexusMinimum = Math.min(nexusBaseMinimum, Math.max(isNarrow ? 160 : 180, Math.round(bodyBudget * 0.4)));
        const sotfShare = isShort ? 0.52 : 0.5;
        const maxSotfHeight = Math.max(sotfMinimum, bodyBudget - nexusMinimum);
        const sotfHeight = clampNumber(bodyBudget * sotfShare, sotfMinimum, maxSotfHeight);
        const nexusHeight = Math.max(0, bodyBudget - sotfHeight);

        panel.style.setProperty("--all-sotf-preview-height", `${Math.round(sotfHeight)}px`);
        panel.style.setProperty("--all-nexus-preview-height", `${Math.round(nexusHeight)}px`);
    }

    function cssPixels(value: string): number {
        const parsed = Number.parseFloat(value);
        return Number.isFinite(parsed) ? parsed : 0;
    }

    function clampNumber(value: number, min: number, max: number): number {
        return Math.min(max, Math.max(min, value));
    }
</script>

<svelte:window on:keydown={handleSearchShortcut} />

<div class="mod-hub" bind:this={modHubElement}>
    <header class="hub-header">
        <div class="hub-heading">
            <span class="hub-eyebrow">Your next playthrough</span>
            <h1>Browse mods</h1>
            <p>Discover mods across your connected sources.</p>
        </div>
        <label class="hub-search">
            <LucideSearch aria-hidden="true" />
            <input
                bind:this={searchInput}
                aria-label="Search mods"
                title="Search mods (Ctrl+F)"
                on:keydown={(event) => { if (event.key === "Escape" && sharedSearchTerm) { event.stopPropagation(); clearSharedSearch(); } }}
                placeholder="Search the library"
                type="text"
                value={sharedSearchTerm}
                on:input={handleSharedSearchInput}
            />
            {#if sharedSearchTerm}
                <button type="button" aria-label="Clear search" on:click={clearSharedSearch} title="Clear search">
                    <LucideX aria-hidden="true" />
                </button>
            {:else}
                <kbd>Ctrl F</kbd>
            {/if}
        </label>
    </header>
    <div class="hub-toolbar">
        <div class="source-switch" role="group" aria-label="Mod source">
            <button
                type="button"
                class:source-selected={activeSource === "all"}
                aria-pressed={activeSource === "all"}
                on:click={() => selectSource("all")}
                title="Browse all stores"
            >
                <LucideLayoutGrid aria-hidden="true" />
                <span class="source-switch-copy">
                    <span>All stores</span>
                </span>
            </button>
            <button
                type="button"
                class:source-selected={activeSource === "sotf"}
                aria-pressed={activeSource === "sotf"}
                on:click={() => selectSource("sotf")}
                title="Browse SOTF Mods"
            >
                <LucideStore aria-hidden="true" />
                <span class="source-switch-copy">
                    <span>SOTF Mods</span>
                </span>
            </button>
            <button
                type="button"
                class:source-selected={activeSource === "nexus"}
                aria-pressed={activeSource === "nexus"}
                on:click={() => selectSource("nexus")}
                title="Browse Nexus and Vortex"
            >
                <LucideCloudDownload aria-hidden="true" />
                <span class="source-switch-copy">
                    <span>Nexus &amp; Vortex</span>
                </span>
            </button>
            <button
                type="button"
                class:source-selected={activeSource === "native"}
                aria-pressed={activeSource === "native"}
                on:click={() => selectSource("native")}
                title="View native packages"
            >
                <LucidePackage aria-hidden="true" />
                <span class="source-switch-copy">
                    <span>Native packages</span>
                </span>
            </button>
        </div>

        {#if activeSource === "all"}
            <span class="hub-summary">{allStoresSummaryLine(sotfSummary, nexusSummary, sharedSearchTerm)}</span>
        {/if}
    </div>

    <div class="source-panel" class:all-source-panel={activeSource === "all"} bind:this={sourcePanelElement}>
            <section class="source-section" class:focused-source={activeSource !== "all"} hidden={activeSource === "nexus" || activeSource === "native"}>
                <div class="source-section-head" hidden={activeSource !== "all"}>
                    <span class="source-section-title">
                        <span class="source-section-name"><LucideStore aria-hidden="true" /> SOTF Mods</span>
                        <small>{sourceSummaryLine(sotfSummary, "Native store")}</small>
                    </span>
                    <span class="source-section-actions">
                        <button
                            type="button"
                            class:source-section-toggle-active={allStoresSotfControlsExpanded}
                            aria-label={allStoresSotfControlsExpanded ? "Hide SOTF Mods controls" : "Show SOTF Mods controls"}
                            aria-pressed={allStoresSotfControlsExpanded}
                            on:click={() => allStoresSotfControlsExpanded = !allStoresSotfControlsExpanded}
                            title={allStoresSotfControlsExpanded ? "Hide SOTF Mods controls" : "Show SOTF Mods controls"}
                        >
                            <LucideSlidersHorizontal aria-hidden="true" />
                        </button>
                        <button type="button" on:click={() => selectSource("sotf")} title="Open SOTF Mods source" aria-label="Open SOTF Mods source">
                            <LucideMaximize2 aria-hidden="true" />
                        </button>
                    </span>
                </div>
                <div class="source-section-body">
                    {#key $gameExePath}
                    <SOTFMods
                        embeddedStorePreview={activeSource === "all"}
                        embeddedControlsExpanded={allStoresSotfControlsExpanded}
                        sharedSearchTerm={sharedSearchTerm}
                        sharedSearchVersion={sharedSearchVersion}
                        showEmbeddedSearch={false}
                        on:searchChange={(event) => setSharedSearchTerm(event.detail)}
                        on:summaryChange={(event) => updateSourceSummary("sotf", event.detail)}
                    />
                    {/key}
                </div>
            </section>

            <section class="source-section" class:focused-source={activeSource !== "all"} hidden={activeSource === "sotf" || activeSource === "native"}>
                <div class="source-section-head" hidden={activeSource !== "all"}>
                    <span class="source-section-title">
                        <span class="source-section-name"><LucideCloudDownload aria-hidden="true" /> Nexus &amp; Vortex</span>
                        <small>{sourceSummaryLine(nexusSummary, "Nexus catalog")}</small>
                    </span>
                    <span class="source-section-actions">
                        <button
                            type="button"
                            class:source-section-toggle-active={allStoresNexusControlsExpanded}
                            aria-label={allStoresNexusControlsExpanded ? "Hide Nexus / Vortex controls" : "Show Nexus / Vortex controls"}
                            aria-pressed={allStoresNexusControlsExpanded}
                            on:click={() => allStoresNexusControlsExpanded = !allStoresNexusControlsExpanded}
                            title={allStoresNexusControlsExpanded ? "Hide Nexus / Vortex controls" : "Show Nexus / Vortex controls"}
                        >
                            <LucideSlidersHorizontal aria-hidden="true" />
                        </button>
                        <button type="button" on:click={() => selectSource("nexus")} title="Open Nexus / Vortex source" aria-label="Open Nexus / Vortex source">
                            <LucideMaximize2 aria-hidden="true" />
                        </button>
                    </span>
                </div>
                <div class="source-section-body nexus-source-section-body">
                    {#key $gameExePath}
                    <NexusVortex
                        isActive={isMounted && isActive && (activeSource === "all" || activeSource === "nexus")}
                        embeddedStorePreview={activeSource === "all"}
                        embeddedControlsExpanded={allStoresNexusControlsExpanded}
                        sharedSearchTerm={sharedSearchTerm}
                        sharedSearchVersion={sharedSearchVersion}
                        showEmbeddedSearch={false}
                        on:searchChange={(event) => setSharedSearchTerm(event.detail)}
                        on:summaryChange={(event) => updateSourceSummary("nexus", event.detail)}
                    />
                    {/key}
                </div>
            </section>

            {#if activeSource === "native"}
                <section class="source-section focused-source native-source">
                    {#if $selectedInstallation?.available && $selectedInstallation.target.kind === "player" && $selectedInstallation.root}
                        <NativePackages gameRoot={$selectedInstallation.root} searchTerm={sharedSearchTerm} />
                    {:else}
                        <div class="native-empty" role="status">
                            <LucidePackage aria-hidden="true" />
                            <h2>Choose a player installation</h2>
                            <p>Select an available player installation to view its native packages.</p>
                        </div>
                    {/if}
                </section>
            {/if}
    </div>
</div>

<style>
    .mod-hub {
        box-sizing: border-box;
        display: flex;
        flex-direction: column;
        gap: 18px;
        height: 100%;
        min-height: 0;
        min-width: 0;
        overflow: hidden;
        text-align: left;
        width: 100%;
    }

    .hub-header {
        align-items: center;
        display: flex;
        flex: 0 0 auto;
        gap: 24px;
        justify-content: space-between;
        min-width: 0;
    }

    .hub-heading { min-width: 0; }
    .hub-eyebrow {
        color: var(--accent, #eeb66b);
        display: block;
        font-size: 10px;
        font-weight: 650;
        letter-spacing: 0.12em;
        margin-bottom: 7px;
        text-transform: uppercase;
    }
    .hub-heading h1 {
        color: var(--text-primary, #eef0f4);
        font-size: clamp(23px, 2.4vw, 30px);
        font-weight: 650;
        letter-spacing: -0.035em;
        line-height: 1.15;
        margin: 0;
    }
    .hub-heading p {
        color: var(--text-secondary, #9da6b6);
        font-size: 12px;
        line-height: 1.5;
        margin: 8px 0 0;
    }

    .hub-search {
        align-items: center;
        background: var(--surface-panel, #171a21);
        border: 1px solid var(--border-subtle, #2b303a);
        border-radius: 10px;
        box-sizing: border-box;
        display: flex;
        flex: 0 1 360px;
        gap: 10px;
        min-width: 240px;
        padding: 0 12px;
        transition: border-color 150ms, box-shadow 150ms;
    }
    .hub-search > :global(svg) {
        color: var(--text-secondary, #9da6b6);
        flex: 0 0 auto;
        height: 17px;
        width: 17px;
    }
    .hub-search:focus-within {
        border-color: var(--accent, #eeb66b);
        box-shadow: 0 0 0 3px var(--accent-soft, rgba(238, 182, 107, 0.1));
    }
    .hub-search input {
        background: transparent;
        border: 0;
        border-radius: 0;
        box-shadow: none;
        color: var(--text-primary, #eef0f4);
        font: inherit;
        font-size: 13px;
        height: 44px;
        margin: 0;
        min-height: 0;
        min-width: 0;
        outline: none;
        padding: 0;
        width: 100%;
    }
    .hub-search input::placeholder { color: var(--text-secondary, #9da6b6); opacity: 0.8; }
    .hub-search kbd {
        border: 1px solid var(--border-subtle, #2b303a);
        border-radius: 4px;
        color: var(--text-secondary, #9da6b6);
        flex: 0 0 auto;
        font-family: inherit;
        font-size: 10px;
        padding: 3px 5px;
    }
    .hub-search button {
        align-items: center;
        background: transparent;
        border: 0;
        border-radius: 6px;
        box-shadow: none;
        color: var(--text-secondary, #9da6b6);
        display: flex;
        flex: 0 0 auto;
        height: 28px;
        justify-content: center;
        margin: 0;
        min-height: 0;
        min-width: 28px;
        padding: 0;
        width: 28px;
    }
    .hub-search button :global(svg) { height: 15px; width: 15px; }
    .hub-search button:hover { background: var(--surface-raised, #20242c); color: var(--text-primary, #eef0f4); }

    .hub-toolbar {
        align-items: center;
        border-bottom: 1px solid var(--border-subtle, #2b303a);
        display: flex;
        flex: 0 0 auto;
        gap: 16px;
        justify-content: space-between;
        min-width: 0;
        width: 100%;
    }
    .source-switch {
        align-items: stretch;
        display: flex;
        gap: 6px;
        min-width: 0;
    }
    .source-switch button {
        align-items: center;
        background: transparent;
        border: 0;
        border-radius: 7px 7px 0 0;
        box-shadow: none;
        color: var(--text-secondary, #9da6b6);
        display: flex;
        font-size: 12px;
        font-weight: 550;
        gap: 8px;
        height: 43px;
        justify-content: flex-start;
        margin: 0;
        min-width: 0;
        padding: 0 12px 11px;
        position: relative;
        text-shadow: none;
        white-space: nowrap;
    }
    .source-switch button::after {
        background: transparent;
        border-radius: 2px 2px 0 0;
        bottom: -1px;
        content: "";
        height: 2px;
        left: 12px;
        position: absolute;
        right: 12px;
    }
    .source-switch button :global(svg) { flex: 0 0 auto; height: 16px; width: 16px; }
    .source-switch button:hover { color: var(--text-primary, #eef0f4); }
    .source-switch button.source-selected { color: var(--accent, #eeb66b); }
    .source-switch button.source-selected::after { background: var(--accent, #eeb66b); }
    .source-switch-copy { min-width: 0; }
    .hub-summary {
        color: var(--text-secondary, #9da6b6);
        font-size: 11px;
        overflow: hidden;
        padding-bottom: 10px;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .source-panel {
        display: flex;
        flex: 1 1 auto;
        flex-direction: column;
        min-height: 0;
        min-width: 0;
        overflow: hidden;
        width: 100%;
    }
    .all-source-panel {
        gap: 14px;
        overflow-y: auto;
        padding-bottom: var(--app-bottom-safe-area, 12px);
        padding-right: 4px;
        scroll-padding-bottom: var(--app-bottom-safe-area, 12px);
        scrollbar-gutter: stable;
    }
    .source-section {
        background: var(--surface-panel, #171a21);
        border: 1px solid var(--border-subtle, #2b303a);
        border-radius: 12px;
        box-sizing: border-box;
        display: flex;
        flex: 0 0 auto;
        flex-direction: column;
        gap: 10px;
        min-height: 0;
        min-width: 0;
        padding: 12px;
        width: 100%;
    }
    .all-source-panel .source-section { scroll-margin-top: 8px; }
    .source-section-head {
        align-items: center;
        display: flex;
        flex: 0 0 auto;
        gap: 14px;
        justify-content: space-between;
        min-width: 0;
    }
    .source-section-title { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 14px; min-width: 0; }
    .source-section-name {
        align-items: center;
        color: var(--text-primary, #eef0f4);
        display: inline-flex;
        font-size: 13px;
        font-weight: 600;
        gap: 8px;
        min-width: 0;
    }
    .source-section-name :global(svg) { color: var(--accent, #eeb66b); height: 16px; width: 16px; }
    .source-section-title small {
        color: var(--text-secondary, #9da6b6);
        font-size: 11px;
        font-weight: 400;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .source-section-actions { display: inline-flex; flex: 0 0 auto; gap: 6px; }
    .source-section-head button {
        align-items: center;
        background: transparent;
        border: 1px solid var(--border-subtle, #2b303a);
        border-radius: 7px;
        box-shadow: none;
        color: var(--text-secondary, #9da6b6);
        display: flex;
        height: 29px;
        justify-content: center;
        margin: 0;
        min-height: 0;
        min-width: 29px;
        padding: 0;
        width: 29px;
    }
    .source-section-head button :global(svg) { height: 14px; width: 14px; }
    .source-section-head button:hover { background: var(--surface-raised, #20242c); color: var(--text-primary, #eef0f4); }
    .source-section-head button.source-section-toggle-active {
        background: var(--accent-soft, rgba(238, 182, 107, 0.1));
        border-color: rgba(238, 182, 107, 0.3);
        color: var(--accent, #eeb66b);
    }
    .source-section-body { min-height: 0; min-width: 0; overflow: hidden; }
    .all-source-panel .source-section-body { height: var(--all-sotf-preview-height, 270px); }
    .all-source-panel .nexus-source-section-body { height: var(--all-nexus-preview-height, 330px); }
    .source-section[hidden], .source-section-head[hidden] { display: none; }
    .source-section.focused-source {
        background: transparent;
        border: 0;
        border-radius: 0;
        flex: 1 1 0;
        padding: 0;
    }
    .focused-source .source-section-body { flex: 1 1 0; height: auto; }
    .source-section.focused-source.native-source { overflow-y: auto; padding-bottom: var(--app-bottom-safe-area, 12px); }
    .native-empty {
        align-items: center;
        background: var(--surface-panel, #171a21);
        border: 1px dashed var(--border-subtle, #2b303a);
        border-radius: 12px;
        display: flex;
        flex: 1 1 auto;
        flex-direction: column;
        justify-content: center;
        min-height: 230px;
        padding: 28px;
        text-align: center;
    }
    .native-empty :global(svg) { color: var(--accent, #eeb66b); height: 28px; margin-bottom: 14px; width: 28px; }
    .native-empty h2 { color: var(--text-primary, #eef0f4); font-size: 17px; font-weight: 600; margin: 0; }
    .native-empty p { color: var(--text-secondary, #9da6b6); font-size: 13px; line-height: 1.6; margin: 8px 0 0; max-width: 370px; }

    @media (max-width: 1100px) {
        .hub-header { gap: 18px; }
        .hub-search { flex-basis: 290px; min-width: 220px; }
        .source-switch { gap: 1px; }
        .source-switch button { gap: 6px; padding-left: 9px; padding-right: 9px; }
        .hub-summary { display: none; }
    }
    @media (max-width: 780px) {
        .hub-header { align-items: stretch; flex-direction: column; gap: 14px; }
        .hub-search { flex: 0 0 auto; min-width: 0; }
        .source-switch { justify-content: space-between; width: 100%; }
        .source-switch button { font-size: 11px; padding-left: 6px; padding-right: 6px; }
    }
    @media (max-height: 720px) {
        .mod-hub { gap: 12px; }
        .hub-eyebrow { display: none; }
        .hub-heading p { margin-top: 5px; }
        .hub-heading h1 { font-size: 25px; }
        .hub-search input { height: 40px; }
        .source-switch button { height: 38px; padding-bottom: 8px; }
        .all-source-panel { gap: 10px; }
        .source-section { gap: 8px; padding: 10px; }
    }
</style>
