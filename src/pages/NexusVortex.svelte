<script lang="ts">
    import { createEventDispatcher, onDestroy, onMount, tick } from "svelte";
    import semver from "semver";
    import SvgSpinnersBlocksWave from "~icons/svg-spinners/blocks-wave";
    import LucideChevronLeft from "~icons/lucide/chevron-left";
    import LucideChevronRight from "~icons/lucide/chevron-right";
    import LucideImages from "~icons/lucide/images";
    import nexusFallbackImage from "../assets/sons-ui/blurred-title-screen-texture2d-14.png";
    import {
        clearNexusApiKey,
        endorseNexusSotfMod,
        fetchNexusFileDependencies,
        fetchNexusModChangelogs,
        fetchNexusModDetails,
        fetchNexusModFiles,
        fetchNexusSotfMods,
        fetchNexusUserEndorsements,
        fetchNexusUserTrackedMods,
        getNexusNxmUrl,
        getNexusModDownloadUrl,
        getNexusModPageUrl,
        getNexusSession,
        NEXUS_CACHE_TTL_MINUTES,
        pickRecommendedNexusFile,
        saveNexusApiKey,
        trackNexusSotfMod,
        untrackNexusSotfMod,
        type NexusCategory,
        type NexusEndorsement,
        type NexusModChangelog,
        type NexusModDependency,
        type NexusModFile,
        type NexusMod,
        type NexusSession,
        type NexusTrackedMod,
        type NexusView
    } from "../lib/nexus";
    import {
        describeInstallSource,
        findMatchingInstall,
        normalizeMatchKey,
        readVortexDeployment,
        scanInstalledInventory,
        setInventoryEntryEnabled,
        type InstalledInventoryEntry
    } from "../lib/modInventory";
    import { getDirectoryPath, isPathValid } from "../lib/store";
    import * as dialog from "@tauri-apps/plugin-dialog"
    import * as shell from "@tauri-apps/plugin-shell"
    import { Command } from "@tauri-apps/plugin-shell";

    export let sharedSearchTerm = "";
    export let sharedSearchVersion = 0;
    export let showEmbeddedSearch = true;

    const dispatch = createEventDispatcher<{ searchChange: string }>();

    type CatalogMode = "online" | "installed";
    type InstallFilter = "all" | "attention" | "installed" | "missing" | "updates" | "disabled" | "vortex" | "native" | "manual" | "tracked" | "endorsements" | "conflicts";
    type ModTypeFilter = "all" | "bepinex-plugin" | "redloader-mod" | "redloader-library" | "vortex" | "native" | "manual";
    type NexusSortMode = "attention" | "updated" | "downloads" | "endorsements" | "name" | "version";
    type InstalledSortMode = "attention" | "name" | "source" | "location" | "state" | "version";
    type DependencyStatus = "installed" | "missing" | "version-mismatch" | "review";
    type InstallPlanTone = "ready" | "review" | "blocked";
    type UpdateTone = "update" | "current" | "tracked" | "review" | "neutral";
    type InstallPlacement = "auto" | "bepinex-plugin" | "redloader-mod" | "redloader-library" | "manual-review";
    type VersionComparison = "same" | "remote-newer" | "local-newer" | "different" | "unknown";
    type UpdateVerdict = {
        label: string;
        tone: UpdateTone;
        isUpdate: boolean;
        needsReview: boolean;
        reason: string;
    };
    type NexusUiPreferences = {
        catalogMode: CatalogMode;
        nexusSearchTerm: string;
        selectedNexusCategory: string;
        selectedInstallFilter: InstallFilter;
        selectedModTypeFilter: ModTypeFilter;
        selectedNexusSort: NexusSortMode;
        selectedInstalledSort: InstalledSortMode;
    };
    type ResolvedDependency = NexusModDependency & {
        match: InstalledInventoryEntry | null;
        status: DependencyStatus;
    };
    type NestedDependencySource = {
        key: string;
        parentFileId: number;
        parentNexusFileId?: string;
        parentName: string;
        depth: number;
        dependency: NexusModDependency;
    };
    type ResolvedNestedDependency = Omit<NestedDependencySource, "dependency"> & {
        dependency: ResolvedDependency;
    };
    type LocalConflict = {
        key: string;
        label: string;
        entries: InstalledInventoryEntry[];
    };
    type InstallPlan = {
        action: string;
        target: string;
        placement: string;
        tone: InstallPlanTone;
        notes: string[];
    };
    type AuthorRequirementLink = {
        key: string;
        label: string;
        url: string;
        source: string;
        mod_id?: number;
    };

    let session: NexusSession = { is_connected: false };
    let apiKey = "";
    let mods: NexusMod[] = [];
    let knownNexusDetails: Record<number, NexusMod> = {};
    let catalogPreviewIndexes: Record<number, number> = {};
    let selectedDetailPreviewIndex = 0;
    let detailDescriptionExpanded = false;
    let currentDetailPreviewUrls: string[] = [];
    let currentDetailPreviewIndex = 0;
    let selectedDetailDescription = "";
    let selectedDetailDescriptionHtml = "";
    let selectedDetailDescriptionCanToggle = false;
    let selectedFileDescriptionHtml = "";
    let selectedFileChangelogHtml = "";
    let selectedFileNotesAvailable = false;
    let nexusCategories: NexusCategory[] = [];
    let endorsements: NexusEndorsement[] = [];
    let endorsementsLoaded = false;
    let trackedMods: NexusTrackedMod[] = [];
    let trackedModsLoaded = false;
    let inventory: InstalledInventoryEntry[] = [];
    let selectedView: NexusView = "all";
    let catalogMode: CatalogMode = "online";
    let nexusSearchTerm = "";
    let lastAppliedSharedSearchVersion = 0;
    let selectedNexusCategory = "all";
    let selectedInstallFilter: InstallFilter = "all";
    let selectedModTypeFilter: ModTypeFilter = "all";
    let selectedNexusSort: NexusSortMode = "attention";
    let selectedInstalledSort: InstalledSortMode = "attention";
    let nexusCatalogLoadedAt: number | null = null;
    let nexusCategoryOptions: string[] = [];
    let visibleNexusMods: NexusMod[] = [];
    let visibleInstalledEntries: InstalledInventoryEntry[] = [];
    let updateCount = 0;
    let onlineAttentionCount = 0;
    let installedAttentionCount = 0;
    let endorsementQueueCount = 0;
    let trackedMissingCount = 0;
    let resolvedDependencies: ResolvedDependency[] = [];
    let hasActiveNexusFilters = false;
    let isLoading = false;
    let isDetailLoading = false;
    let status = "";
    let vortexStagingPath: string | null = null;
    let selectedMod: NexusMod | null = null;
    let selectedModDetails: NexusMod | null = null;
    let selectedModFiles: NexusModFile[] = [];
    let selectedFileId: number | null = null;
    let selectedDependencies: NexusModDependency[] = [];
    let selectedDependencyMessage = "";
    let selectedAuthorRequirements: AuthorRequirementLink[] = [];
    let nestedDependencySources: NestedDependencySource[] = [];
    let resolvedNestedDependencies: ResolvedNestedDependency[] = [];
    let isResolvingNestedDependencies = false;
    let nestedDependencySummary = "";
    let nestedDependencyRequestId = 0;
    let selectedChangelogs: NexusModChangelog[] = [];
    let detailBackStack: NexusMod[] = [];
    let selectedInstallMatch: InstalledInventoryEntry | null = null;
    let selectedInstallConflict: LocalConflict | null = null;
    let selectedInstallPlan: InstallPlan = {
        action: "Choose a file",
        target: "-",
        placement: "Auto",
        tone: "blocked",
        notes: ["Select a Nexus file before handing it to Vortex."]
    };
    let selectedInstallPlacement: InstallPlacement = "auto";
    let selectedInstallActionButtonLabel = "Choose File";
    let selectedNxmUrl = "";
    let lastSelectedNxmUrl = "";
    let nxmCopyState = "";
    let nxmCopyResetTimer: number | null = null;
    let activeNexusActionId: number | null = null;
    let activeNexusEndorseId: number | null = null;
    let activeNexusTrackId: number | null = null;
    let activeConflictEntryKey: string | null = null;
    let autoEndorseDownloadedMods = false;
    let autoEndorseAttemptedIds: number[] = [];
    let autoEndorseEligibleCount = 0;
    let autoEndorsePendingCount = 0;
    let autoEndorseAttemptedInstalledCount = 0;
    let autoEndorseQueueLabel = "Loading endorsement state";
    let ssoSocket: WebSocket | null = null;
    let ssoTimeout: number | null = null;
    let refreshCooldownSeconds = 0;
    let refreshCooldownTimer: number | null = null;
    let nexusUiPreferencesLoaded = false;
    let nexusPageElement: HTMLDivElement | null = null;
    let nexusLayoutObserver: ResizeObserver | null = null;
    let nexusLayoutFrame: number | null = null;
    let nexusWindowResizeHandler: (() => void) | null = null;
    let detailDescriptionSectionElement: HTMLDivElement | null = null;
    let detailFilesSectionElement: HTMLDivElement | null = null;
    let detailDependenciesSectionElement: HTMLDivElement | null = null;
    let detailInstallPlanSectionElement: HTMLDivElement | null = null;
    let detailChangelogSectionElement: HTMLDivElement | null = null;
    let detailFooterElement: HTMLDivElement | null = null;

    const NEXUS_SSO_URL = "wss://sso.nexusmods.com";
    const NEXUS_SSO_APPLICATION_SLUG = "openacai-mod-manager";
    const NEXUS_SSO_PROTOCOL = 2;
    const NEXUS_SSO_UUID_KEY = "openacai-nexus-sso-request-id";
    const NEXUS_SSO_TOKEN_KEY = "openacai-nexus-sso-connection-token";
    const NEXUS_UI_PREFERENCES_KEY = "openacai-nexus-ui-preferences";
    const NEXUS_AUTO_ENDORSE_KEY = "openacai-nexus-auto-endorse-downloaded";
    const NEXUS_AUTO_ENDORSE_ATTEMPTED_KEY = "openacai-nexus-auto-endorse-attempted";
    const MAX_AUTO_ENDORSE_PER_REFRESH = 3;
    const MAX_NESTED_DEPENDENCY_FILES = 8;
    const MAX_NESTED_DEPENDENCY_DEPTH = 2;
    const MAX_AUTHOR_REQUIREMENT_LINKS = 6;
    const NEXUS_MANUAL_REFRESH_COOLDOWN_MS = 60_000;
    const NEXUS_PLACEHOLDER_IMAGE = nexusFallbackImage;
    const NEXUS_DETAIL_PLACEHOLDER_IMAGE = nexusFallbackImage;
    const NEXUS_DESCRIPTION_FALLBACK = "No directions or description are available through the Nexus API for this mod. Open the Nexus page to review author instructions before installing.";
    const NEXUS_RICH_BB_TAGS = new Set(["b", "i", "u", "s", "strike", "del", "sub", "sup", "small", "big", "mark", "abbr", "acronym", "cite", "q", "url", "img", "image", "thumb", "thumbnail", "color", "background", "bgcolor", "highlight", "size", "center", "left", "right", "justify", "align", "indent", "code", "pre", "tt", "kbd", "samp", "var", "quote", "spoiler", "collapse", "details", "accordion", "accordionitem", "font", "heading", "h", "header", "title", "subtitle", "caption", "h1", "h2", "h3", "h4", "h5", "h6", "float", "clear", "anchor", "goto", "jump", "youtube", "video", "media", "embed", "columns", "cols", "column", "col", "nextcol", "tabs", "tab", "note", "info", "warning", "important", "tip", "box", "panel", "fieldset", "notice", "success", "danger", "error"]);
    const NEXUS_SAFE_COLOR_NAMES = new Set(["black", "white", "gray", "grey", "silver", "red", "maroon", "orange", "yellow", "olive", "lime", "green", "aqua", "cyan", "teal", "blue", "navy", "fuchsia", "magenta", "purple", "pink"]);
    const CATALOG_MODES: CatalogMode[] = ["online", "installed"];
    const INSTALL_FILTERS: InstallFilter[] = ["all", "attention", "installed", "missing", "updates", "disabled", "vortex", "native", "manual", "tracked", "endorsements", "conflicts"];
    const MOD_TYPE_FILTERS: ModTypeFilter[] = ["all", "bepinex-plugin", "redloader-mod", "redloader-library", "vortex", "native", "manual"];
    const INSTALL_PLACEMENTS: InstallPlacement[] = ["auto", "bepinex-plugin", "redloader-mod", "redloader-library", "manual-review"];
    const NEXUS_SORT_MODES: NexusSortMode[] = ["attention", "updated", "downloads", "endorsements", "name", "version"];
    const INSTALLED_SORT_MODES: InstalledSortMode[] = ["attention", "name", "source", "location", "state", "version"];
    let nextManualRefreshAt = 0;
    $: manualRefreshButtonLabel = refreshCooldownSeconds > 0 ? `Refresh ${refreshCooldownSeconds}s` : "Refresh";
    $: manualRefreshButtonTitle = refreshCooldownSeconds > 0
        ? `Manual Nexus refresh available in ${refreshCooldownSeconds}s`
        : "Refresh Nexus data";

    $: nexusCategoryOptions = buildNexusCategoryOptions(nexusCategories, mods);
    $: if (selectedNexusCategory !== "all" && nexusCategoryOptions.length > 0 && !nexusCategoryOptions.includes(selectedNexusCategory)) {
        selectedNexusCategory = "all";
    }
    $: installedCount = inventory.length;
    $: vortexCount = inventory.filter(entry => entry.installSource === "vortex").length;
    $: nativeCount = inventory.filter(entry => entry.installSource === "native").length;
    $: manualCount = inventory.filter(entry => entry.installSource === "manual").length;
    $: disabledCount = inventory.filter(entry => !entry.enabled).length;
    $: trackedCount = trackedMods.length;
    $: selectedNexusFile = selectedModFiles.find(file => file.file_id === selectedFileId) ?? null;
    $: recommendedNexusFileId = pickRecommendedNexusFile(selectedModFiles)?.file_id ?? null;
    $: displayedSelectedModFiles = sortNexusFilesForDisplay(selectedModFiles, recommendedNexusFileId);
    $: {
        selectedMod;
        selectedModDetails;
        selectedNexusFile;
        selectedChangelogs;
        currentDetailPreviewUrls = selectedDetailPreviewUrls();
    }
    $: {
        selectedDetailPreviewIndex;
        currentDetailPreviewUrls;
        currentDetailPreviewIndex = selectedDetailPreviewIndexValue(currentDetailPreviewUrls);
    }
    $: {
        selectedMod;
        selectedModDetails;
        selectedNexusFile;
        selectedChangelogs;
        selectedDetailDescription = selectedDetailDescriptionText();
        selectedDetailDescriptionHtml = selectedDetailDescriptionMarkup();
        selectedDetailDescriptionCanToggle = shouldOfferDetailDescriptionToggle(selectedDetailDescription, selectedDetailDescriptionSource());
        selectedAuthorRequirements = extractAuthorRequirementLinks(
            selectedAuthorRequirementSource(),
            selectedModDetails?.mod_id ?? selectedMod?.mod_id
        );
        if (!selectedDetailDescriptionCanToggle && detailDescriptionExpanded) {
            detailDescriptionExpanded = false;
        }
    }
    $: localConflicts = findLocalConflicts(inventory);
    $: localConflictEntryKeys = new Set(localConflicts.flatMap(conflict => conflict.entries.map(inventoryEntryKey)));
    $: conflictCount = localConflictEntryKeys.size;
    $: {
        mods;
        knownNexusDetails;
        updateCount = inventory.filter(hasInventoryUpdate).length;
    }
    $: {
        inventory;
        trackedMods;
        localConflictEntryKeys;
        onlineAttentionCount = mods.filter(nexusNeedsAttention).length;
    }
    $: {
        mods;
        knownNexusDetails;
        localConflictEntryKeys;
        installedAttentionCount = inventory.filter(inventoryNeedsAttention).length;
    }
    $: {
        inventory;
        endorsements;
        endorsementsLoaded;
        endorsementQueueCount = inventory.filter(inventoryEntryNeedsEndorsement).length;
    }
    $: {
        inventory;
        trackedMods;
        trackedMissingCount = trackedMods.filter(tracked => !inventoryHasNexusModId(tracked.mod_id)).length;
    }
    $: {
        nexusSearchTerm;
        selectedNexusCategory;
        selectedInstallFilter;
        selectedModTypeFilter;
        selectedNexusSort;
        inventory;
        trackedMods;
        localConflictEntryKeys;
        visibleNexusMods = sortNexusMods(mods.filter(matchesNexusFilters));
    }
    $: {
        nexusSearchTerm;
        selectedInstallFilter;
        selectedModTypeFilter;
        selectedInstalledSort;
        trackedMods;
        knownNexusDetails;
        localConflictEntryKeys;
        visibleInstalledEntries = sortInstalledEntries(inventory.filter(matchesInstalledFilters));
    }
    $: hasActiveNexusFilters = nexusSearchTerm.trim().length > 0
        || selectedNexusCategory !== "all"
        || selectedInstallFilter !== "all"
        || selectedModTypeFilter !== "all";
    $: if (sharedSearchVersion > 0 && sharedSearchVersion !== lastAppliedSharedSearchVersion) {
        lastAppliedSharedSearchVersion = sharedSearchVersion;
        applySharedSearch(sharedSearchTerm);
    }
    $: {
        inventory;
        resolvedDependencies = selectedDependencies.map(resolveDependencyStatus);
    }
    $: {
        inventory;
        nestedDependencySources;
        resolvedNestedDependencies = nestedDependencySources.map(source => ({
            ...source,
            dependency: resolveDependencyStatus(source.dependency)
        }));
    }
    $: {
        endorsements;
        endorsementsLoaded;
        autoEndorseAttemptedIds;
        const autoEndorseCandidates = inventory
            .map(entry => ({ entry, modId: numericNexusId(entry.nexusModId) }))
            .filter((candidate): candidate is { entry: InstalledInventoryEntry; modId: number } =>
                candidate.entry.installSource === "vortex" && !!candidate.modId
            );
        const unendorsedCandidates = autoEndorseCandidates.filter(candidate => !isNexusModEndorsed(candidate.modId));
        autoEndorseEligibleCount = autoEndorseCandidates.length;
        autoEndorsePendingCount = unendorsedCandidates.filter(candidate => !autoEndorseAttemptedIds.includes(candidate.modId)).length;
        autoEndorseAttemptedInstalledCount = unendorsedCandidates.filter(candidate => autoEndorseAttemptedIds.includes(candidate.modId)).length;
        autoEndorseQueueLabel = autoEndorseQueueSummary();
    }
    $: dependencyIssueCount = resolvedDependencies.filter(dependency =>
        dependency.status === "missing" || dependency.status === "version-mismatch"
    ).length;
    $: dependencyReviewCount = resolvedDependencies.filter(dependency => dependency.status === "review").length;
    $: dependencyInstalledCount = resolvedDependencies.filter(dependency => dependency.status === "installed").length;
    $: dependencyMissingCount = resolvedDependencies.filter(dependency => dependency.status === "missing").length;
    $: dependencyMismatchCount = resolvedDependencies.filter(dependency => dependency.status === "version-mismatch").length;
    $: nestedDependencyIssueCount = resolvedNestedDependencies.filter(source =>
        source.dependency.status === "missing" || source.dependency.status === "version-mismatch"
    ).length;
    $: nestedDependencyReviewCount = resolvedNestedDependencies.filter(source => source.dependency.status === "review").length;
    $: nestedDependencyInstalledCount = resolvedNestedDependencies.filter(source => source.dependency.status === "installed").length;
    $: nestedDependencyMissingCount = resolvedNestedDependencies.filter(source => source.dependency.status === "missing").length;
    $: nestedDependencyMismatchCount = resolvedNestedDependencies.filter(source => source.dependency.status === "version-mismatch").length;
    $: totalDependencyIssueCount = dependencyIssueCount + nestedDependencyIssueCount;
    $: totalDependencyReviewCount = dependencyReviewCount + nestedDependencyReviewCount;
    $: detailDependencyNavText = describeDetailDependencyNav(
        totalDependencyIssueCount,
        totalDependencyReviewCount,
        resolvedDependencies.length,
        resolvedNestedDependencies.length,
        selectedAuthorRequirements.length
    );
    $: selectedInstallMatch = selectedMod
        ? findMatchingInstall(inventory, selectedMod.name, selectedMod.mod_id, [
            selectedMod.author ?? "",
            selectedMod.uploaded_by ?? ""
        ])
        : null;
    $: {
        localConflicts;
        selectedInstallConflict = conflictForEntry(selectedInstallMatch);
    }
    $: selectedInstallPlan = buildInstallPlan(
        selectedModDetails ?? selectedMod,
        selectedNexusFile,
        selectedInstallMatch,
        selectedInstallConflict,
        resolvedDependencies,
        resolvedNestedDependencies,
        selectedAuthorRequirements,
        vortexStagingPath,
        selectedInstallPlacement
    );
    $: selectedInstallActionButtonLabel = selectedInstallPlan.tone === "blocked"
        ? "Choose File"
        : selectedInstallPlan.tone === "review"
            ? `${selectedInstallPlan.action} Anyway`
            : selectedInstallPlan.action;
    $: selectedInstallFileLabel = selectedNexusFile
        ? `${fileChoiceCategoryLabel(selectedNexusFile)} - ${selectedNexusFile.name}`
        : "No file selected";
    $: selectedFileVersionLabel = selectedNexusFile ? fileVersionLabel(selectedNexusFile) : "-";
    $: selectedFileUploadedLabel = selectedNexusFile
        ? formatTimestamp(selectedNexusFile.uploaded_timestamp, selectedNexusFile.uploaded_time)
        : "-";
    $: selectedFileSizeLabel = selectedNexusFile ? formatSizeKb(selectedNexusFile.size) : "-";
    $: selectedFileReviewCount = selectedModFiles.filter(isReviewNexusFile).length;
    $: selectedFileRecommendedCount = selectedModFiles.filter(file =>
        file.file_id === recommendedNexusFileId || file.is_primary || normalizedNexusFileCategory(file) === "main"
    ).length;
    $: fileChoiceReadinessText = describeFileChoiceReadiness(selectedModFiles.length, selectedFileRecommendedCount, selectedFileReviewCount, isDetailLoading);
    $: selectedFileReadinessText = describeSelectedFileReadiness(selectedNexusFile, recommendedNexusFileId, selectedModFiles.length);
    $: fileReviewReadinessText = describeFileReviewReadiness(selectedModFiles.length, selectedFileReviewCount);
    $: {
        selectedNexusFile;
        selectedFileDescriptionHtml = renderOptionalNexusRichText(selectedNexusFile?.description);
        selectedFileChangelogHtml = renderOptionalNexusRichText(selectedNexusFile?.changelog_html);
        selectedFileNotesAvailable = Boolean(selectedFileDescriptionHtml || selectedFileChangelogHtml);
    }
    $: selectedNxmUrl = selectedMod && selectedNexusFile ? getNexusNxmUrl(selectedMod, selectedNexusFile) : "";
    $: if (selectedNxmUrl !== lastSelectedNxmUrl) {
        lastSelectedNxmUrl = selectedNxmUrl;
        clearNxmCopyFeedback();
    }
    $: {
        catalogMode;
        nexusSearchTerm;
        selectedNexusCategory;
        selectedInstallFilter;
        selectedModTypeFilter;
        selectedNexusSort;
        selectedInstalledSort;
        if (nexusUiPreferencesLoaded) {
            persistNexusUiPreferences();
        }
    }
    $: {
        session.is_connected;
        session.rate_limit;
        status;
        catalogMode;
        selectedInstallFilter;
        visibleNexusMods.length;
        visibleInstalledEntries.length;
        localConflicts.length;
        selectedMod;
        selectedModFiles.length;
        selectedDependencies.length;
        resolvedNestedDependencies.length;
        void measureNexusLayoutAfterTick();
    }

    onMount(async () => {
        setupNexusLayoutObserver();
        loadNexusUiPreferences();
        if (sharedSearchVersion > 0) {
            applySharedSearch(sharedSearchTerm);
            lastAppliedSharedSearchVersion = sharedSearchVersion;
        }
        loadEndorsementPreferences();
        await refreshInventory();
        await refreshSession();

        if (session.is_connected) {
            await loadMods();
        }

        await measureNexusLayoutAfterTick();
    });

    onDestroy(() => {
        cleanupSso();
        cleanupManualRefreshCooldown();
        clearNxmCopyFeedback();
        cleanupNexusLayoutObserver();
    });

    function setupNexusLayoutObserver() {
        nexusWindowResizeHandler = () => scheduleNexusLayoutMeasure();
        window.addEventListener("resize", nexusWindowResizeHandler);

        if ("ResizeObserver" in window) {
            nexusLayoutObserver = new ResizeObserver(() => scheduleNexusLayoutMeasure());
            if (nexusPageElement) {
                nexusLayoutObserver.observe(nexusPageElement);
            }
        }

        scheduleNexusLayoutMeasure();
    }

    function cleanupNexusLayoutObserver() {
        if (nexusWindowResizeHandler) {
            window.removeEventListener("resize", nexusWindowResizeHandler);
            nexusWindowResizeHandler = null;
        }

        nexusLayoutObserver?.disconnect();
        nexusLayoutObserver = null;

        if (nexusLayoutFrame !== null) {
            window.cancelAnimationFrame(nexusLayoutFrame);
            nexusLayoutFrame = null;
        }
    }

    async function measureNexusLayoutAfterTick() {
        await tick();
        if (nexusLayoutObserver && nexusPageElement) {
            nexusLayoutObserver.disconnect();
            nexusLayoutObserver.observe(nexusPageElement);
        }
        scheduleNexusLayoutMeasure();
    }

    function scheduleNexusLayoutMeasure() {
        if (nexusLayoutFrame !== null) {
            return;
        }

        nexusLayoutFrame = window.requestAnimationFrame(() => {
            nexusLayoutFrame = null;
            measureNexusLayout();
        });
    }

    function measureNexusLayout() {
        const page = nexusPageElement;
        if (!page) {
            return;
        }

        const catalogPanel = page.querySelector<HTMLElement>(".catalog-panel");
        const scroller = page.querySelector<HTMLElement>(".nexus-scroller");
        if (!catalogPanel || !scroller) {
            return;
        }

        const pageRect = page.getBoundingClientRect();
        const catalogRect = catalogPanel.getBoundingClientRect();
        const catalogStyle = getComputedStyle(catalogPanel);
        const catalogGap = cssPixels(catalogStyle.rowGap || catalogStyle.gap);
        const catalogChrome = Array.from(catalogPanel.children)
            .filter((child): child is HTMLElement => child instanceof HTMLElement && child !== scroller && getComputedStyle(child).display !== "none");

        const chromeHeight = catalogChrome.reduce((sum, child) => sum + child.getBoundingClientRect().height, 0)
            + Math.max(0, catalogChrome.length) * catalogGap;
        const availableCatalogHeight = clampNumber(pageRect.bottom - catalogRect.top, 380, Math.max(380, pageRect.height));
        const targetScrollerHeight = clampNumber(availableCatalogHeight - chromeHeight, 300, availableCatalogHeight);
        const targetCardHeight = clampNumber(targetScrollerHeight / (targetScrollerHeight >= 620 ? 3.8 : 3.2), 126, 190);
        const targetThumbWidth = clampNumber(pageRect.width * 0.16, 126, 230);

        page.style.setProperty("--nexus-catalog-target-height", `${Math.round(availableCatalogHeight)}px`);
        page.style.setProperty("--nexus-scroller-target-height", `${Math.round(targetScrollerHeight)}px`);
        page.style.setProperty("--nexus-card-min-height", `${Math.round(targetCardHeight)}px`);
        page.style.setProperty("--nexus-thumb-width", `${Math.round(targetThumbWidth)}px`);
    }

    function cssPixels(value: string): number {
        const parsed = Number.parseFloat(value);
        return Number.isFinite(parsed) ? parsed : 0;
    }

    function clampNumber(value: number, min: number, max: number): number {
        return Math.min(max, Math.max(min, value));
    }


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

    function loadNexusUiPreferences() {
        try {
            const parsed = JSON.parse(localStorage.getItem(NEXUS_UI_PREFERENCES_KEY) ?? "null") as Partial<NexusUiPreferences> | null;
            if (parsed && typeof parsed === "object") {
                catalogMode = validOption(parsed.catalogMode, CATALOG_MODES, catalogMode);
                nexusSearchTerm = typeof parsed.nexusSearchTerm === "string" ? parsed.nexusSearchTerm : "";
                selectedNexusCategory = typeof parsed.selectedNexusCategory === "string" ? parsed.selectedNexusCategory : "all";
                selectedInstallFilter = validOption(parsed.selectedInstallFilter, INSTALL_FILTERS, selectedInstallFilter);
                selectedModTypeFilter = validOption(parsed.selectedModTypeFilter, MOD_TYPE_FILTERS, selectedModTypeFilter);
                selectedNexusSort = validOption(parsed.selectedNexusSort, NEXUS_SORT_MODES, selectedNexusSort);
                selectedInstalledSort = validOption(parsed.selectedInstalledSort, INSTALLED_SORT_MODES, selectedInstalledSort);
            }
        } catch {
            localStorage.removeItem(NEXUS_UI_PREFERENCES_KEY);
        } finally {
            nexusUiPreferencesLoaded = true;
        }
    }

    function persistNexusUiPreferences() {
        const preferences: NexusUiPreferences = {
            catalogMode,
            nexusSearchTerm,
            selectedNexusCategory,
            selectedInstallFilter,
            selectedModTypeFilter,
            selectedNexusSort,
            selectedInstalledSort
        };
        localStorage.setItem(NEXUS_UI_PREFERENCES_KEY, JSON.stringify(preferences));
    }

    function validOption<T extends string>(value: unknown, options: T[], fallback: T): T {
        return typeof value === "string" && options.includes(value as T) ? value as T : fallback;
    }

    function applySharedSearch(value: string) {
        if (value === nexusSearchTerm) {
            return;
        }

        handleNexusSearchInput(value, false);
    }

    function handleNexusSearchInput(value: string, emit = true) {
        nexusSearchTerm = value;
        if (emit) {
            dispatch("searchChange", value);
        }
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

    function beginManualRefreshCooldown(now = Date.now()) {
        nextManualRefreshAt = now + NEXUS_MANUAL_REFRESH_COOLDOWN_MS;
        refreshCooldownSeconds = secondsUntilManualRefresh(now);
        ensureManualRefreshCooldownTimer();
    }

    function ensureManualRefreshCooldownTimer() {
        if (refreshCooldownTimer !== null) {
            return;
        }

        refreshCooldownTimer = window.setInterval(updateManualRefreshCooldown, 1000);
    }

    function updateManualRefreshCooldown() {
        refreshCooldownSeconds = secondsUntilManualRefresh();

        if (refreshCooldownSeconds > 0) {
            if (status.startsWith("Nexus refresh available in ")) {
                status = `Nexus refresh available in ${refreshCooldownSeconds}s.`;
            }

            return;
        }

        clearManualRefreshCooldownTimer();
        if (status.startsWith("Nexus refresh available in ")) {
            status = "";
        }
    }

    function secondsUntilManualRefresh(now = Date.now()) {
        return Math.max(0, Math.ceil((nextManualRefreshAt - now) / 1000));
    }

    function clearManualRefreshCooldownTimer() {
        if (refreshCooldownTimer !== null) {
            window.clearInterval(refreshCooldownTimer);
            refreshCooldownTimer = null;
        }
    }

    function cleanupManualRefreshCooldown() {
        clearManualRefreshCooldownTimer();
        refreshCooldownSeconds = 0;
    }

    async function disconnect() {
        cleanupSso();
        cleanupManualRefreshCooldown();
        nextManualRefreshAt = 0;
        await clearNexusApiKey();
        session = { is_connected: false };
        mods = [];
        knownNexusDetails = {};
        nexusCategories = [];
        nexusCatalogLoadedAt = null;
        endorsements = [];
        endorsementsLoaded = false;
        trackedMods = [];
        trackedModsLoaded = false;
    }

    async function loadMods(view: NexusView = selectedView, forceRefresh = false) {
        selectedView = view;
        if (!session.is_connected) {
            return;
        }

        if (forceRefresh) {
            const now = Date.now();
            const cooldownSeconds = secondsUntilManualRefresh(now);
            if (cooldownSeconds > 0) {
                refreshCooldownSeconds = cooldownSeconds;
                ensureManualRefreshCooldownTimer();
                status = `Nexus refresh available in ${cooldownSeconds}s.`;
                return;
            }

            beginManualRefreshCooldown(now);
            knownNexusDetails = {};
        }

        isLoading = true;
        status = forceRefresh ? "Refreshing Nexus mods..." : "Loading Nexus mods...";

        try {
            await refreshInventory();
            const response = await fetchNexusSotfMods(selectedView, { force: forceRefresh });
            mods = response.mods;
            nexusCategories = response.categories;
            nexusCatalogLoadedAt = Date.now();
            session = {
                ...session,
                rate_limit: response.rate_limit
            };
            await refreshEndorsements(forceRefresh);
            await refreshTrackedMods(forceRefresh);
            await maybeAutoEndorseInstalledVortexMods();
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

    async function refreshEndorsements(forceRefresh = false) {
        if (!session.is_connected) {
            endorsements = [];
            endorsementsLoaded = false;
            return;
        }

        try {
            const response = await fetchNexusUserEndorsements({ force: forceRefresh });
            endorsements = response.endorsements.filter(endorsement =>
                !endorsement.game_domain_name || endorsement.game_domain_name.toLowerCase() === "sonsoftheforest"
            );
            endorsementsLoaded = true;
            session = {
                ...session,
                rate_limit: response.rate_limit
            };
        } catch (error) {
            console.log("Failed to refresh Nexus endorsements", error);
            endorsementsLoaded = false;
        }
    }

    async function refreshTrackedMods(forceRefresh = false) {
        if (!session.is_connected) {
            trackedMods = [];
            trackedModsLoaded = false;
            return;
        }

        try {
            const response = await fetchNexusUserTrackedMods({ force: forceRefresh });
            trackedMods = response.tracked_mods.filter(tracked =>
                !tracked.game_domain_name || tracked.game_domain_name.toLowerCase() === "sonsoftheforest"
            );
            trackedModsLoaded = true;
            session = {
                ...session,
                rate_limit: response.rate_limit
            };
        } catch (error) {
            console.log("Failed to refresh Nexus tracked mods", error);
            trackedModsLoaded = false;
        }
    }

    async function toggleNexusTracking(mod: NexusMod) {
        await setNexusTracking(mod.mod_id, !isNexusModTracked(mod.mod_id), mod.name);
    }

    async function toggleInstalledTracking(entry: InstalledInventoryEntry) {
        const modId = numericNexusId(entry.nexusModId);
        if (!modId) {
            return;
        }

        await setNexusTracking(modId, !isNexusModTracked(modId), entry.name);
    }

    async function toggleSelectedModTracking() {
        if (selectedMod) {
            await toggleNexusTracking(selectedMod);
        }
    }

    async function setNexusTracking(modId: number, shouldTrack: boolean, label: string) {
        const confirmed = await confirmNexusWrite({
            action: shouldTrack ? "Track" : "Stop tracking",
            label,
            detail: "This updates your Nexus tracked mod list for Sons Of The Forest.",
            okLabel: shouldTrack ? "Track" : "Untrack"
        });
        if (!confirmed) {
            return;
        }

        activeNexusTrackId = modId;
        status = `${shouldTrack ? "Tracking" : "Untracking"} ${label}...`;
        let keepStatus = false;

        try {
            const response = shouldTrack
                ? await trackNexusSotfMod(modId)
                : await untrackNexusSotfMod(modId);
            session = {
                ...session,
                rate_limit: response.rate_limit
            };
            markNexusModTracked(modId, shouldTrack, label);
            status = `${shouldTrack ? "Tracking" : "Stopped tracking"} ${label}.`;
            keepStatus = true;
            window.setTimeout(() => {
                if (status === `${shouldTrack ? "Tracking" : "Stopped tracking"} ${label}.`) {
                    status = "";
                }
            }, 4500);
        } catch (error) {
            await dialog.message(`${error}`, {
                title: "Nexus tracking",
                kind: "error"
            });
        } finally {
            activeNexusTrackId = null;
            if (!keepStatus) {
                status = "";
            }
        }
    }

    async function endorseNexusMod(mod: NexusMod) {
        const match = installedMatch(mod);
        if (!match) {
            await dialog.message("Nexus generally only accepts endorsements for mods your account downloaded. Install or deploy the mod before endorsing it.", {
                title: "Nexus endorsement",
                kind: "info"
            });
            return;
        }

        await endorseKnownNexusMod(mod.mod_id, match.version ?? mod.version, mod.name);
    }

    async function endorseSelectedMod() {
        if (selectedMod) {
            await endorseNexusMod(selectedMod);
        }
    }

    async function endorseInstalledEntry(entry: InstalledInventoryEntry) {
        const modId = numericNexusId(entry.nexusModId);
        if (!modId) {
            return;
        }

        await endorseKnownNexusMod(modId, entry.version, entry.name);
    }

    async function endorseKnownNexusMod(modId: number, version: string | undefined, label: string) {
        const confirmed = await confirmNexusWrite({
            action: "Endorse",
            label,
            detail: "Nexus may require that this account downloaded the mod and may enforce its normal endorsement waiting period.",
            okLabel: "Endorse"
        });
        if (!confirmed) {
            return;
        }

        activeNexusEndorseId = modId;
        status = `Endorsing ${label}...`;
        let keepStatus = false;

        try {
            const response = await endorseNexusSotfMod(modId, version);
            session = {
                ...session,
                rate_limit: response.rate_limit
            };
            markNexusModEndorsed(modId);
            status = `Endorsed ${label}.`;
            keepStatus = true;
            window.setTimeout(() => {
                if (status === `Endorsed ${label}.`) {
                    status = "";
                }
            }, 4500);
        } catch (error) {
            await dialog.message(`Nexus did not accept this endorsement yet. Nexus may require the mod to be downloaded through your account, may enforce a waiting period, or may reject already-endorsed mods.\n\n${error}`, {
                title: "Nexus endorsement",
                kind: "error"
            });
        } finally {
            activeNexusEndorseId = null;
            if (!keepStatus) {
                status = "";
            }
        }
    }

    async function maybeAutoEndorseInstalledVortexMods() {
        if (!autoEndorseDownloadedMods || !session.is_connected || !endorsementsLoaded) {
            return;
        }

        const candidates = inventory
            .map(entry => ({ entry, modId: numericNexusId(entry.nexusModId) }))
            .filter((candidate): candidate is { entry: InstalledInventoryEntry; modId: number } =>
                candidate.entry.installSource === "vortex"
                && !!candidate.modId
                && !isNexusModEndorsed(candidate.modId)
                && !autoEndorseAttemptedIds.includes(candidate.modId)
            )
            .slice(0, MAX_AUTO_ENDORSE_PER_REFRESH);

        for (const candidate of candidates) {
            markAutoEndorseAttempted(candidate.modId);
            try {
                const response = await endorseNexusSotfMod(candidate.modId, candidate.entry.version);
                session = {
                    ...session,
                    rate_limit: response.rate_limit
                };
                markNexusModEndorsed(candidate.modId);
            } catch (error) {
                console.log(`Auto-endorse skipped for Nexus mod ${candidate.modId}`, error);
            }
        }
    }

    function loadEndorsementPreferences() {
        autoEndorseDownloadedMods = localStorage.getItem(NEXUS_AUTO_ENDORSE_KEY) === "true";
        autoEndorseAttemptedIds = readAutoEndorseAttemptedIds();
    }

    async function toggleAutoEndorse(event: Event) {
        const input = event.currentTarget as HTMLInputElement;
        const requestedState = input.checked;

        if (requestedState && !autoEndorseDownloadedMods) {
            const confirmed = await confirmNexusWrite({
                action: "Enable auto-endorse",
                label: "Vortex-managed downloads",
                detail: `The manager will attempt at most ${MAX_AUTO_ENDORSE_PER_REFRESH} eligible endorsements per refresh and remembers attempted Nexus mod IDs locally.`,
                okLabel: "Enable"
            });
            if (!confirmed) {
                input.checked = false;
                autoEndorseDownloadedMods = false;
                return;
            }
        }

        autoEndorseDownloadedMods = requestedState;
        localStorage.setItem(NEXUS_AUTO_ENDORSE_KEY, autoEndorseDownloadedMods ? "true" : "false");

        if (autoEndorseDownloadedMods) {
            void maybeAutoEndorseInstalledVortexMods();
        }
    }

    function readAutoEndorseAttemptedIds(): number[] {
        try {
            const parsed = JSON.parse(localStorage.getItem(NEXUS_AUTO_ENDORSE_ATTEMPTED_KEY) ?? "[]");
            if (!Array.isArray(parsed)) {
                return [];
            }

            return parsed
                .map(numericNexusId)
                .filter((value): value is number => !!value);
        } catch {
            return [];
        }
    }

    function saveAutoEndorseAttemptedIds() {
        localStorage.setItem(NEXUS_AUTO_ENDORSE_ATTEMPTED_KEY, JSON.stringify(autoEndorseAttemptedIds));
    }

    function markAutoEndorseAttempted(modId: number) {
        if (!autoEndorseAttemptedIds.includes(modId)) {
            autoEndorseAttemptedIds = [...autoEndorseAttemptedIds, modId].slice(-500);
            saveAutoEndorseAttemptedIds();
        }
    }

    function autoEndorseQueueSummary(): string {
        if (!endorsementsLoaded) {
            return "Loading endorsement state";
        }

        if (autoEndorseEligibleCount === 0) {
            return "No Vortex Nexus installs";
        }

        if (autoEndorsePendingCount === 0 && autoEndorseAttemptedInstalledCount === 0) {
            return "No pending endorsements";
        }

        return `${autoEndorsePendingCount} pending · ${autoEndorseAttemptedInstalledCount} attempted`;
    }

    async function confirmNexusWrite(options: { action: string; label: string; detail: string; okLabel: string }): Promise<boolean> {
        return await dialog.confirm(`${options.action} ${options.label}?\n\n${options.detail}`, {
            title: "Nexus account action",
            kind: "warning",
            okLabel: options.okLabel,
            cancelLabel: "Cancel"
        });
    }

    function markNexusModEndorsed(modId: number) {
        const wasEndorsed = isNexusModEndorsed(modId);
        endorsements = [
            ...endorsements.filter(endorsement => endorsement.mod_id !== modId),
            { mod_id: modId, game_domain_name: "sonsoftheforest", status: "endorsed", endorsed_at: new Date().toISOString() }
        ];

        if (!wasEndorsed) {
            mods = mods.map(mod => mod.mod_id === modId
                ? {
                    ...mod,
                    endorsement_count: typeof mod.endorsement_count === "number" ? mod.endorsement_count + 1 : mod.endorsement_count
                }
                : mod
            );

            if (selectedModDetails?.mod_id === modId && typeof selectedModDetails.endorsement_count === "number") {
                selectedModDetails = {
                    ...selectedModDetails,
                    endorsement_count: selectedModDetails.endorsement_count + 1
                };
            }
        }
    }

    function markNexusModTracked(modId: number, shouldTrack: boolean, label?: string) {
        if (shouldTrack) {
            trackedMods = [
                ...trackedMods.filter(tracked => tracked.mod_id !== modId),
                { mod_id: modId, game_domain_name: "sonsoftheforest", name: label }
            ];
            trackedModsLoaded = true;
            return;
        }

        trackedMods = trackedMods.filter(tracked => tracked.mod_id !== modId);
    }

    function isNexusModEndorsed(modIdValue?: number | string | null): boolean {
        const modId = numericNexusId(modIdValue);
        if (!modId) {
            return false;
        }

        return endorsements.some(endorsement => {
            const endorsementStatus = endorsement.status?.toLowerCase() ?? "";
            return endorsement.mod_id === modId && !/abstain|unendors/.test(endorsementStatus);
        });
    }

    function isNexusModTracked(modIdValue?: number | string | null): boolean {
        const modId = numericNexusId(modIdValue);
        if (!modId) {
            return false;
        }

        return trackedMods.some(tracked => tracked.mod_id === modId);
    }

    function numericNexusId(value?: number | string | null): number | null {
        if (typeof value === "number" && Number.isFinite(value) && value > 0) {
            return value;
        }

        if (typeof value === "string") {
            const parsed = Number(value.trim());
            return Number.isFinite(parsed) && parsed > 0 ? parsed : null;
        }

        return null;
    }

    async function openApiKeys() {
        await shell.open("https://www.nexusmods.com/users/myaccount?tab=api%20access");
    }

    async function openNexusGame() {
        await shell.open("https://www.nexusmods.com/games/sonsoftheforest");
    }

    async function openExternalTarget(target: string) {
        if (/^[a-z][a-z0-9+.-]*:\/\//i.test(target)) {
            await shell.open(target);
            return;
        }

        try {
            const cmd = Command.create("open-explorer", [target]);
            const result = await cmd.execute();
            if (result.code === 0) {
                return;
            }
        } catch {
            // Fall back to Tauri's shell opener below.
        }

        await shell.open(target);
    }

    async function openVortexStaging() {
        if (!vortexStagingPath) {
            await dialog.message("Vortex deployment metadata was not detected for this game folder yet.", {
                title: "Vortex staging",
                kind: "info"
            });
            return;
        }

        await openExternalTarget(vortexStagingPath);
    }

    function showInstalledInventory() {
        catalogMode = "installed";
        selectedInstallFilter = "all";
    }

    function showCurrentAttention() {
        selectedInstallFilter = "attention";
    }

    function showUpdatesOrDisabled() {
        catalogMode = "installed";
        selectedInstallFilter = updateCount > 0 ? "updates" : "disabled";
    }

    function showLocalConflicts() {
        catalogMode = "installed";
        selectedInstallFilter = "conflicts";
    }

    function showEndorsementQueue() {
        catalogMode = "installed";
        selectedInstallFilter = "endorsements";
        selectedInstalledSort = "attention";
    }

    function showTrackedQueue() {
        catalogMode = "online";
        selectedInstallFilter = "tracked";
        selectedNexusSort = "attention";

        if (selectedView !== "all") {
            void loadMods("all");
        }
    }

    function clearNexusFilters() {
        handleNexusSearchInput("");
        selectedNexusCategory = "all";
        selectedInstallFilter = "all";
        selectedModTypeFilter = "all";
    }

    async function openModPage(mod: NexusMod) {
        await shell.open(getNexusModPageUrl(mod));
    }

    async function openDownloadPage(mod: NexusMod) {
        await shell.open(getNexusModDownloadUrl(mod));
    }

    function catalogPreviewUrls(mod: NexusMod): string[] {
        const detail = knownNexusDetails[mod.mod_id];
        return uniquePreviewUrls([
            ...(mod.image_urls ?? []),
            mod.picture_url,
            ...(detail?.image_urls ?? []),
            detail?.picture_url
        ]);
    }

    function catalogPreviewIndex(mod: NexusMod, urls: string[]): number {
        if (urls.length === 0) {
            return 0;
        }

        const current = catalogPreviewIndexes[mod.mod_id] ?? 0;
        return current >= 0 && current < urls.length ? current : 0;
    }

    function catalogPreviewImage(mod: NexusMod, urls: string[]): string {
        return urls[catalogPreviewIndex(mod, urls)] ?? NEXUS_PLACEHOLDER_IMAGE;
    }

    function selectedDetailPreviewUrls(): string[] {
        if (!selectedMod) {
            return [];
        }

        return uniquePreviewUrls([
            ...(selectedMod.image_urls ?? []),
            selectedMod.picture_url,
            ...(selectedModDetails?.image_urls ?? []),
            selectedModDetails?.picture_url,
            ...selectedRichTextPreviewUrls()
        ]);
    }

    function selectedRichTextPreviewUrls(): string[] {
        const sources = [
            selectedModDetails?.description,
            selectedNexusFile?.description,
            selectedNexusFile?.changelog_html,
            ...selectedChangelogs.map(changelog => changelog.changes)
        ];
        return uniquePreviewUrls(sources.flatMap(source => nexusRichTextImageUrls(source)));
    }

    function selectedDetailPreviewIndexValue(urls: string[]): number {
        if (urls.length === 0) {
            return 0;
        }

        return selectedDetailPreviewIndex >= 0 && selectedDetailPreviewIndex < urls.length
            ? selectedDetailPreviewIndex
            : 0;
    }

    function selectedDetailPreviewImage(urls: string[]): string {
        return urls[selectedDetailPreviewIndexValue(urls)] ?? NEXUS_DETAIL_PLACEHOLDER_IMAGE;
    }

    function selectedDetailDescriptionSource(): string | undefined {
        return selectedModDetails?.description ?? selectedModDetails?.summary ?? selectedMod?.summary;
    }

    function selectedAuthorRequirementSource(): string | undefined {
        const sources = [
            selectedDetailDescriptionSource(),
            selectedNexusFile?.description,
            selectedNexusFile?.changelog_html,
            ...selectedChangelogs.map(changelog => changelog.changes)
        ];
        const seen = new Set<string>();
        const unique = sources
            .map(source => source?.trim())
            .filter((source): source is string => Boolean(source))
            .filter(source => {
                const key = source.replace(/\s+/g, " ").slice(0, 300);
                if (seen.has(key)) {
                    return false;
                }

                seen.add(key);
                return true;
            });

        return unique.length > 0 ? unique.join("\n\n") : undefined;
    }

    function selectedDetailDescriptionText(): string {
        return plainText(selectedDetailDescriptionSource()) || NEXUS_DESCRIPTION_FALLBACK;
    }

    function selectedDetailDescriptionMarkup(): string {
        return renderNexusRichText(selectedDetailDescriptionSource(), NEXUS_DESCRIPTION_FALLBACK);
    }

    function shouldOfferDetailDescriptionToggle(description: string, source?: string): boolean {
        const normalized = normalizeNexusMarkup(source);
        const structuralWeight = (normalized.match(/\[\*\]/g)?.length ?? 0)
            + (normalized.match(/\[(?:img|quote|code|spoiler|heading|h[1-6]|hr|youtube|video|table|tr|olist|indent|columns|cols|tabs|note|info|warning|important|tip)\b/gi)?.length ?? 0) * 2;
        return description.length > 900 || description.split("\n").length > 12 || structuralWeight >= 8;
    }

    function extractAuthorRequirementLinks(source?: string, currentModId?: number): AuthorRequirementLink[] {
        const normalized = normalizeNexusMarkup(source);
        if (!normalized) {
            return [];
        }

        const candidates: AuthorRequirementLink[] = [];
        const seen = new Set<string>();

        const addLink = (urlValue: string, labelValue?: string) => {
            const url = safeNexusUrl(urlValue);
            if (!url || seen.has(url)) {
                return;
            }

            const nexusModId = nexusSotfModIdFromUrl(url);
            if (currentModId && nexusModId === currentModId) {
                return;
            }

            const label = authorRequirementLabel(labelValue, url, nexusModId);
            const linkContext = `${label} ${url}`;
            if (!nexusModId && !looksLikeRequirementContext(linkContext)) {
                return;
            }

            seen.add(url);
            candidates.push({
                key: `${nexusModId ?? "link"}:${url}`,
                label,
                url,
                source: nexusModId ? "Nexus mod link" : "Author link",
                mod_id: nexusModId ?? undefined
            });
        };

        const linkedPattern = /\[url=([^\]]+)\]([\s\S]*?)\[\/url\]/gi;
        let linkedMatch: RegExpExecArray | null;
        while ((linkedMatch = linkedPattern.exec(normalized)) !== null) {
            addLink(linkedMatch[1], plainText(linkedMatch[2]));
        }

        const plainUrlPattern = /\[url\]([\s\S]*?)\[\/url\]|https?:\/\/[^\s<>"'\]]+/gi;
        let plainMatch: RegExpExecArray | null;
        while ((plainMatch = plainUrlPattern.exec(normalized)) !== null) {
            const raw = plainMatch[1] ?? plainMatch[0];
            addLink(raw);
        }

        return candidates.slice(0, MAX_AUTHOR_REQUIREMENT_LINKS);
    }

    function looksLikeRequirementContext(value: string): boolean {
        return /\b(require(?:d|ment|s)?|dependenc(?:y|ies)|prereq(?:uisite)?|needed|install first|runtime|framework|library|bepinex|redloader|sonssdk|harmony|doorstop|\.net|dotnet)\b/i.test(value);
    }

    function authorRequirementLabel(value: string | undefined, url: string, nexusModId: number | null): string {
        const cleaned = plainText(value)
            .replace(/^https?:\/\/\S+$/i, "")
            .replace(/\s+/g, " ")
            .trim();

        if (cleaned) {
            return cleaned.slice(0, 90);
        }

        if (nexusModId) {
            return `Nexus mod ${nexusModId}`;
        }

        try {
            const parsed = new URL(url);
            return parsed.hostname.replace(/^www\./i, "");
        } catch {
            return "Author requirement";
        }
    }

    function nexusSotfModIdFromUrl(value: string): number | null {
        try {
            const url = new URL(value);
            const host = url.hostname.replace(/^www\./i, "").toLowerCase();
            if (host !== "nexusmods.com") {
                return null;
            }

            const match = url.pathname.match(/^\/sonsoftheforest\/mods\/(\d+)(?:\/|$)/i);
            return match ? Number.parseInt(match[1], 10) : null;
        } catch {
            return null;
        }
    }

    function cycleCatalogPreview(mod: NexusMod, urls: string[], step: number, event: MouseEvent) {
        event.stopPropagation();
        if (urls.length < 2) {
            return;
        }

        const current = catalogPreviewIndex(mod, urls);
        catalogPreviewIndexes = {
            ...catalogPreviewIndexes,
            [mod.mod_id]: (current + step + urls.length) % urls.length
        };
    }

    function cycleSelectedDetailPreview(urls: string[], step: number, event: MouseEvent) {
        event.stopPropagation();
        if (urls.length < 2) {
            return;
        }

        const current = selectedDetailPreviewIndexValue(urls);
        selectedDetailPreviewIndex = (current + step + urls.length) % urls.length;
    }

    function selectSelectedDetailPreview(index: number) {
        if (index < 0 || index >= currentDetailPreviewUrls.length) {
            return;
        }

        selectedDetailPreviewIndex = index;
    }

    function handleNexusImageError(event: Event, fallback = NEXUS_PLACEHOLDER_IMAGE) {
        const image = event.currentTarget instanceof HTMLImageElement ? event.currentTarget : null;
        if (!image || image.src === fallback) {
            return;
        }

        image.src = fallback;
    }

    function nexusRichTextImageUrls(source?: string): string[] {
        const normalized = normalizeNexusMarkup(source);
        if (!normalized) {
            return [];
        }

        const urls: string[] = [];
        const pairedImagePattern = /\[img([^\]]*)\]([\s\S]*?)\[\/img\]/gi;
        let pairedMatch: RegExpExecArray | null;
        while ((pairedMatch = pairedImagePattern.exec(normalized)) !== null) {
            const attrUrl = nexusImageUrlAttribute(pairedMatch[1], nexusBbTagAttribute("img", pairedMatch[1]));
            const bodyUrl = collectNexusNodeText(parseNexusRichNodes(pairedMatch[2]));
            urls.push(...[attrUrl, bodyUrl].filter((url): url is string => Boolean(url)));
        }

        const imageOpeningPattern = /\[img([^\]]*)\]/gi;
        let openingMatch: RegExpExecArray | null;
        while ((openingMatch = imageOpeningPattern.exec(normalized)) !== null) {
            const url = nexusImageUrlAttribute(openingMatch[1], nexusBbTagAttribute("img", openingMatch[1]));
            if (url) {
                urls.push(url);
            }
        }

        return uniquePreviewUrls(urls);
    }

    function uniquePreviewUrls(values: Array<string | undefined>): string[] {
        const seen = new Set<string>();
        const urls: string[] = [];

        for (const value of values) {
            const url = normalizePreviewUrl(value);
            if (!url || seen.has(url)) {
                continue;
            }

            seen.add(url);
            urls.push(url);
        }

        return urls;
    }

    function normalizePreviewUrl(value?: string): string | undefined {
        return safeNexusUrl(value) ?? undefined;
    }

    async function openInventoryLocation(entry: InstalledInventoryEntry) {
        const target = entry.packagePath ?? entry.assemblyPath ?? await getDirectoryPath();
        await openExternalTarget(target);
    }

    async function openInstalledEntryDetails(entry: InstalledInventoryEntry) {
        const mod = nexusModForInstalledEntry(entry);
        if (mod) {
            await openModDetails(mod);
        }
    }

    async function openConflictEntryDetails(entry: InstalledInventoryEntry) {
        const mod = nexusModForInstalledEntry(entry);
        if (mod) {
            await openModDetails(mod, { pushCurrent: !!selectedMod });
        }
    }

    async function openModDetails(mod: NexusMod, options: { pushCurrent?: boolean } = {}) {
        if (options.pushCurrent && selectedMod && selectedMod.mod_id !== mod.mod_id) {
            detailBackStack = [...detailBackStack, selectedModDetails ?? selectedMod].slice(-8);
        }

        selectedMod = mod;
        selectedModDetails = mod;
        selectedDetailPreviewIndex = 0;
        detailDescriptionExpanded = false;
        selectedModFiles = [];
        selectedFileId = null;
        selectedInstallPlacement = "auto";
        selectedDependencies = [];
        selectedDependencyMessage = "";
        clearNestedDependencyCheck();
        selectedChangelogs = [];
        isDetailLoading = true;

        try {
            const [detailResponse, fileResponse, changelogResponse] = await Promise.all([
                fetchNexusModDetails(mod.mod_id)
                    .then(details => ({ details, fetched: true }))
                    .catch(() => ({ details: mod, fetched: false })),
                fetchNexusModFiles(mod.mod_id),
                fetchNexusModChangelogs(mod.mod_id).catch(() => ({ changelogs: [], rate_limit: session.rate_limit ?? {} }))
            ]);
            const mergedDetails = { ...mod, ...detailResponse.details };
            const recommendedFileVersion = preferredNexusFileVersion(fileResponse.files);
            selectedModDetails = recommendedFileVersion
                ? { ...mergedDetails, version: recommendedFileVersion }
                : mergedDetails;
            if (detailResponse.fetched) {
                rememberNexusDetails(selectedModDetails);
            }
            selectedModFiles = fileResponse.files;
            selectedChangelogs = changelogResponse.changelogs.slice(0, 5);
            selectedFileId = pickRecommendedNexusFile(fileResponse.files)?.file_id ?? fileResponse.files[0]?.file_id ?? null;
            session = {
                ...session,
                rate_limit: fileResponse.rate_limit
            };

            if (selectedFileId !== null) {
                await loadDependenciesForFile(selectedFileId);
            }
        } catch (error) {
            await dialog.message(`${error}`, {
                title: "Nexus mod details",
                kind: "error"
            });
        } finally {
            isDetailLoading = false;
        }
    }

    function scrollDetailSection(target: "description" | "files" | "dependencies" | "plan" | "changelog" | "deployment") {
        const element = {
            description: detailDescriptionSectionElement,
            files: detailFilesSectionElement,
            dependencies: detailDependenciesSectionElement,
            plan: detailInstallPlanSectionElement,
            changelog: detailChangelogSectionElement,
            deployment: detailFooterElement
        }[target];

        element?.scrollIntoView({ block: "start", behavior: "smooth" });
    }

    function describeDetailDependencyNav(issueCount: number, reviewCount: number, apiCount: number, nestedCount: number, authorCount: number): string {
        if (issueCount > 0) {
            return `${issueCount} issue${issueCount === 1 ? "" : "s"}`;
        }

        if (reviewCount > 0) {
            return `${reviewCount} review`;
        }

        const total = apiCount + nestedCount + authorCount;
        return total > 0 ? `${total} listed` : "None listed";
    }

    function rememberNexusDetails(details: NexusMod) {
        if (!details.mod_id) {
            return;
        }

        knownNexusDetails = {
            ...knownNexusDetails,
            [details.mod_id]: {
                ...(knownNexusDetails[details.mod_id] ?? {}),
                ...details
            }
        };
    }

    function preferredNexusFileVersion(files: NexusModFile[]): string | undefined {
        const recommended = pickRecommendedNexusFile(files) ?? files[0];
        return recommended?.mod_version ?? recommended?.version;
    }

    function fileVersionLabel(file: NexusModFile): string {
        return file.mod_version ?? file.version ?? "-";
    }

    function closeModDetails() {
        selectedMod = null;
        selectedModDetails = null;
        selectedDetailPreviewIndex = 0;
        detailDescriptionExpanded = false;
        selectedModFiles = [];
        selectedFileId = null;
        selectedDependencies = [];
        selectedDependencyMessage = "";
        clearNestedDependencyCheck();
        selectedChangelogs = [];
        detailBackStack = [];
        clearNxmCopyFeedback();
    }

    async function openPreviousDetail() {
        const previous = detailBackStack[detailBackStack.length - 1];
        if (!previous) {
            return;
        }

        detailBackStack = detailBackStack.slice(0, -1);
        await openModDetails(previous);
    }

    async function selectNexusFile(fileId: number) {
        selectedFileId = fileId;
        clearNestedDependencyCheck();
        await loadDependenciesForFile(fileId);
    }

    async function loadDependenciesForFile(fileId: number) {
        selectedDependencies = [];
        selectedDependencyMessage = "Checking Nexus dependency metadata...";
        clearNestedDependencyCheck();
        const selectedFile = selectedModFiles.find(file => file.file_id === fileId);
        try {
            const response = await fetchNexusFileDependencies(fileId, {
                nexusFileId: selectedFile?.nexus_file_id
            });
            if (selectedFileId !== fileId) {
                return;
            }
            selectedDependencies = response.dependencies;
            selectedDependencyMessage = response.dependencies.length > 0
                ? `${response.dependencies.length} API-listed ${response.dependencies.length === 1 ? "dependency" : "dependencies"} returned for this file.`
                : "Nexus returned no API dependency rows for this file.";
            session = {
                ...session,
                rate_limit: response.rate_limit
            };
        } catch (error) {
            if (selectedFileId !== fileId) {
                return;
            }
            selectedDependencies = [];
            selectedDependencyMessage = `Dependency lookup failed: ${error}`;
        }
    }

    function clearNestedDependencyCheck() {
        nestedDependencyRequestId += 1;
        nestedDependencySources = [];
        nestedDependencySummary = "";
        isResolvingNestedDependencies = false;
    }

    function nestedDependencyCheckAvailable(): boolean {
        return resolvedDependencies.some(dependency => Boolean(dependency.file_id));
    }

    function nestedDependencyStatusLabel(source: ResolvedNestedDependency): string {
        return dependencyStatusLabel(source.dependency);
    }

    async function resolveNestedDependencies() {
        if (isResolvingNestedDependencies || resolvedDependencies.length === 0) {
            return;
        }

        const initialQueue = resolvedDependencies
            .filter(dependency => typeof dependency.file_id === "number")
            .map(dependency => ({
                fileId: dependency.file_id as number,
                nexusFileId: dependency.nexus_file_id,
                parentName: dependency.mod_name,
                depth: 1
            }));

        if (initialQueue.length === 0) {
            nestedDependencySources = [];
            nestedDependencySummary = "No dependency file IDs were returned for recursive checks.";
            return;
        }

        isResolvingNestedDependencies = true;
        const requestId = ++nestedDependencyRequestId;
        nestedDependencySummary = `Checking up to ${MAX_NESTED_DEPENDENCY_FILES} dependency files...`;
        const queue = [...initialQueue];
        const visitedFileKeys = new Set<string>();
        const rowKeys = new Set<string>();
        const collected: NestedDependencySource[] = [];
        let checkedFiles = 0;

        try {
            while (queue.length > 0 && checkedFiles < MAX_NESTED_DEPENDENCY_FILES) {
                const current = queue.shift();
                const currentKey = current ? dependencyFileLookupKey(current.fileId, current.nexusFileId) : "";
                if (!current || visitedFileKeys.has(currentKey)) {
                    continue;
                }

                visitedFileKeys.add(currentKey);
                checkedFiles += 1;
                const response = await fetchNexusFileDependencies(current.fileId, {
                    nexusFileId: current.nexusFileId
                });
                if (requestId !== nestedDependencyRequestId) {
                    return;
                }
                session = { ...session, rate_limit: response.rate_limit };

                for (const dependency of response.dependencies) {
                    const rowKey = `${current.fileId}:${dependency.id}:${dependency.mod_id ?? "mod"}:${dependency.file_id ?? "file"}`;
                    if (rowKeys.has(rowKey)) {
                        continue;
                    }

                    rowKeys.add(rowKey);
                    collected.push({
                        key: rowKey,
                        parentFileId: current.fileId,
                        parentNexusFileId: current.nexusFileId,
                        parentName: current.parentName,
                        depth: current.depth,
                        dependency
                    });

                    if (
                        typeof dependency.file_id === "number"
                        && current.depth < MAX_NESTED_DEPENDENCY_DEPTH
                        && !visitedFileKeys.has(dependencyFileLookupKey(dependency.file_id, dependency.nexus_file_id))
                    ) {
                        queue.push({
                            fileId: dependency.file_id,
                            nexusFileId: dependency.nexus_file_id,
                            parentName: dependency.mod_name,
                            depth: current.depth + 1
                        });
                    }
                }
            }

            if (requestId === nestedDependencyRequestId) {
                nestedDependencySources = collected;
                nestedDependencySummary = collected.length > 0
                    ? `${collected.length} nested dependencies found from ${checkedFiles} checked file${checkedFiles === 1 ? "" : "s"}.`
                    : `${checkedFiles} dependency file${checkedFiles === 1 ? "" : "s"} checked; no nested dependencies returned.`;
            }
        } catch (error) {
            if (requestId === nestedDependencyRequestId) {
                nestedDependencySources = collected;
                nestedDependencySummary = `Nested dependency check stopped: ${error}`;
            }
        } finally {
            if (requestId === nestedDependencyRequestId) {
                isResolvingNestedDependencies = false;
            }
        }
    }

    function dependencyFileLookupKey(fileId: number, nexusFileId?: string): string {
        return `${fileId}:${nexusFileId?.trim() ?? ""}`;
    }

    async function installSelectedFileWithVortex() {
        if (!selectedMod || !selectedNexusFile) {
            return;
        }

        await sendFileToVortex(selectedMod, selectedNexusFile);
    }

    async function openSelectedModPage() {
        if (selectedMod) {
            await openModPage(selectedMod);
        }
    }

    async function openSelectedDownloadPage() {
        if (selectedMod) {
            await openDownloadPage(selectedMod);
        }
    }

    async function openSelectedModTab(tab: "description" | "files" | "posts" | "images" | "bugs") {
        if (!selectedMod) {
            return;
        }

        await shell.open(`${getNexusModPageUrl(selectedMod)}?tab=${tab}`);
    }

    async function copySelectedNxmLink() {
        if (!selectedNxmUrl) {
            return;
        }

        try {
            await copyTextToClipboard(selectedNxmUrl);
            nxmCopyState = "Copied";
        } catch (error) {
            nxmCopyState = "Copy failed";
            await dialog.message(`Could not copy automatically.\n\n${selectedNxmUrl}\n\n${error}`, {
                title: "Vortex link",
                kind: "info"
            });
        }

        scheduleNxmCopyFeedbackReset();
    }

    async function copyTextToClipboard(value: string) {
        if (copyTextWithSelection(value)) {
            return;
        }

        if (navigator.clipboard?.writeText) {
            await navigator.clipboard.writeText(value);
            return;
        }

        throw new Error("Clipboard API is unavailable.");
    }

    function copyTextWithSelection(value: string): boolean {
        const activeElement = document.activeElement instanceof HTMLElement ? document.activeElement : null;
        const textArea = document.createElement("textarea");
        textArea.value = value;
        textArea.setAttribute("readonly", "");
        textArea.style.left = "-9999px";
        textArea.style.opacity = "0";
        textArea.style.position = "fixed";
        textArea.style.top = "0";
        document.body.appendChild(textArea);
        textArea.focus();
        textArea.select();

        try {
            return document.execCommand("copy");
        } finally {
            document.body.removeChild(textArea);
            activeElement?.focus();
        }
    }

    function scheduleNxmCopyFeedbackReset() {
        if (nxmCopyResetTimer !== null) {
            window.clearTimeout(nxmCopyResetTimer);
        }

        nxmCopyResetTimer = window.setTimeout(() => {
            nxmCopyState = "";
            nxmCopyResetTimer = null;
        }, 1800);
    }

    function clearNxmCopyFeedback() {
        if (nxmCopyResetTimer !== null) {
            window.clearTimeout(nxmCopyResetTimer);
            nxmCopyResetTimer = null;
        }

        nxmCopyState = "";
    }

    async function installRecommendedWithVortex(mod: NexusMod) {
        activeNexusActionId = mod.mod_id;
        status = `Preparing ${mod.name} for Vortex...`;

        try {
            const response = await fetchNexusModFiles(mod.mod_id);
            session = {
                ...session,
                rate_limit: response.rate_limit
            };
            const file = pickRecommendedNexusFile(response.files);
            if (!file) {
                await dialog.message("Nexus did not return an installable file for this mod. Open the details page and review the files manually.", {
                    title: "Vortex install",
                    kind: "info"
                });
                return;
            }

            await sendFileToVortex(mod, file);
        } catch (error) {
            await dialog.message(`${error}`, {
                title: "Vortex install",
                kind: "error"
            });
        } finally {
            status = "";
            activeNexusActionId = null;
        }
    }

    async function sendFileToVortex(mod: NexusMod, file: NexusModFile) {
        await openExternalTarget(getNexusNxmUrl(mod, file));
        status = `Sent ${mod.name} (${file.name}) to Vortex. Finish deployment in Vortex, then refresh inventory.`;
        window.setTimeout(() => {
            if (status.startsWith("Sent ")) {
                status = "";
            }
        }, 7000);
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

    async function setConflictEntryState(entry: InstalledInventoryEntry, shouldEnable: boolean) {
        const key = inventoryEntryKey(entry);
        activeConflictEntryKey = key;

        try {
            await setInventoryEntryEnabled(entry, shouldEnable);
            await refreshInventory();
        } catch (error) {
            await dialog.message(`${error}`, {
                title: "Conflict state",
                kind: "info"
            });
        } finally {
            if (activeConflictEntryKey === key) {
                activeConflictEntryKey = null;
            }
        }
    }

    function installedMatch(mod: NexusMod): InstalledInventoryEntry | null {
        return findMatchingInstall(inventory, mod.name, mod.mod_id, [
            mod.author ?? "",
            mod.uploaded_by ?? ""
        ]);
    }

    function resolveDependencyStatus(dependency: NexusModDependency): ResolvedDependency {
        const match = findMatchingInstall(inventory, dependency.mod_name, dependency.mod_id, [
            dependency.file_name ?? "",
            dependency.group_name ?? "",
            dependency.id ?? ""
        ]);

        if (!match) {
            return { ...dependency, match: null, status: "missing" };
        }

        if (dependency.version_requirement && !dependency.version) {
            return { ...dependency, match, status: "review" };
        }

        if (dependency.version && !match.version) {
            return { ...dependency, match, status: "review" };
        }

        if (dependency.version && versionsDiffer(match.version, dependency.version)) {
            return { ...dependency, match, status: "version-mismatch" };
        }

        return { ...dependency, match, status: "installed" };
    }

    function dependencySummaryLabel(): string {
        if (resolvedDependencies.length === 0 && resolvedNestedDependencies.length === 0) {
            if (selectedAuthorRequirements.length > 0) {
                return `${selectedAuthorRequirements.length} author link${selectedAuthorRequirements.length === 1 ? "" : "s"}`;
            }

            return "None listed";
        }

        if (totalDependencyIssueCount > 0) {
            return `${totalDependencyIssueCount} need attention`;
        }

        if (totalDependencyReviewCount > 0) {
            return `${totalDependencyReviewCount} review`;
        }

        return "Ready";
    }

    function apiDependencyReadinessLabel(): string {
        if (resolvedDependencies.length === 0) {
            return selectedDependencyMessage ? "No API rows returned" : "No API rows listed";
        }

        const parts = dependencyReadinessParts(
            dependencyInstalledCount,
            dependencyMissingCount,
            dependencyMismatchCount,
            dependencyReviewCount
        );
        return parts.length > 0 ? parts.join(" · ") : "Ready";
    }

    function authorRequirementReadinessLabel(): string {
        if (selectedAuthorRequirements.length === 0) {
            return "No author-linked hints";
        }

        return `${selectedAuthorRequirements.length} requirement ${selectedAuthorRequirements.length === 1 ? "link" : "links"} to review`;
    }

    function nestedDependencyReadinessLabel(): string {
        if (isResolvingNestedDependencies) {
            return "Checking nested files";
        }

        if (resolvedNestedDependencies.length > 0) {
            const parts = dependencyReadinessParts(
                nestedDependencyInstalledCount,
                nestedDependencyMissingCount,
                nestedDependencyMismatchCount,
                nestedDependencyReviewCount
            );
            return parts.length > 0 ? parts.join(" · ") : "Ready";
        }

        if (nestedDependencySummary) {
            return nestedDependencySummary;
        }

        return nestedDependencyCheckAvailable() ? "Nested check available" : "No nested file IDs";
    }

    function dependencyReadinessParts(
        installedCount: number,
        missingCount: number,
        mismatchCount: number,
        reviewCount: number
    ): string[] {
        const parts: string[] = [];
        if (installedCount > 0) {
            parts.push(`${installedCount} installed`);
        }

        if (missingCount > 0) {
            parts.push(`${missingCount} missing`);
        }

        if (mismatchCount > 0) {
            parts.push(`${mismatchCount} version`);
        }

        if (reviewCount > 0) {
            parts.push(`${reviewCount} review`);
        }

        return parts;
    }

    function dependencyStatusLabel(dependency: ResolvedDependency): string {
        switch (dependency.status) {
            case "installed":
                return dependency.match
                    ? `Installed: ${describeInstallSource(dependency.match)}${dependency.version_requirement ? `; ${dependency.version_requirement}` : ""}`
                    : "Installed";
            case "version-mismatch":
                return `Version mismatch: installed ${dependency.match?.version ?? "unknown"}; wants ${dependency.version ?? dependency.version_requirement ?? "review"}`;
            case "review":
                return dependency.version_requirement
                    ? `Installed: ${describeInstallSource(dependency.match)}; ${dependency.version_requirement} needs review`
                    : `Installed: ${describeInstallSource(dependency.match)}; version unknown`;
            default:
                return dependency.version_requirement
                    ? `Missing from this game folder; ${dependency.version_requirement}`
                    : "Missing from this game folder";
        }
    }

    function dependencyRequirementLabel(dependency: NexusModDependency): string {
        return dependency.version ? `v${dependency.version}` : dependency.version_requirement ?? "";
    }

    function buildInstallPlan(
        mod: NexusMod | null,
        file: NexusModFile | null,
        match: InstalledInventoryEntry | null,
        conflict: LocalConflict | null,
        dependencies: ResolvedDependency[],
        nestedDependencies: ResolvedNestedDependency[],
        authorRequirements: AuthorRequirementLink[],
        stagingPath: string | null,
        placement: InstallPlacement): InstallPlan {
        if (!mod || !file) {
            return {
                action: "Choose a file",
                target: "-",
                placement: "Auto",
                tone: "blocked",
                notes: ["Select a Nexus file before handing it to Vortex."]
            };
        }

        const autoTarget = match?.expectedLocation ?? inferNexusInstallTarget(mod, file);
        const explicitTarget = installPlacementTarget(placement);
        const target = explicitTarget ?? autoTarget;
        const notes: string[] = [];
        const missingCount = dependencies.filter(dependency => dependency.status === "missing").length;
        const mismatchCount = dependencies.filter(dependency => dependency.status === "version-mismatch").length;
        const reviewCount = dependencies.filter(dependency => dependency.status === "review").length;
        const nestedMissingCount = nestedDependencies.filter(source => source.dependency.status === "missing").length;
        const nestedMismatchCount = nestedDependencies.filter(source => source.dependency.status === "version-mismatch").length;
        const nestedReviewCount = nestedDependencies.filter(source => source.dependency.status === "review").length;

        if (!stagingPath) {
            notes.push("Vortex deployment metadata was not detected in this game folder yet.");
        }

        if (isReviewNexusFile(file)) {
            notes.push(`The selected file is marked ${fileChoiceCategoryLabel(file)} and should be reviewed before Vortex handoff.`);
        }

        if (missingCount > 0) {
            notes.push(`${missingCount} dependency ${missingCount === 1 ? "is" : "are"} missing locally.`);
        }

        if (mismatchCount > 0) {
            notes.push(`${mismatchCount} dependency ${mismatchCount === 1 ? "has" : "have"} a version mismatch.`);
        }

        if (reviewCount > 0) {
            notes.push(`${reviewCount} installed dependency ${reviewCount === 1 ? "needs" : "need"} a version review.`);
        }

        if (nestedMissingCount > 0) {
            notes.push(`${nestedMissingCount} nested dependency ${nestedMissingCount === 1 ? "is" : "are"} missing locally.`);
        }

        if (nestedMismatchCount > 0) {
            notes.push(`${nestedMismatchCount} nested dependency ${nestedMismatchCount === 1 ? "has" : "have"} a version mismatch.`);
        }

        if (nestedReviewCount > 0) {
            notes.push(`${nestedReviewCount} nested installed dependency ${nestedReviewCount === 1 ? "needs" : "need"} a version review.`);
        }

        if (authorRequirements.length > 0) {
            notes.push(`${authorRequirements.length} author-linked requirement ${authorRequirements.length === 1 ? "needs" : "need"} manual review.`);
        }

        if (conflict) {
            notes.push(`Local conflict detected across ${conflict.entries.length} matching installs.`);
        }

        if (placement === "manual-review") {
            notes.push("Manual placement review selected; confirm the author's directions before Vortex handoff.");
        }

        if (explicitTarget && match && match.expectedLocation !== explicitTarget) {
            notes.push(`Placement override differs from the detected local install target ${match.expectedLocation}.`);
        }

        if (match && !match.enabled) {
            notes.push("The local match is currently disabled.");
        }

        return {
            action: installPlanAction(mod, match),
            target,
            placement: installPlacementLabel(placement),
            tone: notes.length > 0 ? "review" : "ready",
            notes: notes.length > 0 ? notes : ["No local blockers detected from API dependency data."]
        };
    }

    function installPlanAction(mod: NexusMod, match: InstalledInventoryEntry | null): string {
        if (!match) {
            return "Install with Vortex";
        }

        if (updateVerdictForVersions(match.version, mod.version).isUpdate) {
            return "Update with Vortex";
        }

        return match.installSource === "vortex" ? "Repair/redeploy in Vortex" : "Import/update via Vortex";
    }

    function installPlacementLabel(placement: InstallPlacement): string {
        switch (placement) {
            case "bepinex-plugin":
                return "BepInEx plugin";
            case "redloader-mod":
                return "RedLoader mod";
            case "redloader-library":
                return "RedLoader library";
            case "manual-review":
                return "Manual review";
            default:
                return "Auto";
        }
    }

    function installPlacementTarget(placement: InstallPlacement): string | null {
        switch (placement) {
            case "bepinex-plugin":
                return "BepInEx/plugins";
            case "redloader-mod":
                return "Mods";
            case "redloader-library":
                return "Libs";
            case "manual-review":
                return "Manual review";
            default:
                return null;
        }
    }

    function inferNexusInstallTarget(mod: NexusMod, file: NexusModFile): string {
        const text = [
            mod.loader_type,
            mod.category_name,
            mod.name,
            file.category_name,
            file.name,
            file.description
        ].filter(Boolean).join(" ").toLowerCase();

        if (/\b(bepinex|plugin|plugins)\b/.test(text)) {
            return "BepInEx/plugins";
        }

        if (/\b(library|libraries|lib|libs)\b/.test(text)) {
            return "Libs";
        }

        return "Mods";
    }

    function sortNexusFilesForDisplay(files: NexusModFile[], recommendedFileId: number | null): NexusModFile[] {
        return [...files].sort((a, b) =>
            nexusFileDisplayRank(a, recommendedFileId) - nexusFileDisplayRank(b, recommendedFileId)
            || (b.uploaded_timestamp ?? 0) - (a.uploaded_timestamp ?? 0)
            || a.name.localeCompare(b.name)
        );
    }

    function nexusFileDisplayRank(file: NexusModFile, recommendedFileId: number | null): number {
        if (recommendedFileId !== null && file.file_id === recommendedFileId) {
            return 0;
        }

        const category = normalizedNexusFileCategory(file);
        if (file.is_primary || category === "main") {
            return 1;
        }

        if (category === "optional" || category === "miscellaneous") {
            return 2;
        }

        return isReviewNexusFile(file) ? 4 : 3;
    }

    function normalizedNexusFileCategory(file: NexusModFile): string {
        return (file.category_name ?? "unknown").trim().toLowerCase();
    }

    function isReviewNexusFile(file: NexusModFile): boolean {
        return /^(archived|old_version|removed)$/i.test(normalizedNexusFileCategory(file));
    }

    function fileChoiceCategoryLabel(file: NexusModFile): string {
        return (file.category_name ?? "file").replace(/_/g, " ");
    }

    function fileChoiceBadge(file: NexusModFile, recommendedFileId: number | null): string {
        if (recommendedFileId !== null && file.file_id === recommendedFileId) {
            return "Recommended";
        }

        if (isReviewNexusFile(file)) {
            return "Review";
        }

        const category = normalizedNexusFileCategory(file);
        if (file.is_primary) {
            return "Primary";
        }

        if (category === "main") {
            return "Main";
        }

        return "";
    }

    function describeFileChoiceReadiness(fileCount: number, recommendedCount: number, reviewCount: number, loading: boolean): string {
        if (fileCount === 0) {
            return loading ? "Loading files" : "No files returned";
        }

        const parts = [`${fileCount} total`];
        if (recommendedCount > 0) {
            parts.push(`${recommendedCount} main`);
        }

        if (reviewCount > 0) {
            parts.push(`${reviewCount} review`);
        }

        return parts.join(" · ");
    }

    function describeSelectedFileReadiness(file: NexusModFile | null, recommendedFileId: number | null, fileCount: number): string {
        if (!file) {
            return fileCount > 0 ? "Choose a file" : "No file selected";
        }

        const badge = fileChoiceBadge(file, recommendedFileId);
        const review = isReviewNexusFile(file) ? "Review" : "Ready";
        return [
            badge,
            fileChoiceCategoryLabel(file),
            `v${fileVersionLabel(file)}`,
            formatSizeKb(file.size),
            review
        ].filter(Boolean).join(" · ");
    }

    function describeFileReviewReadiness(fileCount: number, reviewCount: number): string {
        if (fileCount === 0) {
            return "No review files";
        }

        if (reviewCount === 0) {
            return "No archived/old files";
        }

        return `${reviewCount} archived/old`;
    }

    function installPlanToneLabel(tone: InstallPlanTone): string {
        switch (tone) {
            case "ready":
                return "Ready";
            case "blocked":
                return "Blocked";
            default:
                return "Review";
        }
    }

    async function openDependencyPage(dependency: ResolvedDependency) {
        if (!dependency.mod_id) {
            return;
        }

        await shell.open(`https://www.nexusmods.com/sonsoftheforest/mods/${dependency.mod_id}`);
    }

    async function openDependencyDetails(dependency: ResolvedDependency) {
        const dependencyMod = nexusModForDependency(dependency);
        if (!dependencyMod) {
            return;
        }

        await openModDetails(dependencyMod, { pushCurrent: true });
    }

    async function openAuthorRequirement(requirement: AuthorRequirementLink) {
        if (requirement.mod_id) {
            const linkedMod = mods.find(mod => mod.mod_id === requirement.mod_id)
                ?? knownNexusDetails[requirement.mod_id]
                ?? {
                    mod_id: requirement.mod_id,
                    name: requirement.label,
                    summary: "Requirement linked from the author's Nexus description.",
                    category_name: "Author requirement",
                    category_source: "inferred" as const,
                    loader_type: "Unknown"
                };
            await openModDetails(linkedMod, { pushCurrent: true });
            return;
        }

        await openExternalTarget(requirement.url);
    }

    async function openDependencyLocation(dependency: ResolvedDependency) {
        if (!dependency.match) {
            return;
        }

        await openInventoryLocation(dependency.match);
    }

    function nexusModForDependency(dependency: ResolvedDependency): NexusMod | null {
        if (!dependency.mod_id) {
            return null;
        }

        const loadedMod = mods.find(mod => mod.mod_id === dependency.mod_id);
        if (loadedMod) {
            return loadedMod;
        }

        const summary = [
            dependency.file_name,
            dependency.group_name,
            dependency.version ? `Requires ${dependency.version}` : ""
        ].filter(Boolean).join(" · ");

        return {
            mod_id: dependency.mod_id,
            name: dependency.mod_name,
            summary: summary || "Dependency returned by the Nexus file dependency API.",
            version: dependency.version,
            category_name: "Dependency",
            category_source: "inferred",
            loader_type: "Unknown"
        };
    }

    function findLocalConflicts(entries: InstalledInventoryEntry[]): LocalConflict[] {
        const groups = new Map<string, InstalledInventoryEntry[]>();

        for (const entry of entries) {
            for (const key of conflictCandidateKeys(entry)) {
                groups.set(key, [...(groups.get(key) ?? []), entry]);
            }
        }

        const seenGroups = new Set<string>();
        const conflicts: LocalConflict[] = [];

        for (const [key, groupedEntries] of groups) {
            const uniqueEntries = uniqueInventoryEntries(groupedEntries);
            if (uniqueEntries.length < 2) {
                continue;
            }

            const distinctInstallTargets = new Set(uniqueEntries.map(conflictInstallTarget));
            if (distinctInstallTargets.size < 2) {
                continue;
            }

            const signature = uniqueEntries.map(inventoryEntryKey).sort().join("||");
            if (seenGroups.has(signature)) {
                continue;
            }

            seenGroups.add(signature);
            conflicts.push({
                key,
                label: conflictLabel(key, uniqueEntries),
                entries: uniqueEntries
            });
        }

        return conflicts.sort((left, right) => left.label.localeCompare(right.label));
    }

    function conflictCandidateKeys(entry: InstalledInventoryEntry): string[] {
        const keys: string[] = [];
        const nexusId = numericNexusId(entry.nexusModId);
        if (nexusId) {
            keys.push(`nexus:${nexusId}`);
        }

        const nameKey = normalizeMatchKey(entry.name);
        if (nameKey.length >= 4) {
            keys.push(`name:${nameKey}`);
        }

        const idKey = normalizeMatchKey(entry.id);
        if (idKey.length >= 4 && idKey !== nameKey) {
            keys.push(`id:${idKey}`);
        }

        const packageKey = normalizeMatchKey(entry.vortexPackage);
        if (packageKey.length >= 4 && packageKey !== nameKey && packageKey !== idKey) {
            keys.push(`package:${packageKey}`);
        }

        return keys;
    }

    function uniqueInventoryEntries(entries: InstalledInventoryEntry[]): InstalledInventoryEntry[] {
        const seen = new Set<string>();
        return entries.filter(entry => {
            const key = inventoryEntryKey(entry);
            if (seen.has(key)) {
                return false;
            }

            seen.add(key);
            return true;
        });
    }

    function inventoryEntryKey(entry: InstalledInventoryEntry): string {
        return [
            entry.installSource,
            entry.expectedLocation,
            entry.nexusModId ?? "",
            entry.packagePath ?? "",
            entry.assemblyPath ?? "",
            entry.vortexPackage ?? "",
            entry.id,
            entry.name
        ].join("|");
    }

    function conflictInstallTarget(entry: InstalledInventoryEntry): string {
        return [
            entry.installSource,
            entry.expectedLocation,
            entry.packagePath ?? entry.assemblyPath ?? entry.vortexPackage ?? entry.id
        ].join("|");
    }

    function conflictLabel(key: string, entries: InstalledInventoryEntry[]): string {
        const displayName = entries
            .map(entry => entry.name)
            .sort((left, right) => left.length - right.length || left.localeCompare(right))[0] ?? "Duplicate mod";

        if (key.startsWith("nexus:")) {
            return `${displayName} (${key.replace("nexus:", "Nexus #")})`;
        }

        return displayName;
    }

    function conflictGroupSummary(conflict: LocalConflict): string {
        const sources = Array.from(new Set(conflict.entries.map(describeInstallSource))).join(" / ");
        const locations = Array.from(new Set(conflict.entries.map(entry => entry.expectedLocation))).join(" / ");
        return `${conflict.entries.length} installs · ${sources} · ${locations}`;
    }

    function conflictEntryPath(entry: InstalledInventoryEntry): string {
        return entry.packagePath ?? entry.assemblyPath ?? entry.vortexPackage ?? entry.id;
    }

    function conflictEntryActionLabel(entry: InstalledInventoryEntry): string {
        if (activeConflictEntryKey === inventoryEntryKey(entry)) {
            return "Saving...";
        }

        if (entry.installSource === "vortex") {
            return "Use Vortex";
        }

        return entry.enabled ? "Disable" : "Enable";
    }

    function isConflictedEntry(entry: InstalledInventoryEntry | null | undefined): boolean {
        return !!entry && localConflictEntryKeys.has(inventoryEntryKey(entry));
    }

    function conflictForEntry(entry: InstalledInventoryEntry | null | undefined): LocalConflict | null {
        if (!entry) {
            return null;
        }

        const key = inventoryEntryKey(entry);
        return localConflicts.find(conflict =>
            conflict.entries.some(conflictEntry => inventoryEntryKey(conflictEntry) === key)
        ) ?? null;
    }

    function hasNexusUpdate(mod: NexusMod): boolean {
        return nexusUpdateLabel(mod) === "Update available";
    }

    function hasInventoryUpdate(entry: InstalledInventoryEntry): boolean {
        return inventoryUpdateLabel(entry) === "Update available";
    }

    function nexusNeedsAttention(mod: NexusMod): boolean {
        const match = installedMatch(mod);
        return hasNexusUpdate(mod)
            || isConflictedEntry(match)
            || (!!match && !match.enabled)
            || (!match && isNexusModTracked(mod.mod_id));
    }

    function inventoryNeedsAttention(entry: InstalledInventoryEntry): boolean {
        return hasInventoryUpdate(entry)
            || isConflictedEntry(entry)
            || !entry.enabled;
    }

    function inventoryEntryNeedsEndorsement(entry: InstalledInventoryEntry): boolean {
        if (!endorsementsLoaded) {
            return false;
        }

        const modId = numericNexusId(entry.nexusModId);
        return !!modId && !isNexusModEndorsed(modId);
    }

    function nexusModNeedsEndorsement(mod: NexusMod): boolean {
        const match = installedMatch(mod);
        return !!match && inventoryEntryNeedsEndorsement(match);
    }

    function inventoryHasNexusModId(modId: number | string | null | undefined): boolean {
        const normalizedModId = numericNexusId(modId);
        return !!normalizedModId && inventory.some(entry => numericNexusId(entry.nexusModId) === normalizedModId);
    }

    function sortNexusMods(items: NexusMod[]): NexusMod[] {
        return [...items].sort((left, right) => {
            switch (selectedNexusSort) {
                case "updated":
                    return compareNumbers(modUpdatedValue(right), modUpdatedValue(left)) || compareText(left.name, right.name);
                case "downloads":
                    return compareNumbers(right.mod_downloads, left.mod_downloads) || compareText(left.name, right.name);
                case "endorsements":
                    return compareNumbers(right.endorsement_count, left.endorsement_count) || compareText(left.name, right.name);
                case "name":
                    return compareText(left.name, right.name);
                case "version":
                    return compareText(left.version ?? "", right.version ?? "") || compareText(left.name, right.name);
                default:
                    return compareNumbers(nexusAttentionRank(left), nexusAttentionRank(right))
                        || compareNumbers(modUpdatedValue(right), modUpdatedValue(left))
                        || compareText(left.name, right.name);
            }
        });
    }

    function sortInstalledEntries(items: InstalledInventoryEntry[]): InstalledInventoryEntry[] {
        return [...items].sort((left, right) => {
            switch (selectedInstalledSort) {
                case "name":
                    return compareText(left.name, right.name);
                case "source":
                    return compareText(left.installSource, right.installSource) || compareText(left.name, right.name);
                case "location":
                    return compareText(left.expectedLocation, right.expectedLocation) || compareText(left.name, right.name);
                case "state":
                    return compareNumbers(Number(right.enabled), Number(left.enabled)) || compareText(left.name, right.name);
                case "version":
                    return compareText(left.version ?? "", right.version ?? "") || compareText(left.name, right.name);
                default:
                    return compareNumbers(installedAttentionRank(left), installedAttentionRank(right))
                        || compareText(left.name, right.name);
            }
        });
    }

    function nexusAttentionRank(mod: NexusMod): number {
        const match = installedMatch(mod);
        if (isConflictedEntry(match)) {
            return 0;
        }

        if (hasNexusUpdate(mod)) {
            return 1;
        }

        if (match && !match.enabled) {
            return 2;
        }

        if (!match && isNexusModTracked(mod.mod_id)) {
            return 3;
        }

        return match ? 4 : 5;
    }

    function installedAttentionRank(entry: InstalledInventoryEntry): number {
        if (isConflictedEntry(entry)) {
            return 0;
        }

        if (hasInventoryUpdate(entry)) {
            return 1;
        }

        if (!entry.enabled) {
            return 2;
        }

        return 3;
    }

    function modUpdatedValue(mod: NexusMod): number {
        if (mod.updated_timestamp) {
            return mod.updated_timestamp;
        }

        const parsed = mod.updated_time ? Date.parse(mod.updated_time) : Number.NaN;
        return Number.isFinite(parsed) ? parsed / 1000 : 0;
    }

    function compareText(left: string, right: string): number {
        return left.localeCompare(right, undefined, { numeric: true, sensitivity: "base" });
    }

    function compareNumbers(left: number | undefined, right: number | undefined): number {
        return (left ?? 0) - (right ?? 0);
    }

    function buildNexusCategoryOptions(categories: NexusCategory[], loadedMods: NexusMod[]): string[] {
        const ordered = categories
            .map(category => category.name)
            .filter((name, index, names) => name && names.findIndex(candidate => candidate.toLowerCase() === name.toLowerCase()) === index);
        const orderedNames = new Set(ordered.map(name => name.toLowerCase()));
        const extra = Array.from(new Set(loadedMods
            .map(mod => mod.category_name)
            .filter((name): name is string => !!name && !orderedNames.has(name.toLowerCase()))))
            .sort((left, right) => left.localeCompare(right));

        return [...ordered, ...extra];
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

        if (!nexusModMatchesTypeFilter(mod, match)) {
            return false;
        }

        if (selectedInstallFilter === "installed" && !match) {
            return false;
        }

        if (selectedInstallFilter === "missing" && match) {
            return false;
        }

        if (selectedInstallFilter === "updates" && !hasNexusUpdate(mod)) {
            return false;
        }

        if (selectedInstallFilter === "disabled" && (!match || match.enabled)) {
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

        if (selectedInstallFilter === "tracked" && !isNexusModTracked(mod.mod_id)) {
            return false;
        }

        if (selectedInstallFilter === "endorsements" && !nexusModNeedsEndorsement(mod)) {
            return false;
        }

        if (selectedInstallFilter === "conflicts" && !isConflictedEntry(match)) {
            return false;
        }

        if (selectedInstallFilter === "attention" && !nexusNeedsAttention(mod)) {
            return false;
        }

        return true;
    }

    function matchesInstalledFilters(entry: InstalledInventoryEntry): boolean {
        const search = nexusSearchTerm.trim().toLowerCase();

        if (search && ![
            entry.name,
            entry.author ?? "",
            entry.version ?? "",
            entry.vortexPackage ?? "",
            entry.expectedLocation,
            entry.loaderType,
            loaderTypeLabel(entry),
            describeInstallSource(entry)
        ].some(value => value.toLowerCase().includes(search))) {
            return false;
        }

        if (!installedEntryMatchesTypeFilter(entry)) {
            return false;
        }

        if (selectedInstallFilter === "vortex" && entry.installSource !== "vortex") {
            return false;
        }

        if (selectedInstallFilter === "native" && entry.installSource !== "native") {
            return false;
        }

        if (selectedInstallFilter === "manual" && entry.installSource !== "manual") {
            return false;
        }

        if (selectedInstallFilter === "updates" && !hasInventoryUpdate(entry)) {
            return false;
        }

        if (selectedInstallFilter === "disabled" && entry.enabled) {
            return false;
        }

        if (selectedInstallFilter === "missing") {
            return false;
        }

        if (selectedInstallFilter === "tracked" && !isNexusModTracked(entry.nexusModId)) {
            return false;
        }

        if (selectedInstallFilter === "endorsements" && !inventoryEntryNeedsEndorsement(entry)) {
            return false;
        }

        if (selectedInstallFilter === "conflicts" && !isConflictedEntry(entry)) {
            return false;
        }

        if (selectedInstallFilter === "attention" && !inventoryNeedsAttention(entry)) {
            return false;
        }

        return true;
    }

    function nexusModMatchesTypeFilter(mod: NexusMod, match: InstalledInventoryEntry | null): boolean {
        if (selectedModTypeFilter === "all") {
            return true;
        }

        if (selectedModTypeFilter === "vortex" || selectedModTypeFilter === "native" || selectedModTypeFilter === "manual") {
            return match?.installSource === selectedModTypeFilter;
        }

        if (match) {
            return match.loaderType === selectedModTypeFilter;
        }

        const text = [
            mod.loader_type,
            mod.category_name,
            mod.name,
            mod.summary,
            mod.description
        ].filter(Boolean).join(" ").toLowerCase();

        switch (selectedModTypeFilter) {
            case "bepinex-plugin":
                return /\b(bepinex|plugin|plugins)\b/.test(text);
            case "redloader-library":
                return /\b(redloader|red loader)\b/.test(text) && /\b(library|libraries|lib|libs)\b/.test(text);
            case "redloader-mod":
                return /\b(redloader|red loader)\b/.test(text) && !/\b(library|libraries|lib|libs)\b/.test(text);
            default:
                return true;
        }
    }

    function installedEntryMatchesTypeFilter(entry: InstalledInventoryEntry): boolean {
        switch (selectedModTypeFilter) {
            case "all":
                return true;
            case "vortex":
            case "native":
            case "manual":
                return entry.installSource === selectedModTypeFilter;
            default:
                return entry.loaderType === selectedModTypeFilter;
        }
    }

    function loaderTypeLabel(entry: InstalledInventoryEntry): string {
        switch (entry.loaderType) {
            case "bepinex-plugin":
                return "BepInEx plugin";
            case "redloader-library":
                return "RedLoader library";
            default:
                return "RedLoader mod";
        }
    }

    function nexusMembershipLabel(): string {
        if (session.user?.is_premium) {
            return session.user.membership_tier
                ? `Premium ${titleCase(session.user.membership_tier)}`
                : "Premium";
        }

        if (session.user?.is_supporter) {
            return session.user.membership_tier
                ? `Supporter ${titleCase(session.user.membership_tier)}`
                : "Supporter";
        }

        if (session.user?.membership_tier) {
            return titleCase(session.user.membership_tier);
        }

        return session.user?.name ? "Member" : "Nexus user";
    }

    function nexusMembershipNote(): string {
        if (session.user?.is_premium) {
            return "Premium benefits detected where the Nexus API permits them.";
        }

        if (session.user?.is_supporter) {
            return "Supporter status detected.";
        }

        if (session.user?.membership_tier) {
            return "Nexus membership tier returned by the API.";
        }

        return "Standard Nexus account.";
    }

    function titleCase(value: string): string {
        return value
            .replace(/[_-]+/g, " ")
            .replace(/\s+/g, " ")
            .trim()
            .replace(/\b\w/g, letter => letter.toUpperCase());
    }

    function vortexActionLabel(mod: NexusMod): string {
        const match = installedMatch(mod);
        if (!match) {
            return "Install with Vortex";
        }

        if (match.installSource === "vortex") {
            return nexusUpdateVerdict(mod, match).isUpdate ? "Update in Vortex" : "Reinstall in Vortex";
        }

        return "Get Vortex File";
    }

    function nexusUpdateVerdict(mod: NexusMod, match: InstalledInventoryEntry | null = installedMatch(mod)): UpdateVerdict {
        if (!match) {
            return isNexusModTracked(mod.mod_id)
                ? {
                    label: "Tracked",
                    tone: "tracked",
                    isUpdate: false,
                    needsReview: false,
                    reason: "Tracked on Nexus but not installed in this game folder."
                }
                : {
                    label: "Not installed",
                    tone: "neutral",
                    isUpdate: false,
                    needsReview: false,
                    reason: "No matching local install was detected."
                };
        }

        return updateVerdictForVersions(match.version, mod.version);
    }

    function nexusUpdateLabel(mod: NexusMod): string {
        return nexusUpdateVerdict(mod).label;
    }

    function selectedModUpdateLabel(): string {
        return selectedMod ? selectedModUpdateVerdict().label : "-";
    }

    function selectedModUpdateTone(): UpdateTone {
        return selectedModUpdateVerdict().tone;
    }

    function selectedModUpdateReason(): string {
        return selectedModUpdateVerdict().reason;
    }

    function selectedModUpdateVerdict(): UpdateVerdict {
        return selectedMod
            ? nexusUpdateVerdict(selectedModDetails ?? selectedMod)
            : {
                label: "-",
                tone: "neutral",
                isUpdate: false,
                needsReview: false,
                reason: "No Nexus mod is selected."
            };
    }

    function inventoryUpdateVerdict(entry: InstalledInventoryEntry): UpdateVerdict {
        const onlineMod = findOnlineModForEntry(entry);
        if (!onlineMod) {
            return entry.installSource === "vortex"
                ? {
                    label: "Refresh Nexus",
                    tone: "review",
                    isUpdate: false,
                    needsReview: true,
                    reason: "This Vortex-managed install has no loaded Nexus catalog match yet."
                }
                : {
                    label: "Local only",
                    tone: "neutral",
                    isUpdate: false,
                    needsReview: false,
                    reason: "No Nexus catalog match was found for this local package."
                };
        }

        return updateVerdictForVersions(entry.version, onlineMod.version);
    }

    function inventoryUpdateLabel(entry: InstalledInventoryEntry): string {
        return inventoryUpdateVerdict(entry).label;
    }

    function findOnlineModForEntry(entry: InstalledInventoryEntry): NexusMod | null {
        const entryNexusId = numericNexusId(entry.nexusModId);
        if (entryNexusId && knownNexusDetails[entryNexusId]) {
            return knownNexusDetails[entryNexusId];
        }

        return mods.find(mod =>
            (entryNexusId && mod.mod_id === entryNexusId)
            || findMatchingInstall([entry], mod.name, mod.mod_id, [
                mod.author ?? "",
                mod.uploaded_by ?? ""
            ]) !== null
        ) ?? null;
    }

    function nexusModForInstalledEntry(entry: InstalledInventoryEntry): NexusMod | null {
        const modId = numericNexusId(entry.nexusModId);
        if (!modId) {
            return null;
        }

        return findOnlineModForEntry(entry) ?? {
            mod_id: modId,
            name: entry.name,
            version: entry.version,
            author: entry.author,
            category_name: "Installed",
            loader_type: loaderTypeLabel(entry),
            summary: `${describeInstallSource(entry)} package detected in ${entry.expectedLocation}.`
        };
    }

    function updateTone(label: string): UpdateTone {
        if (label === "Update available") {
            return "update";
        }

        if (label === "Current" || label === "Local newer") {
            return "current";
        }

        if (label === "Tracked") {
            return "tracked";
        }

        if (label === "Review version" || label === "Refresh Nexus") {
            return "review";
        }

        return "neutral";
    }

    function versionsDiffer(left?: string, right?: string): boolean {
        const comparison = compareVersionValues(left, right);
        return comparison !== "same" && comparison !== "unknown";
    }

    function updateVerdictForVersions(localVersion?: string, nexusVersion?: string): UpdateVerdict {
        const comparison = compareVersionValues(localVersion, nexusVersion);

        switch (comparison) {
            case "remote-newer":
                return {
                    label: "Update available",
                    tone: "update",
                    isUpdate: true,
                    needsReview: false,
                    reason: `Nexus version ${nexusVersion} is newer than local ${localVersion}.`
                };
            case "local-newer":
                return {
                    label: "Local newer",
                    tone: "current",
                    isUpdate: false,
                    needsReview: true,
                    reason: `Local version ${localVersion} appears newer than Nexus ${nexusVersion}.`
                };
            case "different":
                return {
                    label: "Review version",
                    tone: "review",
                    isUpdate: false,
                    needsReview: true,
                    reason: `Local version ${localVersion ?? "-"} differs from Nexus ${nexusVersion ?? "-"} but could not be ordered safely.`
                };
            case "same":
                return {
                    label: "Current",
                    tone: "current",
                    isUpdate: false,
                    needsReview: false,
                    reason: `Local version ${localVersion} matches Nexus ${nexusVersion}.`
                };
            default:
                if (nexusVersion && !localVersion) {
                    return {
                        label: "Review version",
                        tone: "review",
                        isUpdate: false,
                        needsReview: true,
                        reason: `Nexus reports ${nexusVersion}, but the local install does not expose a comparable version.`
                    };
                }

                return {
                    label: "Installed",
                    tone: "neutral",
                    isUpdate: false,
                    needsReview: false,
                    reason: localVersion
                        ? "Nexus did not return a comparable version for this match."
                        : "Neither local inventory nor Nexus exposed a comparable version."
                };
        }
    }

    function compareVersionValues(localVersion?: string, nexusVersion?: string): VersionComparison {
        const local = normalizedSemver(localVersion);
        const remote = normalizedSemver(nexusVersion);

        if (local && remote) {
            if (semver.eq(local, remote)) {
                return "same";
            }

            if (semver.gt(remote, local)) {
                return "remote-newer";
            }

            if (semver.lt(remote, local)) {
                return "local-newer";
            }
        }

        const localToken = normalizedVersionToken(localVersion);
        const remoteToken = normalizedVersionToken(nexusVersion);
        if (!localToken || !remoteToken) {
            return "unknown";
        }

        return localToken === remoteToken ? "same" : "different";
    }

    function normalizedSemver(value?: string): string | null {
        const raw = value?.trim();
        if (!raw) {
            return null;
        }

        const cleaned = raw
            .replace(/^[vV]\s*/, "")
            .replace(/[_\s]+/g, "-")
            .replace(/[^0-9A-Za-z.+-]/g, "");
        const valid = semver.valid(cleaned);
        if (valid) {
            return valid;
        }

        return semver.coerce(cleaned)?.version ?? null;
    }

    function normalizedVersionToken(value?: string): string | null {
        const normalized = value?.trim().toLowerCase()
            .replace(/^[vV]\s*/, "")
            .replace(/[_\s]+/g, "")
            .replace(/[^a-z0-9.+-]/g, "");

        return normalized || null;
    }

    function viewLabel(view: NexusView): string {
        switch (view) {
            case "all":
                return "All";
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

    function formatCatalogLoadedAt(value: number | null): string {
        if (!value) {
            return "Not loaded";
        }

        return `Loaded ${new Date(value).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}`;
    }

    function formatNumber(value?: number): string {
        return typeof value === "number" ? value.toLocaleString() : "-";
    }

    function rateLimitLabel(remaining?: string, limit?: string): string {
        return `${remaining ?? "-"} / ${limit ?? "-"}`;
    }

    function rateLimitStyle(remaining?: string, limit?: string): string {
        return `--rate-percent: ${rateLimitPercent(remaining, limit)}%`;
    }

    function rateLimitPercent(remaining?: string, limit?: string): number {
        const remainingValue = Number(remaining);
        const limitValue = Number(limit);
        if (!Number.isFinite(remainingValue) || !Number.isFinite(limitValue) || limitValue <= 0) {
            return 0;
        }

        return Math.max(0, Math.min(100, Math.round((remainingValue / limitValue) * 100)));
    }

    function isRateLimitLow(remaining?: string, limit?: string): boolean {
        return rateLimitPercent(remaining, limit) > 0 && rateLimitPercent(remaining, limit) <= 10;
    }

    function isRateLimitWarning(remaining?: string, limit?: string): boolean {
        const percent = rateLimitPercent(remaining, limit);
        return percent > 10 && percent <= 25;
    }

    function formatSizeKb(value?: number): string {
        if (!value) {
            return "-";
        }

        if (value > 1024 * 1024) {
            return `${(value / 1024 / 1024).toFixed(1)} GB`;
        }

        if (value > 1024) {
            return `${(value / 1024).toFixed(1)} MB`;
        }

        return `${Math.round(value)} KB`;
    }

    type NexusRichNode =
        | { kind: "text"; value: string }
        | { kind: "tag"; name: string; attr?: string; rawAttrs?: string; children: NexusRichNode[] };
    type NexusBlockChunk =
        | { kind: "block"; value: string }
        | { kind: "line"; value: string }
        | { kind: "list"; value: string; ordered: boolean; style?: string }
        | { kind: "table"; value: string };
    type NexusPlainListLine = {
        explicit: boolean;
        ordered: boolean;
        raw: string;
        style?: string;
        value: string;
    };
    type NexusTableCell = {
        value: string;
        header: boolean;
        colspan?: number;
        rowspan?: number;
    };

    function plainText(value?: string): string {
        return normalizeNexusMarkup(value)
            .replace(/\[img[^\]]*\][\s\S]*?\[\/img\]/gi, " ")
            .replace(/\[url=([^\]]+)\]([\s\S]*?)\[\/url\]/gi, "$2 ($1)")
            .replace(/\[url\]([\s\S]*?)\[\/url\]/gi, "$1")
            .replace(/\[\*\s*=([^\]]+)\]/g, (_match, label: string) => `\n- ${safeNexusListItemLabel(label)} `)
            .replace(/\[\*\]/g, "\n- ")
            .replace(/\[\s*li(?:\s[^\]]*|=[^\]]*)?\]/gi, "\n- ")
            .replace(/\[\/\s*li\]/gi, "\n")
            .replace(/\[\/?(?:table|tbody|thead|tfoot)[^\]]*\]/gi, "\n")
            .replace(/\[\/?tr[^\]]*\]/gi, "\n")
            .replace(/\[(?:td|th)[^\]]*\]/gi, "")
            .replace(/\[\/(?:td|th)\]/gi, " | ")
            .replace(/\[hr\s*\/?\]/gi, "\n---\n")
            .replace(/\[line\s*\/?\]/gi, "\n---\n")
            .replace(/\[(?:rule|divider|separator)\s*\/?\]/gi, "\n---\n")
            .replace(/\[(youtube|video|media|embed)[^\]]*\]([\s\S]*?)\[\/\1\]/gi, (_match, tag: string, body: string) => `${tag === "youtube" ? "YouTube" : "Media"}: ${body}`)
            .replace(/\[(?:nextcol|nextcolumn)\s*\/?\]/gi, "\n")
            .replace(/\[\/?(?:columns|cols|column|col|tabs|tab|note|info|warning|important|tip|box|panel|fieldset|notice|success|danger|error|collapse|details|accordion|accordionitem|caption|dl|dt|dd)[^\]]*\]/gi, "\n")
            .replace(/\[\/?\s*(?:list|ul|ol|olist)[^\]]*\]/gi, "")
            .replace(/\[\/?(?:b|i|u|s|strike|del|sub|sup|small|big|mark|abbr|acronym|cite|q|size|color|background|bgcolor|highlight|font|center|left|right|justify|align|indent|quote|spoiler|code|pre|tt|kbd|samp|var|heading|h|header|title|subtitle|h[1-6]|float|clear|div|p|paragraph|span)[^\]]*\]/gi, "")
            .replace(/\[\/?[a-z0-9_-]+[^\]]*\]/gi, "")
            .replace(/[ \t]+/g, " ")
            .replace(/\n\s+/g, "\n")
            .replace(/\n{3,}/g, "\n\n")
            .trim();
    }

    function renderNexusRichText(value?: string, fallback = ""): string {
        const normalized = normalizeNexusMarkup(value);
        if (!normalized) {
            return `<p>${escapeHtml(fallback)}</p>`;
        }

        return renderNexusBlocks(normalized, 0);
    }

    function renderOptionalNexusRichText(value?: string): string {
        const normalized = normalizeNexusMarkup(value);
        return normalized ? renderNexusBlocks(normalized, 0) : "";
    }

    function normalizeNexusMarkup(value?: string): string {
        let text = (value ?? "")
            .replace(/\r\n?/g, "\n")
            .replace(/<br\s*\/?>/gi, "\n")
            .replace(/<details\b[^>]*>\s*<summary\b[^>]*>([\s\S]*?)<\/summary>([\s\S]*?)<\/details>/gi, (_match, label: string, body: string) => `\n\n[spoiler=${plainText(label)}]${body}[/spoiler]\n\n`)
            .replace(/<(iframe|embed|object|video|audio)\b([^>]*)>([\s\S]*?)<\/\1>/gi, (_match, tag: string, attrs: string, body: string) => {
                return nexusHtmlMediaMarkup(tag, attrs, body);
            })
            .replace(/<(iframe|embed)\b([^>]*)\/?>/gi, (_match, tag: string, attrs: string) => {
                return nexusHtmlMediaMarkup(tag, attrs);
            })
            .replace(/<h([1-6])\b[^>]*>([\s\S]*?)<\/h\1>/gi, "\n\n[heading=$1]$2[/heading]\n\n")
            .replace(/<blockquote\b([^>]*)>/gi, (_match, attrs: string) => {
                const cite = safeNexusLabel(htmlAttribute(attrs, "cite") ?? "");
                return cite ? `\n\n[quote=${cite}]` : "\n\n[quote]";
            })
            .replace(/<\/blockquote>/gi, "[/quote]\n\n")
            .replace(/<hr\b[^>]*\/?>/gi, "\n\n[hr]\n\n")
            .replace(/<pre\b[^>]*>/gi, "\n\n[code]")
            .replace(/<\/pre>/gi, "[/code]\n\n")
            .replace(/<code\b[^>]*>/gi, "[code]")
            .replace(/<\/code>/gi, "[/code]")
            .replace(/<(kbd|samp|tt|var)\b[^>]*>/gi, "[code]")
            .replace(/<\/(?:kbd|samp|tt|var)>/gi, "[/code]")
            .replace(/<center\b[^>]*>/gi, "\n\n[center]")
            .replace(/<\/center>/gi, "[/center]\n\n")
            .replace(/<(p|div|section|article)\b([^>]*)>([\s\S]*?)<\/\1>/gi, (_match, _tag: string, attrs: string, body: string) => {
                const alignment = htmlAlignmentAttribute(attrs);
                const indent = htmlIndentLevelAttribute(attrs);
                const styled = hasNexusBlockishMarkup(body) ? body : nexusHtmlStyledInline(attrs, body);
                const aligned = alignment ? `[align=${alignment}]${styled}[/align]` : styled;
                const indented = indent ? `[indent=${indent}]${aligned}[/indent]` : aligned;
                return `\n\n${indented}\n\n`;
            })
            .replace(/<font\b([^>]*)>([\s\S]*?)<\/font>/gi, (_match, attrs: string, body: string) => {
                return nexusHtmlStyledInline(attrs, body);
            })
            .replace(/<span\b([^>]*)>([\s\S]*?)<\/span>/gi, (_match, attrs: string, body: string) => {
                return nexusHtmlStyledInline(attrs, body);
            })
            .replace(/<small\b[^>]*>([\s\S]*?)<\/small>/gi, "[small]$1[/small]")
            .replace(/<big\b[^>]*>([\s\S]*?)<\/big>/gi, "[big]$1[/big]")
            .replace(/<(abbr|acronym)\b([^>]*)>([\s\S]*?)<\/\1>/gi, (_match, _tag: string, attrs: string, body: string) => {
                const title = safeNexusLabel(htmlAttribute(attrs, "title") ?? "");
                return title ? `[abbr=${title}]${body}[/abbr]` : body;
            })
            .replace(/<cite\b[^>]*>([\s\S]*?)<\/cite>/gi, "[cite]$1[/cite]")
            .replace(/<q\b[^>]*>([\s\S]*?)<\/q>/gi, "[q]$1[/q]")
            .replace(/<mark\b([^>]*)>([\s\S]*?)<\/mark>/gi, (_match, attrs: string, body: string) => {
                const background = htmlBackgroundColorAttribute(attrs) ?? "yellow";
                return `[highlight=${background}]${body}[/highlight]`;
            })
            .replace(/<sub\b[^>]*>([\s\S]*?)<\/sub>/gi, "[sub]$1[/sub]")
            .replace(/<sup\b[^>]*>([\s\S]*?)<\/sup>/gi, "[sup]$1[/sup]")
            .replace(/<strong\b[^>]*>|<b\b[^>]*>/gi, "[b]")
            .replace(/<\/strong>|<\/b>/gi, "[/b]")
            .replace(/<em\b[^>]*>|<i\b[^>]*>/gi, "[i]")
            .replace(/<\/em>|<\/i>/gi, "[/i]")
            .replace(/<u\b[^>]*>/gi, "[u]")
            .replace(/<\/u>/gi, "[/u]")
            .replace(/<s\b[^>]*>|<strike\b[^>]*>/gi, "[s]")
            .replace(/<\/s>|<\/strike>/gi, "[/s]")
            .replace(/<del\b[^>]*>/gi, "[s]")
            .replace(/<\/del>/gi, "[/s]")
            .replace(/<figure\b[^>]*>/gi, "\n\n[box=Media]\n")
            .replace(/<\/figure>/gi, "\n[/box]\n\n")
            .replace(/<figcaption\b[^>]*>([\s\S]*?)<\/figcaption>/gi, "\n[caption]$1[/caption]\n")
            .replace(/<dl\b[^>]*>/gi, "\n[list]\n")
            .replace(/<\/dl>/gi, "\n[/list]\n")
            .replace(/<dt\b[^>]*>/gi, "\n[*][b]")
            .replace(/<\/dt>/gi, "[/b] ")
            .replace(/<dd\b[^>]*>/gi, "")
            .replace(/<\/dd>/gi, "\n")
            .replace(/<table\b[^>]*>/gi, "\n\n[table]\n")
            .replace(/<\/table>/gi, "\n[/table]\n\n")
            .replace(/<\/?(?:tbody|thead|tfoot)\b[^>]*>/gi, "")
            .replace(/<tr\b[^>]*>/gi, "\n[tr]")
            .replace(/<\/tr>/gi, "[/tr]\n")
            .replace(/<th\b([^>]*)>/gi, (_match, attrs: string) => `[th${nexusHtmlTableCellAttributes(attrs)}]`)
            .replace(/<\/th>/gi, "[/th]")
            .replace(/<td\b([^>]*)>/gi, (_match, attrs: string) => `[td${nexusHtmlTableCellAttributes(attrs)}]`)
            .replace(/<\/td>/gi, "[/td]")
            .replace(/<ul\b[^>]*>/gi, "\n[list]\n")
            .replace(/<\/ul>/gi, "\n[/list]\n")
            .replace(/<ol\b([^>]*)>/gi, (_match, attrs: string) => {
                const type = nexusOrderedListType(htmlAttribute(attrs, "type") ?? "");
                return `\n[olist${type ? `=${type}` : ""}]\n`;
            })
            .replace(/<\/ol>/gi, "\n[/olist]\n")
            .replace(/<li\b[^>]*>/gi, "\n[*]")
            .replace(/<\/li>/gi, "\n")
            .replace(/<\/p>/gi, "\n\n")
            .replace(/<p\b[^>]*>/gi, "");

        text = text
            .replace(/<a\b([^>]*)>([\s\S]*?)<\/a>/gi, (_match, attrs: string, label: string) => {
                const href = htmlAttribute(attrs, "href");
                return href ? `[url=${href}]${label}[/url]` : label;
            })
            .replace(/<img\b([^>]*)>/gi, (_match, attrs: string) => {
                const src = htmlAttribute(attrs, "src");
                return src ? `[img${nexusHtmlImageAttributes(attrs)}]${src}[/img]` : "";
            })
            .replace(/<[^>]*>/g, " ");

        return decodeHtmlEntities(text)
            .replace(/\[\/\*\]/g, "")
            .replace(/\[(?:br|break)\s*\/?\]/gi, "\n")
            .replace(/\[(?:pre|raw|noparse)\]/gi, "[code]")
            .replace(/\[\/(?:pre|raw|noparse)\]/gi, "[/code]")
            .replace(/\[(?:p|paragraph)\]/gi, "\n\n")
            .replace(/\[\/(?:p|paragraph)\]/gi, "\n\n")
            .replace(/\[(?:imgleft|imageleft)([^\]]*)\]/gi, (_match, attrs: string) => nexusImageAliasOpeningTag("left", attrs))
            .replace(/\[\/(?:imgleft|imageleft)\]/gi, "[/img]")
            .replace(/\[(?:imgright|imageright)([^\]]*)\]/gi, (_match, attrs: string) => nexusImageAliasOpeningTag("right", attrs))
            .replace(/\[\/(?:imgright|imageright)\]/gi, "[/img]")
            .replace(/\[(?:imgcenter|imagecenter)([^\]]*)\]/gi, (_match, attrs: string) => nexusImageAliasOpeningTag("center", attrs))
            .replace(/\[\/(?:imgcenter|imagecenter)\]/gi, "[/img]")
            .replace(/\[(?:thumb|thumbnail|image)([^\]]*)\]/gi, (_match, attrs: string) => normalizedNexusImageOpeningTag(attrs))
            .replace(/\[\/(?:thumb|thumbnail|image)\]/gi, "[/img]")
            .replace(/\[\s*li(?=[\s=\]/])([^\]]*)\]/gi, (_match, attrs: string) => {
                const label = safeNexusListItemLabel(nexusBbTagAttribute("li", attrs));
                return label ? `\n[*][b]${label}[/b] ` : "\n[*]";
            })
            .replace(/\[\/\s*li\]/gi, "\n")
            .replace(/\[dl[^\]]*\]/gi, "\n[list]\n")
            .replace(/\[\/dl\]/gi, "\n[/list]\n")
            .replace(/\[dt[^\]]*\]/gi, "\n[*][b]")
            .replace(/\[\/dt\]/gi, "[/b] ")
            .replace(/\[dd[^\]]*\]/gi, "")
            .replace(/\[\/dd\]/gi, "\n")
            .replace(/\[h([1-6])\]/gi, "[heading=$1]")
            .replace(/\[\/h[1-6]\]/gi, "[/heading]")
            .replace(/\[(?:headline|subhead|subheading)(?:=([^\]]+))?\]/gi, (_match, level: string | undefined) => `[heading=${safeNexusHeadingLevel(level ?? "3")}]`)
            .replace(/\[\/(?:headline|subhead|subheading)\]/gi, "[/heading]")
            .replace(/\[(url|img|color|background|bgcolor|highlight|size|align|indent|quote|spoiler|collapse|details|accordion|accordionitem|heading|h|header|title|subtitle|caption|float|youtube|video|media|embed|list|olist|ol|ul|columns|cols|column|col|tabs|tab|note|info|warning|important|tip|box|panel|fieldset|notice|success|danger|error|small|big|mark|abbr|acronym)([ \t][^\]]+)\]/gi, (_match, tag: string, attrs: string) => normalizeNexusBbOpeningTag(tag, attrs))
            .replace(/\[(?:h|header|title)(?:=([^\]]+))?\]/gi, (_match, level: string | undefined) => `[heading=${safeNexusHeadingLevel(level ?? "2")}]`)
            .replace(/\[\/(?:h|header|title)\]/gi, "[/heading]")
            .replace(/\[subtitle(?:=[^\]]+)?\]/gi, "[heading=4]")
            .replace(/\[\/subtitle\]/gi, "[/heading]")
            .replace(/\[(?:ul|list)(?:=([^\]]+))?\]/gi, (_match, attr: string | undefined) => {
                if (isOrderedNexusListAttr(attr)) {
                    const type = nexusOrderedListType(attr);
                    return `\n[olist${type ? `=${type}` : ""}]\n`;
                }

                return "\n[list]\n";
            })
            .replace(/\[ol(?:=([^\]]+))?\]/gi, (_match, attr: string | undefined) => {
                const type = nexusOrderedListType(attr);
                return `\n[olist${type ? `=${type}` : ""}]\n`;
            })
            .replace(/\[\/(?:ul|ol|list|olist)\]/gi, "\n[/list]\n")
            .replace(/\[\*\s*=([^\]]+)\]/g, (_match, label: string) => {
                const cleanLabel = safeNexusListItemLabel(label);
                return cleanLabel ? `\n[*][b]${cleanLabel}[/b] ` : "\n[*]";
            })
            .replace(/\[\*\]/g, "\n[*]")
            .replace(/\[(columns|cols)([^\]]*)\]/gi, (_match, tag: string, attrs: string) => `\n\n[${tag.toLowerCase()}${attrs ?? ""}]\n`)
            .replace(/\[\/(columns|cols)\]/gi, (_match, tag: string) => `\n[/${tag.toLowerCase()}]\n\n`)
            .replace(/\[(?:column|col)(?=[\s=\]/\]])([^\]]*)\]/gi, (_match, attrs: string) => `\n[column${attrs ?? ""}]\n`)
            .replace(/\[\/(?:column|col)\]/gi, "\n[/column]\n")
            .replace(/\[(?:nextcol|nextcolumn)\s*\/?\]/gi, "\n[nextcol/]\n")
            .replace(/\[tabs([^\]]*)\]/gi, (_match, attrs: string) => `\n\n[tabs${attrs ?? ""}]\n`)
            .replace(/\[\/tabs\]/gi, "\n[/tabs]\n\n")
            .replace(/\[(note|info|warning|important|tip)([^\]]*)\]/gi, (_match, tag: string, attrs: string) => `\n\n[${tag.toLowerCase()}${attrs ?? ""}]\n`)
            .replace(/\[\/(note|info|warning|important|tip)\]/gi, (_match, tag: string) => `\n[/${tag.toLowerCase()}]\n\n`)
            .replace(/\[(box|panel|fieldset|notice|success|danger|error)([^\]]*)\]/gi, (_match, tag: string, attrs: string) => `\n\n[${tag.toLowerCase()}${attrs ?? ""}]\n`)
            .replace(/\[\/(box|panel|fieldset|notice|success|danger|error)\]/gi, (_match, tag: string) => `\n[/${tag.toLowerCase()}]\n\n`)
            .replace(/\[(collapse|details|accordion|accordionitem)([^\]]*)\]/gi, (_match, tag: string, attrs: string) => `\n\n[${tag.toLowerCase()}${attrs ?? ""}]\n`)
            .replace(/\[\/(collapse|details|accordion|accordionitem)\]/gi, (_match, tag: string) => `\n[/${tag.toLowerCase()}]\n\n`)
            .replace(/\[caption([^\]]*)\]/gi, (_match, attrs: string) => `\n[caption${attrs ?? ""}]`)
            .replace(/\[\/caption\]/gi, "[/caption]\n")
            .replace(/\[(?:row)\]/gi, "[tr]")
            .replace(/\[\/(?:row)\]/gi, "[/tr]")
            .replace(/\[(?:cell)([^\]]*)\]/gi, (_match, attrs: string) => `[td${attrs ?? ""}]`)
            .replace(/\[\/(?:cell)\]/gi, "[/td]")
            .replace(/\[table[^\]]*\]/gi, "\n\n[table]\n")
            .replace(/\[\/table\]/gi, "\n[/table]\n\n")
            .replace(/\[\/?(?:tbody|thead|tfoot)[^\]]*\]/gi, "")
            .replace(/\[tr[^\]]*\]/gi, "[tr]")
            .replace(/\[\/tr\]/gi, "[/tr]\n")
            .replace(/\[(td|th)([^\]]*)\]/gi, (_match, cell: string, attrs: string) => `[${cell.toLowerCase()}${attrs ?? ""}]`)
            .replace(/\[\/(td|th)\]/gi, (_match, cell: string) => `[/${cell.toLowerCase()}]`)
            .replace(/\[hr\s*\/?\]/gi, "\n\n[hr]\n\n")
            .replace(/\[line\s*\/?\]/gi, "\n\n[hr]\n\n")
            .replace(/\[(?:rule|divider|separator)\s*\/?\]/gi, "\n\n[hr]\n\n")
            .replace(/\[\/(?:hr|line|rule|divider|separator)\]/gi, "\n")
            .replace(/\[clear\s*\/?\]/gi, "\n[clear/]\n")
            .replace(/(^|\n)[ \t]*(?:-{3,}|={3,}|_{3,}|\*{3,})[ \t]*(?=\n|$)/g, "$1\n\n[hr]\n\n")
            .replace(/\[(list|olist)([^\]]*)\]\n{2,}/gi, (_match, tag: string, attr: string) => `[${tag.toLowerCase()}${attr ?? ""}]\n`)
            .replace(/\n{2,}\[\/list\]/gi, "\n[/list]")
            .replace(/([^\n])\[list\]/gi, "$1\n\n[list]")
            .replace(/([^\n])\[olist([^\]]*)\]/gi, "$1\n\n[olist$2]")
            .replace(/\[\/list\]([^\n])/gi, "[/list]\n\n$1")
            .replace(/([^\n])\n\[list\]/gi, "$1\n\n[list]")
            .replace(/([^\n])\n\[olist([^\]]*)\]/gi, "$1\n\n[olist$2]")
            .replace(/\[\/list\]\n(?!\n|$)/gi, "[/list]\n\n")
            .replace(/[ \t]+\n/g, "\n")
            .replace(/\n{4,}/g, "\n\n\n")
            .trim();
    }

    function renderNexusBlocks(text: string, depth: number): string {
        if (depth > 3) {
            return `<p>${renderNexusInline(text)}</p>`;
        }

        return nexusBlockChunks(text)
            .map(chunk => renderNexusBlock(chunk, depth))
            .join("");
    }

    function nexusBlockChunks(text: string): NexusBlockChunk[] {
        const chunks: NexusBlockChunk[] = [];
        const structuralPattern = /\[(table|list|olist|quote|spoiler|collapse|details|accordion|accordionitem|indent|center|left|right|align|justify|code|heading|float|youtube|video|media|embed|columns|cols|tabs|note|info|warning|important|tip|box|panel|fieldset|notice|success|danger|error)([^\]]*)\]/gi;
        let cursor = 0;
        let match: RegExpExecArray | null;

        while ((match = structuralPattern.exec(text)) !== null) {
            const tag = match[1].toLowerCase();
            const rawAttrs = match[2] ?? "";
            const attr = nexusBbTagAttribute(tag, rawAttrs);
            const before = text.slice(cursor, match.index);
            if (!isNexusStructuralBoundary(before)) {
                continue;
            }

            const close = findNexusStructureClose(text, structuralPattern.lastIndex, tag);
            if (!close) {
                const remainder = text.slice(structuralPattern.lastIndex).trim();
                if ((tag === "list" || tag === "olist") && /\[\*\]/.test(remainder)) {
                    appendNexusTextChunks(chunks, before);
                    chunks.push({
                        kind: "list",
                        value: remainder,
                        ordered: tag === "olist" || isOrderedNexusListAttr(attr),
                        style: nexusListMarkerStyle(attr, tag === "olist" || isOrderedNexusListAttr(attr)) ?? undefined
                    });
                    cursor = text.length;
                    structuralPattern.lastIndex = text.length;
                    break;
                }

                continue;
            }

            appendNexusTextChunks(chunks, before);
            const value = text.slice(structuralPattern.lastIndex, close.start).trim();
            if (tag === "table") {
                chunks.push({ kind: "table", value });
            } else if (tag === "list" || tag === "olist") {
                chunks.push({
                    kind: "list",
                    value,
                    ordered: tag === "olist" || isOrderedNexusListAttr(attr),
                    style: nexusListMarkerStyle(attr, tag === "olist" || isOrderedNexusListAttr(attr)) ?? undefined
                });
            } else {
                chunks.push({ kind: "block", value: text.slice(match.index, close.end).trim() });
            }

            cursor = close.end;
            structuralPattern.lastIndex = close.end;
        }

        appendNexusTextChunks(chunks, text.slice(cursor));
        return chunks;
    }

    function isNexusStructuralBoundary(value: string): boolean {
        return !value.trim() || /\n\s*$/.test(value);
    }

    function appendNexusTextChunks(chunks: NexusBlockChunk[], value: string) {
        for (const block of value.split(/\n{2,}/)) {
            const trimmed = block.trim();
            if (trimmed) {
                if (looksLikeNexusLooseTable(trimmed)) {
                    chunks.push({ kind: "table", value: trimmed });
                    continue;
                }

                if (looksLikeNexusPipeTable(trimmed)) {
                    chunks.push({ kind: "table", value: nexusPipeTableToBbcode(trimmed) });
                    continue;
                }

                const mixedChunks = nexusMixedPlainListChunks(trimmed);
                if (mixedChunks) {
                    chunks.push(...mixedChunks);
                    continue;
                }

                const listChunk = nexusPlainListChunk(trimmed);
                chunks.push(listChunk ?? {
                    kind: shouldPreserveNexusLineLayout(trimmed) ? "line" : "block",
                    value: trimmed
                });
            }
        }
    }

    function nexusPlainListChunk(value: string): NexusBlockChunk | null {
        if (hasNexusBlockStructure(value)) {
            return null;
        }

        const lines = value
            .split("\n")
            .map(line => line.trim())
            .filter(Boolean);
        if (lines.length < 2) {
            return null;
        }

        const bulletItems = lines.map(line => line.match(/^(?:[-*]|\u2022)\s+(.+)$/)?.[1]?.trim() ?? "");
        const bbcodeItems = lines.map(line => line.match(/^\[\*(?:\s*=[^\]]+)?\]\s*(.+)$/i)?.[1]?.trim() ?? "");
        if (bbcodeItems.every(Boolean)) {
            return {
                kind: "list",
                ordered: false,
                value: bbcodeItems.map(item => `[*]${item}`).join("\n")
            };
        }

        if (bulletItems.every(Boolean)) {
            return {
                kind: "list",
                ordered: false,
                value: bulletItems.map(item => `[*]${item}`).join("\n")
            };
        }

        const numberedItems = lines.map(line => line.match(/^\d+[.)]\s+(.+)$/)?.[1]?.trim() ?? "");
        if (numberedItems.every(Boolean)) {
            return {
                kind: "list",
                ordered: true,
                style: "1",
                value: numberedItems.map(item => `[*]${item}`).join("\n")
            };
        }

        const alphaItems = lines.map(line => line.match(/^[a-z][.)]\s+(.+)$/i)?.[1]?.trim() ?? "");
        if (alphaItems.every(Boolean)) {
            const startsUpper = /^[A-Z][.)]/.test(lines[0]);
            return {
                kind: "list",
                ordered: true,
                style: startsUpper ? "A" : "a",
                value: alphaItems.map(item => `[*]${item}`).join("\n")
            };
        }

        return null;
    }

    function nexusMixedPlainListChunks(value: string): NexusBlockChunk[] | null {
        if (hasNexusBlockStructure(value)) {
            return null;
        }

        const lines = value.split("\n");
        if (lines.length < 2 && !/\[\*(?:\s*=[^\]]+)?\]/i.test(value)) {
            return null;
        }

        const chunks: NexusBlockChunk[] = [];
        let textLines: string[] = [];
        let listLines: NexusPlainListLine[] = [];

        const pushTextLines = () => {
            const text = textLines.join("\n").trim();
            textLines = [];
            if (!text) {
                return;
            }

            chunks.push({
                kind: shouldPreserveNexusLineLayout(text) ? "line" : "block",
                value: text
            });
        };

        const pushListLines = () => {
            if (listLines.length === 0) {
                return;
            }

            const shouldRenderList = listLines.length >= 2 || listLines.some(line => line.explicit);
            if (!shouldRenderList) {
                textLines.push(...listLines.map(line => line.raw.trim()).filter(Boolean));
                listLines = [];
                return;
            }

            pushTextLines();
            const first = listLines[0];
            chunks.push({
                kind: "list",
                ordered: first.ordered,
                style: first.style,
                value: listLines.map(line => `[*]${line.value}`).join("\n")
            });
            listLines = [];
        };

        for (const line of lines) {
            const parsed = nexusPlainListLine(line);
            if (!parsed) {
                pushListLines();
                textLines.push(line);
                continue;
            }

            if (listLines.length === 0) {
                pushTextLines();
                listLines.push(parsed);
                continue;
            }

            const first = listLines[0];
            if (first.ordered === parsed.ordered && first.style === parsed.style) {
                listLines.push(parsed);
            } else {
                pushListLines();
                listLines.push(parsed);
            }
        }

        pushListLines();
        pushTextLines();

        return chunks.some(chunk => chunk.kind === "list") ? chunks : null;
    }

    function nexusPlainListLine(line: string): NexusPlainListLine | null {
        const trimmed = line.trim();
        if (!trimmed) {
            return null;
        }

        const explicit = trimmed.match(/^\[\*(?:\s*=[^\]]+)?\]\s*(.+)$/i);
        if (explicit?.[1]?.trim()) {
            return {
                explicit: true,
                ordered: false,
                raw: line,
                value: explicit[1].trim()
            };
        }

        const bullet = trimmed.match(/^(?:[-*+]|\u2022)\s+(.+)$/);
        if (bullet?.[1]?.trim()) {
            return {
                explicit: false,
                ordered: false,
                raw: line,
                value: bullet[1].trim()
            };
        }

        const numbered = trimmed.match(/^\d+[.)]\s+(.+)$/);
        if (numbered?.[1]?.trim()) {
            return {
                explicit: false,
                ordered: true,
                raw: line,
                style: "1",
                value: numbered[1].trim()
            };
        }

        const alpha = trimmed.match(/^([a-z])[.)]\s+(.+)$/i);
        if (alpha?.[2]?.trim()) {
            return {
                explicit: false,
                ordered: true,
                raw: line,
                style: alpha[1] === alpha[1].toUpperCase() ? "A" : "a",
                value: alpha[2].trim()
            };
        }

        return null;
    }

    function shouldPreserveNexusLineLayout(value: string): boolean {
        if (hasNexusBlockStructure(value)) {
            return false;
        }

        const lines = value.split("\n");
        const nonEmpty = lines.map(line => line.trim()).filter(Boolean);
        if (nonEmpty.length < 3) {
            return false;
        }

        const structuralLines = lines.filter(line => {
            const trimmed = line.trim();
            return /^\s{2,}\S/.test(line)
                || /\S\s{2,}\S/.test(line)
                || /^[A-Za-z0-9][A-Za-z0-9 /&+_.()'-]{1,42}:\s+\S/.test(trimmed)
                || /^(?:[A-Z][A-Z0-9 _/&+.'()-]{3,}|[=*_ -]{4,})$/.test(trimmed)
                || /(?:[A-Za-z]:\\|\.{0,2}\/|\\)[^\s]+/.test(trimmed)
                || /(?:\.dll|\.json|\.cfg|\.ini|\.zip|\.rar|\.7z)\b/i.test(trimmed);
        }).length;

        if (structuralLines >= 2) {
            return true;
        }

        const averageLength = nonEmpty.reduce((total, line) => total + line.length, 0) / nonEmpty.length;
        return nonEmpty.length >= 4 && averageLength <= 64 && !/[.!?]\s+[A-Z]/.test(nonEmpty.join(" "));
    }

    function findNexusStructureClose(text: string, startIndex: number, tag: string): { start: number; end: number } | null {
        const escapedTag = tag.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
        const pattern = tag === "list" || tag === "olist"
            ? /\[\/?(?:list|olist)(?:[^\]]*)\]/gi
            : new RegExp(`\\[\\/?${escapedTag}(?:[^\\]]*)\\]`, "gi");
        pattern.lastIndex = startIndex;
        let depth = 1;
        let match: RegExpExecArray | null;

        while ((match = pattern.exec(text)) !== null) {
            if (match[0].startsWith("[/")) {
                depth -= 1;
                if (depth === 0) {
                    return { start: match.index, end: pattern.lastIndex };
                }
            } else {
                depth += 1;
            }
        }

        return null;
    }

    function renderNexusBlock(chunk: NexusBlockChunk, depth: number): string {
        const block = chunk.value;
        if (!block) {
            return "";
        }

        if (chunk.kind === "list") {
            const listItems = nexusListItems(block);
            if (listItems.length === 0) {
                return "";
            }

            const tag = chunk.ordered ? "ol" : "ul";
            const typeAttribute = chunk.ordered && chunk.style ? ` type="${escapeAttribute(chunk.style)}"` : "";
            const classAttribute = !chunk.ordered && chunk.style ? ` class="nexus-rich-list-${escapeAttribute(chunk.style)}"` : "";
            return `<${tag}${typeAttribute}${classAttribute}>${listItems.map(item => `<li>${renderNexusListItem(item, depth)}</li>`).join("")}</${tag}>`;
        }

        if (chunk.kind === "table") {
            return renderNexusTable(block, depth);
        }

        if (chunk.kind === "line") {
            return `<div class="nexus-rich-line-block">${renderNexusInline(block)}</div>`;
        }

        const columns = block.match(/^\[(columns|cols)(?:=([^\]]+))?\]([\s\S]*?)\[\/\1\]$/i);
        if (columns) {
            return renderNexusColumns(columns[3], depth);
        }

        const tabs = block.match(/^\[tabs(?:=([^\]]+))?\]([\s\S]*?)\[\/tabs\]$/i);
        if (tabs) {
            return renderNexusTabs(tabs[2], depth);
        }

        const media = block.match(/^\[(youtube|video|media|embed)([^\]]*)\]([\s\S]*?)\[\/\1\]$/i);
        if (media) {
            const rendered = renderNexusMediaBlock(media[1], media[2], media[3]);
            if (rendered) {
                return rendered;
            }
        }

        const callout = block.match(/^\[(note|info|warning|important|tip)(?:=([^\]]+))?\]([\s\S]*?)\[\/\1\]$/i);
        if (callout) {
            const kind = safeNexusCalloutKind(callout[1]);
            const label = safeNexusLabel(callout[2]) || nexusCalloutLabel(kind);
            return `<div class="nexus-rich-callout nexus-rich-callout-${kind}"><span>${escapeHtml(label)}</span>${renderNexusBlocks(callout[3], depth + 1)}</div>`;
        }

        const box = block.match(/^\[(box|panel|fieldset|notice|success|danger|error)(?:=([^\]]+))?\]([\s\S]*?)\[\/\1\]$/i);
        if (box) {
            const kind = safeNexusBoxKind(box[1]);
            const label = safeNexusLabel(box[2]) || nexusBoxLabel(kind);
            return `<div class="nexus-rich-box nexus-rich-box-${kind}">${label ? `<span>${escapeHtml(label)}</span>` : ""}${renderNexusBlocks(box[3], depth + 1)}</div>`;
        }

        const collapsible = block.match(/^\[(collapse|details|accordion|accordionitem)(?:=([^\]]+))?\]([\s\S]*?)\[\/\1\]$/i);
        if (collapsible) {
            const label = safeNexusLabel(collapsible[2]) || "Details";
            return `<div class="nexus-rich-spoiler-block nexus-rich-details-block"><span>${escapeHtml(label)}</span>${renderNexusBlocks(collapsible[3], depth + 1)}</div>`;
        }

        if (/^\[hr\]$/i.test(block)) {
            return `<hr class="nexus-rich-rule" />`;
        }

        const heading = block.match(/^\[heading(?:=([1-6]))?\]([\s\S]*?)\[\/heading\]$/i);
        if (heading) {
            const level = safeNexusHeadingLevel(heading[1]);
            return `<p class="nexus-rich-heading nexus-rich-heading-${level}">${renderNexusInline(heading[2])}</p>`;
        }

        const standaloneHeading = nexusStandaloneSectionHeading(block);
        if (standaloneHeading) {
            return `<p class="nexus-rich-heading nexus-rich-heading-3">${renderNexusInline(standaloneHeading)}</p>`;
        }

        const code = block.match(/^\[code\]([\s\S]*?)\[\/code\]$/i);
        if (code) {
            return `<pre><code>${escapeHtml(code[1])}</code></pre>`;
        }

        const aligned = block.match(/^\[(center|left|right)\]([\s\S]*?)\[\/\1\]$/i) ?? block.match(/^\[align=([^\]]+)\]([\s\S]*?)\[\/align\]$/i);
        if (aligned) {
            const alignment = safeNexusAlignment(aligned[1]);
            return `<div class="nexus-rich-align-${alignment}">${renderNexusBlocks(aligned[2], depth + 1)}</div>`;
        }

        const quote = block.match(/^\[quote(?:=([^\]]+))?\]([\s\S]*?)\[\/quote\]$/i);
        if (quote) {
            const cite = safeNexusLabel(quote[1]);
            return `<blockquote>${cite ? `<cite>${escapeHtml(cite)}</cite>` : ""}${renderNexusBlocks(quote[2], depth + 1)}</blockquote>`;
        }

        const spoiler = block.match(/^\[spoiler(?:=([^\]]+))?\]([\s\S]*?)\[\/spoiler\]$/i);
        if (spoiler) {
            const label = safeNexusLabel(spoiler[1]) || "Spoiler";
            return `<div class="nexus-rich-spoiler-block"><span>${escapeHtml(label)}</span>${renderNexusBlocks(spoiler[2], depth + 1)}</div>`;
        }

        const indented = block.match(/^\[indent(?:=([^\]]+))?\]([\s\S]*?)\[\/indent\]$/i);
        if (indented) {
            const level = safeNexusIndentLevel(indented[1]);
            return `<div class="nexus-rich-indent nexus-rich-indent-${level}">${renderNexusBlocks(indented[2], depth + 1)}</div>`;
        }

        const floated = block.match(/^\[float(?:=([^\]]+))?\]([\s\S]*?)\[\/float\]$/i);
        if (floated) {
            return `<div class="nexus-rich-float nexus-rich-float-${safeNexusFloat(floated[1])}">${renderNexusBlocks(floated[2], depth + 1)}</div>`;
        }

        const justified = block.match(/^\[justify\]([\s\S]*?)\[\/justify\]$/i);
        if (justified) {
            return `<div class="nexus-rich-align-justify">${renderNexusBlocks(justified[1], depth + 1)}</div>`;
        }

        return `<p>${renderNexusInline(block)}</p>`;
    }

    function renderNexusListItem(item: string, depth: number): string {
        return hasNexusBlockStructure(item)
            ? renderNexusBlocks(item, depth + 1)
            : renderNexusInline(item);
    }

    function renderNexusColumns(value: string, depth: number): string {
        const columns = splitNexusColumns(value);
        if (columns.length < 2) {
            return renderNexusBlocks(value, depth + 1);
        }

        const count = Math.min(4, columns.length);
        return `<div class="nexus-rich-columns nexus-rich-columns-${count}">${columns.map(column => `<div>${renderNexusBlocks(column, depth + 1)}</div>`).join("")}</div>`;
    }

    function splitNexusColumns(value: string): string[] {
        const explicitColumns: string[] = [];
        const explicitPattern = /\[column(?=[\s=\]])[^\]]*\]([\s\S]*?)\[\/column\]/gi;
        let explicitMatch: RegExpExecArray | null;

        while ((explicitMatch = explicitPattern.exec(value)) !== null) {
            const column = explicitMatch[1].trim();
            if (column) {
                explicitColumns.push(column);
            }
        }

        if (explicitColumns.length > 0) {
            return explicitColumns;
        }

        const splitColumns = value
            .split(/\[nextcol\s*\/?\]/i)
            .map(column => column.trim())
            .filter(Boolean);
        return splitColumns.length > 0 ? splitColumns : value.trim() ? [value.trim()] : [];
    }

    function renderNexusTabs(value: string, depth: number): string {
        const tabs = splitNexusTabs(value);
        if (tabs.length === 0) {
            return renderNexusBlocks(value, depth + 1);
        }

        return `<div class="nexus-rich-tabs">${tabs.map(tab => `<section><b>${escapeHtml(tab.label)}</b>${renderNexusBlocks(tab.value, depth + 1)}</section>`).join("")}</div>`;
    }

    function splitNexusTabs(value: string): Array<{ label: string; value: string }> {
        const tabs: Array<{ label: string; value: string }> = [];
        const tabPattern = /\[tab(?:=([^\]]+))?\]([\s\S]*?)\[\/tab\]/gi;
        let tabMatch: RegExpExecArray | null;

        while ((tabMatch = tabPattern.exec(value)) !== null) {
            const body = tabMatch[2].trim();
            if (!body) {
                continue;
            }

            tabs.push({
                label: safeNexusLabel(tabMatch[1]) || `Section ${tabs.length + 1}`,
                value: body
            });
        }

        return tabs;
    }

    function renderNexusMediaBlock(tag: string, rawAttrs: string, body: string): string {
        const media = nexusMediaRenderData(tag, rawAttrs, nexusBbTagAttribute(tag, rawAttrs), collectNexusNodeText(parseNexusRichNodes(body)));
        if (!media) {
            return "";
        }

        return `<div class="nexus-rich-media-block"><span>${escapeHtml(media.label)}</span><a href="${escapeAttribute(media.url)}" target="_blank" rel="noreferrer noopener">${escapeHtml(media.urlLabel)}</a></div>`;
    }

    function hasNexusBlockStructure(value: string): boolean {
        return /\[(?:table|list|olist|quote|spoiler|collapse|details|accordion|accordionitem|indent|center|left|right|align|justify|code|heading|float|youtube|video|media|embed|columns|cols|tabs|note|info|warning|important|tip|box|panel|fieldset|notice|success|danger|error)(?:=[^\]]+)?\]/i.test(value)
            || looksLikeNexusLooseTable(value);
    }

    function looksLikeNexusLooseTable(value: string): boolean {
        return /\[tr(?:[^\]]*)\][\s\S]*?(?:\[\/tr\]|\[tr(?:[^\]]*)\]|\[\/table\]|$)/i.test(value)
            && /\[(?:td|th)(?:[^\]]*)\][\s\S]*?(?:\[\/(?:td|th)\]|\[(?:td|th)(?:[^\]]*)\]|\[\/tr\]|$)/i.test(value);
    }

    function looksLikeNexusPipeTable(value: string): boolean {
        if (hasNexusBlockStructure(value)) {
            return false;
        }

        const lines = value.split("\n").map(line => line.trim()).filter(Boolean);
        if (lines.length < 2 || lines.length > 16) {
            return false;
        }

        const tableRows = lines.filter(line => splitNexusPipeTableLine(line).length >= 2);
        const separatorRows = lines.filter(isNexusPipeTableSeparator).length;
        return tableRows.length >= 2 && tableRows.length + separatorRows === lines.length;
    }

    function nexusPipeTableToBbcode(value: string): string {
        const lines = value.split("\n").map(line => line.trim()).filter(Boolean);
        const hasHeader = lines.length > 1 && isNexusPipeTableSeparator(lines[1]);
        const rows = lines
            .filter(line => !isNexusPipeTableSeparator(line))
            .map((line, index) => {
                const cellTag = hasHeader && index === 0 ? "th" : "td";
                return `[tr]${splitNexusPipeTableLine(line).map(cell => `[${cellTag}]${cell}[/${cellTag}]`).join("")}[/tr]`;
            });
        return rows.join("\n");
    }

    function splitNexusPipeTableLine(line: string): string[] {
        return line
            .replace(/^\|/, "")
            .replace(/\|$/, "")
            .split("|")
            .map(cell => cell.trim())
            .filter(Boolean);
    }

    function isNexusPipeTableSeparator(line: string): boolean {
        const cells = splitNexusPipeTableLine(line);
        return cells.length >= 2 && cells.every(cell => /^:?-{3,}:?$/.test(cell));
    }

    function nexusStandaloneSectionHeading(block: string): string | null {
        const trimmed = block.trim();
        if (!trimmed || trimmed.includes("\n") || trimmed.length > 160) {
            return null;
        }

        const wrapped = trimmed.match(/^(?:\[(?:b|u|size(?:=[^\]]+)?|color(?:=[^\]]+)?|background(?:=[^\]]+)?|bgcolor(?:=[^\]]+)?|highlight(?:=[^\]]+)?)\]\s*)+([\s\S]*?)(?:\s*\[\/(?:b|u|size|color|background|bgcolor|highlight)\])+$/i);
        const framed = trimmed.match(/^(?:={2,}|#{1,3}\s+)(.+?)(?:\s*={2,})?$/);
        const labeled = nexusStandaloneLabelHeading(trimmed);
        const candidate = wrapped?.[1] ?? framed?.[1] ?? labeled;
        if (!candidate) {
            return null;
        }

        const label = collectNexusNodeText(parseNexusRichNodes(candidate)).replace(/\s+/g, " ").trim();
        if (label.length < 3 || label.length > 80 || /[.!?]\s*$/.test(label)) {
            return null;
        }

        return candidate.trim();
    }

    function nexusStandaloneLabelHeading(value: string): string | null {
        const label = value.replace(/:$/, "").trim();
        if (label.length < 3 || label.length > 80 || /[.!?]\s*$/.test(label)) {
            return null;
        }

        if (!/^[a-z0-9][a-z0-9 /&+_.()'-]+:?$/i.test(value)) {
            return null;
        }

        const knownSection = /^(requirements?|installation|install|setup|usage|features?|compatibility|load order|known issues?|changelog|credits?|permissions?|recommended|optional|manual install|vortex install|uninstall|troubleshooting|faq)$/i.test(label);
        const letters = label.replace(/[^a-z]/gi, "");
        const uppercase = label.replace(/[^A-Z]/g, "");
        const uppercaseRatio = letters.length > 0 ? uppercase.length / letters.length : 0;
        return knownSection || uppercaseRatio >= 0.62 ? label : null;
    }

    function renderNexusTable(block: string, depth: number): string {
        const rows = nexusTableRows(block);
        if (rows.length === 0) {
            const fallback = block
                .replace(/\[\/?tr\]/gi, "\n")
                .replace(/\[(?:td|th)[^\]]*\]/gi, "")
                .replace(/\[\/(?:td|th)\]/gi, " | ")
                .trim();
            return fallback ? `<p>${renderNexusInline(fallback)}</p>` : "";
        }

        return `<div class="nexus-rich-table-wrap"><table class="nexus-rich-table"><tbody>${rows.map(row => {
            const cells = row.cells.map(cell => {
                const tag = cell.header ? "th" : "td";
                const spanAttributes = [
                    cell.colspan ? ` colspan="${cell.colspan}"` : "",
                    cell.rowspan ? ` rowspan="${cell.rowspan}"` : ""
                ].join("");
                return `<${tag}${spanAttributes}>${renderNexusBlocks(cell.value, depth + 1)}</${tag}>`;
            }).join("");
            return `<tr>${cells}</tr>`;
        }).join("")}</tbody></table></div>`;
    }

    function nexusTableRows(block: string): Array<{ cells: NexusTableCell[] }> {
        const rows: Array<{ cells: NexusTableCell[] }> = [];
        const rowPattern = /\[tr\]([\s\S]*?)(?:\[\/tr\]|(?=\[tr\]|\[\/table\]|$))/gi;
        let rowMatch: RegExpExecArray | null;

        while ((rowMatch = rowPattern.exec(block)) !== null) {
            const cells: NexusTableCell[] = [];
            const cellPattern = /\[(td|th)([^\]]*)\]([\s\S]*?)(?:\[\/\1\]|(?=\[(?:td|th)(?:[^\]]*)\]|\[\/tr\]|$))/gi;
            let cellMatch: RegExpExecArray | null;

            while ((cellMatch = cellPattern.exec(rowMatch[1])) !== null) {
                const attrs = cellMatch[2] ?? "";
                cells.push({
                    value: cellMatch[3].trim(),
                    header: cellMatch[1].toLowerCase() === "th",
                    colspan: safeNexusTableSpan(nexusBbAttribute(attrs, "colspan") ?? nexusBbAttribute(attrs, "col") ?? nexusBbFirstAttributeValue(attrs)),
                    rowspan: safeNexusTableSpan(nexusBbAttribute(attrs, "rowspan") ?? nexusBbAttribute(attrs, "row"))
                });
            }

            if (cells.length > 0) {
                rows.push({ cells });
            }
        }

        return rows;
    }

    function nexusListItems(block: string): string[] {
        const cleaned = block
            .replace(/^\[(?:list|olist)(?:=[^\]]+)?\]\s*/i, "")
            .replace(/\s*\[\/list\]$/i, "")
            .replace(/\s*\[\/olist\]$/i, "")
            .trim();
        const markedItems = splitNexusMarkedListItems(cleaned);
        if (markedItems.length > 0) {
            return markedItems;
        }

        const lines = cleaned.split("\n").map(line => line.trim()).filter(Boolean);
        if (lines.length > 0 && lines.every(line => /^(?:\[\*\]|[-*]\s+|\d+[.)]\s+)/.test(line))) {
            return lines.map(line => line.replace(/^(?:\[\*\]|[-*]\s+|\d+[.)]\s+)/, "").trim()).filter(Boolean);
        }

        return lines.length > 1 ? lines : cleaned ? [cleaned] : [];
    }

    function splitNexusMarkedListItems(value: string): string[] {
        const items: string[] = [];
        const tokenPattern = /\[\*(?:\s*=[^\]]+)?\]|\[\/?(?:list|olist|table)(?:=[^\]]+)?\]/gi;
        let listDepth = 0;
        let tableDepth = 0;
        let currentStart: number | null = null;
        let match: RegExpExecArray | null;

        while ((match = tokenPattern.exec(value)) !== null) {
            const token = match[0].toLowerCase();
            if (token.startsWith("[*")) {
                if (listDepth === 0 && tableDepth === 0) {
                    if (currentStart !== null) {
                        const item = value.slice(currentStart, match.index).trim();
                        if (item) {
                            items.push(item);
                        }
                    }

                    currentStart = tokenPattern.lastIndex;
                }
                continue;
            }

            if (token.startsWith("[/table")) {
                tableDepth = Math.max(0, tableDepth - 1);
            } else if (token.startsWith("[table")) {
                tableDepth += 1;
            } else if (token.startsWith("[/list") || token.startsWith("[/olist")) {
                listDepth = Math.max(0, listDepth - 1);
            } else {
                listDepth += 1;
            }
        }

        if (currentStart !== null) {
            const item = value.slice(currentStart).trim();
            if (item) {
                items.push(item);
            }
        }

        return items;
    }

    function renderNexusInline(value: string): string {
        const inlineValue = value
            .replace(/\[\/?(?:ul|ol|olist|list)(?:=[^\]]+)?\]/gi, "\n")
            .replace(/\[\*\]/g, "\n- ");
        return renderNexusNodes(parseNexusRichNodes(inlineValue));
    }

    function parseNexusRichNodes(value: string): NexusRichNode[] {
        const root: { children: NexusRichNode[] } = { children: [] };
        const stack: Array<{ name: string; children: NexusRichNode[] }> = [{ name: "root", children: root.children }];
        const tagPattern = /\[(\/?)([a-z][a-z0-9_-]*)([^\]]*)\]/gi;
        let lastIndex = 0;
        let match: RegExpExecArray | null;

        while ((match = tagPattern.exec(value)) !== null) {
            if (match.index > lastIndex) {
                stack[stack.length - 1].children.push({ kind: "text", value: value.slice(lastIndex, match.index) });
            }

            const full = match[0];
            const name = match[2].toLowerCase();
            const isClosing = match[1] === "/";
            const rawAttrs = match[3] ?? "";
            const isSelfClosing = /\/\s*$/.test(rawAttrs);

            if (!NEXUS_RICH_BB_TAGS.has(name)) {
                stack[stack.length - 1].children.push({ kind: "text", value: full });
            } else if (isClosing) {
                if (stack.length > 1 && stack[stack.length - 1].name === name) {
                    stack.pop();
                } else {
                    stack[stack.length - 1].children.push({ kind: "text", value: full });
                }
            } else {
                const node: NexusRichNode = { kind: "tag", name, attr: nexusBbTagAttribute(name, rawAttrs), rawAttrs, children: [] };
                stack[stack.length - 1].children.push(node);
                if (!isSelfClosing) {
                    stack.push(node);
                }
            }

            lastIndex = tagPattern.lastIndex;
        }

        if (lastIndex < value.length) {
            stack[stack.length - 1].children.push({ kind: "text", value: value.slice(lastIndex) });
        }

        return root.children;
    }

    function renderNexusNodes(nodes: NexusRichNode[]): string {
        return nodes.map(node => renderNexusNode(node)).join("");
    }

    function renderNexusNode(node: NexusRichNode): string {
        if (node.kind === "text") {
            return renderNexusTextNode(node.value);
        }

        const inner = renderNexusNodes(node.children);
        switch (node.name) {
            case "b":
                return `<strong>${inner}</strong>`;
            case "i":
                return `<em>${inner}</em>`;
            case "u":
                return `<u>${inner}</u>`;
            case "s":
            case "strike":
            case "del":
                return `<s>${inner}</s>`;
            case "sub":
                return `<sub>${inner}</sub>`;
            case "sup":
                return `<sup>${inner}</sup>`;
            case "small":
                return `<span class="nexus-rich-size-small">${inner}</span>`;
            case "big":
                return `<span class="nexus-rich-size-large">${inner}</span>`;
            case "mark": {
                const color = safeNexusColor(node.attr) ?? "yellow";
                return `<span class="nexus-rich-highlight" style="background-color: ${escapeAttribute(color)}">${inner}</span>`;
            }
            case "abbr":
            case "acronym": {
                const title = safeNexusLabel(node.attr);
                return title ? `<abbr class="nexus-rich-abbr" title="${escapeAttribute(title)}">${inner}</abbr>` : `<abbr class="nexus-rich-abbr">${inner}</abbr>`;
            }
            case "cite":
                return `<cite class="nexus-rich-cite">${inner}</cite>`;
            case "q":
                return `<q>${inner}</q>`;
            case "url": {
                const url = safeNexusUrl(node.attr ?? collectNexusNodeText(node.children));
                return url ? `<a href="${escapeAttribute(url)}" target="_blank" rel="noreferrer noopener">${inner || escapeHtml(url)}</a>` : inner;
            }
            case "img":
            case "image":
            case "thumb":
            case "thumbnail": {
                const text = collectNexusNodeText(node.children);
                const attrUrl = safeNexusUrl(nexusImageUrlAttribute(node.rawAttrs, node.attr));
                const textUrl = safeNexusUrl(text);
                const url = attrUrl ?? textUrl;
                const alignment = nexusImageAlignment(node.rawAttrs, node.attr);
                const style = nexusImageStyleAttribute(node.rawAttrs);
                const imageClass = ["nexus-rich-image", alignment ? `nexus-rich-image-${alignment}` : ""].filter(Boolean).join(" ");
                const alt = safeNexusImageAlt(nexusBbAttribute(node.rawAttrs, "alt") ?? nexusBbAttribute(node.rawAttrs, "title"));
                return url ? `<img class="${imageClass}" src="${escapeAttribute(url)}" alt="${escapeAttribute(alt)}"${alt ? ` title="${escapeAttribute(alt)}"` : ""}${style} />${attrUrl && inner ? inner : ""}` : inner;
            }
            case "color": {
                const color = safeNexusColor(node.attr);
                return color ? `<span style="color: ${escapeAttribute(color)}">${inner}</span>` : inner;
            }
            case "background":
            case "bgcolor":
            case "highlight": {
                const color = safeNexusColor(node.attr) ?? (node.name === "highlight" ? "yellow" : null);
                return color ? `<span class="nexus-rich-highlight" style="background-color: ${escapeAttribute(color)}">${inner}</span>` : inner;
            }
            case "size":
                return `<span class="${nexusSizeClass(node.attr)}">${inner}</span>`;
            case "center":
            case "left":
            case "right":
                return `<span class="nexus-rich-align-${node.name}">${inner}</span>`;
            case "justify":
                return `<span class="nexus-rich-align-justify">${inner}</span>`;
            case "align":
                return `<span class="nexus-rich-align-${safeNexusAlignment(node.attr)}">${inner}</span>`;
            case "indent": {
                const level = safeNexusIndentLevel(node.attr);
                return `<span class="nexus-rich-indent-inline nexus-rich-indent-${level}">${inner}</span>`;
            }
            case "float":
                return `<span class="nexus-rich-float nexus-rich-float-${safeNexusFloat(node.attr)}">${inner}</span>`;
            case "clear":
                return `<span class="nexus-rich-clear"></span>${inner}`;
            case "columns":
            case "cols":
                return `<span class="nexus-rich-columns-inline">${inner}</span>`;
            case "column":
            case "col":
            case "tab":
                return inner;
            case "nextcol":
                return "<br>";
            case "note":
            case "info":
            case "warning":
            case "important":
            case "tip": {
                const kind = safeNexusCalloutKind(node.name);
                const label = safeNexusLabel(node.attr) || nexusCalloutLabel(kind);
                return `<span class="nexus-rich-callout-inline nexus-rich-callout-${kind}"><b>${escapeHtml(label)}</b> ${inner}</span>`;
            }
            case "code":
            case "pre":
            case "tt":
            case "kbd":
            case "samp":
            case "var":
                return `<code>${escapeHtml(collectNexusNodeText(node.children))}</code>`;
            case "quote":
                return `<blockquote>${inner}</blockquote>`;
            case "spoiler":
                return `<span class="nexus-rich-spoiler">${safeNexusLabel(node.attr) ? `<b>${escapeHtml(safeNexusLabel(node.attr) ?? "")}</b> ` : ""}${inner}</span>`;
            case "collapse":
            case "details":
            case "accordion":
            case "accordionitem":
                return `<span class="nexus-rich-spoiler">${safeNexusLabel(node.attr) ? `<b>${escapeHtml(safeNexusLabel(node.attr) ?? "")}</b> ` : ""}${inner}</span>`;
            case "heading":
                return `<span class="nexus-rich-heading nexus-rich-heading-${safeNexusHeadingLevel(node.attr)}">${inner}</span>`;
            case "h":
            case "header":
            case "title":
                return `<span class="nexus-rich-heading nexus-rich-heading-2">${inner}</span>`;
            case "subtitle":
                return `<span class="nexus-rich-heading nexus-rich-heading-4">${inner}</span>`;
            case "caption":
                return `<span class="nexus-rich-caption">${inner}</span>`;
            case "anchor":
                return `<span class="nexus-rich-anchor">${inner}</span>`;
            case "goto":
            case "jump":
                return `<span class="nexus-rich-anchor-ref">${inner}</span>`;
            case "h1":
            case "h2":
            case "h3":
            case "h4":
            case "h5":
            case "h6":
                return `<span class="nexus-rich-heading nexus-rich-heading-${node.name.slice(1)}">${inner}</span>`;
            case "youtube":
            case "video":
            case "media":
            case "embed": {
                const media = nexusMediaRenderData(node.name, node.rawAttrs, node.attr, collectNexusNodeText(node.children));
                return media ? `<a class="nexus-rich-media-link" href="${escapeAttribute(media.url)}" target="_blank" rel="noreferrer noopener">${escapeHtml(media.inlineLabel)}</a>` : inner;
            }
            case "font": {
                const color = safeNexusColor(nexusBbAttribute(node.rawAttrs, "color"));
                const background = safeNexusColor(
                    nexusBbAttribute(node.rawAttrs, "background")
                    ?? nexusBbAttribute(node.rawAttrs, "background-color")
                    ?? nexusBbAttribute(node.rawAttrs, "bgcolor")
                );
                const fontClass = safeNexusFontClass(
                    nexusBbAttribute(node.rawAttrs, "face")
                    ?? nexusBbAttribute(node.rawAttrs, "font")
                    ?? nexusBbAttribute(node.rawAttrs, "font-family")
                    ?? node.attr
                );
                const classes = ["nexus-rich-font", fontClass ? `nexus-rich-font-${fontClass}` : ""]
                    .filter(Boolean)
                    .join(" ");
                const style = [
                    color ? `color: ${color}` : "",
                    background ? `background-color: ${background}` : ""
                ].filter(Boolean).join("; ");
                return style || fontClass ? `<span class="${classes}"${style ? ` style="${escapeAttribute(style)}"` : ""}>${inner}</span>` : inner;
            }
            case "box":
            case "panel":
            case "fieldset":
            case "notice":
            case "success":
            case "danger":
            case "error": {
                const kind = safeNexusBoxKind(node.name);
                const label = safeNexusLabel(node.attr) || nexusBoxLabel(kind);
                return `<span class="nexus-rich-box-inline nexus-rich-box-${kind}">${label ? `<b>${escapeHtml(label)}</b> ` : ""}${inner}</span>`;
            }
            default:
                return inner;
        }
    }

    function renderNexusTextNode(value: string): string {
        return escapeHtml(value.replace(/\t/g, "    "))
            .replace(/ {2,}/g, spaces => {
                const preserved = Math.min(spaces.length - 1, 8);
                return ` ${"&nbsp;".repeat(preserved)}${spaces.length - preserved - 1 > 0 ? " " : ""}`;
            })
            .replace(/\n/g, "<br>");
    }

    function normalizeNexusBbOpeningTag(tag: string, rawAttrs: string): string {
        const name = tag.toLowerCase();
        if (name === "img") {
            return normalizedNexusImageOpeningTag(rawAttrs);
        }

        const attr = nexusBbTagAttribute(name, rawAttrs);
        if (name === "ol" || name === "olist") {
            const type = nexusOrderedListType(attr);
            return `\n[olist${type ? `=${type}` : ""}]\n`;
        }

        if (name === "ul") {
            const style = nexusListMarkerStyle(attr, false);
            return `\n[list${style ? `=${style}` : ""}]\n`;
        }

        if (name === "list") {
            if (isOrderedNexusListAttr(attr)) {
                const type = nexusOrderedListType(attr);
                return `\n[olist${type ? `=${type}` : ""}]\n`;
            }

            const style = nexusListMarkerStyle(attr, false);
            return `\n[list${style ? `=${style}` : ""}]\n`;
        }

        return attr ? `[${name}=${attr}]` : `[${name}]`;
    }

    function nexusImageAliasOpeningTag(alignment: "left" | "right" | "center", rawAttrs: string): string {
        const attrs = (rawAttrs ?? "").trim();
        if (!attrs) {
            return `[img align="${alignment}"]`;
        }

        if (attrs.startsWith("=")) {
            const source = cleanNexusBbAttributeValue(attrs.slice(1));
            return source ? `[img align="${alignment}" src="${source}"]` : `[img align="${alignment}"]`;
        }

        return `[img align="${alignment}" ${attrs}]`;
    }

    function normalizedNexusImageOpeningTag(rawAttrs: string): string {
        const direct = nexusBbDirectAttribute(rawAttrs);
        const source = safeNexusUrl(nexusBbAttribute(rawAttrs, "src"))
            ?? safeNexusUrl(nexusBbAttribute(rawAttrs, "url"))
            ?? safeNexusUrl(direct)
            ?? undefined;
        const alignment = nexusImageAlignment(rawAttrs, direct);
        const width = safeNexusCssLength(nexusBbAttribute(rawAttrs, "width") ?? nexusBbAttribute(rawAttrs, "w"));
        const height = safeNexusCssLength(nexusBbAttribute(rawAttrs, "height") ?? nexusBbAttribute(rawAttrs, "h"));
        const attrs = [
            source ? `src="${source}"` : "",
            alignment ? `align="${alignment}"` : "",
            width ? `width="${width}"` : "",
            height ? `height="${height}"` : ""
        ].filter(Boolean);

        if (attrs.length > 0) {
            return `[img ${attrs.join(" ")}]`;
        }

        return direct ? `[img=${direct}]` : "[img]";
    }

    function nexusBbTagAttribute(tag: string, rawAttrs?: string): string | undefined {
        const direct = nexusBbDirectAttribute(rawAttrs);
        switch (tag) {
            case "url":
                return nexusBbAttribute(rawAttrs, "href")
                    ?? nexusBbAttribute(rawAttrs, "url")
                    ?? direct
                    ?? nexusBbFirstAttributeValue(rawAttrs);
            case "img":
                return nexusBbAttribute(rawAttrs, "src")
                    ?? nexusBbAttribute(rawAttrs, "url")
                    ?? direct
                    ?? nexusBbFirstAttributeValue(rawAttrs);
            case "quote":
                return nexusBbAttribute(rawAttrs, "name")
                    ?? nexusBbAttribute(rawAttrs, "author")
                    ?? nexusBbAttribute(rawAttrs, "cite")
                    ?? direct
                    ?? nexusBbFirstAttributeValue(rawAttrs);
            case "spoiler":
            case "collapse":
            case "details":
            case "accordion":
            case "accordionitem":
                return nexusBbAttribute(rawAttrs, "title")
                    ?? nexusBbAttribute(rawAttrs, "label")
                    ?? nexusBbAttribute(rawAttrs, "name")
                    ?? direct
                    ?? nexusBbFirstAttributeValue(rawAttrs);
            case "tab":
            case "note":
            case "info":
            case "warning":
            case "important":
            case "tip":
            case "box":
            case "panel":
            case "fieldset":
            case "notice":
            case "success":
            case "danger":
            case "error":
            case "li":
                return nexusBbAttribute(rawAttrs, "title")
                    ?? nexusBbAttribute(rawAttrs, "label")
                    ?? nexusBbAttribute(rawAttrs, "name")
                    ?? direct
                    ?? nexusBbFirstAttributeValue(rawAttrs);
            case "align":
            case "background":
            case "bgcolor":
            case "columns":
            case "cols":
            case "float":
            case "color":
            case "heading":
            case "highlight":
            case "indent":
            case "list":
            case "ol":
            case "olist":
            case "size":
            case "youtube":
            case "video":
            case "media":
            case "embed":
                return nexusBbAttribute(rawAttrs, "type")
                    ?? nexusBbAttribute(rawAttrs, "src")
                    ?? nexusBbAttribute(rawAttrs, "url")
                    ?? nexusBbAttribute(rawAttrs, "href")
                    ?? nexusBbAttribute(rawAttrs, "data")
                    ?? nexusBbAttribute(rawAttrs, "style")
                    ?? nexusBbAttribute(rawAttrs, "color")
                    ?? nexusBbAttribute(rawAttrs, "background")
                    ?? nexusBbAttribute(rawAttrs, "background-color")
                    ?? nexusBbAttribute(rawAttrs, "align")
                    ?? nexusBbAttribute(rawAttrs, "float")
                    ?? nexusBbAttribute(rawAttrs, tag)
                    ?? direct
                    ?? nexusBbFirstAttributeValue(rawAttrs);
            default:
                return direct ?? nexusBbFirstAttributeValue(rawAttrs);
        }
    }

    function nexusBbDirectAttribute(rawAttrs?: string): string | undefined {
        const trimmed = (rawAttrs ?? "").trim().replace(/\/\s*$/, "").trim();
        if (!trimmed.startsWith("=")) {
            return undefined;
        }

        return cleanNexusBbAttributeValue(trimmed.slice(1));
    }

    function nexusBbAttribute(rawAttrs: string | undefined, name: string): string | undefined {
        const attrs = rawAttrs ?? "";
        const pattern = new RegExp(`${name}\\s*=\\s*(?:"([^"]*)"|'([^']*)'|([^\\s\\]]+))`, "i");
        const match = attrs.match(pattern);
        return match ? cleanNexusBbAttributeValue(match[1] ?? match[2] ?? match[3]) : undefined;
    }

    function nexusBbFirstAttributeValue(rawAttrs?: string): string | undefined {
        const trimmed = (rawAttrs ?? "")
            .trim()
            .replace(/\/\s*$/, "")
            .trim();
        if (!trimmed || trimmed.includes("=")) {
            return undefined;
        }

        const quoted = trimmed.match(/^"([^"]*)"|^'([^']*)'/);
        if (quoted) {
            return cleanNexusBbAttributeValue(quoted[1] ?? quoted[2]);
        }

        return cleanNexusBbAttributeValue(trimmed.split(/\s+/)[0]);
    }

    function cleanNexusBbAttributeValue(value?: string): string | undefined {
        const cleaned = decodeHtmlEntities(value ?? "")
            .replace(/^[\s"'=]+|[\s"']+$/g, "")
            .replace(/[\[\]\r\n]/g, " ")
            .replace(/\s+/g, " ")
            .trim()
            .slice(0, 500);
        return cleaned || undefined;
    }

    function collectNexusNodeText(nodes: NexusRichNode[]): string {
        return nodes.map(node => node.kind === "text" ? node.value : collectNexusNodeText(node.children)).join("");
    }

    function htmlAttribute(attrs: string, name: string): string | null {
        const pattern = new RegExp(`${name}\\s*=\\s*(?:"([^"]*)"|'([^']*)'|([^\\s>]+))`, "i");
        const match = attrs.match(pattern);
        return match ? decodeHtmlEntities(match[1] ?? match[2] ?? match[3] ?? "") : null;
    }

    function nexusHtmlMediaMarkup(tag: string, attrs: string, body = ""): string {
        const source = htmlMediaSource(attrs, body);
        if (!source) {
            return "";
        }

        const title = cleanNexusBbAttributeValue(
            htmlAttribute(attrs, "title")
            ?? htmlAttribute(attrs, "aria-label")
            ?? htmlAttribute(attrs, "alt")
            ?? ""
        );
        const normalizedTag = tag.toLowerCase() === "youtube" ? "youtube" : "media";
        const titleAttribute = title ? ` title="${title}"` : "";
        return `\n\n[${normalizedTag}${titleAttribute}]${source}[/${normalizedTag}]\n\n`;
    }

    function htmlMediaSource(attrs: string, body = ""): string | null {
        const direct = htmlAttribute(attrs, "src")
            ?? htmlAttribute(attrs, "data")
            ?? htmlAttribute(attrs, "href");
        if (direct) {
            return direct;
        }

        const source = body.match(/<(?:source|param)\b([^>]*)>/i);
        if (!source) {
            return null;
        }

        return htmlAttribute(source[1], "src")
            ?? htmlAttribute(source[1], "value")
            ?? htmlAttribute(source[1], "data");
    }

    function htmlAlignmentAttribute(attrs: string): "left" | "center" | "right" | "justify" | null {
        const direct = htmlAttribute(attrs, "align");
        const style = htmlAttribute(attrs, "style");
        const classes = htmlAttribute(attrs, "class") ?? "";
        const styled = style?.match(/text-align\s*:\s*([a-z-]+)/i)?.[1];
        const classed = classes.match(/(?:^|\s)(?:align|text)-(left|center|right|justify)(?:\s|$)/i)?.[1]
            ?? classes.match(/(?:^|\s)(left|center|right|justify)(?:\s|$)/i)?.[1];
        const raw = decodeHtmlEntities(direct ?? styled ?? classed ?? "").trim().toLowerCase();
        return raw === "left" || raw === "center" || raw === "right" || raw === "justify" ? raw : null;
    }

    function htmlColorAttribute(attrs: string): string | null {
        const direct = htmlAttribute(attrs, "color");
        const style = htmlAttribute(attrs, "style");
        const styled = style?.match(/(?:^|;)\s*color\s*:\s*([^;]+)/i)?.[1];
        return safeNexusColor(direct ?? styled ?? "");
    }

    function htmlBackgroundColorAttribute(attrs: string): string | null {
        const direct = htmlAttribute(attrs, "bgcolor");
        const style = htmlAttribute(attrs, "style");
        const styled = style?.match(/(?:^|;)\s*background(?:-color)?\s*:\s*([^;]+)/i)?.[1];
        return safeNexusColor(direct ?? styled ?? "");
    }

    function nexusHtmlStyledInline(attrs: string, body: string): string {
        const style = htmlAttribute(attrs, "style") ?? "";
        const color = htmlColorAttribute(attrs);
        const background = htmlBackgroundColorAttribute(attrs);
        const size = htmlSizeAttribute(attrs);
        const fontClass = htmlFontClassAttribute(attrs);
        const weight = style.match(/(?:^|;)\s*font-weight\s*:\s*([^;]+)/i)?.[1]?.trim().toLowerCase();
        const fontStyle = style.match(/(?:^|;)\s*font-style\s*:\s*([^;]+)/i)?.[1]?.trim().toLowerCase();
        const textDecoration = style.match(/(?:^|;)\s*text-decoration(?:-line)?\s*:\s*([^;]+)/i)?.[1]?.trim().toLowerCase() ?? "";
        const wrappers: Array<[string, string]> = [];

        if (fontClass) {
            wrappers.push([`[font=${fontClass}]`, "[/font]"]);
        }

        if (color) {
            wrappers.push([`[color=${color}]`, "[/color]"]);
        }

        if (background) {
            wrappers.push([`[background=${background}]`, "[/background]"]);
        }

        if (size) {
            wrappers.push([`[size=${size}]`, "[/size]"]);
        }

        if (weight && (weight === "bold" || weight === "bolder" || Number.parseInt(weight, 10) >= 600)) {
            wrappers.push(["[b]", "[/b]"]);
        }

        if (fontStyle === "italic" || fontStyle === "oblique") {
            wrappers.push(["[i]", "[/i]"]);
        }

        if (textDecoration.includes("underline")) {
            wrappers.push(["[u]", "[/u]"]);
        }

        if (textDecoration.includes("line-through")) {
            wrappers.push(["[s]", "[/s]"]);
        }

        return wrappers.reduce((text, [open, close]) => `${open}${text}${close}`, body);
    }

    function htmlFontClassAttribute(attrs: string): "mono" | "serif" | "sans" | null {
        const direct = htmlAttribute(attrs, "face")
            ?? htmlAttribute(attrs, "font")
            ?? htmlAttribute(attrs, "font-family");
        const style = htmlAttribute(attrs, "style");
        const styled = style?.match(/(?:^|;)\s*font-family\s*:\s*([^;]+)/i)?.[1];
        const classes = htmlAttribute(attrs, "class") ?? "";
        const classed = classes.match(/(?:^|\s)(mono|monospace|serif|sans|sans-serif|code|preformatted)(?:\s|$)/i)?.[1];
        return safeNexusFontClass(direct ?? styled ?? classed ?? "");
    }

    function hasNexusBlockishMarkup(value: string): boolean {
        return /<(?:blockquote|details|div|dl|figure|h[1-6]|ol|p|pre|section|table|ul)\b/i.test(value)
            || /\[(?:table|list|olist|quote|spoiler|collapse|details|accordion|accordionitem|columns|cols|tabs|box|panel|fieldset|notice|note|info|warning|important|tip)(?:[=\s\]])/i.test(value);
    }

    function htmlSizeAttribute(attrs: string): string | null {
        const direct = htmlAttribute(attrs, "size");
        const style = htmlAttribute(attrs, "style");
        const styled = style?.match(/(?:^|;)\s*font-size\s*:\s*([^;]+)/i)?.[1];
        const raw = decodeHtmlEntities(direct ?? styled ?? "")
            .trim()
            .toLowerCase()
            .replace(/;+\s*$/g, "")
            .replace(/\s*!important\s*$/i, "");
        if (!raw) {
            return null;
        }

        const named = raw.match(/^(?:xx-small|x-small|small|medium|large|x-large|xx-large|smaller|larger)$/)?.[0];
        if (named) {
            return named;
        }

        const numeric = raw.match(/^(\d+(?:\.\d+)?)(px|em|rem|%)?$/);
        if (!numeric) {
            return null;
        }

        const value = Number.parseFloat(numeric[1]);
        if (!Number.isFinite(value)) {
            return null;
        }

        const unit = numeric[2] ?? "px";
        if (unit === "%") {
            return `${Math.max(60, Math.min(180, value))}%`;
        }

        if (unit === "em" || unit === "rem") {
            return `${Math.max(0.7, Math.min(1.8, value))}${unit}`;
        }

        return `${Math.max(9, Math.min(28, value))}px`;
    }

    function htmlIndentLevelAttribute(attrs: string): 1 | 2 | 3 | 4 | null {
        const style = htmlAttribute(attrs, "style") ?? "";
        const raw = style.match(/(?:^|;)\s*(?:margin-left|padding-left)\s*:\s*([^;]+)/i)?.[1];
        if (!raw) {
            return null;
        }

        const numeric = Number.parseFloat(raw);
        if (!Number.isFinite(numeric) || numeric <= 0) {
            return null;
        }

        if (raw.includes("%")) {
            return Math.max(1, Math.min(4, Math.round(numeric / 6))) as 1 | 2 | 3 | 4;
        }

        if (raw.includes("em") || raw.includes("rem")) {
            return Math.max(1, Math.min(4, Math.round(numeric / 1.5))) as 1 | 2 | 3 | 4;
        }

        return Math.max(1, Math.min(4, Math.round(numeric / 28))) as 1 | 2 | 3 | 4;
    }

    function nexusHtmlTableCellAttributes(attrs: string): string {
        const colspan = safeNexusTableSpan(htmlAttribute(attrs, "colspan") ?? htmlAttribute(attrs, "col") ?? undefined);
        const rowspan = safeNexusTableSpan(htmlAttribute(attrs, "rowspan") ?? htmlAttribute(attrs, "row") ?? undefined);
        return [
            colspan ? `colspan="${colspan}"` : "",
            rowspan ? `rowspan="${rowspan}"` : ""
        ].filter(Boolean).map(attribute => ` ${attribute}`).join("");
    }

    function nexusHtmlImageAttributes(attrs: string): string {
        const alignment = htmlImageAlignment(attrs);
        const width = safeNexusCssLength(htmlAttribute(attrs, "width"));
        const height = safeNexusCssLength(htmlAttribute(attrs, "height"));
        return [
            alignment ? ` align="${alignment}"` : "",
            width ? ` width="${width}"` : "",
            height ? ` height="${height}"` : ""
        ].join("");
    }

    function htmlImageAlignment(attrs: string): "left" | "center" | "right" | null {
        const direct = htmlAttribute(attrs, "align");
        const style = htmlAttribute(attrs, "style");
        const floated = style?.match(/(?:^|;)\s*float\s*:\s*([a-z-]+)/i)?.[1];
        const textAlign = style?.match(/(?:^|;)\s*text-align\s*:\s*([a-z-]+)/i)?.[1];
        const marginAuto = style && /margin(?:-left)?\s*:\s*auto/i.test(style) && /margin(?:-right)?\s*:\s*auto/i.test(style);
        const raw = decodeHtmlEntities(direct ?? floated ?? textAlign ?? (marginAuto ? "center" : "")).trim().toLowerCase();
        return raw === "left" || raw === "center" || raw === "right" ? raw : null;
    }

    function nexusImageUrlAttribute(rawAttrs?: string, attr?: string): string | undefined {
        return nexusBbAttribute(rawAttrs, "src")
            ?? nexusBbAttribute(rawAttrs, "url")
            ?? (safeNexusUrl(attr) ? attr : undefined);
    }

    function nexusImageAlignment(rawAttrs?: string, attr?: string): "left" | "center" | "right" | null {
        const candidate = nexusBbAttribute(rawAttrs, "align")
            ?? nexusBbAttribute(rawAttrs, "float")
            ?? nexusBbAttribute(rawAttrs, "position")
            ?? (attr && !safeNexusUrl(attr) ? attr : undefined);
        const normalized = decodeHtmlEntities(candidate ?? "").trim().toLowerCase().replace(/^['"]|['"]$/g, "");
        return normalized === "left" || normalized === "center" || normalized === "right" ? normalized : null;
    }

    function nexusImageStyleAttribute(rawAttrs?: string): string {
        const width = safeNexusCssLength(nexusBbAttribute(rawAttrs, "width") ?? nexusBbAttribute(rawAttrs, "w"));
        const height = safeNexusCssLength(nexusBbAttribute(rawAttrs, "height") ?? nexusBbAttribute(rawAttrs, "h"));
        const style = [
            width ? `width: ${width}` : "",
            height ? `height: ${height}` : ""
        ].filter(Boolean).join("; ");
        return style ? ` style="${escapeAttribute(style)}"` : "";
    }

    function safeNexusFloat(value?: string): "left" | "right" | "center" {
        const normalized = decodeHtmlEntities(value ?? "").trim().toLowerCase().replace(/^['"]|['"]$/g, "");
        if (normalized === "right" || normalized === "center") {
            return normalized;
        }

        return "left";
    }

    function safeNexusCalloutKind(value?: string): "note" | "info" | "warning" | "important" | "tip" {
        const normalized = decodeHtmlEntities(value ?? "").trim().toLowerCase().replace(/^['"]|['"]$/g, "");
        if (normalized === "info" || normalized === "warning" || normalized === "important" || normalized === "tip") {
            return normalized;
        }

        return "note";
    }

    function nexusCalloutLabel(kind: "note" | "info" | "warning" | "important" | "tip"): string {
        switch (kind) {
            case "info":
                return "Info";
            case "warning":
                return "Warning";
            case "important":
                return "Important";
            case "tip":
                return "Tip";
            default:
                return "Note";
        }
    }

    function safeNexusBoxKind(value?: string): "box" | "panel" | "fieldset" | "notice" | "success" | "danger" | "error" {
        const normalized = decodeHtmlEntities(value ?? "").trim().toLowerCase().replace(/^['"]|['"]$/g, "");
        if (normalized === "panel" || normalized === "fieldset" || normalized === "notice" || normalized === "success" || normalized === "danger" || normalized === "error") {
            return normalized;
        }

        return "box";
    }

    function nexusBoxLabel(kind: "box" | "panel" | "fieldset" | "notice" | "success" | "danger" | "error"): string {
        switch (kind) {
            case "panel":
                return "Panel";
            case "fieldset":
                return "Section";
            case "notice":
                return "Notice";
            case "success":
                return "Confirmed";
            case "danger":
            case "error":
                return "Important";
            default:
                return "";
        }
    }

    function safeNexusCssLength(value?: string | null): string | null {
        const raw = decodeHtmlEntities(value ?? "")
            .trim()
            .toLowerCase()
            .replace(/;+\s*$/g, "")
            .replace(/\s*!important\s*$/i, "")
            .replace(/^['"]|['"]$/g, "");
        const percent = raw.match(/^(\d+(?:\.\d+)?)%$/);
        if (percent) {
            const numeric = Number.parseFloat(percent[1]);
            return Number.isFinite(numeric) ? `${Math.max(10, Math.min(100, numeric))}%` : null;
        }

        const pixels = raw.match(/^(\d+(?:\.\d+)?)(?:px)?$/);
        if (pixels) {
            const numeric = Number.parseFloat(pixels[1]);
            return Number.isFinite(numeric) ? `${Math.max(24, Math.min(900, numeric))}px` : null;
        }

        return null;
    }

    function safeNexusUrl(value?: string): string | null {
        const trimmed = decodeHtmlEntities(value ?? "")
            .trim()
            .replace(/^['"]|['"]$/g, "");
        if (!trimmed || /[\r\n<>]/.test(trimmed)) {
            return null;
        }

        let candidate = trimmed;
        if (trimmed.startsWith("//")) {
            candidate = `https:${trimmed}`;
        } else if (/^www\./i.test(trimmed)) {
            candidate = `https://${trimmed}`;
        } else if (trimmed.startsWith("/")) {
            candidate = `https://www.nexusmods.com${trimmed}`;
        } else if (/^(?:sonsoftheforest|games|users|mods)\//i.test(trimmed)) {
            candidate = `https://www.nexusmods.com/${trimmed}`;
        }
        try {
            const url = new URL(candidate);
            return url.protocol === "https:" || url.protocol === "http:" ? url.toString() : null;
        } catch {
            return null;
        }
    }

    function safeNexusAlignment(value?: string): "left" | "center" | "right" | "justify" {
        const alignment = decodeHtmlEntities(value ?? "").trim().toLowerCase().replace(/^['"]|['"]$/g, "");
        if (alignment === "justify") {
            return "justify";
        }

        return alignment === "center" || alignment === "right" ? alignment : "left";
    }

    function safeNexusLabel(value?: string): string {
        return decodeHtmlEntities(value ?? "")
            .replace(/^['"]|['"]$/g, "")
            .replace(/\s+/g, " ")
            .trim()
            .slice(0, 80);
    }

    function safeNexusListItemLabel(value?: string): string {
        return safeNexusLabel(value)
            .replace(/[\[\]\r\n]/g, "")
            .trim()
            .slice(0, 60);
    }

    function safeNexusImageAlt(value?: string): string {
        return safeNexusLabel(value)
            .replace(/[\[\]\r\n]/g, "")
            .trim()
            .slice(0, 120);
    }

    function safeNexusFontClass(value?: string): "mono" | "serif" | "sans" | null {
        const normalized = decodeHtmlEntities(value ?? "")
            .replace(/^['"]|['"]$/g, "")
            .replace(/\s*!important\s*$/i, "")
            .toLowerCase();
        if (!normalized) {
            return null;
        }

        if (/(?:consolas|courier|monaco|menlo|mono|fixed|terminal|cascadia|jetbrains|source code|lucida console|inconsolata)/.test(normalized)) {
            return "mono";
        }

        if (/(?:georgia|garamond|cambria|times|serif)/.test(normalized) && !/sans/.test(normalized)) {
            return "serif";
        }

        if (/(?:arial|verdana|tahoma|trebuchet|calibri|segoe|helvetica|sans)/.test(normalized)) {
            return "sans";
        }

        return null;
    }

    function isOrderedNexusListAttr(value?: string): boolean {
        const attr = decodeHtmlEntities(value ?? "").trim().replace(/^['"]|['"]$/g, "");
        if (!attr) {
            return false;
        }

        const lowered = attr.toLowerCase();
        if (nexusUnorderedListStyle(attr) || lowered === "bullet" || lowered === "bullets") {
            return false;
        }

        return Boolean(nexusOrderedListType(attr)) || lowered === "number" || lowered === "numbers" || lowered === "numeric";
    }

    function nexusListMarkerStyle(value: string | undefined, ordered: boolean): string | null {
        return ordered ? nexusOrderedListType(value) : nexusUnorderedListStyle(value);
    }

    function nexusUnorderedListStyle(value?: string): "disc" | "circle" | "square" | "none" | "dash" | "check" | null {
        const attr = decodeHtmlEntities(value ?? "").trim().replace(/^['"]|['"]$/g, "").toLowerCase();
        if (!attr || attr === "bullet" || attr === "bullets") {
            return null;
        }

        if (attr === "disc" || attr === "circle" || attr === "square") {
            return attr;
        }

        if (attr === "none" || attr === "plain" || attr === "no-bullet" || attr === "nobullet") {
            return "none";
        }

        if (attr === "dash" || attr === "hyphen") {
            return "dash";
        }

        if (attr === "check" || attr === "checklist" || attr === "checkbox" || attr === "tick") {
            return "check";
        }

        return null;
    }

    function nexusOrderedListType(value?: string): "1" | "a" | "A" | "i" | "I" | null {
        const attr = decodeHtmlEntities(value ?? "").trim().replace(/^['"]|['"]$/g, "");
        const lowered = attr.toLowerCase();

        if (attr === "1" || lowered === "decimal" || lowered === "number" || lowered === "numbers" || lowered === "numeric") {
            return "1";
        }

        if (attr === "a" || lowered === "lower-alpha" || lowered === "alpha") {
            return "a";
        }

        if (attr === "A" || lowered === "upper-alpha") {
            return "A";
        }

        if (attr === "i" || lowered === "lower-roman" || lowered === "roman") {
            return "i";
        }

        if (attr === "I" || lowered === "upper-roman") {
            return "I";
        }

        return null;
    }

    function safeNexusHeadingLevel(value?: string): "1" | "2" | "3" | "4" | "5" | "6" {
        const level = decodeHtmlEntities(value ?? "").trim().replace(/^['"]|['"]$/g, "");
        return /^[1-6]$/.test(level) ? level as "1" | "2" | "3" | "4" | "5" | "6" : "3";
    }

    function safeNexusIndentLevel(value?: string): 1 | 2 | 3 | 4 {
        const numeric = Number.parseInt(decodeHtmlEntities(value ?? "").trim().replace(/^['"]|['"]$/g, ""), 10);
        if (!Number.isFinite(numeric)) {
            return 1;
        }

        return Math.max(1, Math.min(4, numeric)) as 1 | 2 | 3 | 4;
    }

    function safeNexusTableSpan(value?: string): number | undefined {
        const numeric = Number.parseInt(decodeHtmlEntities(value ?? "").trim().replace(/^['"]|['"]$/g, ""), 10);
        if (!Number.isFinite(numeric) || numeric <= 1) {
            return undefined;
        }

        return Math.min(6, numeric);
    }

    function safeNexusMediaUrl(kind: string, attr?: string, text?: string): string | null {
        const raw = cleanNexusBbAttributeValue(attr) || cleanNexusBbAttributeValue(text) || "";
        const url = safeNexusUrl(raw);
        if (url) {
            return url;
        }

        if (kind === "youtube" && /^[a-z0-9_-]{6,32}$/i.test(raw)) {
            return `https://www.youtube.com/watch?v=${encodeURIComponent(raw)}`;
        }

        return null;
    }

    function nexusMediaRenderData(kind: string, rawAttrs?: string, attr?: string, text?: string): { url: string; label: string; inlineLabel: string; urlLabel: string } | null {
        const tag = kind.toLowerCase();
        const rawUrl = nexusBbAttribute(rawAttrs, "src")
            ?? nexusBbAttribute(rawAttrs, "url")
            ?? nexusBbAttribute(rawAttrs, "href")
            ?? nexusBbAttribute(rawAttrs, "data")
            ?? (safeNexusUrl(attr) || (tag === "youtube" && /^[a-z0-9_-]{6,32}$/i.test(attr ?? "")) ? attr : undefined);
        const url = safeNexusMediaUrl(tag, rawUrl, text);
        if (!url) {
            return null;
        }

        const label = safeNexusLabel(
            nexusBbAttribute(rawAttrs, "title")
            ?? nexusBbAttribute(rawAttrs, "label")
            ?? nexusBbAttribute(rawAttrs, "name")
            ?? ""
        ) || (tag === "youtube" ? "YouTube Video" : tag === "video" ? "Video" : "Media");
        const inlineLabel = tag === "youtube" ? `Open ${label}` : `Open ${label}`;
        let urlLabel = url;
        try {
            const parsed = new URL(url);
            urlLabel = parsed.hostname.replace(/^www\./i, "") + parsed.pathname;
        } catch {
            urlLabel = url;
        }

        return { url, label, inlineLabel, urlLabel };
    }

    function safeNexusColor(value?: string): string | null {
        const color = decodeHtmlEntities(value ?? "")
            .trim()
            .replace(/;+\s*$/g, "")
            .replace(/\s*!important\s*$/i, "")
            .replace(/^['"]|['"]$/g, "");
        if (/^#[0-9a-f]{3}(?:[0-9a-f]{3})?(?:[0-9a-f]{2})?$/i.test(color)) {
            return color;
        }

        const rgb = color.match(/^rgba?\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})(?:\s*,\s*(0|1|0?\.\d+))?\s*\)$/i);
        if (rgb) {
            const red = Math.max(0, Math.min(255, Number.parseInt(rgb[1], 10)));
            const green = Math.max(0, Math.min(255, Number.parseInt(rgb[2], 10)));
            const blue = Math.max(0, Math.min(255, Number.parseInt(rgb[3], 10)));
            if (rgb[4] !== undefined) {
                const alpha = Math.max(0, Math.min(1, Number.parseFloat(rgb[4])));
                return `rgba(${red}, ${green}, ${blue}, ${alpha})`;
            }

            return `rgb(${red}, ${green}, ${blue})`;
        }

        const lowered = color.toLowerCase();
        return NEXUS_SAFE_COLOR_NAMES.has(lowered) ? lowered : null;
    }

    function nexusSizeClass(value?: string): string {
        const raw = decodeHtmlEntities(value ?? "").trim().toLowerCase();
        if (raw === "xx-small" || raw === "x-small" || raw === "tiny") {
            return "nexus-rich-size-tiny";
        }

        if (raw === "small" || raw === "smaller") {
            return "nexus-rich-size-small";
        }

        if (raw === "large" || raw === "larger" || raw === "x-large") {
            return "nexus-rich-size-large";
        }

        if (raw === "xx-large" || raw === "huge") {
            return "nexus-rich-size-xlarge";
        }

        const numeric = Number.parseFloat(raw);
        if (Number.isFinite(numeric)) {
            const looksLikePixels = raw.includes("px") || numeric > 7;
            const looksLikePercent = raw.includes("%");
            const looksLikeRelative = raw.includes("em") || raw.includes("rem");
            if (!looksLikePixels && !looksLikePercent && !looksLikeRelative) {
                if (numeric <= 1) {
                    return "nexus-rich-size-tiny";
                }

                if (numeric <= 2) {
                    return "nexus-rich-size-small";
                }

                if (numeric >= 7) {
                    return "nexus-rich-size-xlarge";
                }

                if (numeric >= 5) {
                    return "nexus-rich-size-large";
                }

                return "nexus-rich-size-medium";
            }

            if (looksLikePercent ? numeric <= 75 : looksLikeRelative ? numeric <= 0.8 : looksLikePixels ? numeric <= 9 : numeric <= 0.85) {
                return "nexus-rich-size-tiny";
            }

            if (looksLikePercent ? numeric <= 90 : looksLikeRelative ? numeric < 1 : looksLikePixels ? numeric <= 11 : numeric <= 2) {
                return "nexus-rich-size-small";
            }

            if (looksLikePercent ? numeric >= 150 : looksLikeRelative ? numeric >= 1.55 : looksLikePixels ? numeric >= 24 : numeric >= 1.45) {
                return "nexus-rich-size-xlarge";
            }

            if (looksLikePercent ? numeric >= 115 : looksLikeRelative ? numeric >= 1.2 : looksLikePixels ? numeric >= 18 : numeric >= 5) {
                return "nexus-rich-size-large";
            }
        }

        return "nexus-rich-size-medium";
    }

    function decodeHtmlEntities(value: string): string {
        return value
            .replace(/&#x([0-9a-f]+);/gi, (_match, hex: string) => decodeNumericEntity(Number.parseInt(hex, 16)))
            .replace(/&#(\d+);/g, (_match, decimal: string) => decodeNumericEntity(Number.parseInt(decimal, 10)))
            .replace(/&nbsp;/gi, " ")
            .replace(/&quot;/gi, "\"")
            .replace(/&#39;/g, "'")
            .replace(/&apos;/gi, "'")
            .replace(/&#92;/g, "\\")
            .replace(/&amp;/gi, "&")
            .replace(/&lt;/gi, "<")
            .replace(/&gt;/gi, ">");
    }

    function decodeNumericEntity(codePoint: number): string {
        try {
            return Number.isFinite(codePoint) && codePoint >= 0 && codePoint <= 0x10ffff
                ? String.fromCodePoint(codePoint)
                : "";
        } catch {
            return "";
        }
    }

    function escapeHtml(value: string): string {
        return value
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;")
            .replace(/"/g, "&quot;")
            .replace(/'/g, "&#39;");
    }

    function escapeAttribute(value: string): string {
        return escapeHtml(value);
    }
</script>

<div class="column nexus-page" bind:this={nexusPageElement}>
    <section class="account-panel">
        <div class="account-copy">
            <span class="panel-title">Vortex / Nexus Mods</span>
            {#if session.is_connected}
                <span class="panel-subtitle">Connected as {session.user?.name ?? "Nexus user"} · {nexusMembershipLabel()}</span>
            {:else}
                <span class="panel-subtitle">Login through Nexus to inspect Nexus-hosted Sons Of The Forest mods and Vortex deployments. Browser SSO requires Nexus app approval; manual tokens are for development testing only.</span>
            {/if}
        </div>

        {#if session.is_connected}
            <div class="account-actions">
                <label class="auto-endorse-control">
                    <input type="checkbox" checked={autoEndorseDownloadedMods} on:change={toggleAutoEndorse} />
                    <span class="auto-endorse-label">
                        <span>Auto-endorse Vortex downloads</span>
                        <small>{autoEndorseQueueLabel}</small>
                    </span>
                </label>
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
            <div
                class="rate-meter"
                class:rate-meter-warning={isRateLimitWarning(session.rate_limit.hourly_remaining, session.rate_limit.hourly_limit)}
                class:rate-meter-low={isRateLimitLow(session.rate_limit.hourly_remaining, session.rate_limit.hourly_limit)}
                style={rateLimitStyle(session.rate_limit.hourly_remaining, session.rate_limit.hourly_limit)}
            >
                <span><b>Hourly</b> {rateLimitLabel(session.rate_limit.hourly_remaining, session.rate_limit.hourly_limit)}</span>
                <div class="rate-track"><span></span></div>
            </div>
            <div
                class="rate-meter"
                class:rate-meter-warning={isRateLimitWarning(session.rate_limit.daily_remaining, session.rate_limit.daily_limit)}
                class:rate-meter-low={isRateLimitLow(session.rate_limit.daily_remaining, session.rate_limit.daily_limit)}
                style={rateLimitStyle(session.rate_limit.daily_remaining, session.rate_limit.daily_limit)}
            >
                <span><b>Daily</b> {rateLimitLabel(session.rate_limit.daily_remaining, session.rate_limit.daily_limit)}</span>
                <div class="rate-track"><span></span></div>
            </div>
        </div>
    {/if}

    {#if session.is_connected}
        <div class="vortex-summary">
            <div class="summary-card">
                <span class="summary-label">Nexus Account</span>
                <span class="summary-value">{nexusMembershipLabel()}</span>
                <span class="summary-note">{nexusMembershipNote()}</span>
            </div>
            <div class="summary-card" class:active-summary={!!vortexStagingPath}>
                <span class="summary-label">Vortex Deployment</span>
                <span class="summary-value">{vortexStagingPath ? "Detected" : "Not detected"}</span>
            </div>
            <button type="button" class="summary-card summary-action" class:summary-card-selected={catalogMode === "installed" && selectedInstallFilter === "all"} on:click={showInstalledInventory}>
                <span class="summary-label">Installed</span>
                <span class="summary-value">{installedCount}</span>
            </button>
            <button type="button" class="summary-card summary-action" class:summary-card-selected={selectedInstallFilter === "attention"} class:attention-summary={(catalogMode === "online" ? onlineAttentionCount : installedAttentionCount) > 0} on:click={showCurrentAttention}>
                <span class="summary-label">Attention</span>
                <span class="summary-value">{catalogMode === "online" ? onlineAttentionCount : installedAttentionCount}</span>
                <span class="summary-note">{catalogMode === "online" ? "Tracked missing, updates, conflicts" : "Updates, disabled, conflicts"}</span>
            </button>
            <div class="summary-card">
                <span class="summary-label">Vortex / Native / Manual</span>
                <span class="summary-value">{vortexCount} / {nativeCount} / {manualCount}</span>
            </div>
            <button type="button" class="summary-card summary-action" class:summary-card-selected={selectedInstallFilter === "updates" || selectedInstallFilter === "disabled"} class:update-summary={updateCount > 0 || disabledCount > 0} on:click={showUpdatesOrDisabled}>
                <span class="summary-label">Updates / Disabled</span>
                <span class="summary-value">{updateCount} / {disabledCount}</span>
                <span class="summary-note">Local deployment state</span>
            </button>
            <button type="button" class="summary-card summary-action" class:summary-card-selected={selectedInstallFilter === "conflicts"} class:conflict-summary={conflictCount > 0} on:click={showLocalConflicts}>
                <span class="summary-label">Local Conflicts</span>
                <span class="summary-value">{conflictCount}</span>
                <span class="summary-note">{conflictCount > 0 ? "Review duplicate deployments" : "No duplicate installs"}</span>
            </button>
        </div>

        <section class="catalog-panel">
            <div class="catalog-toolbar">
                <div class="catalog-mode-buttons">
                    <button class:cat-btn-selected={catalogMode === "online"} on:click={() => catalogMode = "online"}>Online Nexus</button>
                    <button class:cat-btn-selected={catalogMode === "installed"} on:click={() => catalogMode = "installed"}>Installed ({installedCount})</button>
                </div>
                <div class="view-buttons" class:feed-hidden={catalogMode === "installed"}>
                    <button class="btn-left cat-btn" class:cat-btn-selected={selectedView === "all"} on:click={() => loadMods("all")}>{viewLabel("all")}</button>
                    <button class="cat-btn middle-btn" class:cat-btn-selected={selectedView === "trending"} on:click={() => loadMods("trending")}>{viewLabel("trending")}</button>
                    <button class="cat-btn middle-btn" class:cat-btn-selected={selectedView === "latest_added"} on:click={() => loadMods("latest_added")}>{viewLabel("latest_added")}</button>
                    <button class="btn-right cat-btn" class:cat-btn-selected={selectedView === "latest_updated"} on:click={() => loadMods("latest_updated")}>{viewLabel("latest_updated")}</button>
                </div>
                <button class="cat-btn refresh-btn" disabled={isLoading || refreshCooldownSeconds > 0} title={manualRefreshButtonTitle} on:click={() => loadMods(selectedView, true)}>{manualRefreshButtonLabel}</button>
            </div>

            <div class="notice api-note">
                <span>Nexus requests are cached locally for {NEXUS_CACHE_TTL_MINUTES} minutes. {formatCatalogLoadedAt(nexusCatalogLoadedAt)}.</span>
                {#if catalogMode === "online"}
                    <span>{visibleNexusMods.length} shown from {mods.length} loaded. {onlineAttentionCount > 0 ? `${onlineAttentionCount} need attention.` : ""} {trackedModsLoaded ? `${trackedCount} tracked.` : ""} {conflictCount > 0 ? `${conflictCount} in conflicts.` : ""}</span>
                {:else}
                    <span>{visibleInstalledEntries.length} shown from {installedCount} installed. {installedAttentionCount > 0 ? `${installedAttentionCount} need attention.` : ""} {trackedModsLoaded ? `${trackedCount} tracked.` : ""} {conflictCount > 0 ? `${conflictCount} in conflicts.` : ""}</span>
                {/if}
            </div>

            <div class="action-queue" aria-label="Nexus action queue">
                <button type="button" class="queue-chip" class:queue-chip-hot={endorsementQueueCount > 0} class:queue-chip-selected={catalogMode === "installed" && selectedInstallFilter === "endorsements"} on:click={showEndorsementQueue}>
                    <b>{endorsementQueueCount}</b>
                    <span>
                        <span>Endorse</span>
                        <small>{endorsementsLoaded ? "Installed Nexus mods" : "Loading state"}</small>
                    </span>
                </button>
                <button type="button" class="queue-chip" class:queue-chip-hot={trackedMissingCount > 0} class:queue-chip-selected={catalogMode === "online" && selectedInstallFilter === "tracked"} on:click={showTrackedQueue}>
                    <b>{trackedMissingCount}</b>
                    <span>
                        <span>Tracked Missing</span>
                        <small>{trackedModsLoaded ? "Watch list gaps" : "Loading tracked"}</small>
                    </span>
                </button>
                <button type="button" class="queue-chip" class:queue-chip-hot={updateCount > 0} class:queue-chip-selected={selectedInstallFilter === "updates"} on:click={() => { catalogMode = "installed"; selectedInstallFilter = "updates"; }}>
                    <b>{updateCount}</b>
                    <span>
                        <span>Updates</span>
                        <small>Version review</small>
                    </span>
                </button>
                <button type="button" class="queue-chip" class:queue-chip-hot={disabledCount > 0} class:queue-chip-selected={catalogMode === "installed" && selectedInstallFilter === "disabled"} on:click={() => { catalogMode = "installed"; selectedInstallFilter = "disabled"; }}>
                    <b>{disabledCount}</b>
                    <span>
                        <span>Disabled</span>
                        <small>Local state</small>
                    </span>
                </button>
                <button type="button" class="queue-chip" class:queue-chip-hot={conflictCount > 0} class:queue-chip-selected={selectedInstallFilter === "conflicts"} on:click={showLocalConflicts}>
                    <b>{conflictCount}</b>
                    <span>
                        <span>Conflicts</span>
                        <small>Duplicate installs</small>
                    </span>
                </button>
            </div>

            <div class="nexus-filter-row" class:compact-filter-row={!showEmbeddedSearch}>
                {#if showEmbeddedSearch}
                    <input class="generic-input key-input" value={nexusSearchTerm} on:input={(event) => handleNexusSearchInput((event.currentTarget as HTMLInputElement).value)} placeholder="Search Nexus" />
                {/if}
                <select bind:value={selectedNexusCategory}>
                    <option value="all">All categories</option>
                    {#each nexusCategoryOptions as category}
                        <option value={category}>{category}</option>
                    {/each}
                </select>
                <select bind:value={selectedInstallFilter}>
                    <option value="all">All installs</option>
                    <option value="attention">Needs attention</option>
                    <option value="installed">Installed</option>
                    <option value="missing">Not installed</option>
                    <option value="updates">Updates</option>
                    <option value="disabled">Disabled</option>
                    <option value="vortex">Vortex</option>
                    <option value="native">OpenACAI store</option>
                    <option value="manual">Manual</option>
                    <option value="tracked">Tracked</option>
                    <option value="endorsements">Needs endorsement</option>
                    <option value="conflicts">Conflicts</option>
                </select>
                <select bind:value={selectedModTypeFilter} aria-label="Filter by mod type">
                    <option value="all">All mod types</option>
                    <option value="bepinex-plugin">BepInEx plugins</option>
                    <option value="redloader-mod">RedLoader mods</option>
                    <option value="redloader-library">RedLoader libraries</option>
                    <option value="vortex">Vortex / Nexus</option>
                    <option value="native">OpenACAI native</option>
                    <option value="manual">Manual / local</option>
                </select>
                {#if catalogMode === "online"}
                    <select bind:value={selectedNexusSort} aria-label="Sort Nexus mods">
                        <option value="attention">Sort: attention</option>
                        <option value="updated">Sort: updated</option>
                        <option value="downloads">Sort: downloads</option>
                        <option value="endorsements">Sort: endorsements</option>
                        <option value="name">Sort: name</option>
                        <option value="version">Sort: version</option>
                    </select>
                {:else}
                    <select bind:value={selectedInstalledSort} aria-label="Sort installed mods">
                        <option value="attention">Sort: attention</option>
                        <option value="name">Sort: name</option>
                        <option value="source">Sort: source</option>
                        <option value="location">Sort: location</option>
                        <option value="state">Sort: state</option>
                        <option value="version">Sort: version</option>
                    </select>
                {/if}
                <button class="cat-btn filter-reset-btn" disabled={!hasActiveNexusFilters} on:click={clearNexusFilters}>Clear</button>
            </div>

            {#if selectedInstallFilter === "conflicts" && localConflicts.length > 0}
                <div class="conflict-review" aria-live="polite">
                    <div class="conflict-review-head">
                        <div>
                            <span class="detail-section-title">Conflict Review</span>
                            <p>{localConflicts.length} duplicate deployment {localConflicts.length === 1 ? "group" : "groups"} detected across {conflictCount} installed entries.</p>
                        </div>
                        <button class="cat-btn" on:click={showInstalledInventory}>Show All Installed</button>
                    </div>

                    <div class="conflict-group-list">
                        {#each localConflicts as conflict}
                            <section class="conflict-group">
                                <div class="conflict-group-title">
                                    <span>{conflict.label}</span>
                                    <small>{conflictGroupSummary(conflict)}</small>
                                </div>

                                {#each conflict.entries as entry}
                                    <div class="conflict-entry">
                                        <div class="conflict-entry-main">
                                            <span>{entry.name}</span>
                                            <small>{describeInstallSource(entry)} · {loaderTypeLabel(entry)} · {entry.expectedLocation} · {entry.enabled ? "Enabled" : "Disabled"}</small>
                                            <small>{conflictEntryPath(entry)}</small>
                                        </div>
                                        <div class="conflict-entry-actions">
                                            <span>{entry.version ?? "-"}</span>
                                            <button
                                                class:disable-action={entry.enabled}
                                                disabled={entry.installSource === "vortex" || activeConflictEntryKey === inventoryEntryKey(entry)}
                                                on:click={() => setConflictEntryState(entry, !entry.enabled)}
                                            >
                                                {conflictEntryActionLabel(entry)}
                                            </button>
                                            {#if numericNexusId(entry.nexusModId)}
                                                <button on:click={() => openConflictEntryDetails(entry)}>Details</button>
                                            {/if}
                                            <button on:click={() => openInventoryLocation(entry)}>Open Folder</button>
                                        </div>
                                    </div>
                                {/each}
                            </section>
                        {/each}
                    </div>
                </div>
            {/if}

            <div class="nexus-scroller" aria-live="polite">
                {#if catalogMode === "online"}
                    {#each visibleNexusMods as mod}
                        {@const match = installedMatch(mod)}
                        {@const updateVerdict = nexusUpdateVerdict(mod, match)}
                        {@const conflict = conflictForEntry(match)}
                        {@const previewUrls = catalogPreviewUrls(mod)}
                        {@const previewIndex = catalogPreviewIndex(mod, previewUrls)}
                        <article class="nexus-card" class:nexus-installed={!!match} class:nexus-conflict={!!conflict}>
                            <div class="thumbnail-frame">
                                <button class="thumbnail-button" aria-label={`Open ${mod.name} details`} on:click={() => openModDetails(mod)}>
                                    <img
                                        class="nexus-img"
                                        src={catalogPreviewImage(mod, previewUrls)}
                                        on:error={handleNexusImageError}
                                        alt=""
                                    />
                                </button>
                                {#if previewUrls.length > 1}
                                    <div class="thumbnail-nav" aria-label={`${mod.name} preview images`}>
                                        <button type="button" aria-label="Previous preview image" title="Previous preview image" on:click={(event) => cycleCatalogPreview(mod, previewUrls, -1, event)}>
                                            <LucideChevronLeft class="thumbnail-icon" aria-hidden="true" />
                                        </button>
                                        <span><LucideImages class="thumbnail-icon" aria-hidden="true" />{previewIndex + 1}/{previewUrls.length}</span>
                                        <button type="button" aria-label="Next preview image" title="Next preview image" on:click={(event) => cycleCatalogPreview(mod, previewUrls, 1, event)}>
                                            <LucideChevronRight class="thumbnail-icon" aria-hidden="true" />
                                        </button>
                                    </div>
                                {:else}
                                    <div
                                        class="thumbnail-count"
                                        class:thumbnail-count-fallback={previewUrls.length === 0}
                                        aria-label={previewUrls.length === 1 ? `${mod.name} has one preview image` : `${mod.name} uses the local fallback preview image`}
                                        title={previewUrls.length === 1 ? "1 preview image" : "Local fallback preview"}
                                    >
                                        <LucideImages class="thumbnail-icon" aria-hidden="true" />
                                        <span>{previewUrls.length === 1 ? "1" : "Local"}</span>
                                    </div>
                                {/if}
                            </div>

                            <div class="nexus-body">
                                <div class="card-head">
                                    <button class="title-stack title-button" on:click={() => openModDetails(mod)}>
                                        <span class="mod-title">{mod.name}</span>
                                        <span class="mod-byline">{mod.category_name ?? "Nexus"} · {mod.loader_type ?? "Unknown type"} · {mod.author ?? mod.uploaded_by ?? "Unknown author"}</span>
                                    </button>
                                    <div class="card-pills">
                                        {#if conflict}
                                            <span class="source-pill conflict-pill">Conflict</span>
                                        {/if}
                                        <span class="source-pill" class:source-vortex={match?.installSource === "vortex"} class:source-native={match?.installSource === "native"} class:source-manual={match?.installSource === "manual"}>
                                            {describeInstallSource(match)}
                                        </span>
                                    </div>
                                </div>

                                <button class="description-content description-button" on:click={() => openModDetails(mod)}>{mod.summary ?? "No summary is available from Nexus for this mod."}</button>

                                <div class="facts">
                                    <span>Version <b>{mod.version ?? "-"}</b></span>
                                    <span>State <b title={updateVerdict.reason} class:update-state-update={updateVerdict.tone === "update"} class:update-state-current={updateVerdict.tone === "current"} class:update-state-tracked={updateVerdict.tone === "tracked"} class:update-state-review={updateVerdict.tone === "review"}>{updateVerdict.label}</b></span>
                                    <span>Updated <b>{formatTimestamp(mod.updated_timestamp, mod.updated_time)}</b></span>
                                    <span>Downloads <b>{formatNumber(mod.mod_downloads)}</b></span>
                                    <span>Endorsements <b>{formatNumber(mod.endorsement_count)}</b></span>
                                </div>

                                <div class="nexus-card-footer">
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
                                        <span class="match-detail" class:conflict-detail={!!conflict}>{conflict ? `Conflict: ${conflict.entries.length} matching installs` : `${loaderTypeLabel(match)} in ${match.expectedLocation}`}</span>
                                    {:else}
                                        <span class="match-detail missing-match">Not installed in this game folder.</span>
                                    {/if}

                                    <div class="button-row">
                                        <button class="track-btn" disabled={activeNexusTrackId === mod.mod_id} on:click={() => toggleNexusTracking(mod)}>
                                            {activeNexusTrackId === mod.mod_id ? "Saving..." : isNexusModTracked(mod.mod_id) ? "Tracked" : "Track"}
                                        </button>
                                        {#if match}
                                            {#if isNexusModEndorsed(mod.mod_id)}
                                                <button class="endorsed-btn" disabled>Endorsed</button>
                                            {:else}
                                                <button class="endorse-btn" disabled={activeNexusEndorseId === mod.mod_id} on:click={() => endorseNexusMod(mod)}>
                                                    {activeNexusEndorseId === mod.mod_id ? "Endorsing..." : "Endorse"}
                                                </button>
                                            {/if}
                                        {/if}
                                        <button class="vortex-install-btn" disabled={activeNexusActionId === mod.mod_id} on:click={() => installRecommendedWithVortex(mod)}>
                                            {activeNexusActionId === mod.mod_id ? "Preparing..." : vortexActionLabel(mod)}
                                        </button>
                                        <button on:click={() => openModDetails(mod)}>Details</button>
                                        <button on:click={() => openModPage(mod)}>Open Page</button>
                                    </div>
                                </div>
                            </div>
                        </article>
                    {/each}
                {:else}
                    {#each visibleInstalledEntries as entry}
                        {@const entryNexusModId = numericNexusId(entry.nexusModId)}
                        {@const inventoryVerdict = inventoryUpdateVerdict(entry)}
                        {@const conflict = conflictForEntry(entry)}
                        <article class="nexus-card inventory-card" class:nexus-installed={entry.enabled} class:nexus-conflict={!!conflict}>
                            <div class="inventory-icon">
                                <span>{entry.loaderType === "bepinex-plugin" ? "BEP" : "RED"}</span>
                            </div>

                            <div class="nexus-body">
                                <div class="card-head">
                                    <div class="title-stack">
                                        <span class="mod-title">{entry.name}</span>
                                        <span class="mod-byline">{loaderTypeLabel(entry)} · {entry.author ?? "Unknown author"}</span>
                                    </div>
                                    <div class="card-pills">
                                        {#if conflict}
                                            <span class="source-pill conflict-pill">Conflict</span>
                                        {/if}
                                        <span class="source-pill" class:source-vortex={entry.installSource === "vortex"} class:source-native={entry.installSource === "native"} class:source-manual={entry.installSource === "manual"}>
                                            {describeInstallSource(entry)}
                                        </span>
                                    </div>
                                </div>

                                <div class="facts inventory-facts">
                                    <span>Version <b>{entry.version ?? "-"}</b></span>
                                    <span>Update <b title={inventoryVerdict.reason} class:update-state-update={inventoryVerdict.tone === "update"} class:update-state-current={inventoryVerdict.tone === "current"} class:update-state-review={inventoryVerdict.tone === "review"}>{inventoryVerdict.label}</b></span>
                                    <span>Location <b>{entry.expectedLocation}</b></span>
                                    <span>State <b>{entry.enabled ? "Enabled" : "Disabled"}</b></span>
                                    <span>Store <b>{entry.store}</b></span>
                                </div>

                                {#if entry.vortexPackage}
                                    <span class="description-content">Vortex package: {entry.vortexPackage}</span>
                                {:else if entry.packagePath}
                                    <span class="description-content">{entry.packagePath}</span>
                                {:else}
                                    <span class="description-content">{entry.assemblyPath ?? "No package path was detected."}</span>
                                {/if}

                                <div class="nexus-card-footer">
                                    <label class="nexus-enable" class:vortex-disabled={entry.installSource === "vortex"}>
                                        <input
                                            type="checkbox"
                                            checked={entry.enabled}
                                            disabled={entry.installSource === "vortex"}
                                            on:change={(event) => toggleMatchedMod(entry, event)}
                                        />
                                        <span>{entry.enabled ? "Enabled" : "Disabled"}</span>
                                    </label>
                                    <span class="match-detail" class:conflict-detail={!!conflict}>{conflict ? `Conflict: ${conflict.entries.length} matching installs` : entry.installSource === "vortex" ? "Managed by Vortex deployment metadata" : "Managed by OpenACAI Mod Manager"}</span>

                                    <div class="button-row">
                                        <button on:click={() => openInventoryLocation(entry)}>Open Folder</button>
                                        {#if entryNexusModId}
                                            <button class="track-btn" disabled={activeNexusTrackId === entryNexusModId} on:click={() => toggleInstalledTracking(entry)}>
                                                {activeNexusTrackId === entryNexusModId ? "Saving..." : isNexusModTracked(entryNexusModId) ? "Tracked" : "Track"}
                                            </button>
                                            {#if isNexusModEndorsed(entryNexusModId)}
                                                <button class="endorsed-btn" disabled>Endorsed</button>
                                            {:else}
                                                <button class="endorse-btn" disabled={activeNexusEndorseId === entryNexusModId} on:click={() => endorseInstalledEntry(entry)}>
                                                    {activeNexusEndorseId === entryNexusModId ? "Endorsing..." : "Endorse"}
                                                </button>
                                            {/if}
                                            <button on:click={() => openInstalledEntryDetails(entry)}>Details</button>
                                            <button on:click={() => shell.open(`https://www.nexusmods.com/sonsoftheforest/mods/${entryNexusModId}`)}>Nexus Page</button>
                                        {/if}
                                    </div>
                                </div>
                            </div>
                        </article>
                    {/each}
                {/if}

                {#if isLoading}
                    <div class="loading-line">
                        <SvgSpinnersBlocksWave />
                        <span>{status}</span>
                    </div>
                {/if}

                {#if !isLoading && catalogMode === "online" && visibleNexusMods.length === 0}
                    <div class="notice empty-nexus">No Nexus mods match the current filters.</div>
                {/if}

                {#if !isLoading && catalogMode === "installed" && visibleInstalledEntries.length === 0}
                    <div class="notice empty-nexus">No installed mods match the current filters.</div>
                {/if}
            </div>
        </section>
    {:else}
        <div class="notice">Nexus login is required before this section can compare Vortex packages against native installs.</div>
    {/if}
</div>

{#if selectedMod}
    <div class="detail-backdrop" role="presentation">
        <section class="detail-panel" aria-label={`${selectedMod.name} details`}>
            <div class="detail-header">
                <div class="detail-title">
                    <span class="panel-title">{selectedModDetails?.name ?? selectedMod.name}</span>
                    <span class="panel-subtitle">{selectedModDetails?.category_name ?? "Nexus"} · {selectedModDetails?.loader_type ?? "Unknown type"} · {selectedModDetails?.author ?? selectedModDetails?.uploaded_by ?? "Unknown author"}</span>
                </div>
                <div class="detail-header-actions">
                    {#if detailBackStack.length > 0}
                        <button class="back-detail" on:click={openPreviousDetail}>Back</button>
                    {/if}
                    <button class="close-detail" on:click={closeModDetails}>Close</button>
                </div>
            </div>

            <div class="detail-section-nav" aria-label="Detail sections">
                <button type="button" on:click={() => scrollDetailSection("description")}>
                    <span>Description</span>
                    <b>{selectedDetailDescriptionCanToggle ? "Long" : "Ready"}</b>
                </button>
                <button type="button" on:click={() => scrollDetailSection("files")}>
                    <span>Files</span>
                    <b>{selectedModFiles.length}</b>
                </button>
                <button type="button" on:click={() => scrollDetailSection("dependencies")}>
                    <span>Dependencies</span>
                    <b>{detailDependencyNavText}</b>
                </button>
                <button type="button" on:click={() => scrollDetailSection("plan")}>
                    <span>Plan</span>
                    <b>{installPlanToneLabel(selectedInstallPlan.tone)}</b>
                </button>
                <button type="button" on:click={() => scrollDetailSection("changelog")}>
                    <span>Changelog</span>
                    <b>{selectedChangelogs.length}</b>
                </button>
                <button type="button" on:click={() => scrollDetailSection("deployment")}>
                    <span>Deploy</span>
                    <b>{selectedNexusFile ? "Ready" : "Choose file"}</b>
                </button>
            </div>

            {#if isDetailLoading}
                <div class="loading-line">
                    <SvgSpinnersBlocksWave />
                    <span>Loading mod details...</span>
                </div>
            {/if}

            <div class="detail-grid">
                <div class="detail-main">
                    <div class="detail-media-frame">
                        <img
                            class="detail-img"
                            src={selectedDetailPreviewImage(currentDetailPreviewUrls)}
                            on:error={(event) => handleNexusImageError(event, NEXUS_DETAIL_PLACEHOLDER_IMAGE)}
                            alt=""
                        />
                        {#if currentDetailPreviewUrls.length > 1}
                            <div class="detail-media-nav" aria-label={`${selectedMod.name} preview images`}>
                                <button type="button" aria-label="Previous detail preview image" on:click={(event) => cycleSelectedDetailPreview(currentDetailPreviewUrls, -1, event)}>&lt;</button>
                                <span>{currentDetailPreviewIndex + 1}/{currentDetailPreviewUrls.length}</span>
                                <button type="button" aria-label="Next detail preview image" on:click={(event) => cycleSelectedDetailPreview(currentDetailPreviewUrls, 1, event)}>&gt;</button>
                            </div>
                        {/if}
                    </div>
                    {#if currentDetailPreviewUrls.length > 1}
                        <div class="detail-media-strip" aria-label={`${selectedMod.name} preview thumbnails`}>
                            {#each currentDetailPreviewUrls as previewUrl, previewIndex}
                                <button
                                    type="button"
                                    class:detail-media-thumb-active={previewIndex === currentDetailPreviewIndex}
                                    aria-label={`Show preview image ${previewIndex + 1} of ${currentDetailPreviewUrls.length}`}
                                    aria-pressed={previewIndex === currentDetailPreviewIndex}
                                    on:click={() => selectSelectedDetailPreview(previewIndex)}
                                >
                                    <img
                                        src={previewUrl}
                                        on:error={(event) => handleNexusImageError(event, NEXUS_PLACEHOLDER_IMAGE)}
                                        alt=""
                                    />
                                    <span>{previewIndex + 1}</span>
                                </button>
                            {/each}
                        </div>
                    {/if}

                    <div class="detail-text" bind:this={detailDescriptionSectionElement}>
                        <div class="detail-text-head">
                            <span class="detail-section-title">Directions / Description</span>
                            {#if selectedDetailDescriptionCanToggle}
                                <button class="description-toggle" type="button" on:click={() => detailDescriptionExpanded = !detailDescriptionExpanded}>
                                    {detailDescriptionExpanded ? "Show Less" : "Show More"}
                                </button>
                            {/if}
                        </div>
                        <div
                            class="nexus-rich-text"
                            class:detail-description-collapsed={selectedDetailDescriptionCanToggle && !detailDescriptionExpanded}
                        >
                            {@html selectedDetailDescriptionHtml}
                        </div>
                    </div>

                    <div class="changelog-box" bind:this={detailChangelogSectionElement}>
                        <span class="detail-section-title">Changelog</span>
                        {#if selectedChangelogs.length === 0}
                            <span class="dependency-empty">No API-listed changelog entries were returned for this mod.</span>
                        {:else}
                            {#each selectedChangelogs as changelog}
                                <div class="changelog-row">
                                    <span>{changelog.version}</span>
                                    <div class="nexus-rich-text changelog-rich-text">
                                        {@html renderNexusRichText(changelog.changes, "No changelog text was returned.")}
                                    </div>
                                </div>
                            {/each}
                        {/if}
                    </div>
                </div>

                <div class="detail-side">
                    <div class="detail-facts">
                        <span>Version <b>{selectedModDetails?.version ?? "-"}</b></span>
                        <span>Type <b>{selectedModDetails?.loader_type ?? "Unknown"}</b></span>
                        <span>Update <b title={selectedModUpdateReason()} class:update-state-update={selectedModUpdateTone() === "update"} class:update-state-current={selectedModUpdateTone() === "current"} class:update-state-tracked={selectedModUpdateTone() === "tracked"} class:update-state-review={selectedModUpdateTone() === "review"}>{selectedModUpdateLabel()}</b></span>
                        <span>Tracked <b>{isNexusModTracked(selectedMod.mod_id) ? "Yes" : "No"}</b></span>
                        <span>Updated <b>{formatTimestamp(selectedModDetails?.updated_timestamp, selectedModDetails?.updated_time)}</b></span>
                        <span>Downloads <b>{formatNumber(selectedModDetails?.mod_downloads)}</b></span>
                        <span>Installed <b>{describeInstallSource(installedMatch(selectedMod))}</b></span>
                        <span>Dependencies <b class:update-state-update={totalDependencyIssueCount > 0} class:update-state-tracked={totalDependencyReviewCount > 0 && totalDependencyIssueCount === 0} class:update-state-current={(resolvedDependencies.length > 0 || resolvedNestedDependencies.length > 0) && totalDependencyIssueCount === 0 && totalDependencyReviewCount === 0}>{dependencySummaryLabel()}</b></span>
                    </div>

                    <div
                        class="install-plan"
                        class:install-plan-ready={selectedInstallPlan.tone === "ready"}
                        class:install-plan-review={selectedInstallPlan.tone === "review"}
                        class:install-plan-blocked={selectedInstallPlan.tone === "blocked"}
                        bind:this={detailInstallPlanSectionElement}
                    >
                        <div class="install-plan-head">
                            <span class="detail-section-title">Install Plan</span>
                            <b>{installPlanToneLabel(selectedInstallPlan.tone)}</b>
                        </div>
                        <label class="install-placement-control">
                            <span>Placement</span>
                            <select bind:value={selectedInstallPlacement} aria-label="Install placement override" disabled={!selectedNexusFile}>
                                {#each INSTALL_PLACEMENTS as placement}
                                    <option value={placement}>{installPlacementLabel(placement)}</option>
                                {/each}
                            </select>
                        </label>
                        <div class="install-plan-facts">
                            <span>Action <b>{selectedInstallPlan.action}</b></span>
                            <span>Placement <b>{selectedInstallPlan.placement}</b></span>
                            <span>Target <b>{selectedInstallPlan.target}</b></span>
                            <span class="install-plan-file-fact">File <b>{selectedInstallFileLabel}</b></span>
                            <span>File ID <b>{selectedNexusFile?.file_id ?? "-"}</b></span>
                            <span>File version <b>{selectedFileVersionLabel}</b></span>
                            <span>Uploaded <b>{selectedFileUploadedLabel}</b></span>
                            <span>Size <b>{selectedFileSizeLabel}</b></span>
                        </div>
                        <div class="install-plan-notes">
                            {#each selectedInstallPlan.notes as note}
                                <span>{note}</span>
                            {/each}
                        </div>
                    </div>

                    {#if selectedNxmUrl}
                        <div class="nxm-link-box">
                            <span class="detail-section-title">Vortex Link</span>
                            <div class="nxm-link-row">
                                <code title={selectedNxmUrl}>{selectedNxmUrl}</code>
                                <button on:click={copySelectedNxmLink}>{nxmCopyState || "Copy NXM"}</button>
                            </div>
                        </div>
                    {/if}

                    {#if selectedInstallConflict}
                        <div class="detail-conflict-box" aria-live="polite">
                            <div class="detail-conflict-head">
                                <span class="detail-section-title">Local Conflict</span>
                                <b>{selectedInstallConflict.entries.length} installs</b>
                            </div>
                            <span class="detail-conflict-note">{selectedInstallConflict.label} · {conflictGroupSummary(selectedInstallConflict)}</span>

                            <div class="detail-conflict-list">
                                {#each selectedInstallConflict.entries as entry}
                                    <div class="conflict-entry">
                                        <div class="conflict-entry-main">
                                            <span>{entry.name}</span>
                                            <small>{describeInstallSource(entry)} · {loaderTypeLabel(entry)} · {entry.expectedLocation} · {entry.enabled ? "Enabled" : "Disabled"}</small>
                                            <small>{conflictEntryPath(entry)}</small>
                                        </div>
                                        <div class="conflict-entry-actions">
                                            <span>{entry.version ?? "-"}</span>
                                            <button
                                                class:disable-action={entry.enabled}
                                                disabled={entry.installSource === "vortex" || activeConflictEntryKey === inventoryEntryKey(entry)}
                                                on:click={() => setConflictEntryState(entry, !entry.enabled)}
                                            >
                                                {conflictEntryActionLabel(entry)}
                                            </button>
                                            {#if numericNexusId(entry.nexusModId)}
                                                <button on:click={() => openConflictEntryDetails(entry)}>Details</button>
                                            {/if}
                                            <button on:click={() => openInventoryLocation(entry)}>Open Folder</button>
                                        </div>
                                    </div>
                                {/each}
                            </div>
                        </div>
                    {/if}

                    <div class="file-picker" bind:this={detailFilesSectionElement}>
                        <span class="detail-section-title">Files</span>
                        <div class="file-readiness-grid" aria-label="File choice summary">
                            <span
                                class="file-readiness-chip"
                                class:file-readiness-ok={selectedModFiles.length > 0 && selectedFileReviewCount === 0}
                                class:file-readiness-review={selectedFileReviewCount > 0}
                                title={fileChoiceReadinessText}
                            >
                                <small>Files</small>
                                <b>{selectedModFiles.length}</b>
                                <span>{fileChoiceReadinessText}</span>
                            </span>
                            <span
                                class="file-readiness-chip"
                                class:file-readiness-ok={!!selectedNexusFile && !isReviewNexusFile(selectedNexusFile)}
                                class:file-readiness-review={!!selectedNexusFile && isReviewNexusFile(selectedNexusFile)}
                                class:file-readiness-warn={!selectedNexusFile && selectedModFiles.length > 0}
                                title={selectedFileReadinessText}
                            >
                                <small>Selected</small>
                                <b>{selectedNexusFile ? "1" : "0"}</b>
                                <span>{selectedFileReadinessText}</span>
                            </span>
                            <span
                                class="file-readiness-chip"
                                class:file-readiness-ok={selectedModFiles.length > 0 && selectedFileReviewCount === 0}
                                class:file-readiness-review={selectedFileReviewCount > 0}
                                title={fileReviewReadinessText}
                            >
                                <small>Review</small>
                                <b>{selectedFileReviewCount}</b>
                                <span>{fileReviewReadinessText}</span>
                            </span>
                        </div>
                        {#if selectedModFiles.length === 0 && !isDetailLoading}
                            <div class="notice empty-nexus">No downloadable files were returned by Nexus.</div>
                        {/if}

                        {#each displayedSelectedModFiles as file}
                            <button
                                class="file-row"
                                class:file-row-selected={selectedFileId === file.file_id}
                                class:file-row-recommended={recommendedNexusFileId === file.file_id}
                                class:file-row-review={isReviewNexusFile(file)}
                                on:click={() => selectNexusFile(file.file_id)}
                            >
                                <span class="file-row-head">
                                    <span class="file-name">{file.name}</span>
                                    {#if fileChoiceBadge(file, recommendedNexusFileId)}
                                        <b>{fileChoiceBadge(file, recommendedNexusFileId)}</b>
                                    {/if}
                                </span>
                                <span class="file-meta">{fileChoiceCategoryLabel(file)} · v{fileVersionLabel(file)} · {formatSizeKb(file.size)}</span>
                            </button>
                        {/each}
                    </div>

                    {#if selectedFileNotesAvailable}
                        <div class="selected-file-notes">
                            <div class="selected-file-notes-head">
                                <span class="detail-section-title">Selected File Notes</span>
                                <b>{selectedFileVersionLabel}</b>
                            </div>
                            <span class="selected-file-notes-subtitle" title={selectedInstallFileLabel}>{selectedInstallFileLabel}</span>
                            {#if selectedFileDescriptionHtml}
                                <div class="nexus-rich-text selected-file-rich-text">
                                    {@html selectedFileDescriptionHtml}
                                </div>
                            {/if}
                            {#if selectedFileChangelogHtml}
                                <div class="selected-file-changelog">
                                    <span>File changelog</span>
                                    <div class="nexus-rich-text selected-file-rich-text">
                                        {@html selectedFileChangelogHtml}
                                    </div>
                                </div>
                            {/if}
                        </div>
                    {/if}

                    <div class="dependency-box" bind:this={detailDependenciesSectionElement}>
                        <div class="dependency-box-head">
                            <span class="detail-section-title">Dependencies</span>
                            <button
                                type="button"
                                disabled={!nestedDependencyCheckAvailable() || isResolvingNestedDependencies}
                                title={nestedDependencyCheckAvailable()
                                    ? `Check up to ${MAX_NESTED_DEPENDENCY_FILES} dependency files across ${MAX_NESTED_DEPENDENCY_DEPTH} nested levels`
                                    : "No dependency file IDs were returned for recursive checks"}
                                on:click={resolveNestedDependencies}
                            >
                                {isResolvingNestedDependencies ? "Checking..." : "Check Nested"}
                            </button>
                        </div>
                        <div class="dependency-readiness-grid" aria-label="Dependency readiness summary">
                            <span
                                class="dependency-readiness-chip"
                                class:dependency-readiness-ok={resolvedDependencies.length > 0 && dependencyIssueCount === 0 && dependencyReviewCount === 0}
                                class:dependency-readiness-warn={dependencyIssueCount > 0}
                                class:dependency-readiness-review={dependencyReviewCount > 0 && dependencyIssueCount === 0}
                                title={apiDependencyReadinessLabel()}
                            >
                                <small>API</small>
                                <b>{resolvedDependencies.length}</b>
                                <span>{apiDependencyReadinessLabel()}</span>
                            </span>
                            <span
                                class="dependency-readiness-chip"
                                class:dependency-readiness-review={selectedAuthorRequirements.length > 0}
                                title={authorRequirementReadinessLabel()}
                            >
                                <small>Author</small>
                                <b>{selectedAuthorRequirements.length}</b>
                                <span>{authorRequirementReadinessLabel()}</span>
                            </span>
                            <span
                                class="dependency-readiness-chip"
                                class:dependency-readiness-ok={resolvedNestedDependencies.length > 0 && nestedDependencyIssueCount === 0 && nestedDependencyReviewCount === 0}
                                class:dependency-readiness-warn={nestedDependencyIssueCount > 0}
                                class:dependency-readiness-review={(isResolvingNestedDependencies || nestedDependencyReviewCount > 0 || (resolvedNestedDependencies.length === 0 && nestedDependencyCheckAvailable())) && nestedDependencyIssueCount === 0}
                                title={nestedDependencyReadinessLabel()}
                            >
                                <small>Nested</small>
                                <b>{resolvedNestedDependencies.length}</b>
                                <span>{nestedDependencyReadinessLabel()}</span>
                            </span>
                        </div>
                        {#if nestedDependencySummary}
                            <span class="dependency-empty">{nestedDependencySummary}</span>
                        {/if}
                        {#if selectedDependencyMessage}
                            <span class="dependency-empty">{selectedDependencyMessage}</span>
                        {/if}
                        {#if resolvedDependencies.length === 0}
                            <span class="dependency-empty">{selectedDependencyMessage ? "Still review the author directions for manual requirements." : "No API-listed dependencies for the selected file. Still review the author directions for manual requirements."}</span>
                        {:else}
                            {#each resolvedDependencies as dependency}
                                <div
                                    class="dependency-row"
                                    class:dependency-installed={dependency.status === "installed"}
                                    class:dependency-missing={dependency.status === "missing"}
                                    class:dependency-mismatch={dependency.status === "version-mismatch"}
                                    class:dependency-review={dependency.status === "review"}
                                >
                                    <div class="dependency-head">
                                        <span>{dependency.mod_name}</span>
                                        <b>{dependency.status === "installed" ? "Installed" : dependency.status === "missing" ? "Missing" : dependency.status === "version-mismatch" ? "Version" : "Review"}</b>
                                    </div>
                                    <small>{dependency.file_name ?? dependency.group_name ?? "Candidate file"} {dependencyRequirementLabel(dependency) ? `· ${dependencyRequirementLabel(dependency)}` : ""}</small>
                                    <small>{dependencyStatusLabel(dependency)}</small>
                                    <div class="dependency-actions">
                                        {#if dependency.mod_id}
                                            <button on:click={() => openDependencyDetails(dependency)}>Details</button>
                                            <button on:click={() => openDependencyPage(dependency)}>Nexus</button>
                                        {/if}
                                        {#if dependency.match}
                                            <button on:click={() => openDependencyLocation(dependency)}>Open Folder</button>
                                        {/if}
                                    </div>
                                </div>
                            {/each}
                        {/if}
                        {#if selectedAuthorRequirements.length > 0}
                            <div class="author-requirement-section">
                                <span class="dependency-subtitle">Author-linked requirements</span>
                                <span class="dependency-empty">These links came from Nexus author text, selected file notes, or changelog text and are not API dependency rows.</span>
                                {#each selectedAuthorRequirements as requirement (requirement.key)}
                                    <div class="author-requirement-row">
                                        <div class="author-requirement-main">
                                            <span>{requirement.label}</span>
                                            <small>{requirement.source}{requirement.mod_id ? ` · Mod ${requirement.mod_id}` : ""}</small>
                                        </div>
                                        <button on:click={() => openAuthorRequirement(requirement)}>{requirement.mod_id ? "Details" : "Open"}</button>
                                    </div>
                                {/each}
                            </div>
                        {/if}
                        {#if resolvedNestedDependencies.length > 0}
                            <div class="dependency-nested-section">
                                <span class="dependency-subtitle">Nested dependencies</span>
                                {#each resolvedNestedDependencies as source (source.key)}
                                    <div
                                        class="dependency-row"
                                        class:dependency-installed={source.dependency.status === "installed"}
                                        class:dependency-missing={source.dependency.status === "missing"}
                                        class:dependency-mismatch={source.dependency.status === "version-mismatch"}
                                        class:dependency-review={source.dependency.status === "review"}
                                    >
                                        <div class="dependency-head">
                                            <span>{source.dependency.mod_name}</span>
                                            <b>{source.dependency.status === "installed" ? "Installed" : source.dependency.status === "missing" ? "Missing" : source.dependency.status === "version-mismatch" ? "Version" : "Review"}</b>
                                        </div>
                                        <small>From {source.parentName} · depth {source.depth}</small>
                                        <small>{source.dependency.file_name ?? source.dependency.group_name ?? "Candidate file"} {dependencyRequirementLabel(source.dependency) ? `· ${dependencyRequirementLabel(source.dependency)}` : ""}</small>
                                        <small>{nestedDependencyStatusLabel(source)}</small>
                                        <div class="dependency-actions">
                                            {#if source.dependency.mod_id}
                                                <button on:click={() => openDependencyDetails(source.dependency)}>Details</button>
                                                <button on:click={() => openDependencyPage(source.dependency)}>Nexus</button>
                                            {/if}
                                            {#if source.dependency.match}
                                                <button on:click={() => openDependencyLocation(source.dependency)}>Open Folder</button>
                                            {/if}
                                        </div>
                                    </div>
                                {/each}
                            </div>
                        {/if}
                    </div>

                </div>
            </div>

            <div class="detail-footer" aria-label="Nexus deployment actions" bind:this={detailFooterElement}>
                <div class="detail-footer-copy">
                    <span class="detail-section-title">Deployment</span>
                    <small title={selectedInstallFileLabel}>{selectedInstallFileLabel}</small>
                </div>

                <div class="detail-actions">
                    <button class="install" disabled={!selectedNexusFile} on:click={installSelectedFileWithVortex}>{selectedInstallActionButtonLabel}</button>
                    <button class="track-btn" disabled={activeNexusTrackId === selectedMod.mod_id} on:click={toggleSelectedModTracking}>
                        {activeNexusTrackId === selectedMod.mod_id ? "Saving..." : isNexusModTracked(selectedMod.mod_id) ? "Tracked" : "Track"}
                    </button>
                    {#if installedMatch(selectedMod)}
                        {#if isNexusModEndorsed(selectedMod.mod_id)}
                            <button class="endorsed-btn" disabled>Endorsed</button>
                        {:else}
                            <button class="endorse-btn" disabled={activeNexusEndorseId === selectedMod.mod_id} on:click={endorseSelectedMod}>
                                {activeNexusEndorseId === selectedMod.mod_id ? "Endorsing..." : "Endorse"}
                            </button>
                        {/if}
                    {/if}
                    <button on:click={openSelectedModPage}>Open Page</button>
                </div>

                <div class="detail-link-actions" aria-label="Nexus page sections">
                    <button on:click={() => openSelectedModTab("description")}>Description</button>
                    <button on:click={openSelectedDownloadPage}>Files</button>
                    <button on:click={() => openSelectedModTab("posts")}>Posts</button>
                    <button on:click={() => openSelectedModTab("images")}>Images</button>
                    <button on:click={() => openSelectedModTab("bugs")}>Bugs</button>
                </div>
            </div>
        </section>
    </div>
{/if}

<style>
    .nexus-page {
        --nexus-card-min-height: clamp(138px, 17vh, 178px);
        --nexus-catalog-target-height: 360px;
        --nexus-scroller-target-height: 280px;
        --nexus-thumb-width: clamp(145px, 18vw, 230px);
        gap: clamp(0.55em, 1vh, 0.9em);
        height: 100%;
        justify-content: flex-start;
        min-height: 0;
        overflow: hidden;
    }

    .account-panel {
        align-items: center;
        background: linear-gradient(90deg, rgba(42, 42, 42, 0.92), rgba(24, 24, 24, 0.82));
        border: 1px solid rgba(255, 255, 255, 0.16);
        box-sizing: border-box;
        display: flex;
        flex: 0 0 auto;
        gap: 1em;
        justify-content: space-between;
        padding: clamp(0.65em, 1.25vh, 0.9em) 1em;
        text-align: left;
        width: 100%;
    }

    .account-copy {
        display: flex;
        flex-direction: column;
        min-width: 0;
    }

    .panel-title {
        color: #eefcff;
        font-size: clamp(1em, 1.8vh, 1.2em);
        font-weight: 800;
        letter-spacing: 0.02em;
    }

    .panel-subtitle {
        color: #9eb0bf;
        font-size: 0.85em;
        line-height: 1.25;
    }

    .account-actions,
    .connect-row,
    .button-row,
    .rate-row,
    .catalog-toolbar,
    .catalog-mode-buttons,
    .view-buttons,
    .nexus-card-footer {
        align-items: center;
        display: flex;
        gap: 0.5em;
    }

    .account-actions {
        flex-wrap: wrap;
        justify-content: flex-end;
    }

    .auto-endorse-control {
        align-items: center;
        background: rgba(12, 12, 12, 0.74);
        border: 1px solid rgba(255, 255, 255, 0.14);
        color: #b8c8d8;
        display: inline-flex;
        font-size: 0.75em;
        font-weight: 900;
        gap: 0.5em;
        min-height: 2.5em;
        padding: 0.35em 0.7em;
        text-transform: uppercase;
        text-align: left;
    }

    .auto-endorse-label {
        display: flex;
        flex-direction: column;
        gap: 0.1em;
        line-height: 1.12;
        min-width: 0;
    }

    .auto-endorse-label small {
        color: #8d99a5;
        font-size: 0.82em;
        font-weight: 800;
        text-transform: none;
    }

    .auto-endorse-control input[type="checkbox"] {
        appearance: none;
        background: rgba(8, 8, 8, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.35);
        display: grid;
        float: none;
        height: 1.05em;
        margin: 0;
        padding: 0;
        place-content: center;
        transform: none;
        width: 1.05em;
    }

    .auto-endorse-control input[type="checkbox"]::before {
        box-shadow: inset 1em 1em #78d9f4;
        content: "";
        height: 0.58em;
        transform: scale(0);
        transition: transform 120ms ease-in-out;
        width: 0.58em;
    }

    .auto-endorse-control input[type="checkbox"]:checked::before {
        transform: scale(1);
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
        display: grid;
        flex: 0 0 auto;
        font-size: 0.78em;
        gap: 0.55em;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        line-height: 1.1;
        margin-top: -0.15em;
    }

    .rate-meter {
        background: rgba(18, 18, 18, 0.72);
        border: 1px solid rgba(255, 255, 255, 0.1);
        box-sizing: border-box;
        display: flex;
        flex-direction: column;
        gap: 0.3em;
        min-width: 0;
        padding: 0.42em 0.55em;
    }

    .rate-meter > span {
        display: flex;
        gap: 0.6em;
        justify-content: space-between;
        min-width: 0;
        white-space: nowrap;
    }

    .rate-meter b {
        color: #e4edf4;
    }

    .rate-track {
        background: rgba(255, 255, 255, 0.09);
        height: 4px;
        overflow: hidden;
        width: 100%;
    }

    .rate-track span {
        background: #62f09b;
        display: block;
        height: 100%;
        width: var(--rate-percent, 0%);
    }

    .rate-meter-warning .rate-track span {
        background: #fdc66d;
    }

    .rate-meter-low .rate-track span {
        background: #fd7e7e;
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

    .vortex-summary {
        display: grid;
        flex: 0 0 auto;
        gap: 0.6em;
        grid-template-columns: repeat(auto-fit, minmax(135px, 1fr));
    }

    .summary-card {
        background: rgba(18, 18, 18, 0.76);
        border: 1px solid rgba(255, 255, 255, 0.12);
        box-sizing: border-box;
        display: flex;
        flex-direction: column;
        gap: 0.15em;
        min-width: 0;
        padding: 0.55em 0.75em;
    }

    button.summary-card {
        -webkit-mask-image: none;
        color: inherit;
        cursor: pointer;
        margin: 0;
        mask-image: none;
        text-align: left;
        text-transform: none;
    }

    .summary-action:hover,
    .summary-action:focus-visible {
        border-color: rgba(120, 217, 244, 0.55);
        box-shadow: inset 0 0 0 1px rgba(120, 217, 244, 0.14);
        outline: none;
    }

    .summary-card-selected {
        border-color: rgba(98, 240, 155, 0.62);
        box-shadow: inset 0 0 0 1px rgba(98, 240, 155, 0.16);
    }

    .summary-label {
        color: #8d99a5;
        font-size: 0.68em;
        font-weight: 900;
        letter-spacing: 0.1em;
        overflow-wrap: anywhere;
        text-transform: uppercase;
    }

    .summary-value {
        color: #e5e5e5;
        font-size: 0.9em;
        font-weight: 800;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .summary-note {
        color: #8fa3b5;
        font-size: 0.68em;
        font-weight: 700;
        overflow-wrap: anywhere;
    }

    .active-summary .summary-value {
        color: #62f09b;
    }

    .conflict-summary .summary-value {
        color: #fdc66d;
    }

    .attention-summary .summary-value,
    .update-summary .summary-value {
        color: #fdc66d;
    }

    .catalog-panel {
        display: flex;
        flex: 1 0 var(--nexus-catalog-target-height);
        flex-direction: column;
        gap: 0.65em;
        min-height: min(var(--nexus-catalog-target-height), 100%);
    }

    .catalog-toolbar {
        flex: 0 0 auto;
        justify-content: space-between;
    }

    .catalog-mode-buttons button {
        color: #c8c8c8;
        height: 2.55em;
        margin: 0;
        min-width: 10em;
        padding: 0 0.8em;
    }

    .feed-hidden {
        opacity: 0.28;
        pointer-events: none;
    }

    .api-note {
        align-items: center;
        color: #8d99a5;
        display: flex;
        flex: 0 0 auto;
        font-size: 0.78em;
        justify-content: space-between;
        line-height: 1.2;
        padding: 0.5em 0.75em;
    }

    .action-queue {
        display: grid;
        flex: 0 0 auto;
        gap: 0.5em;
        grid-template-columns: repeat(5, minmax(0, 1fr));
        min-width: 0;
    }

    .queue-chip {
        -webkit-mask-image: none;
        align-items: center;
        background: rgba(15, 15, 15, 0.82);
        border: 1px solid rgba(255, 255, 255, 0.12);
        box-sizing: border-box;
        color: #c9d4dd;
        cursor: pointer;
        display: grid;
        gap: 0.55em;
        grid-template-columns: auto minmax(0, 1fr);
        margin: 0;
        mask-image: none;
        min-height: 3em;
        min-width: 0;
        padding: 0.45em 0.65em;
        text-align: left;
        text-transform: none;
        width: 100%;
    }

    .queue-chip:hover,
    .queue-chip:focus-visible {
        border-color: rgba(120, 217, 244, 0.52);
        box-shadow: inset 0 0 0 1px rgba(120, 217, 244, 0.12);
        outline: none;
    }

    .queue-chip b {
        color: #e6f4fb;
        font-size: 1.1em;
        font-weight: 900;
        min-width: 1.6em;
        text-align: center;
    }

    .queue-chip > span {
        display: flex;
        flex-direction: column;
        gap: 0.05em;
        line-height: 1.05;
        min-width: 0;
    }

    .queue-chip > span > span,
    .queue-chip small {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .queue-chip > span > span {
        color: #d9e5ed;
        font-size: 0.78em;
        font-weight: 900;
    }

    .queue-chip small {
        color: #8295a5;
        font-size: 0.68em;
        font-weight: 800;
    }

    .queue-chip-hot b {
        color: #fdc66d;
    }

    .queue-chip-selected {
        border-color: rgba(98, 240, 155, 0.62);
        box-shadow: inset 0 0 0 1px rgba(98, 240, 155, 0.16);
    }

    .queue-chip-selected b {
        color: #62f09b;
    }

    .nexus-filter-row {
        display: grid;
        flex: 0 0 auto;
        gap: 0.6em;
        grid-template-columns: minmax(14em, 1fr) repeat(4, minmax(8.5em, 0.55fr)) minmax(6.5em, 0.28fr);
        width: 100%;
    }

    .nexus-filter-row.compact-filter-row {
        grid-template-columns: repeat(4, minmax(8.5em, 1fr)) minmax(6.5em, 0.38fr);
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
        height: 2.55em;
        margin: 0;
        padding: 0;
        width: 7em;
    }

    .refresh-btn {
        color: #c8c8c8;
        width: 8em;
    }

    .filter-reset-btn {
        color: #c8c8c8;
        width: 100%;
    }

    .middle-btn {
        border-radius: 0;
    }

    .cat-btn-selected {
        background-color: #111;
        color: #38d68d;
    }

    .conflict-review {
        background: rgba(18, 18, 18, 0.86);
        border: 1px solid rgba(253, 198, 109, 0.28);
        box-sizing: border-box;
        display: flex;
        flex: 0 0 auto;
        flex-direction: column;
        gap: 0.6em;
        max-height: clamp(170px, 25vh, 260px);
        min-height: 0;
        overflow: hidden;
        padding: 0.75em;
        text-align: left;
    }

    .conflict-review-head {
        align-items: center;
        display: flex;
        gap: 1em;
        justify-content: space-between;
        min-width: 0;
    }

    .conflict-review-head p {
        color: #9aa5af;
        font-size: 0.78em;
        font-weight: 700;
        margin: 0.25em 0 0;
    }

    .conflict-review-head .cat-btn {
        width: 10.5em;
    }

    .conflict-group-list {
        display: flex;
        flex-direction: column;
        gap: 0.55em;
        min-height: 0;
        overflow-y: auto;
        padding-right: 0.25em;
    }

    .conflict-group {
        background: rgba(10, 10, 10, 0.55);
        border: 1px solid rgba(255, 255, 255, 0.1);
        display: flex;
        flex-direction: column;
        gap: 0.45em;
        padding: 0.55em;
        text-align: left;
    }

    .conflict-group-title {
        align-items: baseline;
        display: flex;
        gap: 0.7em;
        justify-content: space-between;
        min-width: 0;
    }

    .conflict-group-title span {
        color: #eefcff;
        font-size: 0.88em;
        font-weight: 900;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .conflict-group-title small,
    .conflict-entry small {
        color: #9aa5af;
        font-size: 0.72em;
        font-weight: 700;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .conflict-entry {
        align-items: center;
        border-top: 1px solid rgba(255, 255, 255, 0.08);
        display: flex;
        gap: 0.75em;
        justify-content: space-between;
        min-width: 0;
        padding-top: 0.45em;
    }

    .conflict-entry-main {
        display: flex;
        flex: 1 1 auto;
        flex-direction: column;
        gap: 0.12em;
        min-width: 0;
        text-align: left;
    }

    .conflict-entry-main span {
        color: #d6dde5;
        font-size: 0.82em;
        font-weight: 900;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .conflict-entry-actions {
        align-items: center;
        display: flex;
        flex: 0 0 auto;
        gap: 0.5em;
    }

    .conflict-entry-actions span {
        color: #fdc66d;
        font-size: 0.75em;
        font-weight: 900;
        max-width: 7em;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .conflict-entry-actions button {
        font-size: 0.72em;
        margin: 0;
        min-width: 7.4em;
        padding: 0.48em 0.65em;
    }

    .conflict-entry-actions .disable-action {
        color: #fd9b9d;
    }

    .nexus-scroller {
        display: flex;
        flex: 1 0 var(--nexus-scroller-target-height);
        flex-direction: column;
        gap: 0.65em;
        min-height: min(var(--nexus-scroller-target-height), 100%);
        overflow-y: auto;
        padding: 0 0.45em 0.15em 0;
        scrollbar-gutter: stable;
    }

    .nexus-card {
        background: linear-gradient(90deg, rgba(24, 24, 24, 0.96), rgba(12, 12, 12, 0.88));
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-bottom-color: rgba(255, 255, 255, 0.22);
        box-sizing: border-box;
        display: grid;
        flex: 0 0 auto;
        gap: 0;
        grid-template-columns: var(--nexus-thumb-width) minmax(0, 1fr);
        min-height: var(--nexus-card-min-height);
        overflow: hidden;
    }

    .nexus-installed {
        border-left: 3px solid #62f09b;
    }

    .nexus-conflict {
        border-left: 3px solid #fdc66d;
    }

    .thumbnail-frame {
        background: rgba(8, 8, 8, 0.85);
        min-height: var(--nexus-card-min-height);
        min-width: 0;
        overflow: hidden;
        position: relative;
    }

    .thumbnail-button {
        background: rgba(8, 8, 8, 0.85);
        border: 0;
        box-shadow: none;
        cursor: pointer;
        display: block;
        height: 100%;
        margin: 0;
        min-height: 100%;
        min-width: 0;
        overflow: hidden;
        padding: 0;
        -webkit-mask-image: none;
        mask-image: none;
        width: 100%;
    }

    .nexus-img {
        background: #252525;
        display: block;
        height: 100%;
        min-height: var(--nexus-card-min-height);
        object-fit: cover;
        opacity: 0.9;
        width: 100%;
    }

    .nexus-body {
        box-sizing: border-box;
        display: flex;
        flex: 1;
        flex-direction: column;
        gap: 0.55em;
        min-width: 0;
        padding: 0.75em 0.85em;
        text-align: left;
    }

    .card-head {
        align-items: flex-start;
        display: flex;
        gap: 0.7em;
        justify-content: space-between;
        min-width: 0;
    }

    .card-pills {
        display: flex;
        flex: 0 0 auto;
        flex-wrap: wrap;
        gap: 0.35em;
        justify-content: flex-end;
        max-width: min(32vw, 24em);
        min-width: 0;
    }

    .title-stack {
        display: flex;
        flex-direction: column;
        min-width: 0;
    }

    .title-button,
    .description-button {
        background: transparent;
        border: 0;
        box-shadow: none;
        cursor: pointer;
        font: inherit;
        margin: 0;
        min-width: 0;
        padding: 0;
        text-align: left;
        -webkit-mask-image: none;
        mask-image: none;
    }

    .mod-title {
        color: #eefcff;
        font-size: clamp(0.98em, 1.6vh, 1.1em);
        font-weight: 900;
        line-height: 1.15;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .mod-byline {
        color: #8d99a5;
        font-size: 0.76em;
        font-weight: 800;
        letter-spacing: 0.03em;
        overflow: hidden;
        text-overflow: ellipsis;
        text-transform: uppercase;
        white-space: nowrap;
    }

    .description-content {
        color: #b2bdc8;
        display: -webkit-box;
        font-size: 0.84em;
        line-height: 1.3;
        margin: 0;
        min-height: 2.55em;
        overflow: hidden;
        -webkit-box-orient: vertical;
        -webkit-line-clamp: 2;
        line-clamp: 2;
    }

    .description-button {
        width: 100%;
    }

    .facts {
        color: #87929d;
        display: grid;
        font-size: 0.76em;
        gap: 0.25em 1em;
        grid-template-columns: repeat(5, minmax(0, 1fr));
        min-width: 0;
    }

    .facts span {
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .facts b {
        color: #d6dde5;
        font-weight: 800;
    }

    .nexus-card-footer {
        align-items: flex-end;
        gap: 0.65em;
        justify-content: space-between;
        margin-top: auto;
        min-width: 0;
    }

    .match-detail {
        color: #78d9f4;
        flex: 1 1 auto;
        font-size: 0.76em;
        font-weight: 800;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        text-transform: uppercase;
        white-space: nowrap;
    }

    .missing-match {
        color: #87929d;
    }

    .nexus-enable {
        align-items: center;
        background: rgba(18, 18, 18, 0.88);
        border: 1px solid rgba(255, 255, 255, 0.14);
        color: #62f09b;
        display: inline-flex;
        flex: 0 0 auto;
        font-size: 0.72em;
        font-weight: 900;
        gap: 0.5em;
        padding: 0.32em 0.5em;
        text-transform: uppercase;
        width: max-content;
    }

    .nexus-enable input[type="checkbox"] {
        appearance: none;
        background: rgba(8, 8, 8, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.35);
        display: grid;
        float: none;
        height: 1.05em;
        margin: 0;
        padding: 0;
        place-content: center;
        transform: none;
        width: 1.05em;
    }

    .nexus-enable input[type="checkbox"]::before {
        box-shadow: inset 1em 1em #62f09b;
        content: "";
        height: 0.58em;
        transform: scale(0);
        transition: transform 120ms ease-in-out;
        width: 0.58em;
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
        border-radius: 0;
        color: #bfc8d2;
        flex: 0 0 auto;
        font-size: 0.68em;
        font-weight: 800;
        line-height: 1.2;
        max-width: min(28vw, 18em);
        overflow: hidden;
        padding: 0.35em 0.55em;
        text-overflow: ellipsis;
        text-transform: uppercase;
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

    .conflict-pill {
        border-color: rgba(253, 198, 109, 0.42);
        color: #fdc66d;
    }

    .conflict-detail {
        color: #fdc66d;
    }

    .button-row {
        flex: 0 0 auto;
        margin-left: auto;
    }

    .button-row button {
        font-size: 0.76em;
        margin: 0;
        min-width: 6.6em;
        padding: 0.55em 0.8em;
    }

    .button-row .vortex-install-btn {
        color: #62f09b;
        min-width: 10.5em;
    }

    .button-row .track-btn,
    .detail-actions .track-btn {
        color: #fdc66d;
        min-width: 6.6em;
    }

    .button-row .endorse-btn,
    .detail-actions .endorse-btn {
        color: #78d9f4;
        min-width: 7.6em;
    }

    .button-row .endorsed-btn,
    .detail-actions .endorsed-btn {
        color: #62f09b;
        min-width: 7.6em;
        opacity: 0.76;
    }

    .inventory-card {
        grid-template-columns: clamp(90px, 9vw, 128px) minmax(0, 1fr);
        min-height: clamp(118px, 14vh, 152px);
    }

    .inventory-icon {
        align-items: center;
        background: radial-gradient(circle at 50% 45%, rgba(98, 240, 155, 0.16), rgba(8, 8, 8, 0.92) 62%);
        color: #cfeee0;
        display: flex;
        font-size: clamp(1.2em, 2.2vh, 1.7em);
        font-weight: 900;
        justify-content: center;
        letter-spacing: 0.1em;
        min-height: 100%;
    }

    .inventory-facts {
        grid-template-columns: repeat(5, minmax(0, 1fr));
    }

    .update-state-update {
        color: #fdc66d;
    }

    .update-state-current {
        color: #62f09b;
    }

    .update-state-tracked {
        color: #78d9f4;
    }

    .update-state-review {
        color: #fdc66d;
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
        flex: 0 0 auto;
    }

    .detail-backdrop {
        align-items: center;
        background: rgba(0, 0, 0, 0.72);
        bottom: 0;
        display: flex;
        justify-content: center;
        left: 0;
        padding: clamp(1em, 3vh, 2em);
        position: fixed;
        right: 0;
        top: 0;
        z-index: 20;
    }

    .detail-panel {
        --detail-panel-height: calc(100vh - clamp(1.2em, 3vh, 2.2em));
        --detail-scroll-section-max: clamp(280px, calc((var(--detail-panel-height) - 15em) / 1.65), 560px);
        background:
            linear-gradient(180deg, rgba(18, 18, 18, 0.96), rgba(6, 6, 6, 0.94)),
            rgba(0, 0, 0, 0.92);
        border: 1px solid rgba(255, 255, 255, 0.18);
        box-sizing: border-box;
        display: flex;
        flex-direction: column;
        gap: 0.8em;
        max-height: var(--detail-panel-height);
        max-width: min(97vw, 1440px);
        min-height: min(82vh, 820px);
        padding: clamp(1em, 2vh, 1.35em);
        width: 100%;
    }

    .thumbnail-nav {
        align-items: center;
        background: rgba(0, 0, 0, 0.54);
        border: 1px solid rgba(255, 255, 255, 0.12);
        bottom: 0.45em;
        box-sizing: border-box;
        display: flex;
        gap: 0.25em;
        left: 0.45em;
        max-width: calc(100% - 0.9em);
        padding: 0.2em;
        position: absolute;
        right: 0.45em;
    }

    .thumbnail-nav button {
        align-items: center;
        background: rgba(255, 255, 255, 0.08);
        border: 1px solid rgba(255, 255, 255, 0.12);
        box-shadow: none;
        color: #eefcff;
        display: flex;
        flex: 0 0 1.9em;
        font-size: 0.7em;
        font-weight: 900;
        height: 1.9em;
        justify-content: center;
        margin: 0;
        min-height: 0;
        min-width: 0;
        padding: 0;
        -webkit-mask-image: none;
        mask-image: none;
    }

    .thumbnail-nav span {
        align-items: center;
        color: #d6dde5;
        display: flex;
        flex: 1 1 auto;
        font-size: 0.68em;
        font-weight: 900;
        gap: 0.25em;
        justify-content: center;
        min-width: 0;
        text-align: center;
        white-space: nowrap;
    }

    .thumbnail-nav :global(.thumbnail-icon),
    .thumbnail-count :global(.thumbnail-icon) {
        display: block;
        flex: 0 0 auto;
        height: 1em;
        width: 1em;
    }

    .thumbnail-count {
        align-items: center;
        background: rgba(0, 0, 0, 0.56);
        border: 1px solid rgba(255, 255, 255, 0.12);
        box-sizing: border-box;
        color: #d6dde5;
        display: inline-flex;
        font-size: 0.68em;
        font-weight: 900;
        gap: 0.28em;
        left: 0.45em;
        max-width: calc(100% - 0.9em);
        min-height: 1.85em;
        overflow: hidden;
        padding: 0.2em 0.45em;
        position: absolute;
        text-overflow: ellipsis;
        top: 0.45em;
        white-space: nowrap;
    }

    .thumbnail-count-fallback {
        color: #aab3bd;
    }

    .detail-header {
        align-items: center;
        display: flex;
        gap: 0.75em;
        justify-content: space-between;
    }

    .detail-section-nav {
        display: grid;
        flex: 0 0 auto;
        gap: 0.45em;
        grid-template-columns: repeat(auto-fit, minmax(6.9em, 1fr));
        min-width: 0;
        padding-bottom: 0.05em;
    }

    .detail-section-nav button {
        align-items: start;
        background: rgba(255, 255, 255, 0.045);
        border: 1px solid rgba(255, 255, 255, 0.11);
        box-shadow: none;
        box-sizing: border-box;
        display: grid;
        gap: 0.12em;
        margin: 0;
        min-height: 2.75em;
        min-width: 0;
        padding: 0.38em 0.55em;
        text-align: left;
        -webkit-mask-image: none;
        mask-image: none;
    }

    .detail-section-nav button:hover {
        border-color: rgba(120, 217, 244, 0.42);
    }

    .detail-section-nav span,
    .detail-section-nav b {
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .detail-section-nav span {
        color: #88939e;
        font-size: 0.66em;
        font-weight: 900;
        letter-spacing: 0.07em;
        text-transform: uppercase;
    }

    .detail-section-nav b {
        color: #dce4ea;
        font-size: 0.76em;
        font-weight: 900;
    }

    .detail-footer {
        align-items: center;
        background: rgba(18, 18, 18, 0.94);
        border: 1px solid rgba(255, 255, 255, 0.14);
        box-sizing: border-box;
        display: grid;
        flex: 0 0 auto;
        gap: 0.65em;
        grid-template-columns: minmax(12em, 1fr) minmax(18em, auto);
        padding: 0.65em 0.75em;
    }

    .detail-footer-copy {
        display: flex;
        flex-direction: column;
        gap: 0.2em;
        min-width: 0;
    }

    .detail-footer-copy small {
        color: #aab8c5;
        font-size: 0.78em;
        font-weight: 800;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .detail-actions {
        align-items: center;
        display: flex;
        flex: 0 0 auto;
        flex-wrap: wrap;
        gap: 0.45em;
        justify-content: flex-end;
        min-width: 0;
    }

    .detail-actions button {
        flex: 0 1 auto;
        margin: 0;
        min-height: 2.55em;
        min-width: 6.8em;
        padding: 0.42em 0.7em;
    }

    .detail-actions .install {
        min-width: 11em;
    }

    .detail-link-actions {
        display: flex;
        flex-wrap: wrap;
        gap: 0.45em;
        grid-column: 1 / -1;
        justify-content: flex-end;
        min-width: 0;
    }

    .detail-link-actions button {
        flex: 1 1 6.25em;
        margin: 0;
        min-width: 0;
        padding: 0 0.45em;
    }

    .detail-title {
        display: flex;
        flex-direction: column;
        min-width: 0;
    }

    .detail-header-actions {
        display: flex;
        flex: 0 0 auto;
        gap: 0.5em;
    }

    .detail-header-actions button {
        margin: 0;
    }

    .back-detail {
        min-width: 5.5em;
    }

    .close-detail {
        color: #fd9b9d;
        min-width: 7em;
    }

    .detail-grid {
        display: grid;
        flex: 1 1 auto;
        gap: 1em;
        grid-template-columns: minmax(0, 1.04fr) minmax(420px, 0.9fr);
        min-height: 0;
        overflow: hidden;
    }

    .detail-main,
    .detail-side {
        display: flex;
        flex-direction: column;
        gap: 0.75em;
        min-height: 0;
        min-width: 0;
        overflow-y: auto;
        padding-right: 0.2em;
    }

    .detail-facts {
        order: 1;
    }

    .file-picker {
        order: 2;
    }

    .selected-file-notes {
        order: 3;
    }

    .dependency-box {
        order: 4;
    }

    .install-plan {
        order: 5;
    }

    .nxm-link-box {
        order: 6;
    }

    .detail-conflict-box {
        order: 7;
    }

    .detail-media-frame {
        background: #151515;
        border: 1px solid rgba(255, 255, 255, 0.12);
        box-sizing: border-box;
        flex: 0 0 auto;
        min-height: clamp(156px, 24vh, 288px);
        overflow: hidden;
        position: relative;
        width: 100%;
    }

    .detail-img {
        background: #151515;
        display: block;
        height: clamp(156px, 24vh, 288px);
        object-fit: cover;
        width: 100%;
    }

    .detail-media-nav {
        align-items: center;
        background: rgba(0, 0, 0, 0.58);
        border: 1px solid rgba(255, 255, 255, 0.14);
        bottom: 0.65em;
        box-sizing: border-box;
        display: flex;
        gap: 0.35em;
        left: 0.65em;
        max-width: min(14em, calc(100% - 1.3em));
        padding: 0.28em;
        position: absolute;
    }

    .detail-media-nav button {
        align-items: center;
        background: rgba(255, 255, 255, 0.08);
        border: 1px solid rgba(255, 255, 255, 0.14);
        box-shadow: none;
        color: #eefcff;
        display: flex;
        flex: 0 0 2.15em;
        font-size: 0.76em;
        font-weight: 900;
        height: 2.15em;
        justify-content: center;
        margin: 0;
        min-height: 0;
        min-width: 0;
        padding: 0;
        -webkit-mask-image: none;
        mask-image: none;
    }

    .detail-media-nav span {
        color: #d6dde5;
        flex: 1 1 auto;
        font-size: 0.72em;
        font-weight: 900;
        min-width: 4em;
        text-align: center;
        white-space: nowrap;
    }

    .detail-media-strip {
        display: flex;
        flex: 0 0 auto;
        gap: 0.45em;
        min-height: 0;
        overflow-x: auto;
        overflow-y: hidden;
        padding: 0 0 0.15em;
        scrollbar-width: thin;
    }

    .detail-media-strip button {
        background: rgba(28, 28, 28, 0.88);
        border: 1px solid rgba(255, 255, 255, 0.13);
        box-shadow: none;
        box-sizing: border-box;
        display: block;
        flex: 0 0 clamp(82px, 8vw, 118px);
        height: clamp(44px, 6vh, 66px);
        margin: 0;
        min-height: 0;
        min-width: 0;
        overflow: hidden;
        padding: 0;
        position: relative;
        -webkit-mask-image: none;
        mask-image: none;
    }

    .detail-media-strip button:hover,
    .detail-media-strip .detail-media-thumb-active {
        border-color: rgba(98, 240, 155, 0.72);
    }

    .detail-media-strip img {
        display: block;
        height: 100%;
        object-fit: cover;
        opacity: 0.78;
        width: 100%;
    }

    .detail-media-strip .detail-media-thumb-active img {
        opacity: 1;
    }

    .detail-media-strip span {
        background: rgba(0, 0, 0, 0.62);
        bottom: 0.25em;
        color: #eefcff;
        font-size: 0.68em;
        font-weight: 900;
        line-height: 1;
        padding: 0.18em 0.35em;
        position: absolute;
        right: 0.25em;
    }

    .detail-text,
    .changelog-box,
    .file-picker,
    .selected-file-notes,
    .dependency-box,
    .detail-conflict-box,
    .install-plan,
    .nxm-link-box,
    .detail-facts {
        background: rgba(18, 18, 18, 0.88);
        border: 1px solid rgba(255, 255, 255, 0.12);
        box-sizing: border-box;
        padding: 0.75em;
    }

    .detail-text {
        flex: 1 1 14em;
        max-height: clamp(240px, 38vh, 520px);
        min-height: 0;
        overflow-y: auto;
    }

    .detail-text-head {
        align-items: center;
        display: flex;
        gap: 0.65em;
        justify-content: space-between;
        min-width: 0;
    }

    .detail-text .nexus-rich-text,
    .changelog-row .nexus-rich-text {
        color: #b8c0c8;
        font-size: 0.9em;
        line-height: 1.45;
        margin: 0.55em 0 0;
        overflow-wrap: anywhere;
    }

    .nexus-rich-text::after {
        clear: both;
        content: "";
        display: block;
    }

    .nexus-rich-text :global(p) {
        margin: 0.55em 0 0;
    }

    .nexus-rich-text :global(.nexus-rich-line-block) {
        border-left: 2px solid rgba(120, 217, 244, 0.18);
        color: #c6d0d9;
        line-height: 1.48;
        margin: 0.65em 0 0;
        max-width: 100%;
        overflow-wrap: anywhere;
        padding-left: 0.7em;
    }

    .nexus-rich-text :global(p:first-child),
    .nexus-rich-text :global(ul:first-child),
    .nexus-rich-text :global(ol:first-child),
    .nexus-rich-text :global(blockquote:first-child),
    .nexus-rich-text :global(pre:first-child),
    .nexus-rich-text :global(.nexus-rich-line-block:first-child),
    .nexus-rich-text :global(.nexus-rich-table-wrap:first-child),
    .nexus-rich-text :global(.nexus-rich-spoiler-block:first-child),
    .nexus-rich-text :global(.nexus-rich-box:first-child),
    .nexus-rich-text :global(.nexus-rich-rule:first-child) {
        margin-top: 0;
    }

    .nexus-rich-text :global(ul),
    .nexus-rich-text :global(ol) {
        margin: 0.65em 0 0;
        padding-left: 1.4em;
    }

    .nexus-rich-text :global(ul.nexus-rich-list-circle) {
        list-style-type: circle;
    }

    .nexus-rich-text :global(ul.nexus-rich-list-square) {
        list-style-type: square;
    }

    .nexus-rich-text :global(ul.nexus-rich-list-none),
    .nexus-rich-text :global(ul.nexus-rich-list-dash),
    .nexus-rich-text :global(ul.nexus-rich-list-check) {
        list-style: none;
        padding-left: 0.95em;
    }

    .nexus-rich-text :global(ul.nexus-rich-list-dash li),
    .nexus-rich-text :global(ul.nexus-rich-list-check li) {
        position: relative;
    }

    .nexus-rich-text :global(ul.nexus-rich-list-dash li::before),
    .nexus-rich-text :global(ul.nexus-rich-list-check li::before) {
        color: #78d9f4;
        font-weight: 900;
        left: -0.95em;
        position: absolute;
    }

    .nexus-rich-text :global(ul.nexus-rich-list-dash li::before) {
        content: "-";
    }

    .nexus-rich-text :global(ul.nexus-rich-list-check li::before) {
        content: "+";
    }

    .nexus-rich-text :global(li) {
        margin: 0.18em 0;
    }

    .nexus-rich-text :global(li > p:first-child) {
        display: inline;
        margin-top: 0;
    }

    .nexus-rich-text :global(li > p:first-child + ul),
    .nexus-rich-text :global(li > p:first-child + ol),
    .nexus-rich-text :global(li > p:first-child + .nexus-rich-table-wrap) {
        margin-top: 0.4em;
    }

    .nexus-rich-text :global(strong),
    .nexus-rich-text :global(.nexus-rich-heading) {
        color: #eefcff;
        font-weight: 900;
    }

    .nexus-rich-text :global(.nexus-rich-heading) {
        display: block;
        font-size: 1.05em;
        margin-top: 0.75em;
    }

    .nexus-rich-text :global(.nexus-rich-heading-1) {
        font-size: 1.24em;
    }

    .nexus-rich-text :global(.nexus-rich-heading-2) {
        font-size: 1.14em;
    }

    .nexus-rich-text :global(.nexus-rich-heading-4),
    .nexus-rich-text :global(.nexus-rich-heading-5),
    .nexus-rich-text :global(.nexus-rich-heading-6) {
        font-size: 0.98em;
    }

    .nexus-rich-text :global(a) {
        color: #78d9f4;
        font-weight: 800;
        text-decoration: underline;
        text-decoration-color: rgba(120, 217, 244, 0.45);
        text-underline-offset: 0.18em;
    }

    .nexus-rich-text :global(blockquote) {
        border-left: 3px solid rgba(120, 217, 244, 0.35);
        color: #d6dde5;
        margin: 0.75em 0 0;
        padding: 0.1em 0 0.1em 0.8em;
    }

    .nexus-rich-text :global(blockquote cite) {
        color: #eefcff;
        display: block;
        font-size: 0.82em;
        font-style: normal;
        font-weight: 900;
        margin-bottom: 0.35em;
    }

    .nexus-rich-text :global(.nexus-rich-rule) {
        border: 0;
        border-top: 1px solid rgba(255, 255, 255, 0.16);
        margin: 0.85em 0 0;
    }

    .nexus-rich-text :global(code),
    .nexus-rich-text :global(pre) {
        background: rgba(0, 0, 0, 0.28);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #d6dde5;
        font-family: Consolas, "Courier New", monospace;
    }

    .nexus-rich-text :global(code) {
        padding: 0.08em 0.3em;
    }

    .nexus-rich-text :global(pre) {
        margin: 0.75em 0 0;
        overflow-x: auto;
        padding: 0.65em;
        white-space: pre-wrap;
    }

    .nexus-rich-text :global(.nexus-rich-image) {
        border: 1px solid rgba(255, 255, 255, 0.12);
        box-sizing: border-box;
        display: block;
        margin: 0.75em 0 0;
        max-height: clamp(140px, 28vh, 320px);
        max-width: 100%;
        object-fit: contain;
    }

    .nexus-rich-text :global(.nexus-rich-image-left) {
        float: left;
        margin: 0.35em 0.9em 0.45em 0;
        max-width: min(46%, 18em);
    }

    .nexus-rich-text :global(.nexus-rich-image-right) {
        float: right;
        margin: 0.35em 0 0.45em 0.9em;
        max-width: min(46%, 18em);
    }

    .nexus-rich-text :global(.nexus-rich-image-center) {
        margin-left: auto;
        margin-right: auto;
    }

    .nexus-rich-text :global(.nexus-rich-align-center),
    .nexus-rich-text :global(.nexus-rich-align-left),
    .nexus-rich-text :global(.nexus-rich-align-right),
    .nexus-rich-text :global(.nexus-rich-align-justify) {
        display: block;
    }

    .nexus-rich-text :global(.nexus-rich-align-center) {
        text-align: center;
    }

    .nexus-rich-text :global(.nexus-rich-align-left) {
        text-align: left;
    }

    .nexus-rich-text :global(.nexus-rich-align-right) {
        text-align: right;
    }

    .nexus-rich-text :global(.nexus-rich-align-justify) {
        text-align: justify;
    }

    .nexus-rich-text :global(.nexus-rich-align-center .nexus-rich-image),
    .nexus-rich-text :global(.nexus-rich-align-right .nexus-rich-image) {
        display: inline-block;
    }

    .nexus-rich-text :global(.nexus-rich-indent),
    .nexus-rich-text :global(.nexus-rich-indent-inline) {
        border-left: 2px solid rgba(255, 255, 255, 0.1);
        display: block;
        margin-top: 0.65em;
        padding-left: 0.8em;
    }

    .nexus-rich-text :global(.nexus-rich-indent-2) {
        margin-left: 0.85em;
    }

    .nexus-rich-text :global(.nexus-rich-indent-3) {
        margin-left: 1.45em;
    }

    .nexus-rich-text :global(.nexus-rich-indent-4) {
        margin-left: 2em;
    }

    .nexus-rich-text :global(.nexus-rich-highlight),
    .nexus-rich-text :global(.nexus-rich-font[style*="background-color"]) {
        border-radius: 2px;
        box-decoration-break: clone;
        color: #101820;
        padding: 0.04em 0.2em;
    }

    .nexus-rich-text :global(.nexus-rich-font-mono) {
        font-family: Consolas, "Courier New", monospace;
        white-space: break-spaces;
    }

    .nexus-rich-text :global(.nexus-rich-font-serif) {
        font-family: Georgia, "Times New Roman", serif;
    }

    .nexus-rich-text :global(.nexus-rich-font-sans) {
        font-family: Arial, "Segoe UI", sans-serif;
    }

    .nexus-rich-text :global(.nexus-rich-abbr) {
        border-bottom: 1px dotted rgba(198, 208, 217, 0.55);
        cursor: help;
        text-decoration: none;
    }

    .nexus-rich-text :global(.nexus-rich-cite) {
        color: #9fb1bf;
        font-style: italic;
    }

    .nexus-rich-text :global(.nexus-rich-float) {
        box-sizing: border-box;
        display: block;
        max-width: min(48%, 20em);
    }

    .nexus-rich-text :global(.nexus-rich-float-left) {
        float: left;
        margin: 0.25em 0.9em 0.55em 0;
    }

    .nexus-rich-text :global(.nexus-rich-float-right) {
        float: right;
        margin: 0.25em 0 0.55em 0.9em;
    }

    .nexus-rich-text :global(.nexus-rich-float-center) {
        margin: 0.65em auto 0;
        max-width: 100%;
    }

    .nexus-rich-text :global(.nexus-rich-clear) {
        clear: both;
        display: block;
    }

    .nexus-rich-text :global(.nexus-rich-size-small) {
        font-size: 0.86em;
    }

    .nexus-rich-text :global(.nexus-rich-size-tiny) {
        color: #9aaab6;
        font-size: 0.76em;
    }

    .nexus-rich-text :global(.nexus-rich-size-medium) {
        font-size: 1em;
    }

    .nexus-rich-text :global(.nexus-rich-size-large) {
        color: #eefcff;
        font-size: 1.12em;
        font-weight: 900;
    }

    .nexus-rich-text :global(.nexus-rich-size-xlarge) {
        color: #eefcff;
        display: inline-block;
        font-size: 1.28em;
        font-weight: 900;
        line-height: 1.16;
        margin-top: 0.1em;
    }

    .nexus-rich-text :global(.nexus-rich-caption) {
        color: #8fa3b5;
        display: block;
        font-size: 0.82em;
        font-style: italic;
        margin-top: 0.35em;
        text-align: center;
    }

    .nexus-rich-text :global(.nexus-rich-anchor-ref) {
        color: #9fb1bf;
        font-weight: 800;
    }

    .nexus-rich-text :global(.nexus-rich-spoiler) {
        background: rgba(255, 255, 255, 0.08);
        border: 1px solid rgba(255, 255, 255, 0.12);
        color: #d6dde5;
        padding: 0.05em 0.3em;
    }

    .nexus-rich-text :global(.nexus-rich-spoiler b) {
        color: #eefcff;
        font-weight: 900;
    }

    .nexus-rich-text :global(.nexus-rich-spoiler-block) {
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.12);
        margin: 0.75em 0 0;
        padding: 0.65em 0.75em;
    }

    .nexus-rich-text :global(.nexus-rich-details-block) {
        border-left: 3px solid rgba(120, 217, 244, 0.3);
    }

    .nexus-rich-text :global(.nexus-rich-spoiler-block > span) {
        color: #eefcff;
        display: block;
        font-size: 0.82em;
        font-weight: 900;
        margin-bottom: 0.35em;
        text-transform: uppercase;
    }

    .nexus-rich-text :global(.nexus-rich-columns) {
        clear: both;
        display: grid;
        gap: 0.7em;
        grid-template-columns: repeat(auto-fit, minmax(min(13em, 100%), 1fr));
        margin: 0.8em 0 0;
    }

    .nexus-rich-text :global(.nexus-rich-columns > div) {
        min-width: 0;
    }

    .nexus-rich-text :global(.nexus-rich-columns-inline) {
        display: block;
    }

    .nexus-rich-text :global(.nexus-rich-tabs) {
        display: grid;
        gap: 0.55em;
        margin: 0.8em 0 0;
    }

    .nexus-rich-text :global(.nexus-rich-tabs section) {
        border-left: 2px solid rgba(120, 217, 244, 0.24);
        min-width: 0;
        padding-left: 0.7em;
    }

    .nexus-rich-text :global(.nexus-rich-tabs section > b) {
        color: #eefcff;
        display: block;
        font-size: 0.82em;
        margin-bottom: 0.25em;
        text-transform: uppercase;
    }

    .nexus-rich-text :global(.nexus-rich-callout) {
        background: rgba(120, 217, 244, 0.06);
        border: 1px solid rgba(120, 217, 244, 0.18);
        border-left-width: 3px;
        margin: 0.8em 0 0;
        padding: 0.6em 0.7em;
    }

    .nexus-rich-text :global(.nexus-rich-callout > span),
    .nexus-rich-text :global(.nexus-rich-callout-inline > b) {
        color: #eefcff;
        font-size: 0.82em;
        font-weight: 900;
        text-transform: uppercase;
    }

    .nexus-rich-text :global(.nexus-rich-callout-warning),
    .nexus-rich-text :global(.nexus-rich-callout-important) {
        background: rgba(253, 198, 109, 0.07);
        border-color: rgba(253, 198, 109, 0.28);
    }

    .nexus-rich-text :global(.nexus-rich-callout-tip) {
        background: rgba(98, 240, 155, 0.06);
        border-color: rgba(98, 240, 155, 0.22);
    }

    .nexus-rich-text :global(.nexus-rich-box) {
        background: rgba(15, 18, 20, 0.58);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-left: 3px solid rgba(198, 208, 217, 0.24);
        margin: 0.8em 0 0;
        padding: 0.65em 0.75em;
    }

    .nexus-rich-text :global(.nexus-rich-box > span),
    .nexus-rich-text :global(.nexus-rich-box-inline > b) {
        color: #eefcff;
        display: block;
        font-size: 0.82em;
        font-weight: 900;
        margin-bottom: 0.25em;
        text-transform: uppercase;
    }

    .nexus-rich-text :global(.nexus-rich-box-success) {
        background: rgba(98, 240, 155, 0.06);
        border-color: rgba(98, 240, 155, 0.22);
    }

    .nexus-rich-text :global(.nexus-rich-box-danger),
    .nexus-rich-text :global(.nexus-rich-box-error) {
        background: rgba(253, 126, 126, 0.07);
        border-color: rgba(253, 126, 126, 0.28);
    }

    .nexus-rich-text :global(.nexus-rich-box-notice),
    .nexus-rich-text :global(.nexus-rich-box-panel),
    .nexus-rich-text :global(.nexus-rich-box-fieldset) {
        background: rgba(120, 217, 244, 0.05);
        border-color: rgba(120, 217, 244, 0.18);
    }

    .nexus-rich-text :global(.nexus-rich-box-inline) {
        background: rgba(15, 18, 20, 0.58);
        border: 1px solid rgba(255, 255, 255, 0.13);
        color: #d6dde5;
        display: inline-block;
        margin-top: 0.25em;
        padding: 0.35em 0.5em;
    }

    .nexus-rich-text :global(.nexus-rich-media-link) {
        display: inline-flex;
        margin-top: 0.2em;
    }

    .nexus-rich-text :global(.nexus-rich-media-block) {
        align-items: flex-start;
        background: rgba(120, 217, 244, 0.055);
        border: 1px solid rgba(120, 217, 244, 0.18);
        border-left-width: 3px;
        display: flex;
        flex-wrap: wrap;
        gap: 0.25em 0.65em;
        margin: 0.8em 0 0;
        min-width: 0;
        padding: 0.6em 0.7em;
    }

    .nexus-rich-text :global(.nexus-rich-media-block > span) {
        color: #eefcff;
        flex: 0 0 auto;
        font-size: 0.82em;
        font-weight: 900;
        text-transform: uppercase;
    }

    .nexus-rich-text :global(.nexus-rich-media-block > a) {
        min-width: 0;
        overflow-wrap: anywhere;
    }

    .nexus-rich-text :global(.nexus-rich-table-wrap) {
        margin: 0.8em 0 0;
        max-width: 100%;
        overflow-x: auto;
    }

    .nexus-rich-text :global(.nexus-rich-table) {
        border-collapse: collapse;
        color: #c6d0d9;
        font-size: 0.95em;
        min-width: min(32em, 100%);
        width: 100%;
    }

    .nexus-rich-text :global(.nexus-rich-table th),
    .nexus-rich-text :global(.nexus-rich-table td) {
        border: 1px solid rgba(255, 255, 255, 0.13);
        padding: 0.45em 0.55em;
        text-align: left;
        vertical-align: top;
    }

    .nexus-rich-text :global(.nexus-rich-table th) {
        background: rgba(120, 217, 244, 0.08);
        color: #eefcff;
        font-weight: 900;
    }

    .nexus-rich-text :global(.nexus-rich-table td p),
    .nexus-rich-text :global(.nexus-rich-table th p) {
        margin-top: 0.35em;
    }

    .description-toggle {
        background: rgba(120, 217, 244, 0.08);
        border: 1px solid rgba(120, 217, 244, 0.22);
        box-shadow: none;
        color: #78d9f4;
        font-size: 0.76em;
        font-weight: 900;
        flex: 0 0 auto;
        margin: 0;
        min-height: 2.25em;
        min-width: 7em;
        padding: 0.35em 0.7em;
        -webkit-mask-image: none;
        mask-image: none;
    }

    .description-toggle:hover {
        background: rgba(120, 217, 244, 0.14);
    }

    .detail-section-title {
        color: #eefcff;
        font-size: 0.82em;
        font-weight: 900;
        letter-spacing: 0.09em;
        text-transform: uppercase;
    }

    .detail-facts {
        color: #8d99a5;
        display: grid;
        font-size: 0.78em;
        gap: 0.45em;
        grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .detail-facts b {
        color: #e4e4e4;
    }

    .install-plan {
        flex: 0 0 auto;
    }

    .install-plan-head {
        align-items: center;
        display: flex;
        gap: 0.65em;
        justify-content: space-between;
        min-width: 0;
    }

    .install-plan-head b {
        background: rgba(255, 255, 255, 0.06);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #aab8c5;
        flex: 0 0 auto;
        font-size: 0.72em;
        font-weight: 900;
        padding: 0.2em 0.5em;
        text-transform: uppercase;
    }

    .install-plan-ready .install-plan-head b {
        color: #62f09b;
    }

    .install-plan-review .install-plan-head b {
        color: #fdc66d;
    }

    .install-plan-blocked .install-plan-head b {
        color: #fd9b9d;
    }

    .install-placement-control {
        align-items: center;
        color: #8d99a5;
        display: grid;
        font-size: 0.76em;
        gap: 0.5em;
        grid-template-columns: minmax(5.5em, 0.35fr) minmax(0, 1fr);
        margin-top: 0.55em;
        min-width: 0;
    }

    .install-placement-control span {
        font-weight: 900;
        text-transform: uppercase;
    }

    .install-placement-control select {
        margin: 0;
        min-width: 0;
        width: 100%;
    }

    .install-plan-facts {
        color: #8d99a5;
        display: grid;
        font-size: 0.76em;
        gap: 0.35em 0.75em;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        margin-top: 0.55em;
    }

    .install-plan-facts span,
    .install-plan-facts b {
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .install-plan-facts b {
        color: #d6dde5;
        display: block;
        font-weight: 800;
    }

    .install-plan-file-fact {
        grid-column: 1 / -1;
    }

    .install-plan-file-fact b {
        overflow-wrap: anywhere;
        white-space: normal;
    }

    .install-plan-notes {
        display: flex;
        flex-direction: column;
        gap: 0.18em;
        margin-top: 0.55em;
    }

    .install-plan-notes span {
        color: #9aa5af;
        font-size: 0.76em;
        font-weight: 700;
        min-width: 0;
        overflow-wrap: anywhere;
    }

    .install-plan-ready .install-plan-notes span {
        color: #98d9af;
    }

    .selected-file-notes {
        display: flex;
        flex: 0 0 auto;
        flex-direction: column;
        gap: 0.45em;
        max-height: clamp(170px, 26vh, 320px);
        min-height: 0;
        overflow-y: auto;
    }

    .selected-file-notes-head {
        align-items: center;
        display: flex;
        gap: 0.65em;
        justify-content: space-between;
        min-width: 0;
    }

    .selected-file-notes-head b {
        background: rgba(255, 255, 255, 0.06);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #b9d7df;
        flex: 0 0 auto;
        font-size: 0.72em;
        font-weight: 900;
        max-width: 9em;
        overflow: hidden;
        padding: 0.2em 0.5em;
        text-overflow: ellipsis;
        text-transform: uppercase;
        white-space: nowrap;
    }

    .selected-file-notes-subtitle {
        color: #9aa5af;
        display: block;
        font-size: 0.76em;
        font-weight: 800;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .selected-file-rich-text {
        color: #b8c0c8;
        font-size: 0.82em;
        line-height: 1.38;
        margin: 0;
        overflow-wrap: anywhere;
    }

    .selected-file-changelog {
        border-top: 1px solid rgba(255, 255, 255, 0.1);
        margin-top: 0.15em;
        padding-top: 0.45em;
    }

    .selected-file-changelog > span {
        color: #eefcff;
        display: block;
        font-size: 0.72em;
        font-weight: 900;
        margin-bottom: 0.35em;
        text-transform: uppercase;
    }

    .nxm-link-box {
        display: flex;
        flex: 0 0 auto;
        flex-direction: column;
        gap: 0.45em;
        min-width: 0;
    }

    .nxm-link-row {
        align-items: stretch;
        display: grid;
        gap: 0.5em;
        grid-template-columns: minmax(0, 1fr) minmax(7.5em, auto);
        min-width: 0;
    }

    .nxm-link-row code {
        align-items: center;
        background: rgba(0, 0, 0, 0.28);
        border: 1px solid rgba(255, 255, 255, 0.1);
        box-sizing: border-box;
        color: #b9d7df;
        display: flex;
        font-family: "JetBrains Mono", "Consolas", monospace;
        font-size: 0.72em;
        line-height: 1.25;
        min-height: 2.75em;
        min-width: 0;
        overflow-wrap: anywhere;
        padding: 0.45em 0.65em;
        white-space: normal;
    }

    .nxm-link-row button {
        margin: 0;
        min-width: 0;
        padding: 0 0.7em;
        width: 100%;
    }

    .detail-conflict-box {
        border-color: rgba(253, 198, 109, 0.28);
        flex: 0 0 auto;
        max-height: clamp(135px, 20vh, 230px);
        min-height: 0;
        overflow: hidden;
    }

    .detail-conflict-head {
        align-items: center;
        display: flex;
        gap: 0.65em;
        justify-content: space-between;
        min-width: 0;
    }

    .detail-conflict-head b {
        background: rgba(255, 255, 255, 0.06);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #fdc66d;
        flex: 0 0 auto;
        font-size: 0.72em;
        font-weight: 900;
        padding: 0.2em 0.5em;
        text-transform: uppercase;
    }

    .detail-conflict-note {
        color: #9aa5af;
        display: block;
        font-size: 0.76em;
        font-weight: 700;
        margin-top: 0.45em;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .detail-conflict-list {
        display: flex;
        flex-direction: column;
        gap: 0.45em;
        margin-top: 0.5em;
        max-height: calc(clamp(135px, 20vh, 230px) - 4.5em);
        min-height: 0;
        overflow-y: auto;
        padding-right: 0.2em;
    }

    .detail-conflict-box .conflict-entry {
        align-items: stretch;
        flex-direction: column;
        gap: 0.45em;
    }

    .detail-conflict-box .conflict-entry-actions {
        flex-wrap: wrap;
        width: 100%;
    }

    .detail-conflict-box .conflict-entry-actions span {
        flex: 1 1 4.5em;
        max-width: none;
    }

    .detail-conflict-box .conflict-entry-actions button {
        flex: 1 1 8em;
        min-width: 0;
    }

    .file-picker,
    .dependency-box {
        display: flex;
        flex-direction: column;
        flex: 1 1 15em;
        gap: 0.45em;
        max-height: var(--detail-scroll-section-max);
        min-height: clamp(15em, 28vh, 22em);
        overflow-y: auto;
    }

    .dependency-box-head {
        align-items: center;
        display: flex;
        gap: 0.65em;
        justify-content: space-between;
        min-width: 0;
    }

    .dependency-box-head button {
        flex: 0 0 auto;
        font-size: 0.72em;
        margin: 0;
        min-height: 2.15em;
        min-width: 8.4em;
        padding: 0.35em 0.55em;
    }

    .dependency-box-head button:disabled {
        cursor: default;
        opacity: 0.52;
    }

    .dependency-readiness-grid,
    .file-readiness-grid {
        display: grid;
        gap: 0.45em;
        grid-template-columns: repeat(auto-fit, minmax(8.4em, 1fr));
    }

    .dependency-readiness-chip,
    .file-readiness-chip {
        background: rgba(255, 255, 255, 0.045);
        border: 1px solid rgba(255, 255, 255, 0.11);
        box-sizing: border-box;
        display: grid;
        gap: 0.16em;
        grid-template-columns: auto minmax(0, 1fr);
        min-width: 0;
        padding: 0.45em 0.5em;
    }

    .dependency-readiness-chip small,
    .file-readiness-chip small {
        color: #88939e;
        font-size: 0.68em;
        font-weight: 900;
        grid-column: 1 / -1;
        letter-spacing: 0.07em;
        text-transform: uppercase;
    }

    .dependency-readiness-chip b,
    .file-readiness-chip b {
        color: #dce4ea;
        font-size: 1.05em;
        line-height: 1.05;
    }

    .dependency-readiness-chip span,
    .file-readiness-chip span {
        align-self: center;
        color: #a8b2bc;
        font-size: 0.72em;
        font-weight: 800;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .dependency-readiness-ok,
    .file-readiness-ok {
        border-color: rgba(98, 240, 155, 0.34);
    }

    .dependency-readiness-ok b,
    .dependency-readiness-ok small,
    .file-readiness-ok b,
    .file-readiness-ok small {
        color: #62f09b;
    }

    .dependency-readiness-warn,
    .file-readiness-warn {
        border-color: rgba(253, 198, 109, 0.42);
    }

    .dependency-readiness-warn b,
    .dependency-readiness-warn small,
    .file-readiness-warn b,
    .file-readiness-warn small {
        color: #fdc66d;
    }

    .dependency-readiness-review,
    .file-readiness-review {
        border-color: rgba(120, 217, 244, 0.34);
    }

    .dependency-readiness-review b,
    .dependency-readiness-review small,
    .file-readiness-review b,
    .file-readiness-review small {
        color: #78d9f4;
    }

    .changelog-box {
        flex: 1 1 10em;
        max-height: clamp(180px, 28vh, 340px);
        min-height: 8.5em;
        overflow-y: auto;
    }

    .changelog-row {
        border-bottom: 1px solid rgba(255, 255, 255, 0.08);
        padding: 0.55em 0;
    }

    .changelog-row span {
        color: #78d9f4;
        font-size: 0.82em;
        font-weight: 900;
    }

    .changelog-row .nexus-rich-text {
        color: #b8c0c8;
        font-size: 0.82em;
        line-height: 1.35;
        margin: 0.3em 0 0;
    }

    .file-row {
        background: rgba(44, 44, 44, 0.9);
        border: 1px solid rgba(255, 255, 255, 0.12);
        box-shadow: none;
        display: flex;
        flex-direction: column;
        gap: 0.2em;
        margin: 0.55em 0 0;
        padding: 0.55em 0.7em;
        text-align: left;
        width: 100%;
        -webkit-mask-image: none;
        mask-image: none;
    }

    .file-row-selected {
        border-color: rgba(98, 240, 155, 0.7);
        color: #62f09b;
    }

    .file-row-recommended:not(.file-row-selected) {
        border-color: rgba(98, 240, 155, 0.35);
    }

    .file-row-review:not(.file-row-selected) {
        border-color: rgba(253, 198, 109, 0.35);
    }

    .file-row-head {
        align-items: center;
        display: flex;
        gap: 0.6em;
        justify-content: space-between;
        min-width: 0;
    }

    .file-row-head b {
        background: rgba(255, 255, 255, 0.06);
        border: 1px solid rgba(255, 255, 255, 0.12);
        color: #aab8c5;
        flex: 0 0 auto;
        font-size: 0.68em;
        font-weight: 900;
        padding: 0.2em 0.45em;
        text-transform: uppercase;
    }

    .file-row-recommended .file-row-head b {
        color: #62f09b;
    }

    .file-row-review .file-row-head b {
        color: #fdc66d;
    }

    .file-name {
        color: #eefcff;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        font-weight: 900;
    }

    .file-meta,
    .dependency-empty,
    .dependency-row small {
        color: #9aa5af;
        font-size: 0.78em;
        font-weight: 700;
    }

    .dependency-row {
        border-bottom: 1px solid rgba(255, 255, 255, 0.08);
        display: flex;
        flex-direction: column;
        gap: 0.15em;
        padding: 0.55em 0;
    }

    .dependency-nested-section {
        border-top: 1px solid rgba(255, 255, 255, 0.11);
        display: flex;
        flex-direction: column;
        gap: 0.1em;
        margin-top: 0.25em;
        padding-top: 0.45em;
    }

    .dependency-subtitle {
        color: #d4d9df;
        font-size: 0.75em;
        font-weight: 900;
        text-transform: uppercase;
    }

    .author-requirement-section {
        border-top: 1px solid rgba(120, 217, 244, 0.14);
        display: flex;
        flex-direction: column;
        gap: 0.35em;
        margin-top: 0.3em;
        padding-top: 0.5em;
    }

    .author-requirement-row {
        align-items: center;
        background: rgba(120, 217, 244, 0.05);
        border: 1px solid rgba(120, 217, 244, 0.12);
        display: flex;
        gap: 0.55em;
        justify-content: space-between;
        min-width: 0;
        padding: 0.45em 0.55em;
    }

    .author-requirement-main {
        display: flex;
        flex: 1 1 auto;
        flex-direction: column;
        min-width: 0;
    }

    .author-requirement-main span {
        color: #78d9f4;
        font-size: 0.84em;
        font-weight: 900;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .author-requirement-main small {
        color: #9aa5af;
        font-size: 0.74em;
        font-weight: 700;
    }

    .author-requirement-row button {
        flex: 0 0 auto;
        font-size: 0.72em;
        margin: 0;
        min-height: 2.1em;
        min-width: 5.8em;
        padding: 0.3em 0.5em;
    }

    .dependency-row span {
        color: #fdc66d;
        font-weight: 900;
    }

    .dependency-head {
        align-items: flex-start;
        display: flex;
        gap: 0.55em;
        justify-content: space-between;
        min-width: 0;
    }

    .dependency-head span {
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .dependency-head b {
        background: rgba(255, 255, 255, 0.06);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #aab8c5;
        flex: 0 0 auto;
        font-size: 0.72em;
        font-weight: 900;
        padding: 0.18em 0.45em;
        text-transform: uppercase;
    }

    .dependency-installed .dependency-head span,
    .dependency-installed .dependency-head b {
        color: #62f09b;
    }

    .dependency-missing .dependency-head span,
    .dependency-missing .dependency-head b,
    .dependency-mismatch .dependency-head span,
    .dependency-mismatch .dependency-head b {
        color: #fdc66d;
    }

    .dependency-review .dependency-head span,
    .dependency-review .dependency-head b {
        color: #78d9f4;
    }

    .dependency-actions {
        display: flex;
        flex-wrap: wrap;
        gap: 0.35em;
        margin-top: 0.25em;
    }

    .dependency-actions button {
        font-size: 0.72em;
        margin: 0;
        min-height: 2.15em;
        min-width: 6em;
        padding: 0.35em 0.55em;
    }

    @media (max-width: 1240px) {
        .nexus-filter-row {
            grid-template-columns: minmax(13em, 1fr) repeat(2, minmax(8.5em, 0.55fr));
        }

        .vortex-summary {
            grid-template-columns: repeat(2, minmax(0, 1fr));
        }

        .facts {
            grid-template-columns: repeat(2, minmax(0, 1fr));
        }

        .detail-grid {
            display: flex;
            flex-direction: column;
            overflow-y: auto;
            padding-right: 0.2em;
        }

        .detail-panel {
            --detail-scroll-section-max: none;
            max-height: calc(100vh - 1.5em);
            min-height: 0;
        }

        .detail-main,
        .detail-side {
            flex: 0 0 auto;
            min-height: auto;
            overflow-y: visible;
            padding-right: 0;
        }

        .detail-main {
            display: contents;
        }

        .detail-media-frame {
            order: 1;
        }

        .detail-text {
            order: 2;
        }

        .detail-side {
            order: 3;
        }

        .detail-side {
            display: grid;
            grid-template-columns: repeat(2, minmax(0, 1fr));
            align-items: start;
        }

        .file-picker,
        .selected-file-notes,
        .dependency-box {
            grid-column: 1 / -1;
        }

        .selected-file-notes {
            order: 3;
        }

        .dependency-box {
            order: 4;
        }

        .file-picker {
            order: 2;
        }

        .install-plan {
            order: 5;
        }

        .nxm-link-box {
            order: 6;
        }

        .detail-conflict-box {
            order: 7;
        }

        .file-picker,
        .selected-file-notes,
        .dependency-box,
        .changelog-box {
            max-height: none;
            min-height: 0;
            overflow-y: visible;
        }

        .detail-text {
            flex: 0 0 auto;
            max-height: none;
            overflow-y: visible;
        }

        .detail-text .detail-description-collapsed {
            max-height: clamp(12em, 32vh, 18em);
            overflow: hidden;
        }

        .changelog-box {
            flex: 0 0 auto;
            min-height: 0;
            order: 4;
        }

        .detail-media-frame {
            min-height: clamp(116px, 18vh, 180px);
        }

        .detail-img {
            height: clamp(116px, 18vh, 180px);
        }

        .detail-header {
            align-items: flex-start;
            flex-direction: row;
            gap: 0.65em;
        }

        .detail-header-actions {
            flex-wrap: wrap;
            margin-left: auto;
            width: auto;
        }

        .detail-header-actions button {
            flex: 0 0 auto;
            min-width: 7em;
        }

        .nxm-link-row {
            grid-template-columns: 1fr;
        }

        .detail-footer {
            align-items: stretch;
            grid-template-columns: 1fr;
        }

        .detail-link-actions {
            grid-column: auto;
        }

        .detail-actions,
        .detail-link-actions {
            justify-content: stretch;
        }

        .detail-actions button,
        .detail-link-actions button {
            flex: 1 1 8em;
        }
    }

    @media (max-height: 900px) {
        .nexus-page {
            gap: 0.45em;
        }

        .detail-panel {
            --detail-scroll-section-max: clamp(230px, 34vh, 430px);
            gap: 0.55em;
            padding: 0.85em;
        }

        .detail-section-nav button {
            min-height: 2.35em;
            padding: 0.28em 0.45em;
        }

        .detail-media-frame {
            min-height: clamp(128px, 19vh, 220px);
        }

        .detail-img {
            height: clamp(128px, 19vh, 220px);
        }

        .detail-text {
            max-height: clamp(190px, 30vh, 360px);
        }

        .selected-file-notes {
            max-height: clamp(140px, 22vh, 260px);
        }

        .account-panel {
            padding: 0.48em 0.75em;
        }

        .account-actions {
            gap: 0.38em;
        }

        .account-actions button,
        .auto-endorse-control {
            font-size: 0.7em;
            min-height: 2.25em;
            padding: 0.25em 0.55em;
        }

        .rate-row,
        .vortex-summary,
        .catalog-panel,
        .action-queue,
        .nexus-filter-row {
            gap: 0.38em;
        }

        .vortex-summary {
            grid-template-columns: repeat(7, minmax(0, 1fr));
        }

        .summary-card {
            gap: 0;
            padding: 0.38em 0.5em;
        }

        .summary-note {
            display: none;
        }

        .api-note {
            padding: 0.36em 0.6em;
        }

        .queue-chip {
            min-height: 2.45em;
            padding: 0.32em 0.5em;
        }

        .queue-chip small {
            display: none;
        }

        .nexus-filter-row select,
        .nexus-filter-row .key-input {
            min-height: 2.35em;
            padding-bottom: 0.35em;
            padding-top: 0.35em;
        }

        .nexus-filter-row {
            grid-template-columns: minmax(10em, 1.5fr) repeat(4, minmax(6.5em, 1fr)) minmax(4.8em, 0.55fr);
        }

        .cat-btn,
        .refresh-btn,
        .catalog-mode-buttons button {
            height: 2.3em;
        }

        .nexus-card {
            grid-template-columns: clamp(86px, 10vw, 126px) minmax(0, 1fr);
            min-height: clamp(104px, 13vh, 136px);
        }

        .thumbnail-button {
            display: block;
            min-width: 0;
        }

        .thumbnail-frame,
        .nexus-img {
            min-height: clamp(104px, 13vh, 136px);
        }

        .thumbnail-nav {
            bottom: 0.3em;
            left: 0.3em;
            right: 0.3em;
        }

        .thumbnail-count {
            left: 0.3em;
            top: 0.3em;
        }

        .nexus-body {
            gap: 0.38em;
            padding: 0.55em 0.7em;
        }

        .description-content {
            -webkit-line-clamp: 1;
            line-clamp: 1;
            min-height: auto;
        }

        .facts,
        .inventory-facts {
            grid-template-columns: repeat(5, minmax(0, 1fr));
        }

        .nexus-card-footer {
            align-items: center;
            flex-wrap: wrap;
        }

        .button-row {
            flex-wrap: wrap;
            margin-left: 0;
        }

        .button-row button,
        .button-row .vortex-install-btn {
            flex: 1 1 6.5em;
            min-width: 0;
        }

        .detail-footer {
            gap: 0.45em;
            padding: 0.5em 0.6em;
        }

        .detail-actions button,
        .detail-link-actions button {
            min-height: 2.3em;
        }
    }

    @media (max-width: 860px) {
        .account-panel,
        .connect-row,
        .account-actions,
        .catalog-toolbar,
        .catalog-mode-buttons,
        .nexus-card-footer {
            align-items: stretch;
            flex-direction: column;
        }

        .catalog-toolbar {
            gap: 0.45em;
        }

        .view-buttons {
            display: grid;
            grid-template-columns: repeat(4, minmax(0, 1fr));
            width: 100%;
        }

        .cat-btn,
        .refresh-btn,
        .catalog-mode-buttons button {
            width: 100%;
        }

        .nexus-filter-row,
        .rate-row,
        .vortex-summary,
        .detail-side {
            grid-template-columns: 1fr;
        }

        .detail-header {
            align-items: stretch;
            flex-direction: column;
        }

        .detail-header-actions {
            margin-left: 0;
            width: 100%;
        }

        .detail-header-actions button {
            flex: 1 1 0;
            min-width: 0;
        }

        .nexus-card {
            grid-template-columns: 1fr;
        }

        .nexus-img {
            height: clamp(130px, 22vh, 190px);
        }

        .nexus-rich-text :global(.nexus-rich-image-left),
        .nexus-rich-text :global(.nexus-rich-image-right),
        .nexus-rich-text :global(.nexus-rich-float-left),
        .nexus-rich-text :global(.nexus-rich-float-right) {
            float: none;
            margin: 0.75em 0 0;
            max-width: 100%;
        }

        .source-pill {
            max-width: 100%;
        }

        .button-row {
            margin-left: 0;
            width: 100%;
        }
    }
</style>
