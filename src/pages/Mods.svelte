<script lang="ts">
    import { createEventDispatcher, onDestroy, onMount, tick } from "svelte";
    import { isPathValid } from "../lib/store";
    import ModCard from "../lib/ModCard.svelte";
    import type { Mod, ModCategory } from "../lib/mods";
    import { ModDatabase, modDependencies, modPreviewUrls, Sorting } from "../lib/mods";
    import InfiniteScroll from "../lib/InfiniteScroll.svelte";
    import { debounce } from "lodash";
    import SvgSpinnersBlocksWave from '~icons/svg-spinners/blocks-wave'
    import LucideChevronLeft from "~icons/lucide/chevron-left";
    import LucideChevronRight from "~icons/lucide/chevron-right";
    import LucideExternalLink from "~icons/lucide/external-link";
    import LucideImages from "~icons/lucide/images";
    import LucideRefreshCw from "~icons/lucide/refresh-cw";
    import LucideX from "~icons/lucide/x";

    type DetailDependencyState = "loading" | "available" | "installed" | "missing" | "error";

    type DetailDependencyRow = {
        id: string;
        mod?: Mod;
        state: DetailDependencyState;
        message?: string;
    };

    export let sharedSearchTerm = "";
    export let sharedSearchVersion = 0;
    export let showEmbeddedSearch = true;
    export let embeddedStorePreview = false;

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
    let selectedDetailMod: Mod | null = null;
    let selectedDetailPreviewIndex = 0;
    let selectedDetailPreviewUrls: string[] = [];
    let selectedDetailPreviewUrl = "";
    let selectedDetailPreviewLabel = "";
    let selectedDetailDependencies: string[] = [];
    let selectedDetailDependencyRows: DetailDependencyRow[] = [];
    let detailDependencyLookupToken = 0;

    const SOTF_DETAIL_FALLBACK_IMAGE = "https://placehold.co/900x500/252525/FFF?text=No+Image";
    const detailDependencyCache = new Map<string, Mod | null>();

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
    $: selectedDetailPreviewUrls = selectedDetailMod ? modPreviewUrls(selectedDetailMod) : [];
    $: if (selectedDetailPreviewIndex >= selectedDetailPreviewUrls.length) {
        selectedDetailPreviewIndex = 0;
    }
    $: selectedDetailPreviewUrl = selectedDetailPreviewUrls[selectedDetailPreviewIndex] ?? SOTF_DETAIL_FALLBACK_IMAGE;
    $: selectedDetailPreviewLabel = selectedDetailPreviewUrls.length > 0
        ? `${selectedDetailPreviewIndex + 1}/${selectedDetailPreviewUrls.length}`
        : "Local";
    $: selectedDetailDependencies = selectedDetailMod ? modDependencies(selectedDetailMod) : [];
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
        const targetThumbHeight = clampNumber(targetScrollerHeight / (targetScrollerHeight >= 620 ? 6.1 : 5.25), 70, 104);

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

    async function refreshModsFromDetail() {
        const selectedKey = selectedDetailMod ? modIdentityKey(selectedDetailMod) : "";
        await refreshMods();
        await tick();

        if (!selectedKey) {
            return;
        }

        const updated = filtered.find(mod => modIdentityKey(mod) === selectedKey);
        if (updated) {
            selectedDetailMod = updated;
            resolveSelectedDetailDependencies(updated);
        }
    }

    function openModDetails(mod: Mod) {
        selectedDetailMod = mod;
        selectedDetailPreviewIndex = 0;
        resolveSelectedDetailDependencies(mod);
    }

    function closeModDetails() {
        selectedDetailMod = null;
        selectedDetailDependencyRows = [];
        detailDependencyLookupToken += 1;
    }

    function cycleSelectedDetailPreview(direction: number) {
        if (selectedDetailPreviewUrls.length <= 1) {
            return;
        }

        selectedDetailPreviewIndex = (selectedDetailPreviewIndex + direction + selectedDetailPreviewUrls.length) % selectedDetailPreviewUrls.length;
    }

    function handleDetailImageError(event: Event) {
        const image = event.currentTarget instanceof HTMLImageElement ? event.currentTarget : null;
        if (!image || image.src === SOTF_DETAIL_FALLBACK_IMAGE) {
            return;
        }

        image.src = SOTF_DETAIL_FALLBACK_IMAGE;
    }

    function openSelectedDetailPage() {
        if (selectedDetailMod) {
            ModDatabase.openModPage(selectedDetailMod);
        }
    }

    async function resolveSelectedDetailDependencies(mod: Mod) {
        const dependencyIds = modDependencies(mod);
        const lookupToken = ++detailDependencyLookupToken;
        selectedDetailDependencyRows = dependencyIds.map(id => ({ id, state: "loading" }));

        if (dependencyIds.length === 0) {
            return;
        }

        const rows = await Promise.all(dependencyIds.map(async id => resolveDetailDependency(id)));
        if (lookupToken !== detailDependencyLookupToken || selectedDetailMod !== mod) {
            return;
        }

        selectedDetailDependencyRows = rows;
    }

    async function resolveDetailDependency(id: string): Promise<DetailDependencyRow> {
        const loadedDependency = findLoadedDependency(id);
        if (loadedDependency) {
            return detailDependencyRow(id, loadedDependency);
        }

        const cacheKey = id.trim().toLowerCase();
        try {
            let dependencyMod = detailDependencyCache.get(cacheKey);
            if (!detailDependencyCache.has(cacheKey)) {
                dependencyMod = await ModDatabase.fetchMod(id);
                detailDependencyCache.set(cacheKey, dependencyMod);
            }

            if (!dependencyMod) {
                return {
                    id,
                    state: "missing",
                    message: "No matching SOTF Mods entry was returned for this dependency."
                };
            }

            ModDatabase.initModList([dependencyMod]);
            return detailDependencyRow(id, dependencyMod);
        } catch (error) {
            return {
                id,
                state: "error",
                message: `Dependency lookup failed: ${error}`
            };
        }
    }

    function findLoadedDependency(id: string): Mod | undefined {
        const needle = id.trim().toLowerCase();
        return filtered.find(mod => [
            mod.mod_id,
            mod.slug,
            mod.name
        ].some(value => typeof value === "string" && value.trim().toLowerCase() === needle));
    }

    function detailDependencyRow(id: string, mod: Mod): DetailDependencyRow {
        ModDatabase.initModList([mod]);
        return {
            id,
            mod,
            state: mod.isInstalled ? "installed" : "available",
            message: mod.isInstalled ? detailInstallStateLabel(mod) : "Available from SOTF Mods"
        };
    }

    function detailDependencyStateLabel(row: DetailDependencyRow): string {
        if (row.state === "installed") {
            return "Installed";
        }

        if (row.state === "available") {
            return "Available";
        }

        if (row.state === "missing") {
            return "Missing";
        }

        if (row.state === "error") {
            return "Review";
        }

        return "Checking";
    }

    function detailDependencySubtitle(row: DetailDependencyRow): string {
        if (!row.mod) {
            return row.message ?? "Checking SOTF Mods dependency metadata...";
        }

        return `${detailLoaderLabel(row.mod)} · ${row.mod.latestVersion ?? "Unknown version"} · ${row.mod.user?.name ?? "Unknown author"}`;
    }

    function openDependencyDetails(row: DetailDependencyRow) {
        if (row.mod) {
            openModDetails(row.mod);
        }
    }

    function openDependencyPage(row: DetailDependencyRow) {
        if (row.mod) {
            ModDatabase.openModPage(row.mod);
        }
    }

    function modIdentityKey(mod: Mod): string {
        return mod.mod_id || mod.slug || mod.name;
    }

    function formatDetailDate(dateString: string): string {
        const date = new Date(dateString);
        return date.toLocaleDateString("en-US", {
            year: "numeric",
            month: "2-digit",
            day: "2-digit",
        });
    }

    function detailAuthorName(mod: Mod): string {
        return mod.user?.name ?? mod.installedMod?.manifest?.author ?? "Unknown";
    }

    function detailCategoryLabel(mod: Mod): string {
        return mod.category?.name ?? mod.installedMod?.manifest?.type ?? "-";
    }

    function detailSourceLabel(mod: Mod): string {
        if (!mod.installedMod) {
            return "Online";
        }

        if (mod.installedMod.installSource === "vortex") {
            return "Vortex";
        }

        if (mod.installedMod.installSource === "native") {
            return "Native";
        }

        return "Manual";
    }

    function detailLoaderLabel(mod: Mod): string {
        if (mod.installedMod?.loaderType === "bepinex-plugin") {
            return "BepInEx plugin";
        }

        if (mod.installedMod?.loaderType === "redloader-library" || mod.type === "Library") {
            return "RedLoader library";
        }

        if (mod.type === "Build") {
            return "Build";
        }

        return "RedLoader mod";
    }

    function detailCompatibilityLabel(mod: Mod): string {
        if (mod.requiresAllPlayers) {
            return "Requires all players";
        }

        if (mod.isMultiplayerCompatible) {
            return "Multiplayer compatible";
        }

        if (mod.modSide === "client") {
            return "Client-side";
        }

        return "Review";
    }

    function detailInstallStateLabel(mod: Mod): string {
        if (!mod.isInstalled) {
            return "Not installed";
        }

        if (mod.installedMod?.installSource === "vortex") {
            return "Managed by Vortex";
        }

        return mod.installedMod?.isEnabled ? "Installed enabled" : "Installed disabled";
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

<div
    class="column mods-page"
    class:sotf-embedded-store-preview={embeddedStorePreview}
    class:sotf-embedded-controls-needed={embeddedStorePreview && (installedSelected || selectedCategory !== "all" || selectedType !== "all" || selectedCompatibility !== "all")}
    bind:this={modsPageElement}
>
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
                <ModCard
                    mod={mod}
                    on:details={(event) => openModDetails(event.detail)}
                    on:refreshMods={refreshMods}
                />
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

        {#if selectedDetailMod}
            <div class="sotf-detail-backdrop">
                <button class="sotf-detail-backdrop-dismiss" type="button" aria-label="Close SOTF Mods details" on:click={closeModDetails}></button>
                <div class="sotf-detail-panel" role="dialog" aria-modal="true" aria-labelledby="sotf-detail-title" tabindex="-1">
                    <header class="sotf-detail-header">
                        <div class="sotf-detail-title">
                            <span id="sotf-detail-title">{selectedDetailMod.name}</span>
                            <small>{detailCategoryLabel(selectedDetailMod)} · {detailLoaderLabel(selectedDetailMod)} · {detailAuthorName(selectedDetailMod)}</small>
                        </div>
                        <button class="sotf-detail-close" type="button" aria-label="Close SOTF Mods details" on:click={closeModDetails}>
                            <LucideX aria-hidden="true" />
                            <span>Close</span>
                        </button>
                    </header>

                    <div class="sotf-detail-grid">
                        <div class="sotf-detail-main">
                            <div class="sotf-detail-media" aria-label={`${selectedDetailMod.name} preview images`}>
                                <img src={selectedDetailPreviewUrl} on:error={handleDetailImageError} alt={`Preview for ${selectedDetailMod.name}`} />
                                <div
                                    class="sotf-detail-preview-count"
                                    class:sotf-detail-preview-fallback={selectedDetailPreviewUrls.length === 0}
                                    aria-label={selectedDetailPreviewUrls.length > 0 ? `${selectedDetailMod.name} preview image ${selectedDetailPreviewIndex + 1} of ${selectedDetailPreviewUrls.length}` : `${selectedDetailMod.name} uses the local fallback preview image`}
                                >
                                    <LucideImages aria-hidden="true" />
                                    <span>{selectedDetailPreviewLabel}</span>
                                </div>
                                {#if selectedDetailPreviewUrls.length > 1}
                                    <div class="sotf-detail-preview-controls">
                                        <button type="button" aria-label={`Previous preview image for ${selectedDetailMod.name}`} title="Previous preview image" on:click={() => cycleSelectedDetailPreview(-1)}>
                                            <LucideChevronLeft aria-hidden="true" />
                                        </button>
                                        <button type="button" aria-label={`Next preview image for ${selectedDetailMod.name}`} title="Next preview image" on:click={() => cycleSelectedDetailPreview(1)}>
                                            <LucideChevronRight aria-hidden="true" />
                                        </button>
                                    </div>
                                {/if}
                            </div>

                            <div class="sotf-detail-description">
                                <span class="sotf-detail-section-title">Description</span>
                                <p>{selectedDetailMod.shortDescription || "No description is available from SOTF Mods for this entry."}</p>
                            </div>
                        </div>

                        <aside class="sotf-detail-side">
                            <div class="sotf-detail-facts">
                                <span>Source <b>{detailSourceLabel(selectedDetailMod)}</b></span>
                                <span>State <b>{detailInstallStateLabel(selectedDetailMod)}</b></span>
                                <span>Version <b>{selectedDetailMod.latestVersion ?? "-"}</b></span>
                                <span>Updated <b>{selectedDetailMod.lastReleasedAt ? formatDetailDate(selectedDetailMod.lastReleasedAt) : "-"}</b></span>
                                <span>Category <b>{detailCategoryLabel(selectedDetailMod)}</b></span>
                                <span>Compatibility <b>{detailCompatibilityLabel(selectedDetailMod)}</b></span>
                                <span>Downloads <b>{selectedDetailMod.downloads ?? selectedDetailMod.lastWeekDownloads ?? "-"}</b></span>
                                <span>Author <b>{detailAuthorName(selectedDetailMod)}</b></span>
                            </div>

                            <div class="sotf-detail-install-target">
                                <span class="sotf-detail-section-title">Install Target</span>
                                <div>
                                    <span>Type <b>{detailLoaderLabel(selectedDetailMod)}</b></span>
                                    <span>Package <b>{selectedDetailMod.installedMod?.vortexPackage ?? selectedDetailMod.slug ?? selectedDetailMod.mod_id}</b></span>
                                </div>
                            </div>

                            <div class="sotf-detail-dependencies">
                                <span class="sotf-detail-section-title">Dependencies</span>
                                {#if selectedDetailDependencies.length > 0}
                                    <div class="sotf-detail-dependency-list">
                                        {#each selectedDetailDependencyRows as dependency}
                                            <div
                                                class="sotf-detail-dependency-row"
                                                class:sotf-detail-dependency-installed={dependency.state === "installed"}
                                                class:sotf-detail-dependency-available={dependency.state === "available"}
                                                class:sotf-detail-dependency-review={dependency.state === "missing" || dependency.state === "error"}
                                            >
                                                <div class="sotf-detail-dependency-head">
                                                    <span>{dependency.mod?.name ?? dependency.id}</span>
                                                    <b>{detailDependencyStateLabel(dependency)}</b>
                                                </div>
                                                <small>{detailDependencySubtitle(dependency)}</small>
                                                {#if dependency.mod}
                                                    <div class="sotf-detail-dependency-actions">
                                                        <button type="button" on:click={() => openDependencyDetails(dependency)}>Details</button>
                                                        <button type="button" on:click={() => openDependencyPage(dependency)}>
                                                            <LucideExternalLink aria-hidden="true" />
                                                            <span>Open Page</span>
                                                        </button>
                                                    </div>
                                                {/if}
                                            </div>
                                        {/each}
                                    </div>
                                {:else}
                                    <small>No storefront dependency records.</small>
                                {/if}
                            </div>
                        </aside>
                    </div>

                    <footer class="sotf-detail-footer">
                        <div class="sotf-detail-footer-copy">
                            <span class="sotf-detail-section-title">Deployment</span>
                            <small>{detailInstallStateLabel(selectedDetailMod)} · {detailLoaderLabel(selectedDetailMod)}</small>
                        </div>
                        <div class="sotf-detail-footer-actions">
                            <ModCard
                                detailActionsOnly={true}
                                mod={selectedDetailMod}
                                on:refreshMods={refreshModsFromDetail}
                            />
                            <button class="sotf-detail-open-page" type="button" on:click={openSelectedDetailPage}>
                                <LucideExternalLink aria-hidden="true" />
                                <span>Open Page</span>
                            </button>
                        </div>
                    </footer>
                </div>
            </div>
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

    .sotf-embedded-store-preview {
        --sotf-thumb-height: clamp(58px, 6.4vh, 74px);
        --sotf-thumb-width: clamp(108px, 11vw, 148px);
        gap: clamp(0.24em, 0.45vh, 0.36em);
    }

    .sotf-embedded-store-preview:not(.sotf-embedded-controls-needed) .mods-toolbar,
    .sotf-embedded-store-preview:not(.sotf-embedded-controls-needed) .filter-row {
        display: none;
    }

    .sotf-embedded-store-preview .mods-note {
        font-size: 0.7em;
        padding: 0.28em 0.58em;
    }

    .sotf-embedded-store-preview .scroller {
        overscroll-behavior-y: auto;
        padding-bottom: clamp(0.45em, 2.2vh, 1em);
        scroll-snap-type: y mandatory;
    }

    .sotf-embedded-store-preview :global(.description) {
        margin-bottom: 0.32em;
        padding: 0.46em;
    }

    .sotf-embedded-store-preview :global(.mod-card-row) {
        gap: 0.48em;
        grid-template-columns: var(--sotf-thumb-width) minmax(0, 1fr) minmax(7.8em, 9.6em);
    }

    .sotf-embedded-store-preview :global(.description-content.header-desc) {
        display: -webkit-box;
        -webkit-box-orient: vertical;
        -webkit-line-clamp: 1;
        line-clamp: 1;
        overflow: hidden;
    }

    .sotf-embedded-store-preview :global(.source-pill) {
        font-size: 0.68em;
        padding: 0.2em 0.42em;
    }

    .sotf-embedded-store-preview :global(.fact-row) {
        display: none;
    }

    .sotf-embedded-store-preview :global(.mod-actions) {
        gap: 0.28em;
    }

    .sotf-embedded-store-preview :global(.mod-actions button) {
        font-size: 0.72em;
        min-height: 2.12em;
        padding: 0.34em 0.5em;
    }

    .scroller {
        flex: 1 1 var(--sotf-scroller-target-height);
        height: var(--sotf-scroller-target-height);
        max-height: var(--sotf-scroller-target-height);
        min-height: min(var(--sotf-scroller-target-height), 100%);
        overflow-y: scroll;
        overflow-x: hidden;
        overscroll-behavior: contain;
        padding-bottom: calc(0.6em + var(--app-bottom-safe-area, 42px));
        position: relative;
        scroll-padding-bottom: var(--app-bottom-safe-area, 42px);
        scroll-padding-top: 0.3em;
        scroll-snap-type: y proximity;
        scrollbar-gutter: stable;
        width: 100%;
    }

    .scroller :global(.feature-container) {
        scroll-margin-top: 0.3em;
        scroll-snap-align: start;
        scroll-snap-stop: always;
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

    .sotf-detail-backdrop {
        align-items: center;
        background: rgba(0, 0, 0, 0.58);
        box-sizing: border-box;
        display: flex;
        inset: 0;
        justify-content: center;
        padding: clamp(0.8em, 2vh, 1.4em);
        position: fixed;
        z-index: 20;
    }

    .sotf-detail-backdrop-dismiss {
        background: transparent;
        border: 0;
        box-shadow: none;
        cursor: default;
        height: auto;
        inset: 0;
        margin: 0;
        min-width: 0;
        padding: 0;
        position: absolute;
        width: auto;
        -webkit-mask-image: none;
        mask-image: none;
    }

    .sotf-detail-panel {
        background: rgba(10, 10, 10, 0.97);
        border: 1px solid rgba(255, 255, 255, 0.16);
        box-shadow: 0 1.2em 3em rgba(0, 0, 0, 0.48);
        box-sizing: border-box;
        color: #d8e3ea;
        display: flex;
        flex-direction: column;
        gap: 0.75em;
        max-height: calc(100vh - clamp(1.4em, 4vh, 2.8em));
        max-width: min(1180px, calc(100vw - clamp(1.4em, 4vw, 3em)));
        min-height: min(620px, calc(100vh - 2.8em));
        min-width: 0;
        overflow: hidden;
        padding: clamp(0.8em, 1.6vh, 1.15em);
        position: relative;
        width: min(1180px, 96vw);
        z-index: 1;
    }

    .sotf-detail-header {
        align-items: center;
        display: flex;
        flex: 0 0 auto;
        gap: 1em;
        justify-content: space-between;
        min-width: 0;
    }

    .sotf-detail-title {
        display: flex;
        flex: 1 1 auto;
        flex-direction: column;
        gap: 0.18em;
        min-width: 0;
        text-align: left;
    }

    .sotf-detail-title span {
        color: #eefcff;
        font-size: clamp(1.05em, 1.45vw, 1.42em);
        font-weight: 900;
        line-height: 1.1;
        overflow-wrap: anywhere;
    }

    .sotf-detail-title small {
        color: #9aa5af;
        font-size: 0.78em;
        font-weight: 800;
        line-height: 1.2;
        overflow-wrap: anywhere;
        text-transform: uppercase;
    }

    .sotf-detail-close,
    .sotf-detail-open-page {
        align-items: center;
        display: inline-flex;
        gap: 0.45em;
        justify-content: center;
        margin: 0;
        min-width: 8.5em;
    }

    .sotf-detail-close :global(svg),
    .sotf-detail-open-page :global(svg) {
        flex: 0 0 auto;
        height: 1em;
        width: 1em;
    }

    .sotf-detail-grid {
        display: grid;
        flex: 1 1 auto;
        gap: 0.75em;
        grid-template-columns: minmax(0, 1.12fr) minmax(19em, 0.88fr);
        min-height: 0;
        min-width: 0;
        overflow: hidden;
    }

    .sotf-detail-main,
    .sotf-detail-side {
        display: flex;
        flex-direction: column;
        gap: 0.75em;
        min-height: 0;
        min-width: 0;
    }

    .sotf-detail-media {
        background: rgba(8, 8, 8, 0.92);
        border: 1px solid rgba(255, 255, 255, 0.14);
        flex: 0 0 auto;
        height: clamp(210px, 34vh, 380px);
        min-height: 0;
        overflow: hidden;
        position: relative;
    }

    .sotf-detail-media img {
        display: block;
        height: 100%;
        object-fit: cover;
        width: 100%;
    }

    .sotf-detail-preview-count,
    .sotf-detail-preview-controls {
        align-items: center;
        display: flex;
        gap: 0.25em;
        position: absolute;
        z-index: 2;
    }

    .sotf-detail-preview-count {
        background: rgba(6, 10, 12, 0.82);
        border: 1px solid rgba(255, 255, 255, 0.14);
        color: #d6f3ff;
        font-size: 0.72em;
        font-weight: 900;
        left: 0.65em;
        padding: 0.28em 0.45em;
        text-transform: uppercase;
        top: 0.65em;
    }

    .sotf-detail-preview-count :global(svg) {
        height: 1em;
        width: 1em;
    }

    .sotf-detail-preview-fallback {
        color: #9aa5af;
    }

    .sotf-detail-preview-controls {
        bottom: 0.65em;
        right: 0.65em;
    }

    .sotf-detail-preview-controls button {
        align-items: center;
        background: rgba(10, 14, 18, 0.84);
        border: 1px solid rgba(255, 255, 255, 0.16);
        box-shadow: none;
        color: #62f09b;
        display: inline-flex;
        height: 2.2em;
        justify-content: center;
        margin: 0;
        min-width: 2.2em;
        padding: 0;
        -webkit-mask-image: none;
        mask-image: none;
    }

    .sotf-detail-preview-controls button:hover {
        border-color: rgba(98, 240, 155, 0.48);
    }

    .sotf-detail-preview-controls :global(svg) {
        height: 1.1em;
        width: 1.1em;
    }

    .sotf-detail-description,
    .sotf-detail-facts,
    .sotf-detail-install-target,
    .sotf-detail-dependencies,
    .sotf-detail-footer {
        background: rgba(18, 18, 18, 0.88);
        border: 1px solid rgba(255, 255, 255, 0.12);
        box-sizing: border-box;
        padding: 0.75em;
    }

    .sotf-detail-section-title {
        color: #eefcff;
        font-size: 0.78em;
        font-weight: 900;
        letter-spacing: 0.09em;
        text-transform: uppercase;
    }

    .sotf-detail-description {
        flex: 1 1 auto;
        min-height: 0;
        overflow-y: auto;
        text-align: left;
    }

    .sotf-detail-description p {
        color: #c2ccd5;
        font-size: 0.92em;
        line-height: 1.45;
        margin: 0.55em 0 0;
        overflow-wrap: anywhere;
    }

    .sotf-detail-facts {
        display: grid;
        flex: 0 0 auto;
        gap: 0.45em 0.65em;
        grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .sotf-detail-facts span,
    .sotf-detail-install-target span {
        color: #8d99a5;
        font-size: 0.78em;
        font-weight: 800;
        line-height: 1.25;
        min-width: 0;
        overflow: hidden;
        text-align: left;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .sotf-detail-facts b,
    .sotf-detail-install-target b {
        color: #d6dde5;
        display: block;
        font-weight: 900;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .sotf-detail-install-target,
    .sotf-detail-dependencies {
        display: flex;
        flex: 0 0 auto;
        flex-direction: column;
        gap: 0.55em;
        min-width: 0;
        text-align: left;
    }

    .sotf-detail-install-target > div {
        display: grid;
        gap: 0.45em;
        grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .sotf-detail-dependencies {
        flex: 1 1 auto;
        min-height: 0;
        overflow-y: auto;
    }

    .sotf-detail-dependencies small {
        color: #9aa5af;
        font-size: 0.82em;
        font-weight: 800;
    }

    .sotf-detail-dependency-list {
        display: grid;
        gap: 0.4em;
        min-width: 0;
    }

    .sotf-detail-dependency-row {
        background: rgba(255, 255, 255, 0.045);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #d6dde5;
        display: grid;
        gap: 0.36em;
        min-width: 0;
        overflow-wrap: anywhere;
        padding: 0.48em 0.58em;
    }

    .sotf-detail-dependency-installed {
        border-color: rgba(98, 240, 155, 0.28);
    }

    .sotf-detail-dependency-available {
        border-color: rgba(96, 169, 255, 0.28);
    }

    .sotf-detail-dependency-review {
        border-color: rgba(255, 192, 92, 0.32);
    }

    .sotf-detail-dependency-head {
        align-items: center;
        display: flex;
        gap: 0.5em;
        justify-content: space-between;
        min-width: 0;
    }

    .sotf-detail-dependency-head span {
        color: #eefcff;
        font-size: 0.84em;
        font-weight: 900;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .sotf-detail-dependency-head b {
        color: #62f09b;
        flex: 0 0 auto;
        font-size: 0.68em;
        font-weight: 900;
        text-transform: uppercase;
    }

    .sotf-detail-dependency-review .sotf-detail-dependency-head b {
        color: #ffc05c;
    }

    .sotf-detail-dependency-row small {
        color: #9aa5af;
        font-size: 0.74em;
        font-weight: 800;
        line-height: 1.25;
        overflow-wrap: anywhere;
    }

    .sotf-detail-dependency-actions {
        display: flex;
        flex-wrap: wrap;
        gap: 0.35em;
        min-width: 0;
    }

    .sotf-detail-dependency-actions button {
        align-items: center;
        display: inline-flex;
        flex: 1 1 7.5em;
        gap: 0.35em;
        justify-content: center;
        margin: 0;
        min-height: 2.15em;
        min-width: 0;
        padding: 0.35em 0.5em;
    }

    .sotf-detail-dependency-actions :global(svg) {
        flex: 0 0 auto;
        height: 0.9em;
        width: 0.9em;
    }

    .sotf-detail-footer {
        align-items: stretch;
        display: grid;
        flex: 0 0 auto;
        gap: 0.75em;
        grid-template-columns: minmax(12em, 0.72fr) minmax(0, 1fr);
        min-width: 0;
    }

    .sotf-detail-footer-copy {
        display: flex;
        flex-direction: column;
        gap: 0.3em;
        justify-content: center;
        min-width: 0;
        text-align: left;
    }

    .sotf-detail-footer-copy small {
        color: #9aa5af;
        font-size: 0.78em;
        font-weight: 800;
        line-height: 1.25;
        overflow-wrap: anywhere;
    }

    .sotf-detail-footer-actions {
        align-items: stretch;
        display: grid;
        gap: 0.55em;
        grid-template-columns: minmax(0, 1fr) minmax(9em, auto);
        min-width: 0;
    }

    .sotf-detail-open-page {
        color: #d6dde5;
        min-height: 2.7em;
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

    @media (max-width: 920px) {
        .sotf-detail-panel {
            min-height: 0;
            overflow-y: auto;
        }

        .sotf-detail-grid,
        .sotf-detail-footer {
            grid-template-columns: 1fr;
            overflow: visible;
        }

        .sotf-detail-side {
            overflow: visible;
        }
    }

    @media (max-width: 620px) {
        .sotf-detail-backdrop {
            padding: 0.45em;
        }

        .sotf-detail-panel {
            max-height: calc(100vh - 0.9em);
            max-width: calc(100vw - 0.9em);
            padding: 0.62em;
            width: calc(100vw - 0.9em);
        }

        .sotf-detail-header,
        .sotf-detail-footer-actions {
            grid-template-columns: 1fr;
        }

        .sotf-detail-header {
            align-items: stretch;
            flex-direction: column;
        }

        .sotf-detail-close,
        .sotf-detail-open-page {
            width: 100%;
        }

        .sotf-detail-facts,
        .sotf-detail-install-target > div {
            grid-template-columns: 1fr;
        }
    }
</style>
