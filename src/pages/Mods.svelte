<script lang="ts">
    import { createEventDispatcher, onDestroy, onMount, tick } from "svelte";
    import { isPathValid } from "../lib/store";
    import ModCard from "../lib/ModCard.svelte";
    import type { Mod, ModCategory, ModVersion } from "../lib/mods";
    import { ModDatabase, modDependencies, modPreviewUrls, Sorting } from "../lib/mods";
    import { renderStoreRichText } from "../lib/richText";
    import { fallbackPreviewUrl } from "../lib/modPreview";
    import { modalFocus } from "../lib/modalFocus";
    import InfiniteScroll from "../lib/InfiniteScroll.svelte";
    import { debounce } from "lodash";
    import SvgSpinnersBlocksWave from '~icons/svg-spinners/blocks-wave'
    import LucideChevronLeft from "~icons/lucide/chevron-left";
    import LucideChevronRight from "~icons/lucide/chevron-right";
    import LucideExternalLink from "~icons/lucide/external-link";
    import LucideImages from "~icons/lucide/images";
    import LucidePackage from "~icons/lucide/package";
    import LucideRefreshCw from "~icons/lucide/refresh-cw";
    import LucideX from "~icons/lucide/x";

    type DetailDependencyState = "loading" | "available" | "installed" | "missing" | "error";

    type DetailDependencyRow = {
        id: string;
        mod?: Mod;
        state: DetailDependencyState;
        message?: string;
    };

    type SotfCatalogSort = "updated" | "downloads" | "favorites" | "comments" | "rating" | "name";
    type SourceSummaryEvent = {
        visible: number;
        total: number;
        mode: string;
        note: string;
        activeFilters: boolean;
        attention: number;
    };

    export let sharedSearchTerm = "";
    export let sharedSearchVersion = 0;
    export let showEmbeddedSearch = true;
    export let embeddedStorePreview = false;
    export let embeddedControlsExpanded = false;

    const dispatch = createEventDispatcher<{ searchChange: string; summaryChange: SourceSummaryEvent }>();

    let filtered: Mod[] = [];
    let filterTerm: string = "";
    let lastAppliedSharedSearchVersion = 0;
    let selectedCategory = "all";
    let selectedType = "all";
    let selectedCompatibility = "all";
    let selectedSort: SotfCatalogSort = "updated";
    let categories: ModCategory[] = [];
    let visibleMods: Mod[] = [];
    let activeSortMetricLabel = "";
    let hasActiveModFilters = false;
    let hasActiveModViewOptions = false;
    let hasActiveLocalModFilters = false;
    let hasActiveLocalModViewOptions = false;

    let onlineSelected = true;
    let installedSelected = false;

    let isGrid = false;

    let page = 0;
    let hasMorePages = false;
    let isDestroyed = false;
    let isLoading: boolean = false;
    let hasLoadedOnce = false;
    let catalogError: string = "";
    let onlineCatalogGeneration = 0;
    let latestOnlineFetchId = 0;
    let installedInventoryWarning = "";
    let isScanningInventory = true;
    let inventoryReady = false;
    let modsPageElement: HTMLDivElement | null = null;
    let catalogScroller: HTMLDivElement | null = null;
    let modsLayoutObserver: ResizeObserver | null = null;
    let modsLayoutFrame: number | null = null;
    let modsWindowResizeHandler: (() => void) | null = null;
    let selectedDetailMod: Mod | null = null;
    let selectedDetailPreviewIndex = 0;
    let selectedDetailPreviewUrls: string[] = [];
    let selectedDetailPreviewUrl = "";
    let selectedDetailPreviewLabel = "";
    let selectedDetailDescriptionHtml = "";
    let selectedDetailIsLoading = false;
    let selectedDetailLoadError = "";
    let selectedDetailDependencies: string[] = [];
    let selectedDetailVersions: ModVersion[] = [];
    let selectedDetailHiddenVersionCount = 0;
    let selectedDetailDependencyRows: DetailDependencyRow[] = [];
    let detailDependencyLookupToken = 0;
    let detailModLookupToken = 0;
    let detailGeneration = 0;

    const SOTF_DETAIL_FALLBACK_IMAGE = fallbackPreviewUrl;
    const detailModCache = new Map<string, Mod>();
    const detailDependencyCache = new Map<string, Mod | null>();

    $: {
        onlineSelected;
        filterTerm;
        selectedCategory;
        selectedType;
        selectedCompatibility;
        selectedSort;
        void resetCatalogScroll();
    }

    async function resetCatalogScroll() {
        await tick();
        if (catalogScroller) catalogScroller.scrollTop = 0;
    }

    $: {
        filterTerm;
        selectedCategory;
        selectedType;
        selectedCompatibility;
        selectedSort;
        visibleMods = sortVisibleMods(filtered.filter(matchesClientFilters));
        activeSortMetricLabel = sortMetricLabel(selectedSort);
    }
    $: if (sharedSearchVersion > 0 && sharedSearchVersion !== lastAppliedSharedSearchVersion) {
        lastAppliedSharedSearchVersion = sharedSearchVersion;
        applySharedSearch(sharedSearchTerm);
    }
    $: hasActiveLocalModFilters = selectedCategory !== "all"
        || selectedType !== "all"
        || selectedCompatibility !== "all";
    $: hasActiveModFilters = filterTerm.trim().length > 0 || hasActiveLocalModFilters;
    $: hasActiveLocalModViewOptions = hasActiveLocalModFilters || selectedSort !== "updated";
    $: hasActiveModViewOptions = hasActiveModFilters || selectedSort !== "updated";
    $: dispatch("summaryChange", {
        visible: visibleMods.length,
        total: filtered.length,
        mode: onlineSelected ? "Online" : "Installed",
        note: sotfSummaryNote(),
        activeFilters: installedSelected || hasActiveLocalModViewOptions,
        attention: 0
    });
    $: selectedDetailPreviewUrls = selectedDetailMod ? modPreviewUrls(selectedDetailMod) : [];
    $: if (selectedDetailPreviewIndex >= selectedDetailPreviewUrls.length) {
        selectedDetailPreviewIndex = 0;
    }
    $: selectedDetailPreviewUrl = selectedDetailPreviewUrls[selectedDetailPreviewIndex] ?? SOTF_DETAIL_FALLBACK_IMAGE;
    $: selectedDetailPreviewLabel = selectedDetailPreviewUrls.length > 0
        ? `${selectedDetailPreviewIndex + 1}/${selectedDetailPreviewUrls.length}`
        : "Local";
    $: selectedDetailDescriptionHtml = renderStoreRichText(
        selectedDetailMod?.description || selectedDetailMod?.shortDescription,
        "No description is available from SOTF Mods for this entry."
    );
    $: selectedDetailDependencies = selectedDetailMod ? modDependencies(selectedDetailMod) : [];
    $: selectedDetailVersions = selectedDetailMod ? detailVersions(selectedDetailMod) : [];
    $: selectedDetailHiddenVersionCount = Math.max(0, (selectedDetailMod?.versions?.length ?? 0) - selectedDetailVersions.length);
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

    async function fetchData(requestPage = page + 1, requestGeneration = onlineCatalogGeneration) {
        if (isDestroyed || !onlineSelected) return;
        const fetchId = ++latestOnlineFetchId;
        isLoading = true;
        catalogError = "";
        try {
            let res = await ModDatabase.fetchMods(requestPage, Sorting.newest, true, false, filterTerm, selectedCategory, selectedType);
            if (!isCurrentCatalogRequest(requestGeneration) || !onlineSelected || fetchId !== latestOnlineFetchId) {
                return;
            }

            let mods = res.data;
            await ModDatabase.initModList(mods);
            if (!isCurrentCatalogRequest(requestGeneration) || !onlineSelected || fetchId !== latestOnlineFetchId) {
                return;
            }

            const rows = requestPage === 1 ? mods : [...filtered, ...mods];
            filtered = [...new Map(rows.map(mod => [modIdentityKey(mod), mod])).values()];
            page = requestPage;
            hasMorePages = mods.length > 0 && (!Number.isFinite(res.meta?.pages) || page < res.meta.pages);
            hasLoadedOnce = true;
        } catch (error) {
            if (!isCurrentCatalogRequest(requestGeneration) || !onlineSelected || fetchId !== latestOnlineFetchId) {
                return;
            }

            catalogError = `Failed to load the mod catalog: ${error}`;
        } finally {
            if (fetchId === latestOnlineFetchId && isCurrentCatalogRequest(requestGeneration) && onlineSelected) {
                isLoading = false;
            }
        }
    };

    function isCurrentCatalogRequest(generation: number) {
        return !isDestroyed && generation === onlineCatalogGeneration;
    }

    async function loadMore() {
        if (!onlineSelected || isLoading || !hasMorePages || catalogError || isDestroyed) return;
        await fetchData();
    }

    async function retryCatalog() {
        if (isLoading) return;
        if (onlineSelected) await fetchData();
        else await toggleInstalled();
    }

    onMount(async () => {
        setupModsLayoutObserver();
        ModDatabase.invalidateInstalledMods();
        
        await Promise.all([loadCategories(), reloadOnline(), refreshInstalledState()]);
        if (isDestroyed) return;

        await measureModsLayoutAfterTick();

    });

    onDestroy(() => {
        isDestroyed = true;
        debouncedReloadOnline.cancel();
        onlineCatalogGeneration += 1;
        detailDependencyLookupToken += 1;
        detailModLookupToken += 1;
        detailGeneration += 1;
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
        if (isDestroyed) return;
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
        if (onlineSelected && !isDestroyed) {
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
            onlineCatalogGeneration += 1;
            hasMorePages = false;
            isLoading = true;
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
        if (isDestroyed) return;
        debouncedReloadOnline.cancel();
        onlineSelected = true;
        installedSelected = false;
        onlineCatalogGeneration += 1;
        const generation = onlineCatalogGeneration;
        page = 0;
        filtered = [];
        hasMorePages = false;
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
        selectedSort = "updated";
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
        const generation = ++onlineCatalogGeneration;
        onlineSelected = false;
        installedSelected = true;
        isLoading = true;
        filtered = [];
        catalogError = "";
        hasLoadedOnce = false;
        try {
            const mods = await ModDatabase.getInstalledMods();
            if (!isCurrentCatalogRequest(generation) || !installedSelected) return;
            filtered = mods;
            hasLoadedOnce = true;
            catalogError = "";
        } catch (error) {
            if (!isCurrentCatalogRequest(generation) || !installedSelected) return;
            catalogError = `Failed to scan installed mods: ${error}`;
        } finally {
            if (isCurrentCatalogRequest(generation) && installedSelected) isLoading = false;
        }
        // page = 1;
        // filtered = [];
        // await fetchData();
        //await filter();
    }

    async function refreshInstalledState() {
        if (isDestroyed) return;
        isScanningInventory = true;
        inventoryReady = false;
        try {
            await ModDatabase.loadInstalledMods();
            if (isDestroyed) return;
            ModDatabase.initModList(filtered);
            filtered = [...filtered];
            if (selectedDetailMod) {
                ModDatabase.initModList([selectedDetailMod]);
                selectedDetailMod = { ...selectedDetailMod };
                resolveSelectedDetailDependencies(selectedDetailMod);
            }
            inventoryReady = true;
            installedInventoryWarning = "";
        } catch (error) {
            if (isDestroyed) return;
            ModDatabase.invalidateInstalledMods();
            installedInventoryWarning = `Installed mod scan failed: ${error}`;
            console.log(installedInventoryWarning);
        } finally {
            if (!isDestroyed) isScanningInventory = false;
        }
    }

    async function refreshMods() {
        await refreshInstalledState();
        if (isDestroyed || !inventoryReady) return;
        if (onlineSelected) {
            await toggleOnline();
            return;
        }

        await toggleInstalled();
    }

    async function refreshModsFromDetail() {
        const generation = detailGeneration;
        const selectedKey = selectedDetailMod ? modIdentityKey(selectedDetailMod) : "";
        await refreshMods();
        await tick();

        if (isDestroyed || generation !== detailGeneration || !selectedKey
            || !selectedDetailMod || modIdentityKey(selectedDetailMod) !== selectedKey) {
            return;
        }

        const updated = filtered.find(mod => modIdentityKey(mod) === selectedKey);
        if (updated) {
            selectedDetailMod = updated;
            resolveSelectedDetailDependencies(updated);
            void loadSelectedDetailMod(updated);
        }
    }

    function openModDetails(mod: Mod) {
        detailGeneration += 1;
        selectedDetailMod = mod;
        selectedDetailPreviewIndex = 0;
        selectedDetailLoadError = "";
        resolveSelectedDetailDependencies(mod);
        void loadSelectedDetailMod(mod);
    }

    function closeModDetails() {
        detailGeneration += 1;
        selectedDetailMod = null;
        selectedDetailIsLoading = false;
        selectedDetailLoadError = "";
        selectedDetailDependencyRows = [];
        detailDependencyLookupToken += 1;
        detailModLookupToken += 1;
    }

    async function loadSelectedDetailMod(mod: Mod) {
        const lookupKey = mod.mod_id || mod.slug || mod.name;
        const lookupToken = ++detailModLookupToken;
        selectedDetailIsLoading = true;
        selectedDetailLoadError = "";

        try {
            let detailMod = detailModCache.get(lookupKey);
            if (!detailMod) {
                detailMod = await ModDatabase.fetchMod(lookupKey) ?? undefined;
                if (detailMod) {
                    detailModCache.set(lookupKey, detailMod);
                }
            }

            if (lookupToken !== detailModLookupToken || !selectedDetailMod || modIdentityKey(selectedDetailMod) !== modIdentityKey(mod)) {
                return;
            }

            if (!detailMod) {
                selectedDetailLoadError = "Full author description was not returned by SOTF Mods.";
                return;
            }

            ModDatabase.initModList([detailMod]);
            selectedDetailMod = detailMod;
            resolveSelectedDetailDependencies(detailMod);
        } catch (error) {
            if (lookupToken !== detailModLookupToken) {
                return;
            }

            selectedDetailLoadError = `Full author description failed to load: ${error}`;
        } finally {
            if (lookupToken === detailModLookupToken) {
                selectedDetailIsLoading = false;
            }
        }
    }

    function cycleSelectedDetailPreview(direction: number) {
        if (selectedDetailPreviewUrls.length <= 1) {
            return;
        }

        selectedDetailPreviewIndex = (selectedDetailPreviewIndex + direction + selectedDetailPreviewUrls.length) % selectedDetailPreviewUrls.length;
    }

    function handleDetailImageError(event: Event) {
        const image = event.currentTarget instanceof HTMLImageElement ? event.currentTarget : null;
        if (!image || image.getAttribute("src") === SOTF_DETAIL_FALLBACK_IMAGE) {
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

    function detailVersions(mod: Mod): ModVersion[] {
        return [...(mod.versions ?? [])]
            .sort((left, right) => {
                if (left.isLatest !== right.isLatest) {
                    return left.isLatest ? -1 : 1;
                }

                return dateValue(right.createdAt ?? right.updatedAt) - dateValue(left.createdAt ?? left.updatedAt);
            })
            .slice(0, 5);
    }

    function dateValue(value?: string): number {
        if (!value) {
            return 0;
        }

        const time = new Date(value).getTime();
        return Number.isFinite(time) ? time : 0;
    }

    function detailVersionDate(version: ModVersion): string {
        return version.createdAt || version.updatedAt ? formatDetailDate(version.createdAt ?? version.updatedAt ?? "") : "-";
    }

    function detailVersionDownloads(version: ModVersion): string {
        const downloads = version._count?.downloads;
        return typeof downloads === "number" ? downloads.toLocaleString() : "-";
    }

    function detailVersionChangelogHtml(version: ModVersion): string {
        return renderStoreRichText(version.changelog, "No changelog text.");
    }

    function sortVisibleMods(mods: Mod[]): Mod[] {
        return [...mods].sort(compareModsBySort);
    }

    function compareModsBySort(left: Mod, right: Mod): number {
        let result = 0;

        if (selectedSort === "downloads") {
            result = numericModValue(right.downloads ?? right.lastWeekDownloads) - numericModValue(left.downloads ?? left.lastWeekDownloads);
        } else if (selectedSort === "favorites") {
            result = numericModValue(right.favoritesCount) - numericModValue(left.favoritesCount);
        } else if (selectedSort === "comments") {
            result = numericModValue(right.commentsCount) - numericModValue(left.commentsCount);
        } else if (selectedSort === "rating") {
            result = numericModValue(right.averageRating) - numericModValue(left.averageRating)
                || numericModValue(right.reviewsCount) - numericModValue(left.reviewsCount);
        } else if (selectedSort === "name") {
            result = left.name.localeCompare(right.name, undefined, { sensitivity: "base" });
        } else {
            result = dateValue(right.lastReleasedAt) - dateValue(left.lastReleasedAt);
        }

        return result || sortTieBreak(left, right);
    }

    function numericModValue(value?: number): number {
        return typeof value === "number" && Number.isFinite(value) ? value : 0;
    }

    function sortTieBreak(left: Mod, right: Mod): number {
        return left.name.localeCompare(right.name, undefined, { sensitivity: "base" });
    }

    function selectedSortLabel(): string {
        if (selectedSort === "downloads") {
            return "downloads";
        }

        if (selectedSort === "favorites") {
            return "favorites";
        }

        if (selectedSort === "comments") {
            return "comments";
        }

        if (selectedSort === "rating") {
            return "rating";
        }

        if (selectedSort === "name") {
            return "name";
        }

        return "recent updates";
    }

    function sortMetricLabel(sort: SotfCatalogSort): string {
        if (sort === "downloads") {
            return "Downloads";
        }

        if (sort === "favorites") {
            return "Favorites";
        }

        if (sort === "comments") {
            return "Comments";
        }

        if (sort === "rating") {
            return "Rating";
        }

        return "";
    }

    function rowSortMetricValue(mod: Mod, sort: SotfCatalogSort): string {
        if (sort === "downloads") {
            return detailCount(mod.downloads ?? mod.lastWeekDownloads);
        }

        if (sort === "favorites") {
            return detailCount(mod.favoritesCount);
        }

        if (sort === "comments") {
            return detailCount(mod.commentsCount);
        }

        if (sort === "rating") {
            return detailRatingLabel(mod.averageRating);
        }

        return "";
    }

    function sotfSummaryNote(): string {
        if (selectedSort !== "updated") {
            return `Sorted by ${selectedSortLabel()}`;
        }

        if (hasActiveLocalModFilters) {
            return "Source filters active";
        }

        if (filterTerm.trim().length > 0) {
            return embeddedStorePreview ? "Shared search" : "Search active";
        }

        return onlineSelected ? "Live store" : "Local inventory";
    }

    function detailCount(value?: number): string {
        return typeof value === "number" ? value.toLocaleString() : "-";
    }

    function detailRatingLabel(value?: number): string {
        if (typeof value !== "number" || value <= 0) {
            return "No rating";
        }

        return `${value.toFixed(1)} / 5`;
    }

    function trustedAuthorLabel(mod: Mod): string {
        return mod.user?.isTrusted ? "Trusted" : "Standard";
    }

    function normalizedSourceUrl(mod: Mod): string {
        const raw = mod.sourceUrl?.trim();
        if (!raw) {
            return "";
        }

        try {
            const url = new URL(raw);
            return url.protocol === "http:" || url.protocol === "https:" ? url.toString() : "";
        } catch {
            return "";
        }
    }

    function openSelectedDetailSource() {
        const url = selectedDetailMod ? normalizedSourceUrl(selectedDetailMod) : "";
        if (url) {
            window.open(url);
        }
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
    class:sotf-embedded-controls-needed={embeddedStorePreview && (embeddedControlsExpanded || installedSelected || hasActiveLocalModViewOptions)}
    bind:this={modsPageElement}
>
    {#if $isPathValid}
        <div class="mods-toolbar">
            <span class="catalog-heading">SOTF Mods</span>
            {#if showEmbeddedSearch}
                <input class="generic-input search-input" aria-label="Search SOTF Mods" placeholder="Search SOTF Mods" type="text" value={filterTerm} on:input={(event) => handleSearchInput((event.currentTarget as HTMLInputElement).value)} />
            {/if}
            <div class="mode-buttons" role="group" aria-label="SOTF Mods view">
                <button type="button" class="cat-btn" class:cat-btn-selected={onlineSelected} aria-pressed={onlineSelected} on:click={toggleOnline}>Online</button>
                <button type="button" class="cat-btn" class:cat-btn-selected={installedSelected} aria-pressed={installedSelected} on:click={toggleInstalled}>Installed</button>
            </div>
            <button type="button" class="refresh-small icon-text-button" disabled={isLoading || isScanningInventory} on:click={refreshMods} title="Refresh mods">
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
            <label>
                <span>Sort loaded mods</span>
                <select bind:value={selectedSort}>
                    <option value="updated">Recently updated</option>
                    <option value="downloads">Downloads</option>
                    <option value="favorites">Favorites</option>
                    <option value="comments">Comments</option>
                    <option value="rating">Rating</option>
                    <option value="name">Name</option>
                </select>
            </label>
            <button type="button" class="filter-clear" disabled={!hasActiveModViewOptions || isLoading} on:click={clearModFilters}>Reset filters</button>
        </div>

        <div class="mods-note">
            <span class="result-count"><strong>{visibleMods.length}</strong> shown <span>of {filtered.length} {onlineSelected ? "loaded" : "installed"}</span></span>
            {#if isScanningInventory}<span>Checking installed mods…</span>{/if}
            {#if selectedSort !== "updated"}
                <span>Sorted by {selectedSortLabel()}</span>
            {/if}
            {#if embeddedStorePreview && filterTerm.trim().length > 0 && !hasActiveLocalModViewOptions}
                <span class="active-filter-note">Shared search active</span>
            {:else if hasActiveModFilters}
                <span class="active-filter-note">Filters active</span>
            {/if}
        </div>

        {#if catalogError}
            <div class="catalog-error" role="alert">
                <span>{catalogError}</span>
                <button disabled={isLoading} on:click={retryCatalog}>Retry</button>
            </div>
        {/if}

        {#if installedInventoryWarning}
            <div class="catalog-error warning-note" role="status">
                <span>{installedInventoryWarning}</span>
                <button disabled={isScanningInventory} on:click={refreshMods}>Retry scan</button>
            </div>
        {/if}

        <div class="scroller" class:grid={isGrid} bind:this={catalogScroller}>
            {#each visibleMods as mod (mod.mod_id ?? mod.slug ?? mod.name)}
                <ModCard
                    mod={mod}
                    actionsDisabled={!inventoryReady}
                    sortMetricLabel={activeSortMetricLabel}
                    sortMetricValue={rowSortMetricValue(mod, selectedSort)}
                    on:details={(event) => openModDetails(event.detail)}
                    on:refreshMods={refreshMods}
                />
            {/each}

            {#if isLoading}
                <div class="catalog-loading" role="status">
                    <SvgSpinnersBlocksWave aria-hidden="true" />
                    <span>Loading mods…</span>
                </div>
            {:else if hasLoadedOnce && !catalogError && visibleMods.length === 0}
                <div class="catalog-empty" role="status">
                    <LucidePackage aria-hidden="true" />
                    <h2>{hasActiveModFilters ? "No matching mods" : installedSelected ? "No installed mods" : "No mods available"}</h2>
                    <p>{hasActiveModFilters ? "Try a different search or reset the filters." : installedSelected ? "Mods installed for this game will appear here." : "Refresh the catalog to check for available mods."}</p>
                    {#if hasActiveModViewOptions}
                        <button type="button" on:click={clearModFilters}>Reset filters</button>
                    {/if}
                </div>
            {/if}

            <InfiniteScroll
                hasMore={hasMorePages && onlineSelected && !catalogError}
                loading={isLoading}
                contentKey={`${visibleMods.length}:${isGrid}`}
                threshold={100}
                on:loadMore={loadMore} />
        </div>

        {#if selectedDetailMod}
            <div class="sotf-detail-backdrop">
                <button class="sotf-detail-backdrop-dismiss" type="button" tabindex="-1" aria-label="Close SOTF Mods details" on:click={closeModDetails}></button>
                <div class="sotf-detail-panel" role="dialog" aria-modal="true" aria-labelledby="sotf-detail-title" tabindex="-1" use:modalFocus={{ generation: detailGeneration, close: closeModDetails }}>
                    <header class="sotf-detail-header">
                        <div class="sotf-detail-title">
                            <h2 id="sotf-detail-title">{selectedDetailMod.name}</h2>
                            <small>{detailCategoryLabel(selectedDetailMod)} · {detailLoaderLabel(selectedDetailMod)} · {detailAuthorName(selectedDetailMod)}</small>
                        </div>
                        <button class="sotf-detail-close" type="button" aria-label="Close SOTF Mods details" on:click={closeModDetails}>
                            <LucideX aria-hidden="true" />
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
                                <span class="sotf-detail-section-title">About this mod</span>
                                {#if selectedDetailIsLoading}
                                    <small class="sotf-detail-description-note">Loading full author description…</small>
                                {:else if selectedDetailLoadError}
                                    <small class="sotf-detail-description-note sotf-detail-description-warning">{selectedDetailLoadError}</small>
                                {/if}
                                <div class="sotf-rich-text">
                                    {@html selectedDetailDescriptionHtml}
                                </div>
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
                                <span>Downloads <b>{detailCount(selectedDetailMod.downloads ?? selectedDetailMod.lastWeekDownloads)}</b></span>
                                <span>Author <b>{detailAuthorName(selectedDetailMod)}</b></span>
                            </div>

                            <div class="sotf-detail-community">
                                <span class="sotf-detail-section-title">Community</span>
                                <div class="sotf-detail-community-grid">
                                    <span>Comments <b>{detailCount(selectedDetailMod.commentsCount)}</b></span>
                                    <span>Favorites <b>{detailCount(selectedDetailMod.favoritesCount)}</b></span>
                                    <span>Reviews <b>{detailCount(selectedDetailMod.reviewsCount)}</b></span>
                                    <span>Rating <b>{detailRatingLabel(selectedDetailMod.averageRating)}</b></span>
                                    <span>Author Status <b>{trustedAuthorLabel(selectedDetailMod)}</b></span>
                                    <span>Store Page <b>SOTF Mods</b></span>
                                </div>
                                <div class="sotf-detail-community-actions">
                                    <button type="button" on:click={openSelectedDetailPage}>
                                        <LucideExternalLink aria-hidden="true" />
                                        <span>Open Page</span>
                                    </button>
                                    {#if normalizedSourceUrl(selectedDetailMod)}
                                        <button type="button" on:click={openSelectedDetailSource}>
                                            <LucideExternalLink aria-hidden="true" />
                                            <span>Open Source</span>
                                        </button>
                                    {/if}
                                </div>
                                <small>Counts are returned by SOTF Mods; discussions stay on the store page.</small>
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

                            <div class="sotf-detail-versions">
                                <span class="sotf-detail-section-title">Versions</span>
                                {#if selectedDetailVersions.length > 0}
                                    <div class="sotf-detail-version-list">
                                        {#each selectedDetailVersions as version}
                                            <div class="sotf-detail-version-row" class:sotf-detail-version-latest={version.isLatest}>
                                                <div class="sotf-detail-version-head">
                                                    <span>{version.version}</span>
                                                    <b>{version.isLatest ? "Latest" : detailVersionDate(version)}</b>
                                                </div>
                                                <small>
                                                    {detailVersionDate(version)} · {detailVersionDownloads(version)} downloads
                                                    {#if version.filename}
                                                        · {version.filename}
                                                    {/if}
                                                </small>
                                                <div class="sotf-version-rich-text">
                                                    {@html detailVersionChangelogHtml(version)}
                                                </div>
                                            </div>
                                        {/each}
                                        {#if selectedDetailHiddenVersionCount > 0}
                                            <small class="sotf-detail-version-more">+{selectedDetailHiddenVersionCount} older versions on SOTF Mods.</small>
                                        {/if}
                                    </div>
                                {:else}
                                    <small>No storefront version changelog records.</small>
                                {/if}
                            </div>
                        </aside>
                    </div>

                    <footer class="sotf-detail-footer">
                        <div class="sotf-detail-footer-copy">
                            <span class="sotf-detail-section-title">Installation</span>
                            <small>{detailInstallStateLabel(selectedDetailMod)} · {detailLoaderLabel(selectedDetailMod)}</small>
                        </div>
                        <div class="sotf-detail-footer-actions">
                            <ModCard
                                detailActionsOnly={true}
                                mod={selectedDetailMod}
                                actionsDisabled={!inventoryReady}
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
        <div class="catalog-empty installation-empty" role="status">
            <LucidePackage aria-hidden="true" />
            <h2>Choose your game installation</h2>
            <p>Set the game path in Overview to browse and manage your mods.</p>
        </div>
    {/if}
    
</div>

<style>
    .mods-page {
        --sotf-scroller-target-height: 320px;
        --sotf-thumb-height: 96px;
        --sotf-thumb-width: 172px;
        align-items: stretch;
        display: flex;
        flex-direction: column;
        gap: 12px;
        height: 100%;
        justify-content: flex-start;
        min-height: 0;
        min-width: 0;
        text-align: left;
        width: 100%;
    }
    .sotf-embedded-store-preview { gap: 8px; }
    .sotf-embedded-store-preview:not(.sotf-embedded-controls-needed) .mods-toolbar,
    .sotf-embedded-store-preview:not(.sotf-embedded-controls-needed) .filter-row { display: none; }
    .sotf-embedded-store-preview .catalog-heading { display: none; }
    .sotf-embedded-store-preview .mods-note { padding: 0 2px; }
    .sotf-embedded-store-preview .scroller { overscroll-behavior-y: auto; padding-bottom: 4px; }
    .sotf-embedded-store-preview :global(.description-content.header-desc) {
        display: -webkit-box;
        -webkit-box-orient: vertical;
        -webkit-line-clamp: 1;
        line-clamp: 1;
        overflow: hidden;
    }
    .sotf-embedded-store-preview :global(.fact-row),
    .sotf-embedded-store-preview :global(.meta-row) { display: none; }

    .mods-toolbar {
        align-items: center;
        display: flex;
        flex: 0 0 auto;
        flex-wrap: wrap;
        gap: 10px;
        min-width: 0;
        width: 100%;
    }
    .catalog-heading {
        color: var(--text-primary, #eef0f4);
        font-size: 15px;
        font-weight: 600;
        margin-right: auto;
    }
    .search-input { flex: 1 1 180px; font-size: 13px; margin: 0; min-width: 140px; }
    .mode-buttons {
        background: var(--surface-panel, #171a21);
        border: 1px solid var(--border-subtle, #2b303a);
        border-radius: 8px;
        display: flex;
        flex: 0 0 auto;
        gap: 2px;
        padding: 3px;
    }
    .cat-btn {
        background: transparent;
        border: 0;
        border-radius: 5px;
        box-shadow: none;
        color: var(--text-secondary, #9da6b6);
        font-size: 12px;
        font-weight: 550;
        height: 29px;
        margin: 0;
        min-height: 0;
        min-width: 76px;
        padding: 0 14px;
    }
    .cat-btn:hover { color: var(--text-primary, #eef0f4); }
    .cat-btn-selected { background: var(--surface-raised, #20242c); color: var(--text-primary, #eef0f4); }
    .refresh-small {
        align-items: center;
        background: transparent;
        border: 1px solid var(--border-subtle, #2b303a);
        border-radius: 8px;
        color: var(--text-secondary, #9da6b6);
        display: flex;
        flex: 0 0 auto;
        font-size: 12px;
        gap: 7px;
        height: 37px;
        justify-content: center;
        margin: 0;
        min-height: 0;
        padding: 0 12px;
    }
    .refresh-small:hover:not(:disabled) { background: var(--surface-raised, #20242c); color: var(--text-primary, #eef0f4); }
    .icon-text-button :global(svg) { height: 14px; width: 14px; }
    .filter-row {
        align-items: end;
        background: var(--surface-panel, #171a21);
        border: 1px solid var(--border-subtle, #2b303a);
        border-radius: 10px;
        box-sizing: border-box;
        display: grid;
        flex: 0 0 auto;
        gap: 12px;
        grid-template-columns: 1fr 0.85fr 1.15fr 1.1fr auto;
        margin: 0;
        padding: 12px;
        width: 100%;
    }
    .filter-row label { display: flex; flex-direction: column; gap: 7px; min-width: 0; text-align: left; }
    .filter-row label > span { color: var(--text-secondary, #9da6b6); font-size: 11px; font-weight: 500; line-height: 1.2; }
    .filter-row select {
        background: var(--surface-base, #101217);
        border: 1px solid var(--border-subtle, #2b303a);
        border-radius: 7px;
        box-sizing: border-box;
        color: var(--text-primary, #eef0f4);
        font-family: inherit;
        font-size: 12px;
        font-weight: 400;
        height: 35px;
        min-height: 0;
        min-width: 0;
        padding: 0 8px;
        text-overflow: ellipsis;
        width: 100%;
    }
    .filter-clear {
        background: transparent;
        border: 0;
        border-radius: 7px;
        color: var(--text-secondary, #9da6b6);
        font-size: 11px;
        height: 35px;
        margin: 0;
        min-height: 0;
        padding: 0 6px;
        white-space: nowrap;
    }
    .filter-clear:hover:not(:disabled) { background: var(--surface-raised, #20242c); color: var(--accent, #eeb66b); }
    .mods-note {
        align-items: center;
        color: var(--text-secondary, #9da6b6);
        display: flex;
        flex: 0 0 auto;
        flex-wrap: wrap;
        font-size: 11px;
        gap: 7px 14px;
        line-height: 1.4;
        min-height: 18px;
        padding: 0 2px;
    }
    .result-count { color: var(--text-primary, #eef0f4); margin-right: auto; }
    .result-count strong { font-weight: 600; }
    .result-count > span { color: var(--text-secondary, #9da6b6); margin-left: 3px; }
    .active-filter-note { color: var(--accent, #eeb66b); }
    .scroller {
        box-sizing: border-box;
        flex: 1 1 var(--sotf-scroller-target-height);
        height: var(--sotf-scroller-target-height);
        max-height: var(--sotf-scroller-target-height);
        min-height: 0;
        overflow-x: hidden;
        overflow-y: auto;
        overscroll-behavior: contain;
        padding-bottom: calc(8px + var(--app-bottom-safe-area, 12px));
        position: relative;
        scroll-padding-bottom: var(--app-bottom-safe-area, 12px);
        scrollbar-gutter: stable;
        width: 100%;
    }
    .grid { display: grid; gap: 12px; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); }
    .catalog-error {
        align-items: center;
        background: rgba(238, 182, 107, 0.06);
        border: 1px solid rgba(238, 182, 107, 0.22);
        border-radius: 9px;
        box-sizing: border-box;
        color: var(--accent, #eeb66b);
        display: flex;
        flex: 0 0 auto;
        font-size: 12px;
        gap: 16px;
        justify-content: space-between;
        line-height: 1.5;
        padding: 12px 14px;
        text-align: left;
        width: 100%;
    }
    .catalog-error > span { min-width: 0; overflow-wrap: anywhere; }
    .catalog-error button { flex: 0 0 auto; font-size: 12px; margin: 0; padding: 7px 12px; }
    .warning-note { color: #d7b887; }
    .catalog-loading {
        align-items: center;
        color: var(--text-secondary, #9da6b6);
        display: flex;
        font-size: 12px;
        gap: 12px;
        justify-content: center;
        min-height: 90px;
        padding: 16px;
    }
    .catalog-loading :global(svg) { color: var(--accent, #eeb66b); height: 24px; width: 24px; }
    .catalog-empty {
        align-items: center;
        box-sizing: border-box;
        display: flex;
        flex-direction: column;
        justify-content: center;
        min-height: 220px;
        padding: 28px;
        text-align: center;
    }
    .catalog-empty > :global(svg) { color: var(--accent, #eeb66b); height: 28px; margin-bottom: 14px; width: 28px; }
    .catalog-empty h2 { color: var(--text-primary, #eef0f4); font-size: 17px; font-weight: 600; margin: 0; }
    .catalog-empty p { color: var(--text-secondary, #9da6b6); font-size: 13px; line-height: 1.6; margin: 8px 0 0; max-width: 350px; }
    .catalog-empty button { font-size: 12px; margin: 18px 0 0; padding: 8px 14px; }
    .installation-empty { background: var(--surface-panel, #171a21); border: 1px dashed var(--border-subtle, #2b303a); border-radius: 12px; flex: 1 1 auto; }

    .sotf-detail-backdrop {
        align-items: center;
        background: rgba(5, 7, 11, 0.78);
        backdrop-filter: blur(8px);
        box-sizing: border-box;
        display: flex;
        inset: 0;
        justify-content: center;
        padding: 48px 24px 24px;
        position: fixed;
        z-index: 20;
    }
    .sotf-detail-backdrop-dismiss {
        background: transparent;
        border: 0;
        border-radius: 0;
        box-shadow: none;
        cursor: default;
        height: auto;
        inset: 0;
        margin: 0;
        min-width: 0;
        padding: 0;
        position: absolute;
        width: auto;
    }
    .sotf-detail-panel {
        background: var(--surface-base, #101217);
        border: 1px solid #353b46;
        border-radius: 12px;
        box-shadow: 0 24px 80px rgba(0, 0, 0, 0.5);
        box-sizing: border-box;
        color: var(--text-primary, #eef0f4);
        display: flex;
        flex-direction: column;
        gap: 18px;
        height: min(820px, calc(100vh - 72px));
        max-height: calc(100vh - 72px);
        max-width: calc(100vw - 48px);
        min-height: 0;
        min-width: 0;
        overflow: hidden;
        padding: 22px;
        position: relative;
        text-align: left;
        width: 1180px;
        z-index: 1;
    }
    .sotf-detail-header { align-items: flex-start; display: flex; flex: 0 0 auto; gap: 20px; justify-content: space-between; min-width: 0; }
    .sotf-detail-title { display: flex; flex: 1 1 auto; flex-direction: column; gap: 7px; min-width: 0; }
    .sotf-detail-title h2 { color: var(--text-primary, #eef0f4); font-size: 25px; font-weight: 650; letter-spacing: -0.025em; line-height: 1.2; margin: 0; overflow-wrap: anywhere; }
    .sotf-detail-title small { color: var(--text-secondary, #9da6b6); font-size: 12px; line-height: 1.4; overflow-wrap: anywhere; }
    .sotf-detail-close,
    .sotf-detail-open-page { align-items: center; display: inline-flex; flex: 0 0 auto; gap: 7px; justify-content: center; margin: 0; }
    .sotf-detail-close { background: var(--surface-raised, #20242c); border: 1px solid var(--border-subtle, #2b303a); border-radius: 8px; color: var(--text-secondary, #9da6b6); height: 34px; min-height: 0; min-width: 34px; padding: 0; width: 34px; }
    .sotf-detail-close:hover { color: var(--text-primary, #eef0f4); }
    .sotf-detail-close :global(svg),
    .sotf-detail-open-page :global(svg) { flex: 0 0 auto; height: 16px; width: 16px; }
    .sotf-detail-grid { display: grid; flex: 1 1 auto; gap: 20px; grid-template-columns: minmax(0, 1.5fr) minmax(290px, 1fr); min-height: 0; min-width: 0; overflow: hidden; }
    .sotf-detail-main,
    .sotf-detail-side { display: flex; flex-direction: column; gap: 16px; min-height: 0; min-width: 0; overflow-y: auto; padding-right: 3px; scrollbar-gutter: stable; }
    .sotf-detail-media { background: var(--surface-panel, #171a21); border: 1px solid var(--border-subtle, #2b303a); border-radius: 10px; flex: 0 0 auto; height: clamp(190px, 28vh, 300px); min-height: 0; overflow: hidden; position: relative; }
    .sotf-detail-media img { display: block; height: 100%; object-fit: contain; width: 100%; }
    .sotf-detail-preview-count,
    .sotf-detail-preview-controls { align-items: center; display: flex; gap: 5px; position: absolute; z-index: 2; }
    .sotf-detail-preview-count { background: rgba(16, 18, 23, 0.82); border: 1px solid rgba(255, 255, 255, 0.16); border-radius: 6px; color: #eef0f4; font-size: 11px; left: 12px; padding: 6px 8px; top: 12px; }
    .sotf-detail-preview-count :global(svg) { height: 13px; width: 13px; }
    .sotf-detail-preview-fallback { color: #bcc3cf; }
    .sotf-detail-preview-controls { bottom: 12px; right: 12px; }
    .sotf-detail-preview-controls button { align-items: center; background: rgba(16, 18, 23, 0.88); border: 1px solid rgba(255, 255, 255, 0.18); border-radius: 7px; color: #eef0f4; display: inline-flex; height: 32px; justify-content: center; margin: 0; min-height: 0; min-width: 32px; padding: 0; width: 32px; }
    .sotf-detail-preview-controls button:hover { border-color: var(--accent, #eeb66b); color: var(--accent, #eeb66b); }
    .sotf-detail-preview-controls :global(svg) { height: 16px; width: 16px; }
    .sotf-detail-description { flex: 0 0 auto; min-height: 0; min-width: 0; padding: 2px 1px 12px; }
    .sotf-detail-section-title { color: var(--text-primary, #eef0f4); font-size: 13px; font-weight: 600; line-height: 1.4; }
    .sotf-rich-text { color: #bec5d1; font-size: 13px; line-height: 1.65; margin: 12px 0 0; min-width: 0; overflow-wrap: anywhere; }
    .sotf-detail-description-note { color: var(--text-secondary, #9da6b6); display: block; font-size: 12px; line-height: 1.5; margin-top: 8px; }
    .sotf-detail-description-warning { color: var(--accent, #eeb66b); }
    .sotf-rich-text :global(p),
    .sotf-rich-text :global(ul),
    .sotf-rich-text :global(ol),
    .sotf-rich-text :global(blockquote),
    .sotf-rich-text :global(pre),
    .sotf-rich-text :global(.store-rich-table-wrap),
    .sotf-rich-text :global(.store-rich-disclosure),
    .sotf-rich-text :global(.store-rich-callout),
    .sotf-rich-text :global(.store-rich-definition-list) {
        margin: 0.62em 0 0;
    }

    .sotf-rich-text :global(p:first-child),
    .sotf-rich-text :global(ul:first-child),
    .sotf-rich-text :global(ol:first-child),
    .sotf-rich-text :global(blockquote:first-child),
    .sotf-rich-text :global(pre:first-child),
    .sotf-rich-text :global(.store-rich-table-wrap:first-child),
    .sotf-rich-text :global(.store-rich-disclosure:first-child),
    .sotf-rich-text :global(.store-rich-callout:first-child),
    .sotf-rich-text :global(.store-rich-definition-list:first-child) {
        margin-top: 0;
    }

    .sotf-rich-text :global(p) {
        color: inherit;
        line-height: inherit;
        margin-bottom: 0;
        overflow-wrap: anywhere;
    }

    .sotf-rich-text :global(a) {
        color: var(--accent, #eeb66b);
        font-weight: 550;
        overflow-wrap: anywhere;
        text-decoration: underline;
        text-decoration-thickness: 1px;
        text-underline-offset: 0.18em;
    }

    .sotf-rich-text :global(ul),
    .sotf-rich-text :global(ol) {
        box-sizing: border-box;
        padding-left: 1.35em;
    }

    .sotf-rich-text :global(li) {
        margin: 0.18em 0;
        padding-left: 0.08em;
    }

    .sotf-rich-text :global(blockquote) {
        border-left: 3px solid rgba(238, 182, 107, 0.45);
        color: #dce7ee;
        padding: 0.25em 0 0.25em 0.75em;
    }

    .sotf-rich-text :global(pre) {
        background: rgba(5, 7, 9, 0.76);
        border: 1px solid rgba(255, 255, 255, 0.12);
        box-sizing: border-box;
        color: var(--text-primary, #eef0f4);
        font-size: 0.86em;
        line-height: 1.4;
        max-width: 100%;
        overflow-x: auto;
        padding: 0.75em;
        white-space: pre-wrap;
    }

    .sotf-rich-text :global(code) {
        background: rgba(255, 255, 255, 0.08);
        border-radius: 4px;
        color: var(--text-primary, #eef0f4);
        font-family: Consolas, "Liberation Mono", monospace;
        font-size: 0.92em;
        padding: 0.05em 0.28em;
    }

    .sotf-rich-text :global(pre code) {
        background: transparent;
        border-radius: 0;
        display: block;
        padding: 0;
    }

    .sotf-rich-text :global(.store-rich-heading) {
        color: var(--text-primary, #eef0f4);
        font-weight: 600;
        letter-spacing: 0;
        line-height: 1.18;
        margin-top: 0.9em;
        text-transform: none;
    }

    .sotf-rich-text :global(.store-rich-heading-1),
    .sotf-rich-text :global(.store-rich-heading-2) {
        font-size: 1.08em;
    }

    .sotf-rich-text :global(.store-rich-heading-3),
    .sotf-rich-text :global(.store-rich-heading-4),
    .sotf-rich-text :global(.store-rich-heading-5),
    .sotf-rich-text :global(.store-rich-heading-6) {
        font-size: 0.98em;
    }

    .sotf-rich-text :global(.store-rich-align-center) {
        text-align: center;
    }

    .sotf-rich-text :global(.store-rich-align-right) {
        text-align: right;
    }

    .sotf-rich-text :global(.store-rich-align-justify) {
        text-align: justify;
    }

    .sotf-rich-text :global(.store-rich-indent) {
        border-left: 2px solid rgba(255, 255, 255, 0.1);
        margin-top: 0.62em;
        padding-left: 0.85em;
    }

    .sotf-rich-text :global(.store-rich-indent-2) {
        margin-left: 0.6em;
    }

    .sotf-rich-text :global(.store-rich-indent-3) {
        margin-left: 1.1em;
    }

    .sotf-rich-text :global(.store-rich-indent-4) {
        margin-left: 1.6em;
    }

    .sotf-rich-text :global(.store-rich-highlight) {
        border-radius: 3px;
        box-decoration-break: clone;
        color: #101418;
        padding: 0 0.2em;
    }

    .sotf-rich-text :global(.store-rich-font-mono) {
        font-family: Consolas, "Liberation Mono", monospace;
    }

    .sotf-rich-text :global(.store-rich-font-serif) {
        font-family: Georgia, "Times New Roman", serif;
    }

    .sotf-rich-text :global(.store-rich-font-sans) {
        font-family: Arial, Helvetica, sans-serif;
    }

    .sotf-rich-text :global(.store-rich-big) {
        font-size: 1.14em;
    }

    .sotf-rich-text :global(.store-rich-media) {
        display: block;
        margin: 0.65em 0;
        max-width: 100%;
    }

    .sotf-rich-text :global(.store-rich-media img) {
        border: 1px solid rgba(255, 255, 255, 0.12);
        display: block;
        height: auto;
        max-height: 360px;
        max-width: 100%;
        object-fit: contain;
    }

    .sotf-rich-text :global(.store-rich-media-link) {
        align-items: center;
        background: var(--accent-soft, rgba(238, 182, 107, 0.1));
        border: 1px solid rgba(238, 182, 107, 0.2);
        color: var(--accent, #eeb66b);
        display: inline-flex;
        font-weight: 600;
        margin-top: 0.42em;
        max-width: 100%;
        overflow-wrap: anywhere;
        padding: 0.34em 0.52em;
        text-decoration: none;
    }

    .sotf-rich-text :global(.store-rich-callout) {
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.12);
        box-sizing: border-box;
        color: #dce7ee;
        padding: 0.62em 0.72em;
    }

    .sotf-rich-text :global(.store-rich-callout > span) {
        color: var(--text-primary, #eef0f4);
        display: block;
        font-size: 0.76em;
        font-weight: 600;
        margin-bottom: 0.32em;
        text-transform: none;
    }

    .sotf-rich-text :global(.store-rich-callout-note) {
        border-left: 3px solid rgba(238, 182, 107, 0.45);
    }

    .sotf-rich-text :global(.store-rich-callout-tip) {
        border-left: 3px solid rgba(238, 182, 107, 0.45);
    }

    .sotf-rich-text :global(.store-rich-callout-warning) {
        border-left: 3px solid rgba(255, 192, 92, 0.58);
    }

    .sotf-rich-text :global(.store-rich-callout-box) {
        border-left: 3px solid rgba(255, 255, 255, 0.22);
    }

    .sotf-rich-text :global(.store-rich-disclosure) {
        background: rgba(255, 255, 255, 0.045);
        border: 1px solid rgba(255, 255, 255, 0.12);
        box-sizing: border-box;
        padding: 0.52em 0.64em;
    }

    .sotf-rich-text :global(.store-rich-disclosure-spoiler) {
        border-color: rgba(255, 192, 92, 0.28);
    }

    .sotf-rich-text :global(.store-rich-disclosure > summary) {
        color: var(--text-primary, #eef0f4);
        cursor: pointer;
        font-size: 0.82em;
        font-weight: 600;
        list-style: none;
        text-transform: none;
    }

    .sotf-rich-text :global(.store-rich-disclosure > summary::-webkit-details-marker) {
        display: none;
    }

    .sotf-rich-text :global(.store-rich-disclosure > summary::before) {
        color: var(--accent, #eeb66b);
        content: "+";
        display: inline-block;
        font-weight: 600;
        margin-right: 0.45em;
        width: 0.8em;
    }

    .sotf-rich-text :global(.store-rich-disclosure[open] > summary::before) {
        content: "-";
    }

    .sotf-rich-text :global(.store-rich-disclosure > div) {
        margin-top: 0.45em;
    }

    .sotf-rich-text :global(.store-rich-definition-list) {
        display: grid;
        gap: 0.42em;
    }

    .sotf-rich-text :global(.store-rich-definition-list > div) {
        background: rgba(255, 255, 255, 0.04);
        border: 1px solid rgba(255, 255, 255, 0.1);
        box-sizing: border-box;
        padding: 0.5em 0.6em;
    }

    .sotf-rich-text :global(.store-rich-definition-list dt) {
        color: var(--text-primary, #eef0f4);
        font-weight: 600;
    }

    .sotf-rich-text :global(.store-rich-definition-list dd) {
        margin: 0.24em 0 0;
    }

    .sotf-rich-text :global(.store-rich-table-wrap) {
        max-width: 100%;
        overflow-x: auto;
    }

    .sotf-rich-text :global(table) {
        border-collapse: collapse;
        min-width: min(100%, 28em);
        width: 100%;
    }

    .sotf-rich-text :global(td),
    .sotf-rich-text :global(th) {
        border: 1px solid rgba(255, 255, 255, 0.14);
        padding: 0.42em 0.55em;
        text-align: left;
        vertical-align: top;
    }

    .sotf-rich-text :global(th) {
        color: var(--text-primary, #eef0f4);
        font-weight: 600;
    }

    .sotf-rich-text :global(hr) {
        border: 0;
        border-top: 1px solid rgba(255, 255, 255, 0.16);
        margin: 0.85em 0;
    }

    .sotf-detail-facts,
    .sotf-detail-community,
    .sotf-detail-install-target,
    .sotf-detail-dependencies,
    .sotf-detail-versions {
        background: var(--surface-panel, #171a21);
        border: 1px solid var(--border-subtle, #2b303a);
        border-radius: 10px;
        box-sizing: border-box;
        display: flex;
        flex: 0 0 auto;
        flex-direction: column;
        gap: 12px;
        min-width: 0;
        padding: 15px;
    }
    .sotf-detail-facts { display: grid; gap: 15px 18px; grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .sotf-detail-facts span,
    .sotf-detail-community-grid span,
    .sotf-detail-install-target span { color: var(--text-secondary, #9da6b6); font-size: 11px; line-height: 1.5; min-width: 0; overflow-wrap: anywhere; }
    .sotf-detail-facts b,
    .sotf-detail-community-grid b,
    .sotf-detail-install-target b { color: var(--text-primary, #eef0f4); display: block; font-size: 12px; font-weight: 500; margin-top: 3px; min-width: 0; overflow-wrap: anywhere; }
    .sotf-detail-community-grid { display: grid; gap: 14px 10px; grid-template-columns: repeat(3, minmax(0, 1fr)); min-width: 0; }
    .sotf-detail-community-actions,
    .sotf-detail-dependency-actions { display: flex; flex-wrap: wrap; gap: 7px; min-width: 0; }
    .sotf-detail-community-actions button,
    .sotf-detail-dependency-actions button { align-items: center; background: var(--surface-raised, #20242c); border: 1px solid var(--border-subtle, #2b303a); border-radius: 7px; color: var(--text-primary, #eef0f4); display: inline-flex; flex: 1 1 auto; font-size: 11px; gap: 6px; justify-content: center; margin: 0; min-height: 32px; min-width: 0; padding: 6px 9px; }
    .sotf-detail-community-actions :global(svg),
    .sotf-detail-dependency-actions :global(svg) { flex: 0 0 auto; height: 13px; width: 13px; }
    .sotf-detail-community small,
    .sotf-detail-dependencies > small,
    .sotf-detail-versions > small { color: var(--text-secondary, #9da6b6); font-size: 11px; line-height: 1.55; overflow-wrap: anywhere; }
    .sotf-detail-install-target > div { display: grid; gap: 12px; grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .sotf-detail-dependency-list,
    .sotf-detail-version-list { display: grid; gap: 9px; min-width: 0; }
    .sotf-detail-dependency-row,
    .sotf-detail-version-row { background: var(--surface-base, #101217); border: 1px solid var(--border-subtle, #2b303a); border-radius: 7px; display: grid; gap: 8px; min-width: 0; padding: 11px; }
    .sotf-detail-dependency-installed,
    .sotf-detail-version-latest { border-color: rgba(136, 183, 155, 0.3); }
    .sotf-detail-dependency-available { border-color: #3a4352; }
    .sotf-detail-dependency-review { border-color: rgba(238, 182, 107, 0.3); }
    .sotf-detail-dependency-head,
    .sotf-detail-version-head { align-items: flex-start; display: flex; gap: 12px; justify-content: space-between; min-width: 0; }
    .sotf-detail-dependency-head span,
    .sotf-detail-version-head span { color: var(--text-primary, #eef0f4); font-size: 12px; font-weight: 550; line-height: 1.4; min-width: 0; overflow-wrap: anywhere; }
    .sotf-detail-dependency-head b,
    .sotf-detail-version-head b { color: #9dc7ab; flex: 0 0 auto; font-size: 10px; font-weight: 500; line-height: 1.6; }
    .sotf-detail-dependency-review .sotf-detail-dependency-head b { color: var(--accent, #eeb66b); }
    .sotf-detail-dependency-row small,
    .sotf-detail-version-row small,
    .sotf-detail-version-more { color: var(--text-secondary, #9da6b6); font-size: 11px; line-height: 1.5; overflow-wrap: anywhere; }
    .sotf-version-rich-text { color: #bec5d1; font-size: 12px; line-height: 1.55; min-width: 0; overflow-wrap: anywhere; }
    .sotf-version-rich-text :global(p),
    .sotf-version-rich-text :global(ul),
    .sotf-version-rich-text :global(ol) { margin: 6px 0 0; }
    .sotf-version-rich-text :global(p:first-child),
    .sotf-version-rich-text :global(ul:first-child),
    .sotf-version-rich-text :global(ol:first-child) { margin-top: 0; }
    .sotf-version-rich-text :global(ul),
    .sotf-version-rich-text :global(ol) { padding-left: 18px; }
    .sotf-version-rich-text :global(a) { color: var(--accent, #eeb66b); overflow-wrap: anywhere; }
    .sotf-version-rich-text :global(code) { background: var(--surface-raised, #20242c); border-radius: 4px; color: var(--text-primary, #eef0f4); font-family: Consolas, "Liberation Mono", monospace; padding: 2px 4px; }
    .sotf-detail-footer { align-items: center; border-top: 1px solid var(--border-subtle, #2b303a); display: flex; flex: 0 0 auto; gap: 18px; justify-content: space-between; min-width: 0; padding-top: 16px; }
    .sotf-detail-footer-copy { display: flex; flex: 0 1 auto; flex-direction: column; gap: 4px; min-width: 0; }
    .sotf-detail-footer-copy small { color: var(--text-secondary, #9da6b6); font-size: 11px; line-height: 1.5; overflow-wrap: anywhere; }
    .sotf-detail-footer-actions { align-items: center; display: flex; flex: 1 1 auto; gap: 10px; justify-content: flex-end; min-width: 0; }
    .sotf-detail-open-page { background: var(--surface-raised, #20242c); border: 1px solid var(--border-subtle, #2b303a); border-radius: 8px; color: var(--text-primary, #eef0f4); font-size: 12px; min-height: 36px; padding: 8px 12px; }

    @media (max-width: 1100px) {
        .filter-row { gap: 8px; padding: 10px; }
        .filter-row select { font-size: 11px; padding-left: 7px; }
        .sotf-detail-panel { gap: 16px; padding: 18px; }
        .sotf-detail-grid { gap: 16px; grid-template-columns: minmax(0, 1.35fr) minmax(280px, 1fr); }
        .sotf-detail-community-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    }
    @media (max-height: 720px) {
        .mods-page { gap: 9px; }
        .sotf-embedded-store-preview { gap: 7px; }
        .sotf-detail-panel { gap: 14px; padding: 18px; }
        .sotf-detail-media { height: 185px; }
        .sotf-detail-footer { padding-top: 12px; }
    }
    @media (max-width: 920px) {
        .sotf-detail-panel { overflow-y: auto; }
        .sotf-detail-grid { flex: 1 0 auto; grid-template-columns: 1fr; overflow: visible; }
        .sotf-detail-main,
        .sotf-detail-side { overflow: visible; padding-right: 0; }
        .sotf-detail-media { height: 270px; }
        .sotf-detail-footer { align-items: stretch; flex-direction: column; }
        .sotf-detail-footer-actions { justify-content: stretch; }
    }
    @media (max-width: 780px) {
        .catalog-heading { flex-basis: 100%; }
        .filter-row { grid-template-columns: repeat(2, minmax(0, 1fr)); }
        .filter-clear { justify-self: start; }
        .search-input { flex-basis: 100%; }
        .sotf-detail-backdrop { padding: 44px 12px 12px; }
        .sotf-detail-panel { height: calc(100vh - 56px); max-height: calc(100vh - 56px); max-width: calc(100vw - 24px); padding: 16px; }
        .sotf-detail-title h2 { font-size: 22px; }
        .sotf-detail-footer-actions { flex-wrap: wrap; }
    }
</style>
