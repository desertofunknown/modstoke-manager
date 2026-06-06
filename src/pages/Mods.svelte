<script lang="ts">
    import { createEventDispatcher, onDestroy, onMount, tick } from "svelte";
    import { isPathValid } from "../lib/store";
    import ModCard from "../lib/ModCard.svelte";
    import type { Mod, ModCategory } from "../lib/mods";
    import { ModDatabase, Sorting } from "../lib/mods";
    import InfiniteScroll from "../lib/InfiniteScroll.svelte";
    import { debounce } from "lodash";
    import SvgSpinnersBlocksWave from '~icons/svg-spinners/blocks-wave'
    import LucideRefreshCw from "~icons/lucide/refresh-cw";

    export let sharedSearchTerm = "";
    export let sharedSearchVersion = 0;
    export let showEmbeddedSearch = true;

    const dispatch = createEventDispatcher<{ searchChange: string }>();

    let filtered: Mod[] = [];
    let filterTerm: string = "";
    let lastAppliedSharedSearchVersion = 0;
    let selectedCategory = "all";
    let selectedType = "all";
    let selectedCompatibility = "all";
    let categories: ModCategory[] = [];
    let visibleMods: Mod[] = [];
    let hasActiveModFilters = false;

    let onlineSelected = true;
    let installedSelected = false;

    let isGrid = false;

    let page = 1;
    let newBatch: Mod[] = [];
    let isLoading: boolean = false;
    let hasLoadedOnce = false;
    let catalogError: string = "";
    let onlineCatalogGeneration = 0;
    let latestOnlineFetchId = 0;
    let installedInventoryWarning = "";
    let modsPageElement: HTMLDivElement | null = null;
    let modsLayoutObserver: ResizeObserver | null = null;
    let modsLayoutFrame: number | null = null;
    let modsWindowResizeHandler: (() => void) | null = null;

    $: {
        filterTerm;
        selectedCategory;
        selectedType;
        selectedCompatibility;
        visibleMods = filtered.filter(matchesClientFilters);
    }
    $: if (sharedSearchVersion > 0 && sharedSearchVersion !== lastAppliedSharedSearchVersion) {
        lastAppliedSharedSearchVersion = sharedSearchVersion;
        applySharedSearch(sharedSearchTerm);
    }
    $: hasActiveModFilters = filterTerm.trim().length > 0
        || selectedCategory !== "all"
        || selectedType !== "all"
        || selectedCompatibility !== "all";
    $: {
        visibleMods.length;
        filtered.length;
        isLoading;
        hasLoadedOnce;
        catalogError;
        installedInventoryWarning;
        onlineSelected;
        installedSelected;
        categories.length;
        void measureModsLayoutAfterTick();
    }

    async function fetchData(requestPage = page, requestGeneration = onlineCatalogGeneration) {
        //processing.set(true);
        //processProgress.set(0);
        //processName.set("Loading mods...");
        const fetchId = ++latestOnlineFetchId;
        isLoading = true;
        catalogError = "";
        try {
            let res = await ModDatabase.fetchMods(requestPage, Sorting.newest, true, false, filterTerm, selectedCategory, selectedType);
            if (requestGeneration !== onlineCatalogGeneration || !onlineSelected) {
                return;
            }

            let mods = res.data;
            await ModDatabase.initModList(mods);
            if (requestGeneration !== onlineCatalogGeneration || !onlineSelected) {
                return;
            }

            newBatch = mods;
            filtered = requestPage === 1 ? [...newBatch] : [...filtered, ...newBatch];
            hasLoadedOnce = true;
        } catch (error) {
            if (requestGeneration !== onlineCatalogGeneration || !onlineSelected) {
                return;
            }

            newBatch = [];
            catalogError = `Failed to load the mod catalog: ${error}`;
        } finally {
            if (fetchId === latestOnlineFetchId && requestGeneration === onlineCatalogGeneration && onlineSelected) {
                isLoading = false;
            }
        }
        //processing.set(false);
    };

    // $: filtered = [
	// 	...filtered,
    //     ...newBatch
    // ];

    onMount(async () => {
        setupModsLayoutObserver();
        //processing.set(true);
        //processProgress.set(0);
        //processName.set("Loading mods...");
        //await ModDatabase.refreshAll(false);

        //await filter();
        //processing.set(false);
        
        await loadCategories();
        await reloadOnline();

        try {
            await ModDatabase.initDatabase();
            ModDatabase.initModList(filtered);
            filtered = [...filtered];
            installedInventoryWarning = "";
        } catch (error) {
            installedInventoryWarning = `Installed mod scan failed: ${error}`;
            console.log(installedInventoryWarning);
        }

        await measureModsLayoutAfterTick();

    });

    onDestroy(() => {
        cleanupModsLayoutObserver();
    });

    function setupModsLayoutObserver() {
        modsWindowResizeHandler = () => scheduleModsLayoutMeasure();
        window.addEventListener("resize", modsWindowResizeHandler);

        if ("ResizeObserver" in window) {
            modsLayoutObserver = new ResizeObserver(() => scheduleModsLayoutMeasure());
            if (modsPageElement) {
                modsLayoutObserver.observe(modsPageElement);
            }
        }

        scheduleModsLayoutMeasure();
    }

    function cleanupModsLayoutObserver() {
        if (modsWindowResizeHandler) {
            window.removeEventListener("resize", modsWindowResizeHandler);
            modsWindowResizeHandler = null;
        }

        modsLayoutObserver?.disconnect();
        modsLayoutObserver = null;

        if (modsLayoutFrame !== null) {
            window.cancelAnimationFrame(modsLayoutFrame);
            modsLayoutFrame = null;
        }
    }

    async function measureModsLayoutAfterTick() {
        await tick();
        if (modsLayoutObserver && modsPageElement) {
            modsLayoutObserver.disconnect();
            modsLayoutObserver.observe(modsPageElement);
        }
        scheduleModsLayoutMeasure();
    }

    function scheduleModsLayoutMeasure() {
        if (modsLayoutFrame !== null) {
            return;
        }

        modsLayoutFrame = window.requestAnimationFrame(() => {
            modsLayoutFrame = null;
            measureModsLayout();
        });
    }

    function measureModsLayout() {
        const page = modsPageElement;
        if (!page) {
            return;
        }

        const scroller = page.querySelector<HTMLElement>(".scroller");
        if (!scroller) {
            return;
        }

        const pageRect = page.getBoundingClientRect();
        const pageStyle = getComputedStyle(page);
        const pageGap = cssPixels(pageStyle.rowGap || pageStyle.gap);
        const chrome = Array.from(page.children)
            .filter((child): child is HTMLElement => child instanceof HTMLElement && child !== scroller && getComputedStyle(child).display !== "none");
        const chromeHeight = chrome.reduce((sum, child) => sum + child.getBoundingClientRect().height, 0)
            + Math.max(0, chrome.length - 1) * pageGap;
        const availableHeight = Math.max(120, pageRect.height);
        const minimumScrollerHeight = clampNumber(availableHeight * 0.42, 180, 300);
        const targetScrollerHeight = clampNumber(availableHeight - chromeHeight - pageGap, minimumScrollerHeight, availableHeight);
        const targetThumbWidth = clampNumber(pageRect.width * 0.15, 136, 210);
        const targetThumbHeight = clampNumber(targetScrollerHeight / (targetScrollerHeight >= 620 ? 5.6 : 4.9), 74, 112);

        page.style.setProperty("--sotf-scroller-target-height", `${Math.round(targetScrollerHeight)}px`);
        page.style.setProperty("--sotf-thumb-width", `${Math.round(targetThumbWidth)}px`);
        page.style.setProperty("--sotf-thumb-height", `${Math.round(targetThumbHeight)}px`);
    }

    function cssPixels(value: string): number {
        const parsed = Number.parseFloat(value);
        return Number.isFinite(parsed) ? parsed : 0;
    }

    function clampNumber(value: number, min: number, max: number): number {
        return Math.min(max, Math.max(min, value));
    }

    // async function filter() {
    //     let modBucket: Mod[] = [];
    //     if(filterTerm.startsWith("unapproved:"))
    //     {
    //         modBucket = await ModDatabase.getUnapprovedMods();
    //     }
    //     else if(filterTerm.startsWith("nsfw:"))
    //     {
    //         modBucket = await ModDatabase.getNsfwMods();
    //     }
    //     else
    //     {
    //         modBucket = await ModDatabase.getMods();
    //     }

    //     let split =  filterTerm.split(":");
    //     let term = split[split.length - 1];

    //     filtered = modBucket.filter((mod) => {
    //         let passesTerm = mod.name.toLowerCase().includes(term.toLowerCase());
    //         let passesScope = (onlineSelected && !mod.isInstalled) || (installedSelected && mod.isInstalled);
    //         return passesTerm && passesScope;
    //     });
    // }

    const debouncedReloadOnline = debounce(async () => {
        if (onlineSelected) {
            await reloadOnline();
        }
    }, 600);

    function applySharedSearch(value: string) {
        const forceClearReload = value.trim() === "" && onlineSelected && hasLoadedOnce && filtered.length === 0 && !isLoading && !catalogError;
        if (value === filterTerm && !forceClearReload) {
            return;
        }

        handleSearchInput(value, false, value.trim() === "" || forceClearReload);
    }

    function handleSearchInput(value: string, emit = true, reloadImmediately = false) {
        filterTerm = value;
        if (emit) {
            dispatch("searchChange", value);
        }
        if (onlineSelected) {
            if (reloadImmediately || value.trim() === "") {
                debouncedReloadOnline.cancel();
                void reloadOnline();
            } else {
                debouncedReloadOnline();
            }
        }
    }

    async function loadCategories() {
        try {
            categories = await ModDatabase.fetchCategories();
        } catch (error) {
            console.log("failed to fetch SOTF categories", error);
        }
    }

    async function reloadOnline() {
        onlineSelected = true;
        installedSelected = false;
        onlineCatalogGeneration += 1;
        const generation = onlineCatalogGeneration;
        page = 1;
        filtered = [];
        newBatch = [];
        hasLoadedOnce = false;
        await fetchData(1, generation);
    }

    async function handleFacetChange() {
        if (onlineSelected) {
            await reloadOnline();
        }
    }

    async function clearModFilters() {
        debouncedReloadOnline.cancel();
        filterTerm = "";
        selectedCategory = "all";
        selectedType = "all";
        selectedCompatibility = "all";
        dispatch("searchChange", "");

        if (onlineSelected) {
            await reloadOnline();
        }
    }

    async function toggleOnline() {
        // onlineSelected = !onlineSelected;

        await reloadOnline();
        //await filter();
    }

    async function toggleInstalled() {
        // installedSelected = !installedSelected;

        debouncedReloadOnline.cancel();
        onlineCatalogGeneration += 1;
        onlineSelected = false;
        installedSelected = true;
        isLoading = true;
        filtered = [];
        try {
            filtered = await ModDatabase.getInstalledMods();
            catalogError = "";
        } catch (error) {
            catalogError = `Failed to scan installed mods: ${error}`;
        } finally {
            isLoading = false;
        }
        // page = 1;
        // filtered = [];
        // await fetchData();
        //await filter();
    }

    async function refreshMods() {
        try {
            await ModDatabase.loadInstalledMods();
            installedInventoryWarning = "";
        } catch (error) {
            installedInventoryWarning = `Installed mod scan failed: ${error}`;
            console.log(installedInventoryWarning);
        }
        
        if (onlineSelected) {
            await toggleOnline();
            return;
        }

        await toggleInstalled();
    }

    function matchesClientFilters(mod: Mod): boolean {
        const search = filterTerm.trim().toLowerCase();
        if (search && ![
            mod.name,
            mod.shortDescription ?? "",
            mod.user?.name ?? "",
            mod.latestVersion ?? "",
            mod.type ?? "",
            mod.category?.name ?? "",
            mod.installedMod?.manifest?.id ?? "",
            mod.installedMod?.vortexPackage ?? "",
            mod.installedMod?.store ?? ""
        ].some(value => value.toLowerCase().includes(search))) {
            return false;
        }

        if (selectedCategory !== "all" && mod.category?.slug !== selectedCategory) {
            return false;
        }

        if (selectedType !== "all" && mod.type !== selectedType && mod.installedMod?.manifest?.type !== selectedType) {
            return false;
        }

        if (selectedCompatibility === "mp-compatible" && !mod.isMultiplayerCompatible) {
            return false;
        }

        if (selectedCompatibility === "all-players" && !mod.requiresAllPlayers) {
            return false;
        }

        if (selectedCompatibility === "client" && mod.modSide !== "client") {
            return false;
        }

        return true;
    }

