<script lang="ts">
    import { onDestroy, onMount, tick } from "svelte";
    import SOTFMods from "./Mods.svelte";
    import NexusVortex from "./NexusVortex.svelte";
    import LucideCloudDownload from "~icons/lucide/cloud-download";
    import LucideLayoutGrid from "~icons/lucide/layout-grid";
    import LucideMaximize2 from "~icons/lucide/maximize-2";
    import LucideSearch from "~icons/lucide/search";
    import LucideSlidersHorizontal from "~icons/lucide/sliders-horizontal";
    import LucideStore from "~icons/lucide/store";
    import LucideX from "~icons/lucide/x";

    type ModHubSource = "all" | "sotf" | "nexus";
    type SourceSummary = {
        visible: number;
        total: number;
        mode: string;
        note: string;
        activeFilters: boolean;
        attention: number;
    };

    const MOD_HUB_SOURCE_KEY = "openacai-mod-hub-source";
    const MOD_HUB_SEARCH_KEY = "openacai-mod-hub-search";

    let activeSource: ModHubSource = "all";
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
        if (savedSource === "all" || savedSource === "sotf" || savedSource === "nexus") {
            activeSource = savedSource;
        }

        const savedSearch = localStorage.getItem(MOD_HUB_SEARCH_KEY) ?? "";
        if (savedSearch) {
            sharedSearchTerm = savedSearch;
            sharedSearchVersion += 1;
        }

        setupHubLayoutObserver();
        void measureHubLayoutAfterTick();
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

    function updateSourceSummary(source: Exclude<ModHubSource, "all">, detail: SourceSummary) {
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
        const sotfShare = isShort ? 0.42 : width > 1320 ? 0.47 : 0.45;
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
    <div class="hub-toolbar">
        <div class="source-switch" aria-label="Mod source">
            <button
                type="button"
                class:source-selected={activeSource === "all"}
                aria-pressed={activeSource === "all"}
                on:click={() => selectSource("all")}
                title="Browse all stores"
            >
                <LucideLayoutGrid aria-hidden="true" />
                <span class="source-switch-copy">
                    <span>All Stores</span>
                    <small>{allStoresSummaryLine(sotfSummary, nexusSummary, sharedSearchTerm)}</small>
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
                    <small>{sourceSummaryLine(sotfSummary, "Native store")}</small>
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
                    <span>Nexus / Vortex</span>
                    <small>{sourceSummaryLine(nexusSummary, "Nexus catalog")}</small>
                </span>
            </button>
        </div>

        <label class="hub-search">
            <LucideSearch aria-hidden="true" />
            <input
                bind:this={searchInput}
                aria-label="Search mods"
                title="Search mods (Ctrl+F)"
                on:keydown={(event) => { if (event.key === "Escape" && sharedSearchTerm) { event.stopPropagation(); clearSharedSearch(); } }}
                placeholder="Search mods"
                type="text"
                value={sharedSearchTerm}
                on:input={handleSharedSearchInput}
            />
            <button
                type="button"
                aria-label="Clear search"
                disabled={!sharedSearchTerm}
                on:click={clearSharedSearch}
                title="Clear search"
            >
                <LucideX aria-hidden="true" />
            </button>
        </label>
    </div>

    <div class="source-panel" class:all-source-panel={activeSource === "all"} bind:this={sourcePanelElement}>
            <section class="source-section" class:focused-source={activeSource !== "all"} hidden={activeSource === "nexus"}>
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
                    <SOTFMods
                        embeddedStorePreview={activeSource === "all"}
                        embeddedControlsExpanded={allStoresSotfControlsExpanded}
                        sharedSearchTerm={sharedSearchTerm}
                        sharedSearchVersion={sharedSearchVersion}
                        showEmbeddedSearch={false}
                        on:searchChange={(event) => setSharedSearchTerm(event.detail)}
                        on:summaryChange={(event) => updateSourceSummary("sotf", event.detail)}
                    />
                </div>
            </section>

            <section class="source-section" class:focused-source={activeSource !== "all"} hidden={activeSource === "sotf"}>
                <div class="source-section-head" hidden={activeSource !== "all"}>
                    <span class="source-section-title">
                        <span class="source-section-name"><LucideCloudDownload aria-hidden="true" /> Nexus / Vortex</span>
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
                    <NexusVortex
                        embeddedStorePreview={activeSource === "all"}
                        embeddedControlsExpanded={allStoresNexusControlsExpanded}
                        sharedSearchTerm={sharedSearchTerm}
                        sharedSearchVersion={sharedSearchVersion}
                        showEmbeddedSearch={false}
                        on:searchChange={(event) => setSharedSearchTerm(event.detail)}
                        on:summaryChange={(event) => updateSourceSummary("nexus", event.detail)}
                    />
                </div>
            </section>

    </div>
</div>

<style>
    .mod-hub {
        box-sizing: border-box;
        display: flex;
        flex-direction: column;
        gap: clamp(0.45em, 0.8vh, 0.7em);
        height: 100%;
        min-height: 0;
        overflow: hidden;
        width: 100%;
    }

    .hub-toolbar {
        align-items: stretch;
        display: grid;
        flex: 0 0 auto;
        gap: 0.5em;
        grid-template-columns: minmax(28rem, 1.15fr) minmax(12rem, 0.85fr);
        width: 100%;
    }

    .source-switch {
        align-items: stretch;
        display: grid;
        flex: 0 0 auto;
        gap: 0.45em;
        grid-template-columns: repeat(3, minmax(0, 1fr));
        width: 100%;
    }

    .source-switch button {
        align-items: center;
        color: #aeb6bb;
        display: grid;
        gap: 0.48em;
        grid-template-columns: auto minmax(0, auto);
        height: clamp(2.45em, 5.2vh, 3em);
        justify-content: center;
        margin: 0;
        min-width: 0;
        padding: 0 0.8em;
        white-space: nowrap;
        width: 100%;
    }

    .source-switch button :global(svg) {
        display: block;
        flex: 0 0 auto;
        font-size: 1.08em;
        stroke-width: 2.3;
    }

    .source-switch-copy {
        display: grid;
        gap: 0.08em;
        min-width: 0;
        overflow: hidden;
        text-align: left;
    }

    .source-switch-copy span,
    .source-switch-copy small {
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .source-switch-copy small {
        color: #7f8992;
        font-size: 0.64em;
        font-weight: 800;
        line-height: 1.05;
        text-shadow: none;
        text-transform: none;
    }

    .source-switch button.source-selected {
        background:
            linear-gradient(180deg, rgba(50, 64, 58, 0.95), rgba(17, 28, 23, 0.96)),
            rgba(12, 22, 17, 0.94);
        border-color: rgba(98, 240, 155, 0.46);
        color: #62f09b;
        text-shadow: -0.55px 0 rgba(255, 64, 64, 0.58), 0.55px 0 rgba(66, 232, 255, 0.58);
    }

    .hub-search {
        align-items: center;
        background: rgba(12, 12, 12, 0.82);
        border: 1px solid rgba(255, 255, 255, 0.13);
        box-sizing: border-box;
        display: grid;
        gap: 0.45em;
        grid-template-columns: auto minmax(0, 1fr) auto;
        min-width: 0;
        padding: 0 0.42em 0 0.7em;
    }

    .hub-search :global(svg) {
        color: #8d99a5;
        display: block;
        font-size: 1.05em;
        stroke-width: 2.35;
    }

    .hub-search:focus-within {
        border-color: rgba(98, 240, 155, 0.7);
    }

    .hub-search input {
        background: transparent;
        border: 0;
        color: #e8eef3;
        font: inherit;
        font-size: 0.92em;
        font-weight: 800;
        height: 100%;
        min-height: clamp(2.25em, 4.8vh, 2.75em);
        min-width: 0;
        outline: none;
        padding: 0;
        width: 100%;
    }

    .hub-search input::placeholder {
        color: #77818b;
        opacity: 1;
        text-transform: uppercase;
    }

    .hub-search button {
        align-items: center;
        background: transparent;
        border: 0;
        box-shadow: none;
        color: #aeb6bb;
        display: grid;
        height: 2em;
        justify-content: center;
        margin: 0;
        min-height: 0;
        min-width: 2em;
        padding: 0;
        width: 2em;
        -webkit-mask-image: none;
        mask-image: none;
    }

    .hub-search button:not(:disabled):hover {
        color: #62f09b;
    }

    .hub-search button:disabled {
        opacity: 0.28;
    }

    .source-panel {
        display: flex;
        flex: 1 1 auto;
        flex-direction: column;
        min-height: 0;
        overflow: hidden;
        width: 100%;
    }

    .all-source-panel {
        gap: 0.55em;
        overflow-y: auto;
        padding-bottom: var(--app-bottom-safe-area, 42px);
        padding-right: 0.28em;
        scroll-padding-bottom: var(--app-bottom-safe-area, 42px);
        scrollbar-gutter: stable;
    }

    .all-source-panel .source-section {
        padding: 0.52em;
        scroll-margin-top: 0.5em;
    }

    .all-source-panel .source-section-head {
        background:
            linear-gradient(180deg, rgba(16, 16, 16, 0.98), rgba(9, 9, 9, 0.92)),
            rgba(10, 10, 10, 0.96);
        border-bottom: 1px solid rgba(255, 255, 255, 0.1);
        margin: -0.52em -0.52em 0;
        padding: 0.52em;
        position: sticky;
        top: -0.52em;
        z-index: 4;
    }

    .all-source-panel .source-section-body {
        height: var(--all-sotf-preview-height, clamp(23em, 43vh, 33em));
    }

    .all-source-panel .nexus-source-section-body {
        height: var(--all-nexus-preview-height, clamp(34em, 58vh, 45em));
    }

    .source-section {
        background: rgba(10, 10, 10, 0.34);
        border: 1px solid rgba(255, 255, 255, 0.1);
        box-sizing: border-box;
        display: flex;
        flex: 0 0 auto;
        flex-direction: column;
        gap: 0.5em;
        min-height: 0;
        padding: 0.65em;
        width: 100%;
    }

    .source-section-head {
        align-items: center;
        display: flex;
        flex: 0 0 auto;
        gap: 0.65em;
        justify-content: space-between;
        min-width: 0;
    }

    .source-section-title {
        align-items: center;
        color: #d8e3ea;
        display: grid;
        gap: 0.1em;
        font-size: 0.82em;
        font-weight: 900;
        min-width: 0;
        overflow: hidden;
    }

    .source-section-name {
        align-items: center;
        display: inline-flex;
        gap: 0.45em;
        letter-spacing: 0.08em;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        text-transform: uppercase;
        white-space: nowrap;
    }

    .source-section-title small {
        color: #81909a;
        font-size: 0.78em;
        font-weight: 800;
        letter-spacing: 0;
        line-height: 1.1;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        text-transform: none;
        white-space: nowrap;
    }

    .source-section-head :global(svg) {
        color: #62f09b;
        flex: 0 0 auto;
        font-size: 1.02em;
        stroke-width: 2.35;
    }

    .source-section-actions {
        display: inline-grid;
        flex: 0 0 auto;
        gap: 0.35em;
        grid-auto-flow: column;
    }

    .source-section-head button {
        align-items: center;
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.12);
        box-shadow: none;
        color: #aeb6bb;
        display: grid;
        height: 2.15em;
        justify-content: center;
        margin: 0;
        min-height: 0;
        min-width: 2.35em;
        padding: 0;
        width: 2.35em;
        -webkit-mask-image: none;
        mask-image: none;
    }

    .source-section-head button:hover {
        border-color: rgba(98, 240, 155, 0.44);
        color: #62f09b;
    }

    .source-section-head button.source-section-toggle-active {
        background: rgba(98, 240, 155, 0.08);
        border-color: rgba(98, 240, 155, 0.5);
        color: #62f09b;
    }

    .source-section-body {
        height: clamp(31em, 62vh, 46em);
        min-height: 0;
        overflow: hidden;
    }

    .nexus-source-section-body {
        height: clamp(44em, 80vh, 58em);
    }

    .source-section[hidden],
    .source-section-head[hidden] {
        display: none;
    }

    .source-section.focused-source {
        flex: 1 1 0;
        padding: 0;
        border: 0;
        background: none;
    }

    .focused-source .source-section-body {
        flex: 1 1 0;
        height: auto;
    }

    @media (max-width: 1100px) {
        .source-switch button {
            font-size: 0.86em;
            padding: 0 0.45em;
            gap: 0.35em;
        }
    }

    @media (max-width: 780px) {
        .hub-toolbar {
            grid-template-columns: 1fr;
        }

        .source-switch {
            gap: 0.35em;
        }

        .source-switch button {
            font-size: 0.82em;
            padding: 0 0.55em;
        }

        .source-switch-copy small {
            display: none;
        }

        .source-section {
            padding: 0.5em;
        }

        .source-section-body,
        .nexus-source-section-body {
            height: clamp(34em, 86vh, 54em);
        }

        .all-source-panel .source-section-body,
        .all-source-panel .nexus-source-section-body {
            height: var(--all-sotf-preview-height, clamp(32em, 82vh, 52em));
        }

        .all-source-panel .nexus-source-section-body {
            height: var(--all-nexus-preview-height, clamp(32em, 82vh, 52em));
        }
    }

    @media (max-height: 720px) {
        .mod-hub {
            gap: 0.35em;
        }

        .source-switch button,
        .hub-search input {
            height: 2.05em;
            min-height: 2.05em;
        }

        .all-source-panel {
            gap: 0.42em;
        }

        .all-source-panel .source-section {
            padding: 0.44em;
        }

        .all-source-panel .source-section-body {
            height: var(--all-sotf-preview-height, clamp(14em, 34vh, 21em));
        }

        .all-source-panel .nexus-source-section-body {
            height: var(--all-nexus-preview-height, clamp(18em, 43vh, 27em));
        }
    }
</style>