</script>

<div class="column mods-page" bind:this={modsPageElement}>
    {#if $isPathValid}
        <div class="row-center mods-toolbar">
            {#if showEmbeddedSearch}
                <input class="generic-input search-input" placeholder="Search" type="text" value={filterTerm} on:input={(event) => handleSearchInput((event.currentTarget as HTMLInputElement).value)} />
            {/if}
            <div class="mode-buttons">
                <button class="btn-left cat-btn" class:cat-btn-selected={onlineSelected} on:click={toggleOnline}>Online</button>
                <button class="btn-right cat-btn" class:cat-btn-selected={installedSelected} on:click={toggleInstalled}>Installed</button>
            </div>
            <button class="refresh-small icon-text-button" disabled={isLoading} on:click={refreshMods} title="Refresh mods">
                <LucideRefreshCw aria-hidden="true" />
                <span>Refresh</span>
            </button>
        </div>

        <div class="filter-row">
            <label>
                <span>Category</span>
                <select bind:value={selectedCategory} on:change={handleFacetChange}>
                    <option value="all">All categories</option>
                    {#each categories as category}
                        <option value={category.slug}>{category.name}</option>
                    {/each}
                </select>
            </label>
            <label>
                <span>Type</span>
                <select bind:value={selectedType} on:change={handleFacetChange}>
                    <option value="all">All types</option>
                    <option value="Mod">Mods</option>
                    <option value="Library">Libraries</option>
                    <option value="Build">Builds</option>
                </select>
            </label>
            <label>
                <span>Compatibility</span>
                <select bind:value={selectedCompatibility}>
                    <option value="all">All compatibility</option>
                    <option value="mp-compatible">Multiplayer compatible</option>
                    <option value="all-players">Requires all players</option>
                    <option value="client">Client-side</option>
                </select>
            </label>
            <button class="filter-clear" disabled={!hasActiveModFilters || isLoading} on:click={clearModFilters}>Clear</button>
        </div>

        <div class="mods-note">
            <span>{visibleMods.length} shown from {filtered.length} {onlineSelected ? "loaded" : "installed"}.</span>
            <span>Compact list.</span>
            {#if categories.length > 0}
                <span>{categories.length} categories.</span>
            {/if}
            {#if hasActiveModFilters}
                <span>Filters active.</span>
            {/if}
        </div>

        {#if catalogError}
            <div class="catalog-error">
                <span>{catalogError}</span>
                <button on:click={() => fetchData()}>Retry</button>
            </div>
        {/if}

        {#if installedInventoryWarning && onlineSelected}
            <div class="catalog-error subtle warning-note">{installedInventoryWarning}</div>
        {/if}

        <div class="scroller" class:grid={isGrid}>
            {#each visibleMods as mod}
                <ModCard mod={mod} on:refreshMods={refreshMods}/>
            {/each}

            {#if isLoading}
            <SvgSpinnersBlocksWave style="font-size: 2em; color: #38d68d; position: absolute; bottom: 40px;" />
            {/if}

            <InfiniteScroll
                hasMore={newBatch.length !== 0 && !installedSelected}
                threshold={100}
                on:loadMore={() => {page++; fetchData(page, onlineCatalogGeneration)}} />
        </div>

        {#if hasLoadedOnce && !isLoading && !catalogError && visibleMods.length === 0}
            <div class="catalog-error subtle">No mods are available for the current filter.</div>
        {/if}

    {:else}
        <b>Set the correct path in the main tab to start browsing mods.</b>
    {/if}
    
</div>

<style>
    .mods-page {
        --sotf-scroller-target-height: 320px;
        --sotf-thumb-height: clamp(74px, 9vh, 112px);
        --sotf-thumb-width: clamp(136px, 15vw, 210px);
        gap: clamp(0.34em, 0.64vh, 0.58em);
        height: 100%;
        justify-content: flex-start;
        min-height: 0;
    }

    .scroller {
        flex: 1 1 var(--sotf-scroller-target-height);
        height: var(--sotf-scroller-target-height);
        max-height: var(--sotf-scroller-target-height);
        min-height: min(var(--sotf-scroller-target-height), 100%);
        overflow-y: scroll;
        overflow-x: hidden;
        overscroll-behavior: contain;
        padding-bottom: 0.6em;
        position: relative;
        scrollbar-gutter: stable;
        width: 100%;
    }

    .grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
        grid-gap: 1em;
    }

    .mods-toolbar {
        align-items: center;
        display: flex;
        flex-wrap: wrap;
        gap: 0.45em;
        margin-bottom: 0;
        width: 100%;
    }

    .search-input {
        flex: 1 1 18rem;
        margin: 0;
        min-width: 14rem;
    }

    .mode-buttons {
        display: flex;
        flex: 0 0 auto;
    }

    .filter-row {
        display: grid;
        gap: 0.6em;
        grid-template-columns: repeat(3, minmax(0, 1fr)) minmax(7em, 0.45fr);
        margin: 0;
        width: 100%;
    }

    .filter-row label {
        align-items: flex-start;
        display: flex;
        flex-direction: column;
        gap: 0.2em;
        text-align: left;
    }

    .filter-row span {
        color: #a2a2a2;
        font-size: 0.74em;
        font-weight: 800;
        letter-spacing: 0.12em;
        line-height: 1.1;
        text-transform: uppercase;
    }

    .filter-row select {
        background: rgba(18, 18, 18, 0.92);
        border: 1px solid rgba(255, 255, 255, 0.16);
        color: #e8e8e8;
        font-family: inherit;
        font-weight: 700;
        min-height: 2.35em;
        padding: 0.36em 0.65em;
        text-transform: uppercase;
        width: 100%;
    }

    .filter-clear {
        align-self: end;
        color: #a2a2a2;
        height: 2.35em;
        margin: 0;
        padding: 0;
        width: 100%;
    }

    .mods-note {
        align-items: center;
        background: rgba(18, 18, 18, 0.7);
        border: 1px solid rgba(255, 255, 255, 0.1);
        box-sizing: border-box;
        color: #8d99a5;
        display: flex;
        flex: 0 0 auto;
        flex-wrap: wrap;
        font-size: 0.78em;
        font-weight: 800;
        gap: 0.7em;
        line-height: 1.2;
        margin: 0;
        padding: 0.42em 0.7em;
        text-align: left;
        width: 100%;
    }

    .cat-btn {
        color: #a2a2a2;
        flex: 0 0 auto;
        height: 2.42em;
        margin: 0;
        padding: 0;
        width: 6em;
    }

    .refresh-small {
        align-items: center;
        color: #a2a2a2;
        display: flex;
        flex: 0 0 auto;
        gap: 0.42em;
        height: 2.42em;
        justify-content: center;
        margin: 0;
        min-width: 7em;
        padding: 0 0.8em;
    }

    .icon-text-button :global(svg) {
        display: block;
        font-size: 1.05em;
        stroke-width: 2.25;
    }

    .cat-btn-selected {
        background-color: #111;
        color: #38d68d;
    }

    .catalog-error {
        align-items: center;
        background: rgba(44, 44, 44, 0.88);
        border: 1px solid rgba(255, 255, 255, 0.15);
        color: #fdc66d;
        display: flex;
        font-weight: 700;
        gap: 1em;
        justify-content: space-between;
        margin: 0;
        padding: 0.8em 1em;
        text-align: left;
        width: 100%;
    }

    @media (max-height: 820px) {
        .mods-page {
            gap: 0.34em;
        }

        .filter-row {
            gap: 0.45em;
        }

        .filter-row span {
            font-size: 0.68em;
        }

        .filter-row select,
        .filter-clear,
        .cat-btn,
        .refresh-small {
            height: 2.2em;
            min-height: 2.2em;
        }

        .mods-note {
            font-size: 0.72em;
            padding: 0.32em 0.62em;
        }
    }

    .catalog-error button {
        flex: 0 0 auto;
        margin: 0;
        min-width: 8em;
    }

    .catalog-error.subtle {
        color: #a2a2a2;
        justify-content: center;
    }

    .warning-note {
        color: #fdc66d;
        justify-content: flex-start;
    }

    @media (max-width: 850px) {
        .search-input {
            flex-basis: 100%;
            min-width: 0;
        }

        .filter-row {
            grid-template-columns: 1fr;
        }
    }
</style>
