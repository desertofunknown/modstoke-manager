<script lang="ts">
    import { createEventDispatcher, onDestroy, onMount, tick } from "svelte";
    import semver from "semver";
    import SvgSpinnersBlocksWave from "~icons/svg-spinners/blocks-wave";
    import LucideChevronLeft from "~icons/lucide/chevron-left";
    import LucideChevronRight from "~icons/lucide/chevron-right";
    import LucideExternalLink from "~icons/lucide/external-link";
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
        mergeNexusMods,
        NEXUS_CACHE_TTL_MINUTES,
        pickRecommendedNexusFile,
        saveNexusApiKey,
        trackNexusSotfMod,
        untrackNexusSotfMod,
        type NexusCategory,
        type NexusCategorySource,
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
    export let embeddedStorePreview = false;
    export let embeddedControlsExpanded = false;

    type SourceSummaryEvent = {
        visible: number;
        total: number;
        mode: string;
        note: string;
        activeFilters: boolean;
        attention: number;
    };

    const dispatch = createEventDispatcher<{ searchChange: string; summaryChange: SourceSummaryEvent }>();

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
    type DetailSectionTarget = "description" | "files" | "dependencies" | "plan" | "changelog" | "deployment";
    type NexusFileListFilter = "all" | "main" | "review" | "selected";
    type NexusSurfaceTab = "description" | "files" | "posts" | "images" | "bugs";
    type NexusSurfaceLink = {
        key: NexusSurfaceTab;
        label: string;
        status: string;
        title: string;
    };
    type DeploymentChecklistTone = "ready" | "review" | "blocked";
    type UpdateVerdict = {
        label: string;
        tone: UpdateTone;
        isUpdate: boolean;
        needsReview: boolean;
        reason: string;
    };
    type DeploymentChecklistItem = {
        key: string;
        label: string;
        value: string;
        detail: string;
        tone: DeploymentChecklistTone;
        target: DetailSectionTarget;
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
    type NexusLocalCatalogCache = {
        schema: number;
        updatedAt: number;
        mods: NexusMod[];
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
    type AuthorRequirementKind = "link" | "runtime";
    type AuthorRequirementStatus = "detected" | "review" | "warning";
    type AuthorRequirementLink = {
        key: string;
        label: string;
        url?: string;
        source: string;
        kind: AuthorRequirementKind;
        status: AuthorRequirementStatus;
        detail?: string;
        excerpt?: string;
        match?: InstalledInventoryEntry | null;
        mod_id?: number;
    };
    type AuthorRequirementSource = {
        key: string;
        label: string;
        value: string;
    };
    type AuthorInstructionKind = "install" | "configure" | "usage" | "warning" | "requirement";
    type AuthorInstructionHint = {
        key: string;
        source: string;
        title: string;
        detail: string;
        kind: AuthorInstructionKind;
        target: DetailSectionTarget;
    };

    let session: NexusSession = { is_connected: false };
    let apiKey = "";
    let mods: NexusMod[] = [];
    let knownNexusDetails: Record<number, NexusMod> = {};
    let knownNexusPreviewUrls: Record<number, string[]> = {};
    let catalogPreviewCacheVersion = 0;
    let catalogPreviewIndexes: Record<number, number> = {};
    let activeCatalogPreviewLoadId: number | null = null;
    let catalogPreviewLoadNotices: Record<number, string> = {};
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
    let nexusLocalCatalogLoadedAt: number | null = null;
    let nexusLocalCatalogCount = 0;
    let nexusLatestApiRowCount = 0;
    let nexusCatalogUsedLocalFallback = false;
    let nexusCategoryOptions: string[] = [];
    let nexusCategoryFilterOptions: FilterCountOption[] = [];
    let installFilterOptions: FilterCountOption<InstallFilter>[] = [];
    let modTypeFilterOptions: FilterCountOption<ModTypeFilter>[] = [];
    let visibleNexusMods: NexusMod[] = [];
    let visibleInstalledEntries: InstalledInventoryEntry[] = [];
    let nexusEmptyTitle = "";
    let nexusEmptyDetail = "";
    let nexusEmptyChips: string[] = [];
    let updateCount = 0;
    let onlineAttentionCount = 0;
    let installedAttentionCount = 0;
    let endorsementQueueCount = 0;
    let trackedMissingCount = 0;
    let showFullActionQueue = true;
    let resolvedDependencies: ResolvedDependency[] = [];
    let detailDependenciesFirst = false;
    let hasActiveNexusFilters = false;
    let isLoading = false;
    let isDetailLoading = false;
    let status = "";
    let vortexStagingPath: string | null = null;
    let selectedMod: NexusMod | null = null;
    let selectedModDetails: NexusMod | null = null;
    let selectedModFiles: NexusModFile[] = [];
    let selectedFileId: number | null = null;
    let selectedFileListFilter: NexusFileListFilter = "all";
    let recommendedNexusFile: NexusModFile | null = null;
    let selectedFileCanUseRecommended = false;
    let selectedFileRecommendedActionTitle = "";
    let selectedFileFooterReviewVisible = false;
    let selectedFileFooterReviewTitle = "";
    let selectedDependencies: NexusModDependency[] = [];
    let selectedDependencyMessage = "";
    let selectedAuthorRequirements: AuthorRequirementLink[] = [];
    let selectedAuthorInstructions: AuthorInstructionHint[] = [];
    let nexusSurfaceLinks: NexusSurfaceLink[] = [];
    let authorRequirementReviewCount = 0;
    let authorRequirementDetectedCount = 0;
    let authorRequirementWarningCount = 0;
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
    let deploymentChecklistItems: DeploymentChecklistItem[] = [];
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
    let isRefreshingInventory = false;
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
    let nexusCatalogAutoRefreshTimer: number | null = null;
    let nexusCatalogAutoRefreshInFlight = false;
    let nexusUiPreferencesLoaded = false;
    let nexusPageElement: HTMLDivElement | null = null;
    let nexusLayoutObserver: ResizeObserver | null = null;
    let nexusLayoutFrame: number | null = null;
    let nexusWindowResizeHandler: (() => void) | null = null;
    let nexusCatalogTight = false;
    let nexusCatalogVeryTight = false;
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
    const NEXUS_LOCAL_CATALOG_KEY = "openacai-nexus-sotf-catalog-v1";
    const NEXUS_LOCAL_CATALOG_SCHEMA = 1;
    const NEXUS_LOCAL_CATALOG_MAX_MODS = 5000;
    const NEXUS_CATALOG_AUTO_REFRESH_MS = NEXUS_CACHE_TTL_MINUTES * 60 * 1000;
    const NEXUS_AUTO_ENDORSE_KEY = "openacai-nexus-auto-endorse-downloaded";
    const NEXUS_AUTO_ENDORSE_ATTEMPTED_KEY = "openacai-nexus-auto-endorse-attempted";
    const MAX_AUTO_ENDORSE_PER_REFRESH = 3;
    const MAX_NESTED_DEPENDENCY_FILES = 8;
    const MAX_NESTED_DEPENDENCY_DEPTH = 2;
    const MAX_AUTHOR_REQUIREMENT_LINKS = 6;
    const MAX_AUTHOR_REQUIREMENT_HINTS = 8;
    const MAX_AUTHOR_INSTRUCTION_HINTS = 5;
    const NEXUS_MANUAL_REFRESH_COOLDOWN_MS = 60_000;
    const NEXUS_PLACEHOLDER_IMAGE = nexusFallbackImage;
    const NEXUS_DETAIL_PLACEHOLDER_IMAGE = nexusFallbackImage;
    const NEXUS_DESCRIPTION_FALLBACK = "No directions or description are available through the Nexus API for this mod. Open the Nexus page to review author instructions before installing.";
    const NEXUS_RICH_BB_TAGS = new Set(["b", "i", "u", "s", "strike", "strikethrough", "del", "sub", "sup", "small", "big", "mark", "abbr", "acronym", "cite", "q", "url", "img", "image", "thumb", "thumbnail", "color", "colour", "background", "bgcolor", "bgcolour", "backgroundcolour", "bcolor", "highlight", "size", "center", "centre", "left", "right", "justify", "align", "indent", "code", "pre", "tt", "kbd", "samp", "var", "quote", "spoiler", "collapse", "details", "accordion", "accordionitem", "font", "heading", "h", "header", "title", "subtitle", "caption", "h1", "h2", "h3", "h4", "h5", "h6", "float", "clear", "anchor", "bookmark", "target", "goto", "jump", "youtube", "video", "media", "embed", "columns", "cols", "column", "col", "nextcol", "tabs", "tab", "dl", "dt", "dd", "note", "info", "warning", "important", "tip", "box", "panel", "fieldset", "notice", "success", "danger", "error"]);
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
    $: recommendedNexusFile = selectedModFiles.find(file => file.file_id === recommendedNexusFileId) ?? null;
    $: selectedFileCanUseRecommended = Boolean(
        selectedNexusFile
        && recommendedNexusFile
        && selectedNexusFile.file_id !== recommendedNexusFile.file_id
    );
    $: selectedFileRecommendedActionTitle = recommendedNexusFile
        ? `Switch back to the recommended Nexus file: ${recommendedNexusFile.name}.`
        : "Nexus did not return a recommended file for this mod.";
    $: selectedFileFooterReviewVisible = Boolean(selectedNexusFile && isReviewNexusFile(selectedNexusFile));
    $: selectedFileFooterReviewTitle = selectedNexusFile
        ? `The selected file is marked ${fileChoiceCategoryLabel(selectedNexusFile)}. Jump to Files to review the safer recommended choice before Vortex handoff.`
        : "Jump to file choices before Vortex handoff.";
    $: displayedSelectedModFiles = filterDisplayedNexusFiles(
        selectedModFiles,
        recommendedNexusFileId,
        selectedFileId,
        selectedFileListFilter
    );
    $: {
        selectedMod;
        selectedModDetails;
        selectedNexusFile;
        selectedChangelogs;
        currentDetailPreviewUrls = selectedDetailPreviewUrls();
        if (selectedMod) {
            rememberNexusPreviewUrls(selectedMod.mod_id, currentDetailPreviewUrls);
        }
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
        const authorSources = selectedAuthorRequirementSources();
        selectedAuthorRequirements = extractAuthorRequirements(
            authorSources,
            selectedModDetails?.mod_id ?? selectedMod?.mod_id,
            inventory
        );
        selectedAuthorInstructions = extractAuthorInstructions(authorSources);
        nexusSurfaceLinks = buildNexusSurfaceLinks(
            selectedDetailDescription,
            selectedModFiles.length,
            currentDetailPreviewUrls.length
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
    $: showFullActionQueue = !endorsementsLoaded
        || !trackedModsLoaded
        || endorsementQueueCount > 0
        || trackedMissingCount > 0
        || updateCount > 0
        || disabledCount > 0
        || conflictCount > 0
        || isActionQueueFilter(selectedInstallFilter);
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
        || (catalogMode === "online" && selectedNexusCategory !== "all")
        || selectedInstallFilter !== "all"
        || selectedModTypeFilter !== "all";
    $: dispatch("summaryChange", {
        visible: catalogMode === "online" ? visibleNexusMods.length : visibleInstalledEntries.length,
        total: catalogMode === "online" ? mods.length : installedCount,
        mode: catalogMode === "online" ? viewLabel(selectedView) : "Installed",
        note: nexusSummaryNote(),
        activeFilters: (catalogMode === "online" && selectedNexusCategory !== "all")
            || selectedInstallFilter !== "all"
            || selectedModTypeFilter !== "all",
        attention: catalogMode === "online" ? onlineAttentionCount : installedAttentionCount
    });
    $: {
        catalogMode;
        mods;
        inventory;
        nexusSearchTerm;
        selectedNexusCategory;
        selectedInstallFilter;
        selectedModTypeFilter;
        nexusCategoryOptions;
        trackedMods;
        localConflictEntryKeys;
        knownNexusDetails;
        endorsements;
        endorsementsLoaded;
        nexusCategoryFilterOptions = buildNexusCategoryFilterOptions();
        installFilterOptions = buildInstallFilterOptions();
        modTypeFilterOptions = buildModTypeFilterOptions();
    }
    $: {
        catalogMode;
        selectedView;
        nexusSearchTerm;
        selectedNexusCategory;
        selectedInstallFilter;
        selectedModTypeFilter;
        selectedNexusSort;
        selectedInstalledSort;
        mods.length;
        inventory.length;
        visibleNexusMods.length;
        visibleInstalledEntries.length;
        hasActiveNexusFilters;
        const emptyState = buildNexusEmptyState();
        nexusEmptyTitle = emptyState.title;
        nexusEmptyDetail = emptyState.detail;
        nexusEmptyChips = emptyState.chips;
    }
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
    $: authorRequirementReviewCount = selectedAuthorRequirements.filter(requirement => requirement.status === "review").length;
    $: authorRequirementDetectedCount = selectedAuthorRequirements.filter(requirement => requirement.status === "detected").length;
    $: authorRequirementWarningCount = selectedAuthorRequirements.filter(requirement => requirement.status === "warning").length;
    $: totalDependencyIssueCount = dependencyIssueCount + nestedDependencyIssueCount;
    $: totalDependencyReviewCount = dependencyReviewCount + nestedDependencyReviewCount + authorRequirementReviewCount + authorRequirementWarningCount;
    $: detailDependenciesFirst = totalDependencyIssueCount > 0
        || totalDependencyReviewCount > 0
        || resolvedDependencies.length > 0
        || resolvedNestedDependencies.length > 0
        || selectedAuthorRequirements.length > 0
        || isResolvingNestedDependencies;
    $: detailDependencyNavText = describeDetailDependencyNav(
        totalDependencyIssueCount,
        totalDependencyReviewCount,
        resolvedDependencies.length,
        resolvedNestedDependencies.length,
        selectedAuthorRequirements.length,
        authorRequirementWarningCount
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
        selectedDependencyMessage,
        nestedDependencyCheckAvailable(),
        isResolvingNestedDependencies,
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
    $: selectedFileRecommendedCount = selectedModFiles.filter(file => isMainNexusFileChoice(file, recommendedNexusFileId)).length;
    $: fileChoiceReadinessText = describeFileChoiceReadiness(selectedModFiles.length, selectedFileRecommendedCount, selectedFileReviewCount, isDetailLoading);
    $: selectedFileReadinessText = describeSelectedFileReadiness(selectedNexusFile, recommendedNexusFileId, selectedModFiles.length);
    $: fileReviewReadinessText = describeFileReviewReadiness(selectedModFiles.length, selectedFileReviewCount);
    $: {
        selectedNexusFile;
        recommendedNexusFileId;
        selectedInstallFileLabel;
        selectedFileReadinessText;
        selectedInstallPlan;
        selectedInstallPlacement;
        selectedDependencyMessage;
        totalDependencyIssueCount;
        totalDependencyReviewCount;
        resolvedDependencies;
        selectedAuthorRequirements;
        resolvedNestedDependencies;
        nestedDependencySources;
        nestedDependencySummary;
        isResolvingNestedDependencies;
        vortexStagingPath;
        selectedInstallConflict;
        selectedInstallMatch;
        deploymentChecklistItems = buildDeploymentChecklistItems();
    }
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
        setupNexusCatalogAutoRefresh();
        loadNexusUiPreferences();
        if (sharedSearchVersion > 0) {
            applySharedSearch(sharedSearchTerm);
            lastAppliedSharedSearchVersion = sharedSearchVersion;
        }
        loadEndorsementPreferences();
        await refreshInventory();
        await refreshSession();

        if (session.is_connected) {
            applyNexusLocalCatalogCache();
            await loadMods();
        }

        await measureNexusLayoutAfterTick();
    });

    onDestroy(() => {
        cleanupSso();
        cleanupManualRefreshCooldown();
        cleanupNexusCatalogAutoRefresh();
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
        const pageStyle = getComputedStyle(page);
        const bottomGuard = cssPixels(pageStyle.getPropertyValue("--nexus-scroll-bottom-guard"))
            || cssPixels(getComputedStyle(document.documentElement).getPropertyValue("--app-bottom-safe-area"))
            || 42;
        const catalogStyle = getComputedStyle(catalogPanel);
        const catalogGap = cssPixels(catalogStyle.rowGap || catalogStyle.gap);
        const catalogChrome = Array.from(catalogPanel.children)
            .filter((child): child is HTMLElement => child instanceof HTMLElement && child !== scroller && getComputedStyle(child).display !== "none");

        const chromeHeight = catalogChrome.reduce((sum, child) => sum + child.getBoundingClientRect().height, 0)
            + Math.max(0, catalogChrome.length) * catalogGap;
        const availableCatalogHeight = clampNumber(pageRect.bottom - catalogRect.top - bottomGuard, 320, Math.max(320, pageRect.height - bottomGuard));
        const targetScrollerHeight = clampNumber(availableCatalogHeight - chromeHeight, 260, availableCatalogHeight);
        const shouldTightenCatalog = pageRect.height < 920 || targetScrollerHeight < 430;
        const shouldVeryTightenCatalog = pageRect.height < 760 || targetScrollerHeight < 340;
        const desiredVisibleRows = targetScrollerHeight >= 620 ? 3.9 : shouldTightenCatalog ? 2.7 : 3.2;
        const cardMin = shouldTightenCatalog ? 112 : 126;
        const cardMax = shouldTightenCatalog ? 156 : 190;
        const thumbWidthRatio = shouldTightenCatalog ? 0.11 : 0.16;
        const targetCardHeight = clampNumber(targetScrollerHeight / desiredVisibleRows, cardMin, cardMax);
        const targetThumbWidth = clampNumber(pageRect.width * thumbWidthRatio, shouldTightenCatalog ? 96 : 126, shouldTightenCatalog ? 164 : 230);

        page.style.setProperty("--nexus-catalog-target-height", `${Math.round(availableCatalogHeight)}px`);
        page.style.setProperty("--nexus-scroller-target-height", `${Math.round(targetScrollerHeight)}px`);
        page.style.setProperty("--nexus-card-min-height", `${Math.round(targetCardHeight)}px`);
        page.style.setProperty("--nexus-thumb-width", `${Math.round(targetThumbWidth)}px`);
        nexusCatalogTight = shouldTightenCatalog;
        nexusCatalogVeryTight = shouldVeryTightenCatalog;
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

    function readNexusLocalCatalog(): NexusLocalCatalogCache | null {
        try {
            const parsed = JSON.parse(localStorage.getItem(NEXUS_LOCAL_CATALOG_KEY) ?? "null") as Partial<NexusLocalCatalogCache> | null;
            if (!parsed || parsed.schema !== NEXUS_LOCAL_CATALOG_SCHEMA || !Array.isArray(parsed.mods)) {
                return null;
            }

            const cachedMods = mergeNexusMods(parsed.mods
                .map(mod => cacheableNexusCatalogMod(mod))
                .filter(mod => mod.mod_id > 0))
                .slice(0, NEXUS_LOCAL_CATALOG_MAX_MODS);
            return {
                schema: NEXUS_LOCAL_CATALOG_SCHEMA,
                updatedAt: typeof parsed.updatedAt === "number" ? parsed.updatedAt : 0,
                mods: cachedMods
            };
        } catch {
            localStorage.removeItem(NEXUS_LOCAL_CATALOG_KEY);
            return null;
        }
    }

    function applyNexusLocalCatalogCache(): NexusMod[] {
        const cache = readNexusLocalCatalog();
        if (!cache) {
            nexusLocalCatalogLoadedAt = null;
            nexusLocalCatalogCount = 0;
            return [];
        }

        nexusLocalCatalogLoadedAt = cache.updatedAt || null;
        nexusLocalCatalogCount = cache.mods.length;
        if (cache.mods.length > 0) {
            mods = mergeNexusMods([...mods, ...cache.mods]);
            catalogPreviewCacheVersion += 1;
        }

        return cache.mods;
    }

    function persistNexusLocalCatalog(rows: NexusMod[]): NexusMod[] {
        const cachedRows = readNexusLocalCatalog()?.mods ?? [];
        const mergedRows = mergeNexusMods([
            ...rows.map(mod => cacheableNexusCatalogMod(mod)),
            ...cachedRows
        ])
            .filter(mod => mod.mod_id > 0)
            .slice(0, NEXUS_LOCAL_CATALOG_MAX_MODS);
        const cache: NexusLocalCatalogCache = {
            schema: NEXUS_LOCAL_CATALOG_SCHEMA,
            updatedAt: Date.now(),
            mods: mergedRows
        };

        try {
            localStorage.setItem(NEXUS_LOCAL_CATALOG_KEY, JSON.stringify(cache));
        } catch (error) {
            console.log("Failed to persist Nexus catalog cache", error);
        }

        nexusLocalCatalogLoadedAt = cache.updatedAt;
        nexusLocalCatalogCount = mergedRows.length;
        return mergedRows;
    }

    function cacheableNexusCatalogMod(mod: NexusMod): NexusMod {
        const imageUrls = uniquePreviewUrls([
            ...(mod.image_urls ?? []),
            mod.picture_url
        ]).slice(0, 12);

        return {
            mod_id: Number(mod.mod_id) || 0,
            name: mod.name ?? `Nexus Mod #${mod.mod_id}`,
            summary: mod.summary,
            version: mod.version,
            author: mod.author,
            uploaded_by: mod.uploaded_by,
            picture_url: mod.picture_url ?? imageUrls[0],
            image_urls: imageUrls,
            category_id: mod.category_id,
            category_name: mod.category_name,
            category_source: mod.category_source,
            endorsement_count: mod.endorsement_count,
            mod_downloads: mod.mod_downloads,
            mod_unique_downloads: mod.mod_unique_downloads,
            created_timestamp: mod.created_timestamp,
            updated_timestamp: mod.updated_timestamp,
            created_time: mod.created_time,
            updated_time: mod.updated_time,
            loader_type: mod.loader_type
        };
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

    async function refreshLocalInventoryFromUi() {
        if (!$isPathValid) {
            await dialog.message("Choose a valid Sons Of The Forest executable before scanning local mod inventory.", {
                title: "Local inventory",
                kind: "info"
            });
            return;
        }

        isRefreshingInventory = true;
        status = "Refreshing local mod inventory...";

        try {
            await refreshInventory();
            await tick();
            const sourceSummary = [
                `${inventory.length} installed`,
                `${vortexCount} Vortex`,
                `${nativeCount} native`,
                `${manualCount} manual`
            ].join(" · ");
            status = `Local inventory refreshed: ${sourceSummary}.`;
            window.setTimeout(() => {
                if (status.startsWith("Local inventory refreshed:")) {
                    status = "";
                }
            }, 6000);
        } catch (error) {
            status = "";
            await dialog.message(`${error}`, {
                title: "Local inventory",
                kind: "error"
            });
        } finally {
            isRefreshingInventory = false;
            await measureNexusLayoutAfterTick();
        }
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
            applyNexusLocalCatalogCache();
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
            applyNexusLocalCatalogCache();
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

    function setupNexusCatalogAutoRefresh() {
        if (nexusCatalogAutoRefreshTimer !== null) {
            return;
        }

        nexusCatalogAutoRefreshTimer = window.setInterval(() => {
            void maybeAutoRefreshNexusCatalog();
        }, NEXUS_CATALOG_AUTO_REFRESH_MS);
    }

    function cleanupNexusCatalogAutoRefresh() {
        if (nexusCatalogAutoRefreshTimer !== null) {
            window.clearInterval(nexusCatalogAutoRefreshTimer);
            nexusCatalogAutoRefreshTimer = null;
        }

        nexusCatalogAutoRefreshInFlight = false;
    }

    async function maybeAutoRefreshNexusCatalog() {
        if (!session.is_connected || isLoading || nexusCatalogAutoRefreshInFlight) {
            return;
        }

        const cacheAge = nexusLocalCatalogLoadedAt ? Date.now() - nexusLocalCatalogLoadedAt : NEXUS_CATALOG_AUTO_REFRESH_MS;
        if (cacheAge < NEXUS_CATALOG_AUTO_REFRESH_MS) {
            return;
        }

        nexusCatalogAutoRefreshInFlight = true;
        try {
            const response = await fetchNexusSotfMods("all");
            nexusLatestApiRowCount = response.mods.length;
            const rememberedRows = persistNexusLocalCatalog(response.mods);
            if (selectedView === "all") {
                mods = mergeNexusMods([...rememberedRows, ...mods]);
            }
            nexusCategories = response.categories;
            nexusCatalogLoadedAt = Date.now();
            nexusCatalogUsedLocalFallback = false;
            session = {
                ...session,
                rate_limit: response.rate_limit
            };
        } catch (error) {
            console.log("Background Nexus catalog sync failed", error);
            nexusCatalogUsedLocalFallback = (readNexusLocalCatalog()?.mods.length ?? 0) > 0;
        } finally {
            nexusCatalogAutoRefreshInFlight = false;
        }
    }

    async function disconnect() {
        cleanupSso();
        cleanupManualRefreshCooldown();
        nextManualRefreshAt = 0;
        await clearNexusApiKey();
        session = { is_connected: false };
        mods = [];
        knownNexusDetails = {};
        knownNexusPreviewUrls = {};
        catalogPreviewCacheVersion += 1;
        nexusCategories = [];
        nexusCatalogLoadedAt = null;
        nexusLatestApiRowCount = 0;
        nexusCatalogUsedLocalFallback = false;
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

        const isAllCatalog = selectedView === "all";
        const localRows = isAllCatalog ? applyNexusLocalCatalogCache() : [];
        nexusCatalogUsedLocalFallback = false;

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
        }

        isLoading = true;
        status = forceRefresh
            ? "Refreshing Nexus catalog..."
            : localRows.length > 0
                ? "Syncing Nexus catalog..."
                : "Loading Nexus catalog...";

        try {
            await refreshInventory();
            const response = await fetchNexusSotfMods(selectedView, { force: forceRefresh });
            nexusLatestApiRowCount = response.mods.length;
            const rememberedRows = persistNexusLocalCatalog(response.mods);
            mods = isAllCatalog ? rememberedRows : response.mods;
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
            const cachedRows = isAllCatalog ? applyNexusLocalCatalogCache() : [];
            if (cachedRows.length > 0) {
                nexusCatalogUsedLocalFallback = true;
                return;
            }

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

    function reviewLocalConflict(conflict: LocalConflict | null = null) {
        catalogMode = "installed";
        selectedInstallFilter = "conflicts";
        selectedInstalledSort = "attention";
        if (selectedMod) {
            closeModDetails();
        }

        const message = conflict
            ? `Showing conflict review for ${conflict.label}.`
            : "Showing local conflict review.";
        status = message;
        window.setTimeout(() => {
            if (status === message) {
                status = "";
            }
        }, 4500);
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

    function isActionQueueFilter(filter: InstallFilter): boolean {
        return filter === "endorsements"
            || filter === "tracked"
            || filter === "updates"
            || filter === "disabled"
            || filter === "conflicts";
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

    function catalogPreviewNotice(mod: NexusMod): string {
        return catalogPreviewLoadNotices[mod.mod_id] ?? "";
    }

    function setCatalogPreviewNotice(modId: number, notice: string) {
        catalogPreviewLoadNotices = {
            ...catalogPreviewLoadNotices,
            [modId]: notice
        };
    }

    function clearCatalogPreviewNotice(modId: number) {
        const next = { ...catalogPreviewLoadNotices };
        delete next[modId];
        catalogPreviewLoadNotices = next;
    }

    async function loadCatalogPreview(mod: NexusMod, event: Event) {
        event.preventDefault();
        event.stopPropagation();

        if (activeCatalogPreviewLoadId === mod.mod_id) {
            return;
        }

        activeCatalogPreviewLoadId = mod.mod_id;
        setCatalogPreviewNotice(mod.mod_id, "Loading");

        try {
            const details = await fetchNexusModDetails(mod.mod_id);
            const mergedDetails = {
                ...mod,
                ...details,
                mod_id: details.mod_id ?? mod.mod_id
            };
            rememberNexusDetails(mergedDetails);
            const previewUrls = catalogPreviewUrlsFromLoadedDetails(mergedDetails, null, []);

            if (previewUrls.length === 0) {
                setCatalogPreviewNotice(mod.mod_id, "No preview");
                return;
            }

            clearCatalogPreviewNotice(mod.mod_id);
            catalogPreviewIndexes = {
                ...catalogPreviewIndexes,
                [mod.mod_id]: 0
            };
            rememberNexusPreviewUrls(mod.mod_id, previewUrls);
        } catch {
            setCatalogPreviewNotice(mod.mod_id, "Preview failed");
        } finally {
            if (activeCatalogPreviewLoadId === mod.mod_id) {
                activeCatalogPreviewLoadId = null;
            }
        }
    }

    async function handleCatalogThumbnailClick(mod: NexusMod, previewUrls: string[], event: MouseEvent) {
        if (previewUrls.length === 0) {
            await loadCatalogPreview(mod, event);
            return;
        }

        await openModDetails(mod);
    }

    function catalogPreviewUrls(mod: NexusMod, cacheVersion = catalogPreviewCacheVersion): string[] {
        void cacheVersion;
        const detail = knownNexusDetails[mod.mod_id];
        return uniquePreviewUrls([
            ...(mod.image_urls ?? []),
            mod.picture_url,
            ...(detail?.image_urls ?? []),
            detail?.picture_url,
            ...(knownNexusPreviewUrls[mod.mod_id] ?? [])
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

    function selectedAuthorRequirementSources(): AuthorRequirementSource[] {
        const sources = [
            { key: "description", label: "Description", value: selectedDetailDescriptionSource() },
            { key: "selected-file-notes", label: "Selected file notes", value: selectedNexusFile?.description },
            { key: "selected-file-changelog", label: "Selected file changelog", value: selectedNexusFile?.changelog_html },
            ...selectedChangelogs.map((changelog, index) => ({
                key: `mod-changelog-${changelog.version ?? index}`,
                label: changelog.version ? `Mod changelog ${changelog.version}` : `Mod changelog ${index + 1}`,
                value: changelog.changes
            }))
        ];
        const seen = new Set<string>();
        return sources
            .map(source => ({ ...source, value: source.value?.trim() ?? "" }))
            .filter((source): source is AuthorRequirementSource => Boolean(source.value))
            .filter(source => {
                const key = source.value.replace(/\s+/g, " ").slice(0, 300);
                if (seen.has(key)) {
                    return false;
                }

                seen.add(key);
                return true;
            });
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

    function extractAuthorInstructions(sources: AuthorRequirementSource[]): AuthorInstructionHint[] {
        const hints: AuthorInstructionHint[] = [];
        const seen = new Set<string>();

        for (const source of sources) {
            const text = plainText(source.value);
            if (!text) {
                continue;
            }

            const blocks = authorInstructionBlocks(text);
            for (let index = 0; index < blocks.length; index += 1) {
                if (hints.length >= MAX_AUTHOR_INSTRUCTION_HINTS) {
                    return hints;
                }

                const current = blocks[index];
                if (!current) {
                    continue;
                }

                const next = blocks[index + 1] ?? "";
                const kind = authorInstructionKind(current, next);
                if (!kind) {
                    continue;
                }

                if (source.key.includes("changelog") && !authorInstructionHeadingLike(current)) {
                    continue;
                }

                const detail = authorInstructionExcerpt(current, next, kind);
                if (!detail || detail.length < 18) {
                    continue;
                }

                const dedupeKey = detail.toLowerCase().replace(/[^a-z0-9\u4e00-\u9fff]+/g, " ").trim().slice(0, 180);
                if (!dedupeKey || seen.has(dedupeKey)) {
                    continue;
                }

                seen.add(dedupeKey);
                hints.push({
                    key: `${source.key}:${index}:${dedupeKey}`,
                    source: source.label,
                    title: authorInstructionTitle(kind, current),
                    detail,
                    kind,
                    target: authorInstructionTarget(source.key)
                });
            }
        }

        return hints;
    }

    function authorInstructionBlocks(text: string): string[] {
        return text
            .replace(/\n\s*[-*]\s+/g, "\n- ")
            .split(/\n{2,}|\n(?=(?:install(?:ation)?|setup|usage|how to|requirements?|warning|important|配置|安装|使用|说明|要求|注意)[:：]?)/i)
            .map(block => block.replace(/\s+/g, " ").trim())
            .filter(Boolean);
    }

    function authorInstructionKind(block: string, nextBlock: string): AuthorInstructionKind | null {
        const headingLike = authorInstructionHeadingLike(block);
        const value = headingLike ? `${block} ${nextBlock.slice(0, 180)}` : block;

        if (/^(?:install(?:ation|ing)?|setup|how to install|deploy|安装|安装步骤|安装方法|部署)\b|(?:\b(?:extract|copy|place|put|drop|move|drag)\b.{0,80}\b(?:plugins?|mods?|libs?|folder|directory|game folder|\.dll|zip)\b)|(?:\bdownload\b.{0,80}\b(?:bepinex|redloader|zip|archive|\.dll)\b)|(?:下载.{0,80}(?:BepInEx|RedLoader|zip|\.dll|压缩包)|解压|(?:复制|移动).{0,80}(?:文件夹|目录|BepInEx|RedLoader|plugins?|mods?|\.dll)|放入|放到|拖入|找到游戏文件夹)/i.test(value)) {
            return "install";
        }

        if (/^(?:usage|use|how to use|controls?|使用|说明|教程)\b|(?:\b(?:press|open menu|launch|start|toggle|command)\b.{0,80}\b(?:menu|game|mod|feature|tab|key)\b)|(?:按.{0,12}打开|启动|切换|命令)/i.test(value)) {
            return "usage";
        }

        if (/\b(?:warning|important|caution|do not|don't|dont|must not|should not|before installing|before install|remove before|delete before|uninstall|disable)\b|(?:注意|警告|重要|请勿|不要|不可|先删除|先移除|卸载|禁用)/i.test(value)) {
            return "warning";
        }

        if (/^(?:requirements?|dependencies?|prereq(?:uisites?)?|要求|依赖|前置)\b|(?:\b(?:requires?|needed|must have|needs)\b.{0,80}\b(?:runtime|framework|library|bepinex|redloader|sonssdk|harmony|\.net|dotnet)\b)|(?:需要.{0,30}(?:运行库|框架|BepInEx|RedLoader|SonsSdk))/i.test(value)) {
            return "requirement";
        }

        if (/^(?:config(?:ure|uration)?|settings?|options?|配置|设置|选项)\b|(?:\b(?:ini|json|toml|yaml|keybind|hotkey)\b)|(?:快捷键|热键)/i.test(value)) {
            return "configure";
        }

        return null;
    }

    function authorInstructionHeadingLike(block: string): boolean {
        return block.length <= 90
            && /^(?:install(?:ation|ing)?|setup|usage|use|how to|requirements?|dependencies?|config(?:ure|uration)?|settings?|warning|important|note|安装|部署|使用|说明|教程|要求|依赖|前置|配置|设置|注意|警告|重要)[:：]?$/i.test(block.trim());
    }

    function authorInstructionExcerpt(block: string, nextBlock: string, kind: AuthorInstructionKind): string {
        const headingLike = authorInstructionHeadingLike(block);
        const value = headingLike && nextBlock
            ? `${block.replace(/[:：]?\s*$/, "")}: ${nextBlock}`
            : block;
        const context = !headingLike && value.length > 320
            ? authorInstructionContext(value, kind)
            : value;
        return context
            .replace(/\s+-\s+/g, "; ")
            .replace(/\s+/g, " ")
            .trim()
            .slice(0, 260);
    }

    function authorInstructionContext(value: string, kind: AuthorInstructionKind): string {
        const pattern = {
            configure: /\b(?:config(?:ure|uration)?|settings?|options?|ini|json|toml|yaml|keybind|hotkey)\b|(?:配置|设置|选项|快捷键|热键)/i,
            install: /\b(?:install(?:ation|ing)?|setup|deploy|extract|copy|place|put|drop|move|drag)\b.{0,90}\b(?:plugins?|mods?|libs?|folder|directory|game folder|\.dll|zip)\b|(?:安装步骤|安装方法|部署|下载.{0,80}(?:BepInEx|RedLoader|zip|\.dll|压缩包)|解压|(?:复制|移动).{0,80}(?:文件夹|目录|BepInEx|RedLoader|plugins?|mods?|\.dll)|放入|放到|拖入|找到游戏文件夹)/i,
            requirement: /\b(?:requirements?|dependencies?|requires?|needed|must have|runtime|framework|library|bepinex|redloader|sonssdk|harmony|\.net|dotnet)\b|(?:要求|依赖|前置|需要|运行库|框架)/i,
            usage: /\b(?:usage|use|how to|controls?|open menu|press|launch|start|toggle|command)\b|(?:使用|说明|教程|按.{0,12}打开|启动|切换|命令)/i,
            warning: /\b(?:warning|important|caution|do not|don't|dont|must not|before install|uninstall|disable)\b|(?:注意|警告|重要|请勿|不要|不可|卸载|禁用)/i
        }[kind];
        const match = value.match(pattern);
        const index = match?.index ?? -1;
        if (index < 0) {
            return value;
        }

        const sentence = authorInstructionSentenceContext(value, index, match?.[0]?.length ?? 0);
        if (sentence) {
            return sentence;
        }

        const start = Math.max(0, index - 60);
        const end = Math.min(value.length, index + (match?.[0]?.length ?? 0) + 160);
        return `${start > 0 ? "... " : ""}${value.slice(start, end)}${end < value.length ? " ..." : ""}`;
    }

    function authorInstructionSentenceContext(value: string, index: number, matchLength: number): string | null {
        const before = value.slice(0, index);
        const startBoundary = Math.max(
            before.lastIndexOf("。"),
            before.lastIndexOf("."),
            before.lastIndexOf(";"),
            before.lastIndexOf("；"),
            before.lastIndexOf("\n")
        );
        const afterStart = index + matchLength;
        const after = value.slice(afterStart);
        const endMatch = after.match(/[。.!?；;]\s*/);
        const start = Math.max(0, startBoundary + 1);
        const end = endMatch?.index !== undefined
            ? Math.min(value.length, afterStart + endMatch.index + endMatch[0].length)
            : Math.min(value.length, afterStart + 150);
        const sentence = value.slice(start, end).trim();
        return sentence.length >= 16 ? sentence : null;
    }

    function authorInstructionTitle(kind: AuthorInstructionKind, block: string): string {
        const label = block
            .replace(/[:：].*$/, "")
            .replace(/^\W+|\W+$/g, "")
            .trim();
        if (label && label.length <= 42 && authorInstructionKind(label, "")) {
            return label;
        }

        switch (kind) {
            case "configure":
                return "Configuration note";
            case "usage":
                return "Usage note";
            case "warning":
                return "Author warning";
            case "requirement":
                return "Requirement note";
            default:
                return "Install instruction";
        }
    }

    function authorInstructionTarget(sourceKey: string): DetailSectionTarget {
        if (sourceKey.startsWith("selected-file")) {
            return "files";
        }

        if (sourceKey.startsWith("mod-changelog")) {
            return "changelog";
        }

        return "description";
    }

    function extractAuthorRequirements(sources: AuthorRequirementSource[], currentModId: number | undefined, entries: InstalledInventoryEntry[]): AuthorRequirementLink[] {
        const candidates: AuthorRequirementLink[] = [];
        const seen = new Set<string>();
        let linkedCount = 0;

        const addLink = (sourceLabel: string, urlValue: string, labelValue?: string) => {
            const url = safeNexusUrl(urlValue);
            if (!url || seen.has(url) || linkedCount >= MAX_AUTHOR_REQUIREMENT_LINKS) {
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
            linkedCount += 1;
            candidates.push({
                key: `${nexusModId ?? "link"}:${url}`,
                label,
                url,
                source: nexusModId ? `${sourceLabel} Nexus link` : `${sourceLabel} link`,
                kind: "link",
                status: "review",
                detail: `Found in ${sourceLabel}. Open and confirm whether this author-linked target is required for this file.`,
                mod_id: nexusModId ?? undefined
            });
        };

        for (const source of sources) {
            const normalized = normalizeNexusMarkup(source.value);
            if (!normalized) {
                continue;
            }

            const linkedPattern = /\[url=([^\]]+)\]([\s\S]*?)\[\/url\]/gi;
            let linkedMatch: RegExpExecArray | null;
            while ((linkedMatch = linkedPattern.exec(normalized)) !== null) {
                addLink(source.label, linkedMatch[1], plainText(linkedMatch[2]));
            }

            const plainUrlPattern = /\[url\]([\s\S]*?)\[\/url\]|https?:\/\/[^\s<>"'\]]+/gi;
            let plainMatch: RegExpExecArray | null;
            while ((plainMatch = plainUrlPattern.exec(normalized)) !== null) {
                const raw = plainMatch[1] ?? plainMatch[0];
                addLink(source.label, raw);
            }

            for (const hint of inferAuthorRuntimeRequirements(normalized, source.label, entries)) {
                if (candidates.length >= MAX_AUTHOR_REQUIREMENT_HINTS) {
                    break;
                }

                if (seen.has(hint.key)) {
                    continue;
                }

                seen.add(hint.key);
                candidates.push(hint);
            }
        }

        return candidates.slice(0, MAX_AUTHOR_REQUIREMENT_HINTS);
    }

    function inferAuthorRuntimeRequirements(source: string, sourceLocation: string, entries: InstalledInventoryEntry[]): AuthorRequirementLink[] {
        const text = plainText(source);
        if (!text) {
            return [];
        }

        const hints: AuthorRequirementLink[] = [];
        const seen = new Set<string>();
        const addRuntimeHint = (
            key: string,
            label: string,
            kindLabel: string,
            pattern: RegExp,
            matchPredicate: (entry: InstalledInventoryEntry) => boolean,
            reviewDetail: string
        ) => {
            const matched = text.match(pattern);
            if (!matched || seen.has(key)) {
                return;
            }

            seen.add(key);
            const context = authorRequirementMatchContext(text, matched);
            if (!context.warning && !authorRequirementContextLooksLikeActionableDependency(context.excerpt ?? matched[0] ?? "")) {
                return;
            }

            const warningDetail = authorRequirementCompatibilityWarningDetail(label, sourceLocation, context.excerpt);
            const match = entries.find(matchPredicate) ?? null;
            hints.push({
                key: `runtime:${key}`,
                label,
                source: kindLabel,
                kind: "runtime",
                status: warningDetail ? "warning" : match ? "detected" : "review",
                match,
                detail: warningDetail
                    ? `${warningDetail}${match ? ` Local match detected as ${describeInstallSource(match)} in ${match.expectedLocation}.` : ""}`
                    : match
                    ? `Found in ${sourceLocation}. Detected locally as ${describeInstallSource(match)} in ${match.expectedLocation}.`
                    : `Found in ${sourceLocation}. ${reviewDetail}`,
                excerpt: context.excerpt
            });
        };

        addRuntimeHint(
            "bepinex",
            "BepInEx / IL2CPP runtime",
            "Author runtime hint",
            /\b(?:bepinex|bepex|il2cpp|unity\.?il2cpp|bepinex\s*6|bepinex6)\b/i,
            entry => entry.loaderType === "bepinex-plugin" || /bepinex|il2cpp|openacai\s*loader|redloaderbepinexcompat/i.test(authorRequirementEntryText(entry)),
            "Author text mentions BepInEx or IL2CPP. Confirm the BepInEx runtime is installed before Vortex deployment."
        );
        addRuntimeHint(
            "redloader",
            "RedLoader",
            "Author runtime hint",
            /\b(?:red\s*loader|redloader)\b/i,
            entry => entry.loaderType === "redloader-mod" || entry.loaderType === "redloader-library" || /red\s*loader|redloader/i.test(authorRequirementEntryText(entry)),
            "Author text mentions RedLoader. Confirm RedLoader-compatible loader support is installed before Vortex deployment."
        );
        addRuntimeHint(
            "sonssdk",
            "SonsSdk",
            "Author library hint",
            /\b(?:sons\s*sdk|sonssdk|sons\s+sdk)\b/i,
            entry => /sons\s*sdk|sonssdk/i.test(authorRequirementEntryText(entry)),
            "Author text mentions SonsSdk. Confirm the required library or bundled loader support is present."
        );
        addRuntimeHint(
            "harmony",
            "Harmony",
            "Author library hint",
            /\b(?:harmony|0harmony)\b/i,
            entry => /harmony|0harmony/i.test(authorRequirementEntryText(entry)),
            "Author text mentions Harmony. Confirm it is bundled by the selected file or already present locally."
        );
        addRuntimeHint(
            "dotnet",
            ".NET runtime",
            "Author runtime hint",
            /\b(?:\.net|dotnet|net\s*(?:6|7|8|9|10|11)|netstandard)\b/i,
            entry => /\.net|dotnet|net\s*(?:6|7|8|9|10|11)|openacai\s*loader/i.test(authorRequirementEntryText(entry)),
            "Author text mentions a .NET runtime requirement. Confirm the loader/runtime track matches this game install."
        );
        addRuntimeHint(
            "openacai-loader",
            "OpenACAI Endnight Loader",
            "Author loader hint",
            /\b(?:openacai(?:\s+endnight)?\s+loader|openacailoader)\b/i,
            entry => /openacai\s*(?:endnight)?\s*loader|openacailoader/i.test(authorRequirementEntryText(entry)),
            "Author text mentions OpenACAI loader support. Confirm the public loader is installed and healthy on the Main tab."
        );

        return hints;
    }

    function authorRequirementMatchContext(text: string, match: RegExpMatchArray): { excerpt?: string; warning: boolean } {
        const index = match.index ?? -1;
        const phrase = match[0]?.trim();
        if (index < 0 || !phrase) {
            return { excerpt: phrase, warning: false };
        }

        const start = Math.max(0, index - 70);
        const end = Math.min(text.length, index + phrase.length + 70);
        const compact = text.slice(start, end).replace(/\s+/g, " ").trim();
        if (!compact) {
            return { warning: false };
        }

        const excerpt = `${start > 0 ? "... " : ""}${compact}${end < text.length ? " ..." : ""}`.slice(0, 220);
        return {
            excerpt,
            warning: authorRequirementContextLooksLikeWarning(compact)
        };
    }

    function authorRequirementContextLooksLikeWarning(value: string): boolean {
        return /\b(?:cannot|can't|cant|must\s+not|should\s+not|do\s+not|don't|dont|not\s+compatible|incompatible|conflicts?|conflicting|cannot\s+coexist|can't\s+coexist|will\s+not\s+work|won't\s+work|does\s+not\s+work|remove|delete|uninstall|disable|only\s+one|not\s+supported|must\s+first\s+delete|must\s+first\s+remove)\b|(?:不能|不可|不兼容|冲突|删除|移除|卸载|禁用|无法|不会|只允许|必须先删除|必须先移除)/i.test(value);
    }

    function authorRequirementContextLooksLikeActionableDependency(value: string): boolean {
        if (/\b(?:credits?|credit\s+to|thanks?|thank\s+you|contributors?|attribution|acknowledgements?|license|licensed|pardeike|used\s+throughout\s+many\s+unity\s+modding)\b|(?:鸣谢|致谢|贡献者|署名|许可)/i.test(value)) {
            return false;
        }

        return /\b(?:requires?|requirements?|dependenc(?:y|ies)|prereq(?:uisites?)?|needed|needs|must\s+have|install\s+first|depends?\s+on|before\s+install(?:ing)?|download\s+and\s+install|install(?:ed|ing)?\s+(?:bepinex|redloader|sonssdk|harmony|doorstop|\.net|dotnet|runtime|framework|library|loader)|runtime|framework|library|required\s+loader|loader\s+required)\b|(?:要求|依赖|前置|需要|必须|安装|下载|运行库|框架|库文件)/i.test(value);
    }

    function authorRequirementCompatibilityWarningDetail(label: string, sourceLocation: string, excerpt?: string): string | null {
        if (!excerpt || !authorRequirementContextLooksLikeWarning(excerpt)) {
            return null;
        }

        return `Found in ${sourceLocation}. Author text mentions ${label} in an incompatibility, removal, or conflict context; review before deployment.`;
    }

    function authorRequirementEntryText(entry: InstalledInventoryEntry): string {
        return [
            entry.id,
            entry.name,
            entry.author,
            entry.manifestType,
            entry.expectedLocation,
            entry.store,
            entry.vortexPackage,
            entry.packagePath,
            entry.assemblyPath,
            ...entry.matchKeys
        ].filter(Boolean).join(" ");
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

    async function openCatalogPreviewImage(mod: NexusMod, urls: string[], event: MouseEvent) {
        event.stopPropagation();
        const url = urls[catalogPreviewIndex(mod, urls)];
        if (!url) {
            return;
        }

        await shell.open(url);
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

    async function openSelectedDetailPreviewImage(event: MouseEvent) {
        event.stopPropagation();
        const url = currentDetailPreviewUrls[currentDetailPreviewIndex];
        if (!url) {
            return;
        }

        await shell.open(url);
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
        selectedFileListFilter = "all";
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
            const mergedDetails = {
                ...mod,
                ...detailResponse.details,
                mod_id: detailResponse.details.mod_id ?? mod.mod_id
            };
            const recommendedFileVersion = preferredNexusFileVersion(fileResponse.files);
            selectedModDetails = recommendedFileVersion
                ? { ...mergedDetails, version: recommendedFileVersion }
                : mergedDetails;
            if (detailResponse.fetched) {
                rememberNexusDetails(selectedModDetails);
            }
            const recommendedFile = pickRecommendedNexusFile(fileResponse.files) ?? fileResponse.files[0] ?? null;
            const fetchedChangelogs = changelogResponse.changelogs.slice(0, 5);
            selectedModFiles = fileResponse.files;
            selectedChangelogs = fetchedChangelogs;
            selectedFileId = recommendedFile?.file_id ?? null;
            rememberNexusPreviewUrls(
                mod.mod_id,
                catalogPreviewUrlsFromLoadedDetails(selectedModDetails, recommendedFile, fetchedChangelogs)
            );
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

    function scrollDetailSection(target: DetailSectionTarget) {
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

    function describeDetailDependencyNav(issueCount: number, reviewCount: number, apiCount: number, nestedCount: number, authorCount: number, authorWarningCount: number): string {
        if (issueCount > 0) {
            return `${issueCount} issue${issueCount === 1 ? "" : "s"}`;
        }

        if (authorWarningCount > 0) {
            const remainingReviewCount = Math.max(0, reviewCount - authorWarningCount);
            return remainingReviewCount > 0
                ? `${authorWarningCount} warning · ${remainingReviewCount} review`
                : `${authorWarningCount} warning`;
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

        const mergedDetails = {
            ...(knownNexusDetails[details.mod_id] ?? {}),
            ...details
        };
        knownNexusDetails = {
            ...knownNexusDetails,
            [details.mod_id]: mergedDetails
        };

        const rememberedRows = persistNexusLocalCatalog([mergedDetails]);
        if (selectedView === "all") {
            mods = mergeNexusMods([mergedDetails, ...mods, ...rememberedRows]);
        }
    }

    function rememberNexusPreviewUrls(modId: number | undefined, urls: string[]) {
        if (!modId || urls.length === 0) {
            return;
        }

        const merged = uniquePreviewUrls([
            ...(knownNexusPreviewUrls[modId] ?? []),
            ...urls
        ]);
        if (previewUrlListsEqual(knownNexusPreviewUrls[modId] ?? [], merged)) {
            return;
        }

        knownNexusPreviewUrls = {
            ...knownNexusPreviewUrls,
            [modId]: merged
        };
        catalogPreviewCacheVersion += 1;
    }

    function catalogPreviewUrlsFromLoadedDetails(details: NexusMod, file: NexusModFile | null, changelogs: NexusModChangelog[]): string[] {
        const richTextSources = [
            details.description,
            file?.description,
            file?.changelog_html,
            ...changelogs.map(changelog => changelog.changes)
        ];
        return uniquePreviewUrls([
            ...(details.image_urls ?? []),
            details.picture_url,
            ...richTextSources.flatMap(source => nexusRichTextImageUrls(source))
        ]);
    }

    function previewUrlListsEqual(left: string[], right: string[]): boolean {
        return left.length === right.length && left.every((value, index) => value === right[index]);
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
        selectedFileListFilter = "all";
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

    async function selectRecommendedNexusFile() {
        if (!recommendedNexusFile || selectedFileId === recommendedNexusFile.file_id) {
            return;
        }

        await selectNexusFile(recommendedNexusFile.file_id);
        selectedFileListFilter = "selected";
        await tick();
        detailFilesSectionElement?.scrollIntoView({ block: "start", behavior: "smooth" });
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

    async function openSelectedModTab(tab: NexusSurfaceTab) {
        if (!selectedMod) {
            return;
        }

        await shell.open(`${getNexusModPageUrl(selectedMod)}?tab=${tab}`);
    }

    function buildNexusSurfaceLinks(description: string, fileCount: number, previewCount: number): NexusSurfaceLink[] {
        return [
            {
                key: "description",
                label: "Description",
                status: description ? "In manager" : "Nexus",
                title: description
                    ? "Open the Nexus Description tab. A sanitized formatted copy is already shown in this drawer."
                    : "Open the Nexus Description tab."
            },
            {
                key: "files",
                label: "Files",
                status: fileCount > 0 ? `${fileCount} file${fileCount === 1 ? "" : "s"}` : "Nexus",
                title: fileCount > 0
                    ? `Open Nexus Files. The manager is showing ${fileCount} API-returned file choice${fileCount === 1 ? "" : "s"} here.`
                    : "Open Nexus Files. No file choices were returned to the manager yet."
            },
            {
                key: "images",
                label: "Images",
                status: previewCount > 0 ? `${previewCount} preview${previewCount === 1 ? "" : "s"}` : "Nexus",
                title: previewCount > 0
                    ? `Open the Nexus Images tab. The manager currently has ${previewCount} API-returned preview image${previewCount === 1 ? "" : "s"}; the full gallery stays on Nexus.`
                    : "Open the Nexus Images tab. The current API payload did not include preview images for the drawer."
            },
            {
                key: "posts",
                label: "Posts",
                status: "Nexus",
                title: "Open Nexus Posts. The supported API surfaces used by this manager do not expose comments/posts here, so the app links instead of scraping."
            },
            {
                key: "bugs",
                label: "Bugs",
                status: "Nexus",
                title: "Open Nexus Bugs. The supported API surfaces used by this manager do not expose bug threads here, so the app links instead of scraping."
            }
        ];
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
                return authorRequirementSummaryLabel();
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
            switch (installPlanDependencyLookupState(selectedDependencyMessage)) {
                case "checking":
                    return "Checking API rows";
                case "failed":
                    return "Lookup failed";
                case "empty":
                    return "No API rows returned";
                default:
                    return "No API rows listed";
            }
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
            return "No author hints";
        }

        return authorRequirementSummaryLabel();
    }

    function authorRequirementSummaryLabel(): string {
        const parts: string[] = [];
        if (authorRequirementWarningCount > 0) {
            parts.push(`${authorRequirementWarningCount} warning`);
        }

        if (authorRequirementReviewCount > 0) {
            parts.push(`${authorRequirementReviewCount} review`);
        }

        if (authorRequirementDetectedCount > 0) {
            parts.push(`${authorRequirementDetectedCount} detected`);
        }

        return parts.length > 0 ? parts.join(" · ") : "No author hints";
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

    function buildDeploymentChecklistItems(): DeploymentChecklistItem[] {
        const dependencyLookupState = installPlanDependencyLookupState(selectedDependencyMessage);
        const hasDependencyReview = totalDependencyIssueCount > 0
            || totalDependencyReviewCount > 0
            || dependencyLookupState === "checking"
            || dependencyLookupState === "failed"
            || nestedDependencyCheckAvailable();
        const hasPlacementReview = selectedInstallPlacement === "manual-review"
            || selectedInstallPlan.notes.some(note => /placement override|manual placement/i.test(note));
        const hasDeploymentReview = !vortexStagingPath
            || !!selectedInstallConflict
            || !!(selectedInstallMatch && !selectedInstallMatch.enabled);

        return [
            {
                key: "file",
                label: "File",
                value: selectedNexusFile
                    ? isReviewNexusFile(selectedNexusFile) ? "Review" : "Ready"
                    : "Choose",
                detail: selectedNexusFile
                    ? `${selectedInstallFileLabel}. ${selectedFileReadinessText}`
                    : "Choose a Nexus file before handing it to Vortex.",
                tone: selectedNexusFile
                    ? isReviewNexusFile(selectedNexusFile) ? "review" : "ready"
                    : "blocked",
                target: "files"
            },
            {
                key: "dependencies",
                label: "Deps",
                value: deploymentDependencyChecklistValue(),
                detail: `${apiDependencyReadinessLabel()} · ${authorRequirementReadinessLabel()} · ${nestedDependencyReadinessLabel()}`,
                tone: hasDependencyReview ? "review" : "ready",
                target: "dependencies"
            },
            {
                key: "target",
                label: "Target",
                value: selectedInstallPlan.placement,
                detail: `Target ${selectedInstallPlan.target}. Action ${selectedInstallPlan.action}.`,
                tone: selectedInstallPlan.tone === "blocked"
                    ? "blocked"
                    : hasPlacementReview ? "review" : "ready",
                target: "plan"
            },
            {
                key: "deploy",
                label: "Deploy",
                value: selectedInstallConflict
                    ? "Conflict"
                    : vortexStagingPath ? "Vortex" : "Review",
                detail: selectedInstallConflict
                    ? `Local conflict across ${selectedInstallConflict.entries.length} matching installs.`
                    : vortexStagingPath
                        ? `Vortex staging detected at ${vortexStagingPath}.`
                        : "Vortex deployment metadata was not detected in this game folder yet.",
                tone: hasDeploymentReview ? "review" : "ready",
                target: selectedInstallConflict ? "deployment" : "plan"
            }
        ];
    }

    function deploymentDependencyChecklistValue(): string {
        if (totalDependencyIssueCount > 0) {
            return `${totalDependencyIssueCount} issue${totalDependencyIssueCount === 1 ? "" : "s"}`;
        }

        const reviewParts: string[] = [];
        if (authorRequirementWarningCount > 0) {
            reviewParts.push(`${authorRequirementWarningCount} warn`);
        }

        const nonWarningReviewCount = Math.max(0, totalDependencyReviewCount - authorRequirementWarningCount);
        if (nonWarningReviewCount > 0) {
            reviewParts.push(`${nonWarningReviewCount} rev`);
        }

        if (reviewParts.length > 0) {
            return reviewParts.join(" · ");
        }

        if (resolvedDependencies.length > 0 || resolvedNestedDependencies.length > 0 || authorRequirementDetectedCount > 0) {
            return "Ready";
        }

        switch (installPlanDependencyLookupState(selectedDependencyMessage)) {
            case "checking":
                return "Checking";
            case "failed":
                return "Failed";
            default:
                return "None";
        }
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

    function dependencyPrimaryActionLabel(dependency: ResolvedDependency): string {
        switch (dependency.status) {
            case "missing":
                return "Install";
            case "version-mismatch":
                return "Update";
            case "review":
                return "Review";
            default:
                return "Details";
        }
    }

    function dependencyPrimaryActionTitle(dependency: ResolvedDependency): string {
        switch (dependency.status) {
            case "missing":
                return "Open this dependency in the manager so you can choose a file and hand it to Vortex.";
            case "version-mismatch":
                return "Open this dependency in the manager to review the required version.";
            case "review":
                return "Open this dependency in the manager to review version or range details.";
            default:
                return "Open this dependency in the manager.";
        }
    }

    function shouldShowDependencyFooterReviewAction(): boolean {
        return totalDependencyIssueCount > 0
            || totalDependencyReviewCount > 0
            || selectedAuthorRequirements.length > 0
            || nestedDependencyCheckAvailable();
    }

    function dependencyFooterReviewLabel(): string {
        if (totalDependencyIssueCount > 0) {
            return "Review Dependencies";
        }

        if (authorRequirementWarningCount > 0) {
            return "Review Warnings";
        }

        if (totalDependencyReviewCount > 0 || selectedAuthorRequirements.length > 0) {
            return "Review Requirements";
        }

        if (nestedDependencyCheckAvailable()) {
            return "Check Dependencies";
        }

        return "Review Dependencies";
    }

    function dependencyFooterReviewTitle(): string {
        if (totalDependencyIssueCount > 0) {
            return "Jump to missing or version-mismatched dependency rows before Vortex handoff.";
        }

        if (totalDependencyReviewCount > 0 || selectedAuthorRequirements.length > 0) {
            return "Jump to API, nested, or author requirement review details before Vortex handoff.";
        }

        if (nestedDependencyCheckAvailable()) {
            return "Jump to dependency checks; nested dependency lookup is available for this file.";
        }

        return "Jump to dependency details.";
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
        dependencyMessage: string,
        nestedCheckAvailable: boolean,
        isCheckingNestedDependencies: boolean,
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
        const reviewNotes: string[] = [];
        const infoNotes: string[] = [];
        const missingCount = dependencies.filter(dependency => dependency.status === "missing").length;
        const mismatchCount = dependencies.filter(dependency => dependency.status === "version-mismatch").length;
        const reviewCount = dependencies.filter(dependency => dependency.status === "review").length;
        const nestedMissingCount = nestedDependencies.filter(source => source.dependency.status === "missing").length;
        const nestedMismatchCount = nestedDependencies.filter(source => source.dependency.status === "version-mismatch").length;
        const nestedReviewCount = nestedDependencies.filter(source => source.dependency.status === "review").length;
        const dependencyLookupState = installPlanDependencyLookupState(dependencyMessage);
        const dependencyFileIdCount = dependencies.filter(dependency => typeof dependency.file_id === "number").length;

        if (!stagingPath) {
            reviewNotes.push("Vortex deployment metadata was not detected in this game folder yet.");
        }

        if (isReviewNexusFile(file)) {
            reviewNotes.push(`The selected file is marked ${fileChoiceCategoryLabel(file)} and should be reviewed before Vortex handoff.`);
        }

        if (dependencyLookupState === "checking") {
            reviewNotes.push("Dependency lookup is still running for the selected file.");
        } else if (dependencyLookupState === "failed") {
            reviewNotes.push("Dependency lookup failed; review requirements manually before Vortex handoff.");
        } else if (dependencies.length === 0 && dependencyLookupState === "empty") {
            infoNotes.push("Nexus returned no API dependency rows for this file; still review the author's directions.");
        }

        if (missingCount > 0) {
            reviewNotes.push(`${missingCount} dependency ${missingCount === 1 ? "is" : "are"} missing locally.`);
        }

        if (mismatchCount > 0) {
            reviewNotes.push(`${mismatchCount} dependency ${mismatchCount === 1 ? "has" : "have"} a version mismatch.`);
        }

        if (reviewCount > 0) {
            reviewNotes.push(`${reviewCount} installed dependency ${reviewCount === 1 ? "needs" : "need"} a version review.`);
        }

        if (dependencies.length > 0 && missingCount === 0 && mismatchCount === 0 && reviewCount === 0) {
            infoNotes.push(`${dependencies.length} API dependency ${dependencies.length === 1 ? "row looks" : "rows look"} clear locally.`);
        }

        if (isCheckingNestedDependencies) {
            reviewNotes.push("Nested dependency check is still running.");
        } else if (nestedCheckAvailable && nestedDependencies.length === 0) {
            infoNotes.push(`Nested dependency check is available for ${dependencyFileIdCount} returned dependency file ${dependencyFileIdCount === 1 ? "ID" : "IDs"}.`);
        }

        if (nestedMissingCount > 0) {
            reviewNotes.push(`${nestedMissingCount} nested dependency ${nestedMissingCount === 1 ? "is" : "are"} missing locally.`);
        }

        if (nestedMismatchCount > 0) {
            reviewNotes.push(`${nestedMismatchCount} nested dependency ${nestedMismatchCount === 1 ? "has" : "have"} a version mismatch.`);
        }

        if (nestedReviewCount > 0) {
            reviewNotes.push(`${nestedReviewCount} nested installed dependency ${nestedReviewCount === 1 ? "needs" : "need"} a version review.`);
        }

        if (nestedDependencies.length > 0 && nestedMissingCount === 0 && nestedMismatchCount === 0 && nestedReviewCount === 0) {
            infoNotes.push(`${nestedDependencies.length} nested dependency ${nestedDependencies.length === 1 ? "row looks" : "rows look"} clear locally.`);
        }

        const authorWarningCount = authorRequirements.filter(requirement => requirement.status === "warning").length;
        const authorReviewCount = authorRequirements.filter(requirement => requirement.status === "review").length;
        const authorDetectedCount = authorRequirements.filter(requirement => requirement.status === "detected").length;
        if (authorWarningCount > 0) {
            reviewNotes.push(`${authorWarningCount} author compatibility ${authorWarningCount === 1 ? "warning needs" : "warnings need"} review.`);
        }

        if (authorReviewCount > 0) {
            reviewNotes.push(`${authorReviewCount} author requirement ${authorReviewCount === 1 ? "needs" : "need"} manual review.`);
        }

        if (authorDetectedCount > 0) {
            infoNotes.push(`${authorDetectedCount} author runtime/library ${authorDetectedCount === 1 ? "hint was" : "hints were"} detected locally.`);
        }

        if (conflict) {
            reviewNotes.push(`Local conflict detected across ${conflict.entries.length} matching installs.`);
        }

        if (placement === "manual-review") {
            reviewNotes.push("Manual placement review selected; confirm the author's directions before Vortex handoff.");
        }

        if (explicitTarget && match && match.expectedLocation !== explicitTarget) {
            reviewNotes.push(`Placement override differs from the detected local install target ${match.expectedLocation}.`);
        }

        if (match && !match.enabled) {
            reviewNotes.push("The local match is currently disabled.");
        }

        const notes = [...reviewNotes, ...infoNotes];
        return {
            action: installPlanAction(mod, match),
            target,
            placement: installPlacementLabel(placement),
            tone: reviewNotes.length > 0 ? "review" : "ready",
            notes: notes.length > 0 ? notes : ["No local blockers detected from API dependency data."]
        };
    }

    function installPlanDependencyLookupState(message: string): "checking" | "failed" | "empty" | "loaded" | "unknown" {
        const normalized = message.trim().toLowerCase();
        if (!normalized) {
            return "unknown";
        }

        if (normalized.includes("checking")) {
            return "checking";
        }

        if (normalized.includes("failed")) {
            return "failed";
        }

        if (normalized.includes("no api") || normalized.includes("returned no")) {
            return "empty";
        }

        if (normalized.includes("returned")) {
            return "loaded";
        }

        return "unknown";
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

    function filterDisplayedNexusFiles(
        files: NexusModFile[],
        recommendedFileId: number | null,
        selectedId: number | null,
        filter: NexusFileListFilter
    ): NexusModFile[] {
        const sorted = sortNexusFilesForDisplay(files, recommendedFileId);

        switch (filter) {
            case "main":
                return sorted.filter(file => isMainNexusFileChoice(file, recommendedFileId));
            case "review":
                return sorted.filter(isReviewNexusFile);
            case "selected":
                return selectedId === null ? [] : sorted.filter(file => file.file_id === selectedId);
            default:
                return sorted;
        }
    }

    function isMainNexusFileChoice(file: NexusModFile, recommendedFileId: number | null): boolean {
        return !isReviewNexusFile(file) && (file.file_id === recommendedFileId || file.is_primary || normalizedNexusFileCategory(file) === "main");
    }

    function describeFileListFilter(filter: NexusFileListFilter): string {
        switch (filter) {
            case "main":
                return "current main or recommended files";
            case "review":
                return "archived, old, or removed files";
            case "selected":
                return "the selected file";
            default:
                return "files";
        }
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

        if (requirement.match) {
            await openInventoryLocation(requirement.match);
            return;
        }

        if (requirement.url) {
            await openExternalTarget(requirement.url);
        }
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
            .filter((name, index, names) => name && names.findIndex(candidate => nexusCategoryKey(candidate) === nexusCategoryKey(name)) === index);
        const orderedNames = new Set(ordered.map(name => nexusCategoryKey(name)));
        const extra = Array.from(new Set(loadedMods
            .map(mod => mod.category_name)
            .filter((name): name is string => !!name && !orderedNames.has(nexusCategoryKey(name)))))
            .sort((left, right) => left.localeCompare(right));

        return [...ordered, ...extra];
    }

    function buildNexusCategoryFilterOptions(): FilterCountOption[] {
        const baseMods = mods.filter(mod => {
            const match = installedMatch(mod);
            return matchesNexusSearch(mod)
                && nexusModMatchesInstallFilter(mod, match, selectedInstallFilter)
                && nexusModMatchesTypeFilterValue(mod, match, selectedModTypeFilter);
        });

        return [
            {
                count: baseMods.length,
                label: filterCountLabel(catalogMode === "installed" ? "All categories (online)" : "All categories", baseMods.length),
                value: "all"
            },
            ...nexusCategoryOptions.map(category => {
                const count = baseMods.filter(mod => nexusModMatchesCategoryFilter(mod, category)).length;
                return {
                    count,
                    label: filterCountLabel(category, count),
                    value: category
                };
            })
        ];
    }

    function buildInstallFilterOptions(): FilterCountOption<InstallFilter>[] {
        if (catalogMode === "installed") {
            const baseEntries = inventory.filter(entry =>
                matchesInstalledSearch(entry)
                && installedEntryMatchesTypeFilterValue(entry, selectedModTypeFilter)
            );
            return INSTALL_FILTERS.map(filter => {
                const count = baseEntries.filter(entry => installedEntryMatchesInstallFilter(entry, filter)).length;
                return {
                    count,
                    label: filterCountLabel(installFilterLabel(filter), count),
                    value: filter
                };
            });
        }

        const baseMods = mods.filter(mod => {
            const match = installedMatch(mod);
            return matchesNexusSearch(mod)
                && nexusModMatchesCategoryFilter(mod, selectedNexusCategory)
                && nexusModMatchesTypeFilterValue(mod, match, selectedModTypeFilter);
        });

        return INSTALL_FILTERS.map(filter => {
            const count = baseMods.filter(mod => nexusModMatchesInstallFilter(mod, installedMatch(mod), filter)).length;
            return {
                count,
                label: filterCountLabel(installFilterLabel(filter), count),
                value: filter
            };
        });
    }

    function buildModTypeFilterOptions(): FilterCountOption<ModTypeFilter>[] {
        if (catalogMode === "installed") {
            const baseEntries = inventory.filter(entry =>
                matchesInstalledSearch(entry)
                && installedEntryMatchesInstallFilter(entry, selectedInstallFilter)
            );
            return MOD_TYPE_FILTERS.map(filter => {
                const count = baseEntries.filter(entry => installedEntryMatchesTypeFilterValue(entry, filter)).length;
                return {
                    count,
                    label: filterCountLabel(modTypeFilterLabel(filter), count),
                    value: filter
                };
            });
        }

        const baseMods = mods.filter(mod => {
            const match = installedMatch(mod);
            return matchesNexusSearch(mod)
                && nexusModMatchesCategoryFilter(mod, selectedNexusCategory)
                && nexusModMatchesInstallFilter(mod, match, selectedInstallFilter);
        });

        return MOD_TYPE_FILTERS.map(filter => {
            const count = baseMods.filter(mod => nexusModMatchesTypeFilterValue(mod, installedMatch(mod), filter)).length;
            return {
                count,
                label: filterCountLabel(modTypeFilterLabel(filter), count),
                value: filter
            };
        });
    }

    function filterCountLabel(label: string, count: number): string {
        return `${label} (${count})`;
    }

    function installFilterLabel(filter: InstallFilter): string {
        switch (filter) {
            case "attention":
                return "Needs attention";
            case "installed":
                return "Installed";
            case "missing":
                return "Not installed";
            case "updates":
                return "Updates";
            case "disabled":
                return "Disabled";
            case "vortex":
                return "Vortex";
            case "native":
                return "OpenACAI store";
            case "manual":
                return "Manual";
            case "tracked":
                return "Tracked";
            case "endorsements":
                return "Needs endorsement";
            case "conflicts":
                return "Conflicts";
            default:
                return "All installs";
        }
    }

    function modTypeFilterLabel(filter: ModTypeFilter): string {
        switch (filter) {
            case "bepinex-plugin":
                return "BepInEx plugins";
            case "redloader-mod":
                return "RedLoader mods";
            case "redloader-library":
                return "RedLoader libraries";
            case "vortex":
                return "Vortex / Nexus";
            case "native":
                return "OpenACAI native";
            case "manual":
                return "Manual / local";
            default:
                return "All mod types";
        }
    }

    function nexusSortLabel(sort: NexusSortMode): string {
        switch (sort) {
            case "updated":
                return "Updated";
            case "downloads":
                return "Downloads";
            case "endorsements":
                return "Endorsements";
            case "name":
                return "Name";
            case "version":
                return "Version";
            default:
                return "Attention";
        }
    }

    function installedSortLabel(sort: InstalledSortMode): string {
        switch (sort) {
            case "name":
                return "Name";
            case "source":
                return "Source";
            case "location":
                return "Location";
            case "state":
                return "State";
            case "version":
                return "Version";
            default:
                return "Attention";
        }
    }

    function buildNexusEmptyState(): { title: string; detail: string; chips: string[] } {
        const isOnline = catalogMode === "online";
        const loadedCount = isOnline ? mods.length : inventory.length;
        const visibleCount = isOnline ? visibleNexusMods.length : visibleInstalledEntries.length;
        const modeLabel = isOnline ? "Online Nexus" : "Installed inventory";
        const chips = buildNexusEmptyChips();

        if (visibleCount > 0) {
            return { title: "", detail: "", chips };
        }

        if (hasActiveNexusFilters && loadedCount > 0) {
            return {
                title: isOnline ? "No Nexus mods match these filters" : "No installed mods match these filters",
                detail: `${formatNumber(loadedCount)} ${isOnline ? "catalog" : "installed"} ${loadedCount === 1 ? "row is" : "rows are"} loaded, but the active filters hide every result. Clear filters or adjust the chips below.`,
                chips
            };
        }

        if (!isOnline) {
            return {
                title: "No installed mods detected",
                detail: "Refresh inventory after installing with Vortex, the native store, or a manual local package.",
                chips
            };
        }

        return {
            title: "No Nexus catalog rows loaded",
            detail: selectedView === "all"
                ? "The official Nexus catalog sync and local cache do not have visible Sons Of The Forest rows yet. Refresh the catalog after connecting Nexus, or choose a narrower feed."
                : `The ${viewLabel(selectedView)} feed for ${modeLabel} did not return visible rows yet. Refresh the catalog or choose another feed.`,
            chips
        };
    }

    function nexusSummaryNote(): string {
        const attentionCount = catalogMode === "online" ? onlineAttentionCount : installedAttentionCount;
        if (attentionCount > 0) {
            return `${attentionCount} need attention`;
        }

        if (selectedInstallFilter !== "all" || selectedModTypeFilter !== "all") {
            return "Source filters active";
        }

        if (catalogMode === "online" && selectedNexusCategory !== "all") {
            return "Category filter active";
        }

        if (nexusSearchTerm.trim().length > 0) {
            return embeddedStorePreview ? "Shared search" : "Search active";
        }

        if (catalogMode === "installed") {
            return `${vortexCount} Vortex / ${nativeCount} native / ${manualCount} manual`;
        }

        if (trackedModsLoaded && trackedCount > 0) {
            return `${trackedCount} tracked`;
        }

        return catalogMode === "online" ? "Known Nexus catalog" : "Cached Nexus feed";
    }

    function nexusCatalogCacheSummary(): string {
        const parts = [`Official metadata sync cached for ${NEXUS_CACHE_TTL_MINUTES} minutes.`];

        if (nexusLocalCatalogCount > 0) {
            parts.push(`${formatNumber(nexusLocalCatalogCount)} remembered locally.`);
        }

        if (nexusLatestApiRowCount > 0) {
            parts.push(`${formatNumber(nexusLatestApiRowCount)} refreshed from Nexus.`);
        }

        if (nexusCatalogUsedLocalFallback) {
            parts.push("Using local catalog fallback.");
        }

        if (nexusCatalogLoadedAt) {
            parts.push(formatCatalogLoadedAt(nexusCatalogLoadedAt));
        } else if (nexusLocalCatalogLoadedAt) {
            parts.push(`Remembered ${new Date(nexusLocalCatalogLoadedAt).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}.`);
        }

        return parts.join(" ");
    }

    function buildNexusEmptyChips(): string[] {
        const chips: string[] = [
            catalogMode === "online" ? "Mode: Online Nexus" : "Mode: Installed inventory"
        ];

        if (catalogMode === "online") {
            chips.push(`Feed: ${viewLabel(selectedView)}`);
        }

        const search = nexusSearchTerm.trim();
        if (search) {
            chips.push(`Search: ${truncateChipValue(search)}`);
        }

        if (catalogMode === "online" && selectedNexusCategory !== "all") {
            chips.push(`Category: ${selectedNexusCategory}`);
        }

        if (selectedInstallFilter !== "all") {
            chips.push(`Install: ${installFilterLabel(selectedInstallFilter)}`);
        }

        if (selectedModTypeFilter !== "all") {
            chips.push(`Type: ${modTypeFilterLabel(selectedModTypeFilter)}`);
        }

        chips.push(catalogMode === "online"
            ? `Sort: ${nexusSortLabel(selectedNexusSort)}`
            : `Sort: ${installedSortLabel(selectedInstalledSort)}`);

        return chips;
    }

    function truncateChipValue(value: string): string {
        return value.length > 36 ? `${value.slice(0, 33)}...` : value;
    }

    function nexusCategoryKey(category: string | undefined): string {
        return (category ?? "").trim().replace(/\s+/g, " ").toLowerCase();
    }

    function nexusCategorySourceBadge(source: NexusCategorySource | undefined): string | null {
        switch (source) {
            case "local":
                return "taxonomy";
            case "inferred":
                return "inferred";
            default:
                return null;
        }
    }

    function nexusCategorySourceTitle(mod: NexusMod | null | undefined): string {
        const category = mod?.category_name ?? "Nexus";
        switch (mod?.category_source) {
            case "api":
                return `${category} category from Nexus game metadata.`;
            case "local":
                return `${category} category from the local Sons Of The Forest taxonomy cache.`;
            case "inferred":
                return `${category} category inferred from API-provided mod text because the feed omitted category data.`;
            default:
                return `${category} category.`;
        }
    }

    function matchesNexusSearch(mod: NexusMod): boolean {
        const search = nexusSearchTerm.trim().toLowerCase();

        return !search || [
            mod.name,
            mod.summary ?? "",
            mod.author ?? "",
            mod.uploaded_by ?? ""
        ].some(value => value.toLowerCase().includes(search));
    }

    function matchesInstalledSearch(entry: InstalledInventoryEntry): boolean {
        const search = nexusSearchTerm.trim().toLowerCase();

        return !search || [
            entry.name,
            entry.author ?? "",
            entry.version ?? "",
            entry.vortexPackage ?? "",
            entry.expectedLocation,
            entry.loaderType,
            loaderTypeLabel(entry),
            describeInstallSource(entry)
        ].some(value => value.toLowerCase().includes(search));
    }

    function matchesNexusFilters(mod: NexusMod): boolean {
        const match = installedMatch(mod);
        return matchesNexusSearch(mod)
            && nexusModMatchesCategoryFilter(mod, selectedNexusCategory)
            && nexusModMatchesTypeFilterValue(mod, match, selectedModTypeFilter)
            && nexusModMatchesInstallFilter(mod, match, selectedInstallFilter);
    }

    function nexusModMatchesCategoryFilter(mod: NexusMod, category: string): boolean {
        return category === "all" || nexusCategoryKey(mod.category_name) === nexusCategoryKey(category);
    }

    function nexusModMatchesInstallFilter(mod: NexusMod, match: InstalledInventoryEntry | null, filter: InstallFilter): boolean {
        if (filter === "installed" && !match) {
            return false;
        }

        if (filter === "missing" && match) {
            return false;
        }

        if (filter === "updates" && !hasNexusUpdate(mod)) {
            return false;
        }

        if (filter === "disabled" && (!match || match.enabled)) {
            return false;
        }

        if (filter === "vortex" && match?.installSource !== "vortex") {
            return false;
        }

        if (filter === "native" && match?.installSource !== "native") {
            return false;
        }

        if (filter === "manual" && match?.installSource !== "manual") {
            return false;
        }

        if (filter === "tracked" && !isNexusModTracked(mod.mod_id)) {
            return false;
        }

        if (filter === "endorsements" && !nexusModNeedsEndorsement(mod)) {
            return false;
        }

        if (filter === "conflicts" && !isConflictedEntry(match)) {
            return false;
        }

        if (filter === "attention" && !nexusNeedsAttention(mod)) {
            return false;
        }

        return true;
    }

    function matchesInstalledFilters(entry: InstalledInventoryEntry): boolean {
        return matchesInstalledSearch(entry)
            && installedEntryMatchesTypeFilterValue(entry, selectedModTypeFilter)
            && installedEntryMatchesInstallFilter(entry, selectedInstallFilter);
    }

    function installedEntryMatchesInstallFilter(entry: InstalledInventoryEntry, filter: InstallFilter): boolean {
        if (filter === "vortex" && entry.installSource !== "vortex") {
            return false;
        }

        if (filter === "native" && entry.installSource !== "native") {
            return false;
        }

        if (filter === "manual" && entry.installSource !== "manual") {
            return false;
        }

        if (filter === "updates" && !hasInventoryUpdate(entry)) {
            return false;
        }

        if (filter === "disabled" && entry.enabled) {
            return false;
        }

        if (filter === "missing") {
            return false;
        }

        if (filter === "tracked" && !isNexusModTracked(entry.nexusModId)) {
            return false;
        }

        if (filter === "endorsements" && !inventoryEntryNeedsEndorsement(entry)) {
            return false;
        }

        if (filter === "conflicts" && !isConflictedEntry(entry)) {
            return false;
        }

        if (filter === "attention" && !inventoryNeedsAttention(entry)) {
            return false;
        }

        return true;
    }

    function nexusModMatchesTypeFilterValue(mod: NexusMod, match: InstalledInventoryEntry | null, filter: ModTypeFilter): boolean {
        if (filter === "all") {
            return true;
        }

        if (filter === "vortex" || filter === "native" || filter === "manual") {
            return match?.installSource === filter;
        }

        if (match) {
            return match.loaderType === filter;
        }

        const text = [
            mod.loader_type,
            mod.category_name,
            mod.name,
            mod.summary,
            mod.description
        ].filter(Boolean).join(" ").toLowerCase();

        switch (filter) {
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

    function installedEntryMatchesTypeFilterValue(entry: InstalledInventoryEntry, filter: ModTypeFilter): boolean {
        switch (filter) {
            case "all":
                return true;
            case "vortex":
            case "native":
            case "manual":
                return entry.installSource === filter;
            default:
                return entry.loaderType === filter;
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
            return "Premium status returned by Nexus.";
        }

        if (session.user?.is_supporter) {
            return "Supporter status detected.";
        }

        if (session.user?.membership_tier) {
            return "Nexus membership tier returned by the API.";
        }

        return "Standard Nexus account.";
    }

    function nexusMembershipBenefitNote(): string {
        if (session.user?.is_premium) {
            return "Premium detected · Nexus/Vortex handles downloads";
        }

        if (session.user?.is_supporter) {
            return "Supporter detected · downloads stay on Nexus/Vortex";
        }

        if (session.user?.membership_tier) {
            return "Tier returned · account actions stay user-controlled";
        }

        return "Local token · cached requests";
    }

    function nexusMembershipBenefitTitle(): string {
        const parts = [
            nexusMembershipNote(),
            "Download entitlement, speed, and queue behavior remain handled by Nexus and Vortex.",
            "The manager does not scrape, rehost, or bypass Nexus-hosted files.",
            "Tokens stay local and requests are cached."
        ];

        return parts.join(" ");
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
        | { kind: "line"; value: string; mono?: boolean }
        | { kind: "list"; value: string; ordered: boolean; style?: string; start?: number }
        | { kind: "table"; value: string }
        | { kind: "definition"; value: string };
    type FilterCountOption<T extends string = string> = {
        count: number;
        label: string;
        value: T;
    };
    type NexusPlainListLine = {
        explicit: boolean;
        indent: number;
        ordered: boolean;
        raw: string;
        start?: number;
        style?: string;
        value: string;
    };
    type NexusTableCell = {
        value: string;
        header: boolean;
        colspan?: number;
        rowspan?: number;
        width?: string;
        align?: "left" | "center" | "right" | "justify";
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
            .replace(/\[\/?(?:columns|cols|column|col|tabs|tab|note|info|warning|important|tip|box|panel|fieldset|notice|success|danger|error|collapse|details|accordion|accordionitem|hidden|spoilerblock|caption|dl|dt|dd)[^\]]*\]/gi, "\n")
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
            .replace(/<details\b[^>]*>\s*<summary\b[^>]*>([\s\S]*?)<\/summary>([\s\S]*?)<\/details>/gi, (_match, label: string, body: string) => {
                const safeLabel = safeNexusBbLabel(plainText(label));
                return `\n\n[details${safeLabel ? `=${safeLabel}` : ""}]${body}[/details]\n\n`;
            })
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
            .replace(/<dl\b[^>]*>/gi, "\n\n[dl]\n")
            .replace(/<\/dl>/gi, "\n[/dl]\n\n")
            .replace(/<dt\b[^>]*>/gi, "\n[dt]")
            .replace(/<\/dt>/gi, "[/dt]\n")
            .replace(/<dd\b[^>]*>/gi, "[dd]")
            .replace(/<\/dd>/gi, "[/dd]\n")
            .replace(/<table\b[^>]*>/gi, "\n\n[table]\n")
            .replace(/<\/table>/gi, "\n[/table]\n\n")
            .replace(/<\/?(?:tbody|thead|tfoot)\b[^>]*>/gi, "")
            .replace(/<tr\b[^>]*>/gi, "\n[tr]")
            .replace(/<\/tr>/gi, "[/tr]\n")
            .replace(/<th\b([^>]*)>/gi, (_match, attrs: string) => `[th${nexusHtmlTableCellAttributes(attrs)}]`)
            .replace(/<\/th>/gi, "[/th]")
            .replace(/<td\b([^>]*)>/gi, (_match, attrs: string) => `[td${nexusHtmlTableCellAttributes(attrs)}]`)
            .replace(/<\/td>/gi, "[/td]")
            .replace(/<ul\b([^>]*)>/gi, (_match, attrs: string) => nexusHtmlListOpeningTag(attrs, false))
            .replace(/<\/ul>/gi, "\n[/list]\n")
            .replace(/<ol\b([^>]*)>/gi, (_match, attrs: string) => nexusHtmlListOpeningTag(attrs, true))
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
            .replace(/\[(?:br|break|nl|newline)\s*\/?\]/gi, "\n")
            .replace(/\[(?:pre|raw|noparse|codebox|plaintext|plain|fixed)(?:=[^\]]+|[ \t][^\]]*)?\]/gi, "[code]")
            .replace(/\[\/(?:pre|raw|noparse|codebox|plaintext|plain|fixed)\]/gi, "[/code]")
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
            .replace(/\[(?:centre)([^\]]*)\]/gi, (_match, attrs: string) => `[center${attrs ?? ""}]`)
            .replace(/\[\/(?:centre)\]/gi, "[/center]")
            .replace(/\[(?:colour)([^\]]*)\]/gi, (_match, attrs: string) => `[color${attrs ?? ""}]`)
            .replace(/\[\/(?:colour)\]/gi, "[/color]")
            .replace(/\[(?:bgcolour|backgroundcolour|bcolor)([^\]]*)\]/gi, (_match, attrs: string) => `[background${attrs ?? ""}]`)
            .replace(/\[\/(?:bgcolour|backgroundcolour|bcolor)\]/gi, "[/background]")
            .replace(/\[(?:strikethrough|strikeout)\]/gi, "[s]")
            .replace(/\[\/(?:strikethrough|strikeout)\]/gi, "[/s]")
            .replace(/\[dl[^\]]*\]/gi, "\n\n[dl]\n")
            .replace(/\[\/dl\]/gi, "\n[/dl]\n\n")
            .replace(/\[dt[^\]]*\]/gi, "\n[dt]")
            .replace(/\[\/dt\]/gi, "[/dt]\n")
            .replace(/\[dd[^\]]*\]/gi, "[dd]")
            .replace(/\[\/dd\]/gi, "[/dd]\n")
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
            .replace(/\[(?:hidden|spoilerblock)([^\]]*)\]/gi, (_match, attrs: string) => `\n\n[spoiler${attrs ?? ""}]\n`)
            .replace(/\[\/(?:hidden|spoilerblock)\]/gi, "\n[/spoiler]\n\n")
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
            .replace(/\[(?:hr|line|rule|divider|separator|hrule|horizontalrule)([^\]]*)\]/gi, (_match, attrs: string) => nexusRuleMarkup(attrs))
            .replace(/\[\/(?:hr|line|rule|divider|separator|hrule|horizontalrule)\]/gi, "\n")
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

    function nexusRuleMarkup(rawAttrs?: string): string {
        const label = safeNexusBbLabel(
            nexusBbAttribute(rawAttrs, "title")
            ?? nexusBbAttribute(rawAttrs, "label")
            ?? nexusBbAttribute(rawAttrs, "name")
            ?? nexusBbTagAttribute("hr", rawAttrs)
            ?? nexusBbFirstAttributeValue(rawAttrs)
        );
        return label ? `\n\n[hr]\n[heading=4]${label}[/heading]\n\n` : "\n\n[hr]\n\n";
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
        const structuralPattern = /\[(table|list|olist|dl|quote|spoiler|collapse|details|accordion|accordionitem|indent|center|left|right|align|justify|code|heading|float|youtube|video|media|embed|columns|cols|tabs|note|info|warning|important|tip|box|panel|fieldset|notice|success|danger|error)([^\]]*)\]/gi;
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
                    const ordered = tag === "olist" || isOrderedNexusListAttr(attr);
                    appendNexusTextChunks(chunks, before);
                    chunks.push({
                        kind: "list",
                        value: remainder,
                        ordered,
                        style: nexusListMarkerStyle(attr, ordered) ?? undefined,
                        start: ordered ? nexusOrderedListStart(rawAttrs, attr) : undefined
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
            } else if (tag === "dl") {
                chunks.push({ kind: "definition", value });
            } else if (tag === "list" || tag === "olist") {
                const ordered = tag === "olist" || isOrderedNexusListAttr(attr);
                chunks.push({
                    kind: "list",
                    value,
                    ordered,
                    style: nexusListMarkerStyle(attr, ordered) ?? undefined,
                    start: ordered ? nexusOrderedListStart(rawAttrs, attr) : undefined
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
                chunks.push(listChunk ?? nexusTextBlockChunk(trimmed));
            }
        }
    }

    function nexusTextBlockChunk(value: string): NexusBlockChunk {
        if (!shouldPreserveNexusLineLayout(value)) {
            return { kind: "block", value };
        }

        return {
            kind: "line",
            value,
            mono: shouldUseNexusMonospaceLineLayout(value)
        };
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

        const parsedLines = lines.map(line => nexusPlainListLine(line));
        if (parsedLines.every(Boolean)) {
            const first = parsedLines[0];
            if (!first) {
                return null;
            }

            return {
                kind: "list",
                ordered: first.ordered,
                start: first.start,
                style: first.style,
                value: nexusPlainListLinesToBbcode(parsedLines as NexusPlainListLine[])
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

            chunks.push(nexusTextBlockChunk(text));
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
                start: first.start,
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
            const baseIndent = Math.min(...listLines.map(line => line.indent));
            if (parsed.indent > baseIndent || (first.ordered === parsed.ordered && first.style === parsed.style)) {
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
                indent: nexusPlainListIndentLevel(line),
                ordered: false,
                raw: line,
                value: explicit[1].trim()
            };
        }

        const bullet = trimmed.match(/^(?:[-*+]|\u2022)\s+(.+)$/);
        if (bullet?.[1]?.trim()) {
            return {
                explicit: false,
                indent: nexusPlainListIndentLevel(line),
                ordered: false,
                raw: line,
                value: bullet[1].trim()
            };
        }

        const numbered = trimmed.match(/^(\d+)[.)]\s+(.+)$/);
        if (numbered?.[2]?.trim()) {
            const start = Number.parseInt(numbered[1], 10);
            return {
                explicit: false,
                indent: nexusPlainListIndentLevel(line),
                ordered: true,
                raw: line,
                start: Number.isFinite(start) && start > 1 ? Math.min(999, start) : undefined,
                style: "1",
                value: numbered[2].trim()
            };
        }

        const alpha = trimmed.match(/^([a-z])[.)]\s+(.+)$/i);
        if (alpha?.[2]?.trim()) {
            return {
                explicit: false,
                indent: nexusPlainListIndentLevel(line),
                ordered: true,
                raw: line,
                style: alpha[1] === alpha[1].toUpperCase() ? "A" : "a",
                value: alpha[2].trim()
            };
        }

        return null;
    }

    function nexusPlainListIndentLevel(line: string): number {
        const leading = line.match(/^[\t ]*/)?.[0] ?? "";
        const width = leading.replace(/\t/g, "    ").length;
        return Math.max(0, Math.min(4, Math.floor(width / 2)));
    }

    function nexusPlainListLinesToBbcode(lines: NexusPlainListLine[]): string {
        if (lines.length === 0) {
            return "";
        }

        const minIndent = Math.min(...lines.map(line => line.indent));
        const parts: string[] = [];
        const nestedLists: Array<{ ordered: boolean; style?: string }> = [];
        let previousLevel = 0;

        for (const [index, line] of lines.entries()) {
            let level = Math.max(0, line.indent - minIndent);
            if (index === 0) {
                level = 0;
            }

            level = Math.min(3, Math.min(level, previousLevel + 1));

            while (nestedLists.length > level) {
                parts.push("[/list]");
                nestedLists.pop();
            }

            while (nestedLists.length < level) {
                parts.push(nexusPlainListOpeningTag(line));
                nestedLists.push({ ordered: line.ordered, style: line.style });
            }

            if (level > 0) {
                const active = nestedLists[level - 1];
                if (active && (active.ordered !== line.ordered || active.style !== line.style)) {
                    parts.push("[/list]");
                    nestedLists.pop();
                    parts.push(nexusPlainListOpeningTag(line));
                    nestedLists.push({ ordered: line.ordered, style: line.style });
                }
            }

            parts.push(`[*]${line.value}`);
            previousLevel = level;
        }

        while (nestedLists.length > 0) {
            parts.push("[/list]");
            nestedLists.pop();
        }

        return parts.join("\n");
    }

    function nexusPlainListOpeningTag(line: NexusPlainListLine): string {
        if (line.ordered) {
            if (line.start) {
                return `[olist${line.style ? ` type="${line.style}"` : ""} start="${line.start}"]`;
            }

            return `[olist${line.style ? `=${line.style}` : ""}]`;
        }

        return `[list${line.style ? `=${line.style}` : ""}]`;
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

    function shouldUseNexusMonospaceLineLayout(value: string): boolean {
        const lines = value.split("\n").filter(line => line.trim());
        if (lines.length < 2) {
            return false;
        }

        const tabbedLines = lines.filter(line => /\S\t+\S/.test(line)).length;
        const spacedColumnLines = lines.filter(line => /\S[ \t]{2,}\S/.test(line)).length;
        const fileOrPathLines = lines.filter(line => /(?:[A-Za-z]:\\|\.{0,2}\/|\\)[^\s]+|(?:\.dll|\.json|\.cfg|\.ini|\.zip|\.rar|\.7z)\b/i.test(line)).length;
        const keyValueGridLines = lines.filter(line => /^[A-Za-z0-9][A-Za-z0-9 /&+_.()'-]{1,32}:\s{2,}\S/.test(line.trim())).length;

        return tabbedLines > 0
            || spacedColumnLines >= 2
            || fileOrPathLines >= 2
            || keyValueGridLines >= 2;
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
            const startAttribute = chunk.ordered && chunk.start ? ` start="${chunk.start}"` : "";
            const classAttribute = !chunk.ordered && chunk.style ? ` class="nexus-rich-list-${escapeAttribute(chunk.style)}"` : "";
            return `<${tag}${typeAttribute}${startAttribute}${classAttribute}>${listItems.map(item => `<li>${renderNexusListItem(item, depth)}</li>`).join("")}</${tag}>`;
        }

        if (chunk.kind === "table") {
            return renderNexusTable(block, depth);
        }

        if (chunk.kind === "definition") {
            return renderNexusDefinitionList(block, depth);
        }

        if (chunk.kind === "line") {
            const classes = ["nexus-rich-line-block", chunk.mono ? "nexus-rich-line-block-mono" : ""]
                .filter(Boolean)
                .join(" ");
            return `<div class="${classes}">${renderNexusInline(block)}</div>`;
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
            return renderNexusDisclosure(label, collapsible[3], depth + 1, "details");
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

        const code = block.match(/^\[code(?:=([^\]]+)|[ \t]([^\]]+))?\]([\s\S]*?)\[\/code\]$/i);
        if (code) {
            const label = safeNexusLabel(code[1] ?? code[2]);
            return `<pre${label ? ` data-nexus-label="${escapeAttribute(label)}"` : ""}><code>${escapeHtml(code[3])}</code></pre>`;
        }

        const directAligned = block.match(/^\[(center|centre|left|right)(?:=[^\]]+|[ \t][^\]]*)?\]([\s\S]*?)\[\/\1\]$/i);
        const attributedAligned = directAligned ? null : block.match(/^\[align(?:=([^\]]+)|[ \t]([^\]]+))?\]([\s\S]*?)\[\/align\]$/i);
        if (directAligned || attributedAligned) {
            const tagAlignment = directAligned?.[1]?.toLowerCase() === "centre" ? "center" : directAligned?.[1];
            const alignment = safeNexusAlignment(attributedAligned?.[1] ?? attributedAligned?.[2] ?? tagAlignment);
            const body = directAligned?.[2] ?? attributedAligned?.[3] ?? "";
            return `<div class="nexus-rich-align-${alignment}">${renderNexusBlocks(body, depth + 1)}</div>`;
        }

        const quote = block.match(/^\[quote(?:=([^\]]+))?\]([\s\S]*?)\[\/quote\]$/i);
        if (quote) {
            const cite = safeNexusLabel(quote[1]);
            return `<blockquote>${cite ? `<cite>${escapeHtml(cite)}</cite>` : ""}${renderNexusBlocks(quote[2], depth + 1)}</blockquote>`;
        }

        const spoiler = block.match(/^\[spoiler(?:=([^\]]+))?\]([\s\S]*?)\[\/spoiler\]$/i);
        if (spoiler) {
            const label = safeNexusLabel(spoiler[1]) || "Spoiler";
            return renderNexusDisclosure(label, spoiler[2], depth + 1, "spoiler");
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

    function renderNexusDisclosure(label: string, body: string, depth: number, kind: "details" | "spoiler"): string {
        const bodyHtml = renderNexusBlocks(body, depth);
        return `<details class="nexus-rich-spoiler-block nexus-rich-disclosure nexus-rich-disclosure-${kind}"><summary>${escapeHtml(label)}</summary><div>${bodyHtml}</div></details>`;
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

    function renderNexusDefinitionList(value: string, depth: number): string {
        const entries = nexusDefinitionListEntries(value);
        if (entries.length === 0) {
            return renderNexusBlocks(value, depth + 1);
        }

        return `<dl class="nexus-rich-definition-list">${entries.map(entry => {
            const details = entry.details.length > 0 ? entry.details : [""];
            return `<div><dt>${renderNexusInline(entry.term)}</dt>${details.map(detail => `<dd>${detail ? renderNexusBlocks(detail, depth + 1) : ""}</dd>`).join("")}</div>`;
        }).join("")}</dl>`;
    }

    function nexusDefinitionListEntries(value: string): Array<{ term: string; details: string[] }> {
        const entries: Array<{ term: string; details: string[] }> = [];
        const entryPattern = /\[dt(?:[^\]]*)\]([\s\S]*?)(?:\[\/dt\])?([\s\S]*?)(?=\[dt(?:[^\]]*)\]|$)/gi;
        let entryMatch: RegExpExecArray | null;

        while ((entryMatch = entryPattern.exec(value)) !== null) {
            const term = entryMatch[1].trim();
            const body = entryMatch[2] ?? "";
            if (!term) {
                continue;
            }

            const details: string[] = [];
            const detailPattern = /\[dd(?:[^\]]*)\]([\s\S]*?)(?:\[\/dd\]|(?=\[dd(?:[^\]]*)\]|\[dt(?:[^\]]*)\]|$))/gi;
            let detailMatch: RegExpExecArray | null;
            while ((detailMatch = detailPattern.exec(body)) !== null) {
                const detail = detailMatch[1].trim();
                if (detail) {
                    details.push(detail);
                }
            }

            if (details.length === 0) {
                const fallback = body
                    .replace(/\[\/?dd(?:[^\]]*)\]/gi, "")
                    .trim();
                if (fallback) {
                    details.push(fallback);
                }
            }

            entries.push({ term, details });
        }

        return entries;
    }

    function renderNexusMediaBlock(tag: string, rawAttrs: string, body: string): string {
        const media = nexusMediaRenderData(tag, rawAttrs, nexusBbTagAttribute(tag, rawAttrs), collectNexusNodeText(parseNexusRichNodes(body)));
        if (!media) {
            return "";
        }

        return `<div class="nexus-rich-media-block"><span>${escapeHtml(media.label)}</span><a href="${escapeAttribute(media.url)}" target="_blank" rel="noreferrer noopener">${escapeHtml(media.urlLabel)}</a></div>`;
    }

    function hasNexusBlockStructure(value: string): boolean {
        return /\[(?:table|list|olist|dl|quote|spoiler|collapse|details|accordion|accordionitem|indent|center|left|right|align|justify|code|heading|float|youtube|video|media|embed|columns|cols|tabs|note|info|warning|important|tip|box|panel|fieldset|notice|success|danger|error)(?:=[^\]]+|[ \t][^\]]*)?\]/i.test(value)
            || looksLikeNexusLooseTable(value);
    }

    function looksLikeNexusLooseTable(value: string): boolean {
        return (/\[tr(?:[^\]]*)\][\s\S]*?(?:\[\/tr\]|\[tr(?:[^\]]*)\]|\[\/table\]|$)/i.test(value)
            && /\[(?:td|th)(?:[^\]]*)\][\s\S]*?(?:\[\/(?:td|th)\]|\[(?:td|th)(?:[^\]]*)\]|\[\/tr\]|$)/i.test(value)
        ) || looksLikeNexusCellOnlyTable(value);
    }

    function looksLikeNexusCellOnlyTable(value: string): boolean {
        if (/\[tr(?:[^\]]*)\]/i.test(value)) {
            return false;
        }

        const cellMatches = value.match(/\[(?:td|th)(?:[^\]]*)\]/gi) ?? [];
        return cellMatches.length >= 2
            && /\[\/(?:td|th)\]/i.test(value);
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
                const style = [
                    cell.width ? `width: ${cell.width}` : "",
                    cell.align ? `text-align: ${cell.align}` : ""
                ].filter(Boolean).join("; ");
                return `<${tag}${spanAttributes}${style ? ` style="${escapeAttribute(style)}"` : ""}>${renderNexusBlocks(cell.value, depth + 1)}</${tag}>`;
            }).join("");
            return `<tr>${cells}</tr>`;
        }).join("")}</tbody></table></div>`;
    }

    function nexusTableRows(block: string): Array<{ cells: NexusTableCell[] }> {
        const rows: Array<{ cells: NexusTableCell[] }> = [];
        const rowPattern = /\[tr\]([\s\S]*?)(?:\[\/tr\]|(?=\[tr\]|\[\/table\]|$))/gi;
        let rowMatch: RegExpExecArray | null;

        while ((rowMatch = rowPattern.exec(block)) !== null) {
            const cells = nexusTableCells(rowMatch[1]);
            if (cells.length > 0) {
                rows.push({ cells });
            }
        }

        if (rows.length === 0) {
            return nexusCellOnlyTableRows(block);
        }

        return rows;
    }

    function nexusCellOnlyTableRows(block: string): Array<{ cells: NexusTableCell[] }> {
        if (!looksLikeNexusCellOnlyTable(block)) {
            return [];
        }

        const groups = block
            .split(/\n{2,}/)
            .map(group => group.trim())
            .filter(Boolean);
        const groupedRows = groups
            .map(group => ({ cells: nexusTableCells(group) }))
            .filter(row => row.cells.length > 0);

        if (groupedRows.length > 1 && groupedRows.some(row => row.cells.length > 1)) {
            return groupedRows;
        }

        const cells = nexusTableCells(block);
        return cells.length > 0 ? [{ cells }] : [];
    }

    function nexusTableCells(value: string): NexusTableCell[] {
        const cells: NexusTableCell[] = [];
        const cellPattern = /\[(td|th)([^\]]*)\]([\s\S]*?)(?:\[\/\1\]|(?=\[(?:td|th)(?:[^\]]*)\]|\[\/tr\]|$))/gi;
        let cellMatch: RegExpExecArray | null;

        while ((cellMatch = cellPattern.exec(value)) !== null) {
            const attrs = cellMatch[2] ?? "";
            cells.push({
                value: cellMatch[3].trim(),
                header: cellMatch[1].toLowerCase() === "th",
                colspan: nexusTableCellColspan(attrs),
                rowspan: safeNexusTableSpan(nexusBbAttribute(attrs, "rowspan") ?? nexusBbAttribute(attrs, "row")),
                width: nexusTableCellWidth(attrs),
                align: nexusTableCellAlignment(attrs)
            });
        }

        return cells;
    }

    function nexusTableCellColspan(attrs: string): number | undefined {
        const explicit = nexusBbAttribute(attrs, "colspan") ?? nexusBbAttribute(attrs, "col");
        if (explicit) {
            return safeNexusTableSpan(explicit);
        }

        const direct = nexusBbFirstAttributeValue(attrs);
        if (!direct || !/^\d{1,2}$/.test(direct.trim())) {
            return undefined;
        }

        return safeNexusTableSpan(direct);
    }

    function nexusTableCellWidth(attrs: string): string | undefined {
        const direct = nexusBbFirstAttributeValue(attrs);
        const candidate = nexusBbAttribute(attrs, "width")
            ?? nexusBbAttribute(attrs, "w")
            ?? (direct && (!/^\d{1,2}$/.test(direct.trim()) || Number.parseInt(direct, 10) > 6) ? direct : undefined);
        return safeNexusCssLength(candidate) ?? undefined;
    }

    function nexusTableCellAlignment(attrs: string): "left" | "center" | "right" | "justify" | undefined {
        const candidate = nexusBbAttribute(attrs, "align")
            ?? nexusBbAttribute(attrs, "text-align")
            ?? nexusBbAttribute(attrs, "halign");
        if (!candidate) {
            return undefined;
        }

        return safeNexusAlignment(candidate);
    }

    function nexusListItems(block: string): string[] {
        const cleaned = block
            .replace(/^\[(?:list|olist)(?:=[^\]]+|[ \t][^\]]*)?\]\s*/i, "")
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
        const tokenPattern = /\[\*(?:\s*=[^\]]+)?\]|\[\/?(?:list|olist|table)(?:=[^\]]+|[ \t][^\]]*)?\]/gi;
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
            .replace(/\[\/?(?:ul|ol|olist|list)(?:=[^\]]+|[ \t][^\]]*)?\]/gi, "\n")
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
            const isSelfClosing = /\/\s*$/.test(rawAttrs) || isNexusVoidRichTag(name, rawAttrs);

            if (!NEXUS_RICH_BB_TAGS.has(name)) {
                stack[stack.length - 1].children.push({ kind: "text", value: full });
            } else if (isClosing) {
                if (stack.length > 1 && stack[stack.length - 1].name === name) {
                    stack.pop();
                } else {
                    const openTagIndex = lastNexusOpenTagIndex(stack, name);
                    if (openTagIndex > 0) {
                        stack.length = openTagIndex;
                    } else if (isNexusLooseClosingTag(name)) {
                        // Standalone Nexus image aliases often leave a harmless closing token behind.
                    }
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

    function lastNexusOpenTagIndex(stack: Array<{ name: string; children: NexusRichNode[] }>, name: string): number {
        for (let index = stack.length - 1; index > 0; index -= 1) {
            if (stack[index].name === name) {
                return index;
            }
        }

        return -1;
    }

    function isNexusVoidRichTag(name: string, rawAttrs?: string): boolean {
        if (name === "clear" || name === "nextcol") {
            return true;
        }

        if ((name === "anchor" || name === "bookmark" || name === "target") && nexusBbTagAttribute(name, rawAttrs)) {
            return true;
        }

        if (name !== "img" && name !== "image" && name !== "thumb" && name !== "thumbnail") {
            return false;
        }

        const direct = nexusBbDirectAttribute(rawAttrs);
        return Boolean(
            safeNexusUrl(nexusBbAttribute(rawAttrs, "src"))
            ?? safeNexusUrl(nexusBbAttribute(rawAttrs, "url"))
            ?? safeNexusUrl(direct)
        );
    }

    function isNexusLooseClosingTag(name: string): boolean {
        return name === "img"
            || name === "image"
            || name === "thumb"
            || name === "thumbnail"
            || name === "clear"
            || name === "nextcol"
            || name === "anchor"
            || name === "bookmark"
            || name === "target";
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
            case "strikethrough":
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
                const title = safeNexusImageAlt(nexusBbAttribute(node.rawAttrs, "title") ?? nexusBbAttribute(node.rawAttrs, "alt"));
                return url ? `<img class="${imageClass}" src="${escapeAttribute(url)}" alt="${escapeAttribute(alt)}"${title ? ` title="${escapeAttribute(title)}"` : ""}${style} loading="lazy" />${attrUrl && inner ? inner : ""}` : inner;
            }
            case "color": {
                const color = safeNexusColor(node.attr);
                return color ? `<span style="color: ${escapeAttribute(color)}">${inner}</span>` : inner;
            }
            case "colour": {
                const color = safeNexusColor(node.attr);
                return color ? `<span style="color: ${escapeAttribute(color)}">${inner}</span>` : inner;
            }
            case "background":
            case "bgcolor":
            case "bgcolour":
            case "backgroundcolour":
            case "bcolor":
            case "highlight": {
                const color = safeNexusColor(node.attr) ?? (node.name === "highlight" ? "yellow" : null);
                return color ? `<span class="nexus-rich-highlight" style="background-color: ${escapeAttribute(color)}">${inner}</span>` : inner;
            }
            case "size":
                return `<span class="${nexusSizeClass(node.attr)}">${inner}</span>`;
            case "center":
            case "centre":
            case "left":
            case "right":
                return `<span class="nexus-rich-align-${node.name === "centre" ? "center" : node.name}">${inner}</span>`;
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
            case "dl":
            case "dt":
            case "dd":
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
            case "quote": {
                const cite = safeNexusLabel(node.attr);
                return `<blockquote>${cite ? `<cite>${escapeHtml(cite)}</cite>` : ""}${inner}</blockquote>`;
            }
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
            case "bookmark":
            case "target": {
                const label = safeNexusLabel(node.attr);
                return `<span class="nexus-rich-anchor"${label ? ` title="${escapeAttribute(label)}"` : ""}>${inner}</span>`;
            }
            case "goto":
            case "jump":
                return `<span class="nexus-rich-anchor-ref">${inner || escapeHtml(safeNexusLabel(node.attr) || "Jump")}</span>`;
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
                const size = nexusBbAttribute(node.rawAttrs, "size")
                    ?? nexusBbAttribute(node.rawAttrs, "font-size");
                const fontClass = safeNexusFontClass(
                    nexusBbAttribute(node.rawAttrs, "face")
                    ?? nexusBbAttribute(node.rawAttrs, "font")
                    ?? nexusBbAttribute(node.rawAttrs, "font-family")
                    ?? node.attr
                );
                const classes = ["nexus-rich-font", fontClass ? `nexus-rich-font-${fontClass}` : "", size ? nexusSizeClass(size) : ""]
                    .filter(Boolean)
                    .join(" ");
                const style = [
                    color ? `color: ${color}` : "",
                    background ? `background-color: ${background}` : ""
                ].filter(Boolean).join("; ");
                return style || fontClass || size ? `<span class="${classes}"${style ? ` style="${escapeAttribute(style)}"` : ""}>${inner}</span>` : inner;
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
            return normalizedNexusOrderedListOpeningTag(attr, rawAttrs);
        }

        if (name === "ul") {
            const style = nexusListMarkerStyle(attr, false);
            return `\n[list${style ? `=${style}` : ""}]\n`;
        }

        if (name === "list") {
            if (isOrderedNexusListAttr(attr)) {
                return normalizedNexusOrderedListOpeningTag(attr, rawAttrs);
            }

            const style = nexusListMarkerStyle(attr, false);
            return `\n[list${style ? `=${style}` : ""}]\n`;
        }

        return attr ? `[${name}=${attr}]` : `[${name}]`;
    }

    function normalizedNexusOrderedListOpeningTag(attr: string | undefined, rawAttrs?: string): string {
        const type = nexusOrderedListType(attr);
        const start = nexusOrderedListStart(rawAttrs);
        if (start) {
            return `\n[olist${type ? ` type="${type}"` : ""} start="${start}"]\n`;
        }

        return `\n[olist${type ? `=${type}` : ""}]\n`;
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
        const alt = safeNexusImageAlt(nexusBbAttribute(rawAttrs, "alt"));
        const title = safeNexusImageAlt(nexusBbAttribute(rawAttrs, "title"));
        const attrs = [
            source ? `src="${source}"` : "",
            alignment ? `align="${alignment}"` : "",
            width ? `width="${width}"` : "",
            height ? `height="${height}"` : "",
            nexusSafeBbAttribute("alt", alt),
            nexusSafeBbAttribute("title", title)
        ].filter(Boolean);

        if (attrs.length > 0) {
            return `[img ${attrs.join(" ")}]`;
        }

        return direct ? `[img=${direct}]` : "[img]";
    }

    function nexusSafeBbAttribute(name: string, value?: string): string {
        const cleaned = value?.trim();
        return cleaned ? `${name}="${escapeAttribute(cleaned)}"` : "";
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
            case "anchor":
            case "bookmark":
            case "target":
            case "goto":
            case "jump":
                return nexusBbAttribute(rawAttrs, "id")
                    ?? nexusBbAttribute(rawAttrs, "href")
                    ?? nexusBbAttribute(rawAttrs, "name")
                    ?? nexusBbAttribute(rawAttrs, "target")
                    ?? nexusBbAttribute(rawAttrs, "title")
                    ?? nexusBbAttribute(rawAttrs, "label")
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
            || /\[(?:table|list|olist|dl|quote|spoiler|collapse|details|accordion|accordionitem|columns|cols|tabs|box|panel|fieldset|notice|note|info|warning|important|tip)(?:[=\s\]])/i.test(value);
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

    function nexusHtmlListOpeningTag(attrs: string, ordered: boolean): string {
        const style = htmlAttribute(attrs, "type") ?? htmlListStyleTypeAttribute(attrs) ?? "";
        if (ordered) {
            const type = nexusOrderedListType(style);
            const start = nexusOrderedListStart(` start="${htmlAttribute(attrs, "start") ?? ""}"`);
            if (start) {
                return `\n[olist${type ? ` type="${type}"` : ""} start="${start}"]\n`;
            }

            return `\n[olist${type ? `=${type}` : ""}]\n`;
        }

        const marker = nexusUnorderedListStyle(style);
        return `\n[list${marker ? `=${marker}` : ""}]\n`;
    }

    function htmlListStyleTypeAttribute(attrs: string): string | null {
        const style = htmlAttribute(attrs, "style") ?? "";
        const styled = style.match(/(?:^|;)\s*list-style(?:-type)?\s*:\s*([^;]+)/i)?.[1];
        const classes = htmlAttribute(attrs, "class") ?? "";
        const classed = classes.match(/(?:^|\s)(?:list-style-)?(disc|circle|square|none|dash|hyphen|check|checklist|decimal|lower-alpha|upper-alpha|lower-latin|upper-latin|lower-roman|upper-roman|roman)(?:\s|$)/i)?.[1];
        return styled ?? classed ?? null;
    }

    function nexusHtmlTableCellAttributes(attrs: string): string {
        const colspan = safeNexusTableSpan(htmlAttribute(attrs, "colspan") ?? htmlAttribute(attrs, "col") ?? undefined);
        const rowspan = safeNexusTableSpan(htmlAttribute(attrs, "rowspan") ?? htmlAttribute(attrs, "row") ?? undefined);
        const style = htmlAttribute(attrs, "style") ?? "";
        const styledWidth = style.match(/(?:^|;)\s*width\s*:\s*([^;]+)/i)?.[1];
        const width = safeNexusCssLength(htmlAttribute(attrs, "width") ?? styledWidth);
        const alignment = htmlAlignmentAttribute(attrs);
        return [
            colspan ? `colspan="${colspan}"` : "",
            rowspan ? `rowspan="${rowspan}"` : "",
            width ? `width="${width}"` : "",
            alignment ? `align="${alignment}"` : ""
        ].filter(Boolean).map(attribute => ` ${attribute}`).join("");
    }

    function nexusHtmlImageAttributes(attrs: string): string {
        const alignment = htmlImageAlignment(attrs);
        const width = safeNexusCssLength(htmlAttribute(attrs, "width"));
        const height = safeNexusCssLength(htmlAttribute(attrs, "height"));
        const alt = safeNexusImageAlt(htmlAttribute(attrs, "alt") ?? htmlAttribute(attrs, "aria-label") ?? undefined);
        const title = safeNexusImageAlt(htmlAttribute(attrs, "title") ?? htmlAttribute(attrs, "aria-label") ?? undefined);
        return [
            alignment ? ` align="${alignment}"` : "",
            width ? ` width="${width}"` : "",
            height ? ` height="${height}"` : "",
            alt ? ` alt="${escapeAttribute(alt)}"` : "",
            title ? ` title="${escapeAttribute(title)}"` : ""
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

    function safeNexusBbLabel(value?: string): string {
        return safeNexusLabel(value)
            .replace(/[\[\]\r\n]/g, "")
            .trim()
            .slice(0, 70);
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
        const attr = nexusListStyleToken(value).toLowerCase();
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
        const attr = nexusListStyleToken(value);
        const lowered = attr.toLowerCase();

        if (attr === "1" || lowered === "decimal" || lowered === "decimal-leading-zero" || lowered === "number" || lowered === "numbers" || lowered === "numeric") {
            return "1";
        }

        if (attr === "a" || lowered === "lower-alpha" || lowered === "lower-latin" || lowered === "alpha") {
            return "a";
        }

        if (attr === "A" || lowered === "upper-alpha" || lowered === "upper-latin") {
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

    function nexusListStyleToken(value?: string): string {
        const normalized = decodeHtmlEntities(value ?? "")
            .trim()
            .replace(/^['"]|['"]$/g, "")
            .replace(/\s*!important\s*$/i, "");
        const styled = normalized.match(/(?:^|;)\s*list-style(?:-type)?\s*:\s*([^;]+)/i)?.[1];
        return (styled ?? normalized)
            .replace(/^['"]|['"]$/g, "")
            .replace(/;.*$/g, "")
            .trim();
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

    function nexusOrderedListStart(rawAttrs?: string, attr?: string): number | undefined {
        const raw = nexusBbAttribute(rawAttrs, "start")
            ?? nexusBbAttribute(rawAttrs, "value")
            ?? (attr && /^\d{1,3}$/.test(attr.trim()) ? attr : undefined);
        const numeric = Number.parseInt(decodeHtmlEntities(raw ?? "").trim().replace(/^['"]|['"]$/g, ""), 10);
        if (!Number.isFinite(numeric) || numeric <= 1) {
            return undefined;
        }

        return Math.min(999, numeric);
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
        const raw = decodeHtmlEntities(value ?? "")
            .trim()
            .toLowerCase()
            .replace(/;+\s*$/g, "")
            .replace(/\s*!important\s*$/i, "")
            .replace(/^['"]|['"]$/g, "");
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

        const relative = raw.match(/^([+-])\s*(\d+(?:\.\d+)?)$/);
        if (relative) {
            const delta = Number.parseFloat(relative[2]);
            if (relative[1] === "-") {
                return delta >= 2 ? "nexus-rich-size-tiny" : "nexus-rich-size-small";
            }

            return delta >= 2 ? "nexus-rich-size-xlarge" : "nexus-rich-size-large";
        }

        const numeric = Number.parseFloat(raw);
        if (Number.isFinite(numeric)) {
            const looksLikePixels = raw.includes("px") || raw.includes("pt") || numeric > 7;
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

<div
    class="column nexus-page"
    class:nexus-catalog-tight={nexusCatalogTight}
    class:nexus-catalog-very-tight={nexusCatalogVeryTight}
    class:nexus-embedded-store-preview={embeddedStorePreview}
    class:nexus-embedded-controls-expanded={embeddedStorePreview && embeddedControlsExpanded}
    class:nexus-session-connected={session.is_connected}
    bind:this={nexusPageElement}
>
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
                <label class="auto-endorse-control" title={autoEndorseQueueLabel}>
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
                <button class="inventory-refresh-btn" disabled={isRefreshingInventory} title="Rescan local and Vortex-managed mod inventory" on:click={refreshLocalInventoryFromUi}>
                    {isRefreshingInventory ? "Scanning..." : "Refresh Inventory"}
                </button>
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
            <div
                class="summary-card account-summary-card"
                class:premium-summary={!!session.user?.is_premium}
                class:supporter-summary={!session.user?.is_premium && !!session.user?.is_supporter}
                title={nexusMembershipBenefitTitle()}
            >
                <span class="summary-label">Nexus Account</span>
                <span class="summary-value">{nexusMembershipLabel()}</span>
                <span class="summary-note">{nexusMembershipBenefitNote()}</span>
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
                <span>{nexusCatalogCacheSummary()}</span>
                {#if catalogMode === "online"}
                    <span>{visibleNexusMods.length} shown from {mods.length} known. {onlineAttentionCount > 0 ? `${onlineAttentionCount} need attention.` : ""} {trackedModsLoaded ? `${trackedCount} tracked.` : ""} {conflictCount > 0 ? `${conflictCount} in conflicts.` : ""}</span>
                {:else}
                    <span>{visibleInstalledEntries.length} shown from {installedCount} installed. {installedAttentionCount > 0 ? `${installedAttentionCount} need attention.` : ""} {trackedModsLoaded ? `${trackedCount} tracked.` : ""} {conflictCount > 0 ? `${conflictCount} in conflicts.` : ""}</span>
                {/if}
            </div>

            {#if showFullActionQueue}
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
            {:else}
                <div class="action-queue-empty" aria-label="Nexus action queue">
                    <b>Queue clear</b>
                    <span>No endorsements, tracked gaps, updates, disabled mods, or conflicts.</span>
                </div>
            {/if}

            <div class="nexus-filter-row" class:compact-filter-row={!showEmbeddedSearch}>
                {#if showEmbeddedSearch}
                    <input class="generic-input key-input" value={nexusSearchTerm} on:input={(event) => handleNexusSearchInput((event.currentTarget as HTMLInputElement).value)} placeholder="Search Nexus" />
                {/if}
                <select
                    bind:value={selectedNexusCategory}
                    disabled={catalogMode === "installed"}
                    title={catalogMode === "installed" ? "Category filters apply to the online Nexus catalog." : "Filter online Nexus mods by category."}
                    aria-label="Filter by Nexus category"
                >
                    {#each nexusCategoryFilterOptions as option (option.value)}
                        <option value={option.value}>{option.label}</option>
                    {/each}
                </select>
                <select bind:value={selectedInstallFilter}>
                    {#each installFilterOptions as option (option.value)}
                        <option value={option.value}>{option.label}</option>
                    {/each}
                </select>
                <select bind:value={selectedModTypeFilter} aria-label="Filter by mod type">
                    {#each modTypeFilterOptions as option (option.value)}
                        <option value={option.value}>{option.label}</option>
                    {/each}
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
                        {#each localConflicts as conflict (conflict.key)}
                            <section class="conflict-group">
                                <div class="conflict-group-title">
                                    <span>{conflict.label}</span>
                                    <small>{conflictGroupSummary(conflict)}</small>
                                </div>

                                {#each conflict.entries as entry (inventoryEntryKey(entry))}
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
                    {#each visibleNexusMods as mod (mod.mod_id)}
                        {@const match = installedMatch(mod)}
                        {@const updateVerdict = nexusUpdateVerdict(mod, match)}
                        {@const conflict = conflictForEntry(match)}
                        {@const previewCacheVersion = catalogPreviewCacheVersion}
                        {@const previewUrls = catalogPreviewUrls(mod, previewCacheVersion)}
                        {@const previewIndex = catalogPreviewIndex(mod, previewUrls)}
                        {@const previewNotice = catalogPreviewNotice(mod)}
                        {@const categoryBadge = nexusCategorySourceBadge(mod.category_source)}
                        <article class="nexus-card" class:nexus-installed={!!match} class:nexus-conflict={!!conflict}>
                            <div class="thumbnail-frame">
                                <button
                                    class="thumbnail-button"
                                    aria-label={previewUrls.length === 0 ? `Load preview image for ${mod.name}` : `Open ${mod.name} details`}
                                    title={previewUrls.length === 0 ? "Load real preview from Nexus details" : `Open ${mod.name} details`}
                                    on:click={(event) => handleCatalogThumbnailClick(mod, previewUrls, event)}
                                >
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
                                        <button type="button" aria-label={`Open current preview image for ${mod.name}`} title="Open current preview image" on:click={(event) => openCatalogPreviewImage(mod, previewUrls, event)}>
                                            <LucideExternalLink class="thumbnail-icon" aria-hidden="true" />
                                        </button>
                                    </div>
                                {:else if previewUrls.length === 1}
                                    <div
                                        class="thumbnail-count thumbnail-count-action"
                                        aria-label={`${mod.name} has one preview image`}
                                        title="1 preview image"
                                    >
                                        <span class="thumbnail-count-label">
                                            <LucideImages class="thumbnail-icon" aria-hidden="true" />
                                            <span>1</span>
                                        </span>
                                        <button type="button" aria-label={`Open current preview image for ${mod.name}`} title="Open current preview image" on:click={(event) => openCatalogPreviewImage(mod, previewUrls, event)}>
                                            <LucideExternalLink class="thumbnail-icon" aria-hidden="true" />
                                        </button>
                                    </div>
                                {:else}
                                    <div
                                        class="thumbnail-count thumbnail-preview-fetch"
                                        class:thumbnail-count-fallback={previewUrls.length === 0}
                                        aria-label={`${mod.name} uses the local fallback preview image. Load a real preview from cached Nexus details if available.`}
                                        title="Load real preview from Nexus details"
                                    >
                                        <button
                                            type="button"
                                            disabled={activeCatalogPreviewLoadId === mod.mod_id}
                                            on:pointerdown={(event) => loadCatalogPreview(mod, event)}
                                            on:click={(event) => loadCatalogPreview(mod, event)}
                                        >
                                            {#if activeCatalogPreviewLoadId === mod.mod_id}
                                                <SvgSpinnersBlocksWave class="thumbnail-icon" aria-hidden="true" />
                                            {:else}
                                                <LucideImages class="thumbnail-icon" aria-hidden="true" />
                                            {/if}
                                            <span>{previewNotice || "Load preview"}</span>
                                        </button>
                                    </div>
                                {/if}
                            </div>

                            <div class="nexus-body">
                                <div class="card-head">
                                    <button class="title-stack title-button" on:click={() => openModDetails(mod)}>
                                        <span class="mod-title">{mod.name}</span>
                                        <span class="mod-byline" title={nexusCategorySourceTitle(mod)}>
                                            {mod.category_name ?? "Nexus"}
                                            {#if categoryBadge}
                                                <span class="category-source-badge" class:category-source-inferred={mod.category_source === "inferred"}>{categoryBadge}</span>
                                            {/if}
                                            <span class="byline-divider">·</span>
                                            {mod.loader_type ?? "Unknown type"}
                                            <span class="byline-divider">·</span>
                                            {mod.author ?? mod.uploaded_by ?? "Unknown author"}
                                        </span>
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
                                        {#if previewUrls.length === 0}
                                            <button
                                                class="preview-load-btn"
                                                disabled={activeCatalogPreviewLoadId === mod.mod_id}
                                                title="Load real preview from cached Nexus details"
                                                on:pointerdown={(event) => loadCatalogPreview(mod, event)}
                                                on:click={(event) => loadCatalogPreview(mod, event)}
                                            >
                                                {activeCatalogPreviewLoadId === mod.mod_id ? "Loading..." : previewNotice || "Preview"}
                                            </button>
                                        {/if}
                                        {#if conflict}
                                            <button class="conflict-review-btn" on:click={() => reviewLocalConflict(conflict)}>Review Conflict</button>
                                        {/if}
                                        <button on:click={() => openModDetails(mod)}>Details</button>
                                        <button on:click={() => openModPage(mod)}>Open Page</button>
                                    </div>
                                </div>
                            </div>
                        </article>
                    {/each}
                {:else}
                    {#each visibleInstalledEntries as entry (inventoryEntryKey(entry))}
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
                                        {#if conflict}
                                            <button class="conflict-review-btn" on:click={() => reviewLocalConflict(conflict)}>Review Conflict</button>
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

                {#if !isLoading && ((catalogMode === "online" && visibleNexusMods.length === 0) || (catalogMode === "installed" && visibleInstalledEntries.length === 0))}
                    <div class="notice empty-nexus empty-nexus-panel">
                        <div class="empty-nexus-copy">
                            <span>{nexusEmptyTitle}</span>
                            <small>{nexusEmptyDetail}</small>
                        </div>
                        {#if nexusEmptyChips.length > 0}
                            <div class="empty-nexus-chips" aria-label="Active Nexus catalog context">
                                {#each nexusEmptyChips as chip (chip)}
                                    <span>{chip}</span>
                                {/each}
                            </div>
                        {/if}
                        {#if hasActiveNexusFilters}
                            <button type="button" class="cat-btn empty-nexus-clear" on:click={clearNexusFilters}>Clear filters</button>
                        {/if}
                    </div>
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
                    <span class="panel-subtitle" title={nexusCategorySourceTitle(selectedModDetails ?? selectedMod)}>
                        {selectedModDetails?.category_name ?? selectedMod.category_name ?? "Nexus"}
                        {#if nexusCategorySourceBadge(selectedModDetails?.category_source ?? selectedMod.category_source)}
                            <span class="category-source-badge" class:category-source-inferred={(selectedModDetails?.category_source ?? selectedMod.category_source) === "inferred"}>{nexusCategorySourceBadge(selectedModDetails?.category_source ?? selectedMod.category_source)}</span>
                        {/if}
                        <span class="byline-divider">·</span>
                        {selectedModDetails?.loader_type ?? selectedMod.loader_type ?? "Unknown type"}
                        <span class="byline-divider">·</span>
                        {selectedModDetails?.author ?? selectedModDetails?.uploaded_by ?? selectedMod.author ?? selectedMod.uploaded_by ?? "Unknown author"}
                    </span>
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
                        {#if currentDetailPreviewUrls.length > 0}
                            <div class="detail-media-nav" class:detail-media-nav-single={currentDetailPreviewUrls.length === 1} aria-label={`${selectedMod.name} preview images`}>
                                {#if currentDetailPreviewUrls.length > 1}
                                    <button type="button" aria-label="Previous detail preview image" title="Previous preview image" on:click={(event) => cycleSelectedDetailPreview(currentDetailPreviewUrls, -1, event)}>
                                        <LucideChevronLeft class="detail-media-icon" aria-hidden="true" />
                                    </button>
                                {/if}
                                <span><LucideImages class="detail-media-icon" aria-hidden="true" />{currentDetailPreviewIndex + 1}/{currentDetailPreviewUrls.length}</span>
                                {#if currentDetailPreviewUrls.length > 1}
                                    <button type="button" aria-label="Next detail preview image" title="Next preview image" on:click={(event) => cycleSelectedDetailPreview(currentDetailPreviewUrls, 1, event)}>
                                        <LucideChevronRight class="detail-media-icon" aria-hidden="true" />
                                    </button>
                                {/if}
                                <button type="button" aria-label="Open current detail preview image" title="Open current preview image" on:click={openSelectedDetailPreviewImage}>
                                    <LucideExternalLink class="detail-media-icon" aria-hidden="true" />
                                </button>
                            </div>
                        {/if}
                    </div>
                    {#if currentDetailPreviewUrls.length > 1}
                        <div class="detail-media-strip" aria-label={`${selectedMod.name} preview thumbnails`}>
                            {#each currentDetailPreviewUrls as previewUrl, previewIndex (previewUrl)}
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

                    {#if selectedChangelogs.length === 0}
                        <div class="changelog-compact-note" bind:this={detailChangelogSectionElement}>
                            <span class="detail-section-title">Changelog</span>
                            <span class="dependency-empty">No API-listed changelog entries were returned for this mod.</span>
                        </div>
                    {:else}
                        <div class="changelog-box" bind:this={detailChangelogSectionElement}>
                            <span class="detail-section-title">Changelog</span>
                            {#each selectedChangelogs as changelog (`${changelog.version}:${changelog.updated_at ?? ""}`)}
                                <div class="changelog-row">
                                    <span>{changelog.version}</span>
                                    <div class="nexus-rich-text changelog-rich-text">
                                        {@html renderNexusRichText(changelog.changes, "No changelog text was returned.")}
                                    </div>
                                </div>
                            {/each}
                        </div>
                    {/if}
                </div>

                <div class="detail-side" class:detail-side-dependencies-first={detailDependenciesFirst}>
                    <div class="detail-facts">
                        <span>Version <b>{selectedModDetails?.version ?? "-"}</b></span>
                        <span>Type <b>{selectedModDetails?.loader_type ?? "Unknown"}</b></span>
                        <span>Update <b title={selectedModUpdateReason()} class:update-state-update={selectedModUpdateTone() === "update"} class:update-state-current={selectedModUpdateTone() === "current"} class:update-state-tracked={selectedModUpdateTone() === "tracked"} class:update-state-review={selectedModUpdateTone() === "review"}>{selectedModUpdateLabel()}</b></span>
                        <span>Tracked <b>{isNexusModTracked(selectedMod.mod_id) ? "Yes" : "No"}</b></span>
                        <span>Updated <b>{formatTimestamp(selectedModDetails?.updated_timestamp, selectedModDetails?.updated_time)}</b></span>
                        <span>Downloads <b>{formatNumber(selectedModDetails?.mod_downloads)}</b></span>
                        <span>Installed <b>{describeInstallSource(installedMatch(selectedMod))}</b></span>
                        <span>Dependencies <b class:update-state-update={totalDependencyIssueCount > 0} class:update-state-tracked={totalDependencyReviewCount > 0 && totalDependencyIssueCount === 0} class:update-state-current={(resolvedDependencies.length > 0 || resolvedNestedDependencies.length > 0 || authorRequirementDetectedCount > 0) && totalDependencyIssueCount === 0 && totalDependencyReviewCount === 0}>{dependencySummaryLabel()}</b></span>
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
                                {#each INSTALL_PLACEMENTS as placement (placement)}
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
                            <span>API deps <b title={apiDependencyReadinessLabel()}>{resolvedDependencies.length}</b></span>
                            <span>Author hints <b title={authorRequirementReadinessLabel()}>{authorRequirementReadinessLabel()}</b></span>
                            <span>Instructions <b>{selectedAuthorInstructions.length > 0 ? `${selectedAuthorInstructions.length} found` : "Review text"}</b></span>
                            <span class="install-plan-file-fact">Nested <b title={nestedDependencyReadinessLabel()}>{nestedDependencyReadinessLabel()}</b></span>
                        </div>
                        <div class="install-plan-notes">
                            {#each selectedInstallPlan.notes as note (note)}
                                <span>{note}</span>
                            {/each}
                        </div>
                        {#if selectedAuthorInstructions.length > 0}
                            <div class="author-instructions" aria-label="Author installation instructions">
                                <div class="author-instructions-head">
                                    <span class="detail-section-title">Author Instructions</span>
                                    <b>{selectedAuthorInstructions.length}</b>
                                </div>
                                <div class="author-instruction-list">
                                    {#each selectedAuthorInstructions as instruction (instruction.key)}
                                        <button
                                            type="button"
                                            class="author-instruction-row"
                                            class:author-instruction-warning={instruction.kind === "warning"}
                                            class:author-instruction-requirement={instruction.kind === "requirement"}
                                            title={`${instruction.source}: ${instruction.detail}`}
                                            on:click={() => scrollDetailSection(instruction.target)}
                                        >
                                            <small>{instruction.source} · {instruction.kind}</small>
                                            <span>{instruction.title}</span>
                                            <p>{instruction.detail}</p>
                                        </button>
                                    {/each}
                                </div>
                            </div>
                        {/if}
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
                                <div>
                                    <span class="detail-section-title">Local Conflict</span>
                                    <small>Review the duplicate deployment group before handing this file to Vortex.</small>
                                </div>
                                <button class="conflict-review-btn" type="button" on:click={() => reviewLocalConflict(selectedInstallConflict)}>Review Conflict</button>
                                <b>{selectedInstallConflict.entries.length} installs</b>
                            </div>
                            <span class="detail-conflict-note">{selectedInstallConflict.label} · {conflictGroupSummary(selectedInstallConflict)}</span>

                            <div class="detail-conflict-list">
                                {#each selectedInstallConflict.entries as entry (inventoryEntryKey(entry))}
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
                        <div class="file-picker-head">
                            <span class="detail-section-title">Files</span>
                            {#if selectedFileCanUseRecommended}
                                <button
                                    class="use-recommended-file-btn"
                                    type="button"
                                    title={selectedFileRecommendedActionTitle}
                                    on:click={selectRecommendedNexusFile}
                                >
                                    Use Recommended
                                </button>
                            {/if}
                        </div>
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
                        <div class="file-filter-row" aria-label="File choice filters">
                            <button
                                type="button"
                                class:file-filter-selected={selectedFileListFilter === "all"}
                                aria-pressed={selectedFileListFilter === "all"}
                                disabled={selectedModFiles.length === 0}
                                on:click={() => selectedFileListFilter = "all"}
                            >
                                All <b>{selectedModFiles.length}</b>
                            </button>
                            <button
                                type="button"
                                class:file-filter-selected={selectedFileListFilter === "main"}
                                aria-pressed={selectedFileListFilter === "main"}
                                disabled={selectedFileRecommendedCount === 0}
                                on:click={() => selectedFileListFilter = "main"}
                            >
                                Main <b>{selectedFileRecommendedCount}</b>
                            </button>
                            <button
                                type="button"
                                class:file-filter-selected={selectedFileListFilter === "review"}
                                aria-pressed={selectedFileListFilter === "review"}
                                disabled={selectedFileReviewCount === 0}
                                on:click={() => selectedFileListFilter = "review"}
                            >
                                Review <b>{selectedFileReviewCount}</b>
                            </button>
                            <button
                                type="button"
                                class:file-filter-selected={selectedFileListFilter === "selected"}
                                aria-pressed={selectedFileListFilter === "selected"}
                                disabled={!selectedNexusFile}
                                on:click={() => selectedFileListFilter = "selected"}
                            >
                                Selected <b>{selectedNexusFile ? 1 : 0}</b>
                            </button>
                        </div>
                        {#if selectedModFiles.length === 0 && !isDetailLoading}
                            <div class="notice empty-nexus">No downloadable files were returned by Nexus.</div>
                        {:else if displayedSelectedModFiles.length === 0 && !isDetailLoading}
                            <div class="notice empty-nexus">No {describeFileListFilter(selectedFileListFilter)} match this file set.</div>
                        {/if}

                        {#if displayedSelectedModFiles.length > 0}
                            <div class="file-row-list">
                                {#each displayedSelectedModFiles as file (file.file_id)}
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
                        {/if}
                    </div>

                    {#snippet selectedFileNotesBlock()}
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
                    {/snippet}

                    {#snippet authorRequirementSectionBlock()}
                        {#if selectedAuthorRequirements.length > 0}
                            <div class="author-requirement-section">
                                <span class="dependency-subtitle">Author requirement hints</span>
                                {#each selectedAuthorRequirements as requirement (requirement.key)}
                                    <div
                                        class="author-requirement-row"
                                        class:author-requirement-detected={requirement.status === "detected"}
                                        class:author-requirement-warning={requirement.status === "warning"}
                                    >
                                        <div class="author-requirement-main">
                                            <span>{requirement.label}</span>
                                            <small>{requirement.source}{requirement.mod_id ? ` · Mod ${requirement.mod_id}` : ""} · {requirement.status === "detected" ? "Detected locally" : requirement.status === "warning" ? "Compatibility warning" : "Review manually"}</small>
                                            {#if requirement.detail}
                                                <small>{requirement.detail}</small>
                                            {/if}
                                            {#if requirement.excerpt}
                                                <small class="author-requirement-excerpt">Matched: {requirement.excerpt}</small>
                                            {/if}
                                        </div>
                                        {#if requirement.mod_id || requirement.url || requirement.match}
                                            <button on:click={() => openAuthorRequirement(requirement)}>
                                                {requirement.mod_id ? "Details" : requirement.match ? "Folder" : "Open"}
                                            </button>
                                        {/if}
                                    </div>
                                {/each}
                                <span class="dependency-empty">These came from Nexus author text, selected file notes, or changelog text and are not API dependency rows.</span>
                            </div>
                        {/if}
                    {/snippet}

                    {#snippet dependencyBoxBlock()}
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
                                    class:dependency-readiness-ok={selectedAuthorRequirements.length > 0 && authorRequirementWarningCount === 0 && authorRequirementReviewCount === 0}
                                    class:dependency-readiness-warn={authorRequirementWarningCount > 0}
                                    class:dependency-readiness-review={authorRequirementReviewCount > 0 && authorRequirementWarningCount === 0}
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
                            {#if resolvedDependencies.length === 0}
                                {@render authorRequirementSectionBlock()}
                            {/if}
                            {#if nestedDependencySummary}
                                <span class="dependency-empty">{nestedDependencySummary}</span>
                            {/if}
                            {#if selectedDependencyMessage}
                                <span class="dependency-empty">{selectedDependencyMessage}</span>
                            {/if}
                            {#if resolvedDependencies.length === 0}
                                <span class="dependency-empty">{selectedDependencyMessage ? "Still review the author directions for manual requirements." : "No API-listed dependencies for the selected file. Still review the author directions for manual requirements."}</span>
                            {:else}
                                {#each resolvedDependencies as dependency (`${dependency.id}:${dependency.mod_id ?? ""}:${dependency.file_id ?? ""}:${dependency.nexus_file_id ?? ""}`)}
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
                                                <button
                                                    class="dependency-primary-action"
                                                    class:dependency-primary-warn={dependency.status === "missing" || dependency.status === "version-mismatch"}
                                                    title={dependencyPrimaryActionTitle(dependency)}
                                                    on:click={() => openDependencyDetails(dependency)}
                                                >
                                                    {dependencyPrimaryActionLabel(dependency)}
                                                </button>
                                                <button on:click={() => openDependencyPage(dependency)}>Nexus</button>
                                            {/if}
                                            {#if dependency.match}
                                                <button on:click={() => openDependencyLocation(dependency)}>Open Folder</button>
                                            {/if}
                                        </div>
                                    </div>
                                {/each}
                            {/if}
                            {#if resolvedDependencies.length > 0}
                                {@render authorRequirementSectionBlock()}
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
                                                    <button
                                                        class="dependency-primary-action"
                                                        class:dependency-primary-warn={source.dependency.status === "missing" || source.dependency.status === "version-mismatch"}
                                                        title={dependencyPrimaryActionTitle(source.dependency)}
                                                        on:click={() => openDependencyDetails(source.dependency)}
                                                    >
                                                        {dependencyPrimaryActionLabel(source.dependency)}
                                                    </button>
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
                    {/snippet}

                    {#if detailDependenciesFirst}
                        {@render dependencyBoxBlock()}
                        {@render selectedFileNotesBlock()}
                    {:else}
                        {@render selectedFileNotesBlock()}
                        {@render dependencyBoxBlock()}
                    {/if}

                </div>
            </div>

            <div class="detail-footer" aria-label="Nexus deployment actions" bind:this={detailFooterElement}>
                <div class="detail-footer-copy">
                    <span class="detail-section-title">Deployment</span>
                    <small title={selectedInstallFileLabel}>{selectedInstallFileLabel}</small>
                </div>

                <div class="deployment-checklist" aria-label="Deployment readiness checklist">
                    {#each deploymentChecklistItems as item (item.key)}
                        <button
                            type="button"
                            class="deployment-check-item"
                            class:deployment-check-ready={item.tone === "ready"}
                            class:deployment-check-review={item.tone === "review"}
                            class:deployment-check-blocked={item.tone === "blocked"}
                            title={item.detail}
                            on:click={() => scrollDetailSection(item.target)}
                        >
                            <small>{item.label}</small>
                            <b>{item.value}</b>
                        </button>
                    {/each}
                </div>

                <div class="detail-actions">
                    {#if selectedFileFooterReviewVisible}
                        <button
                            class="file-review-btn"
                            title={selectedFileFooterReviewTitle}
                            on:click={() => scrollDetailSection("files")}
                        >
                            Review File Choice
                        </button>
                    {/if}
                    {#if shouldShowDependencyFooterReviewAction()}
                        <button
                            class="dependency-review-btn"
                            title={dependencyFooterReviewTitle()}
                            on:click={() => scrollDetailSection("dependencies")}
                        >
                            {dependencyFooterReviewLabel()}
                        </button>
                    {/if}
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
                    {#if selectedInstallConflict}
                        <button class="conflict-review-btn" on:click={() => reviewLocalConflict(selectedInstallConflict)}>Review Conflict</button>
                    {/if}
                    <button class="inventory-refresh-btn" disabled={isRefreshingInventory} title="Rescan local and Vortex-managed mod inventory" on:click={refreshLocalInventoryFromUi}>
                        {isRefreshingInventory ? "Scanning..." : "Refresh Inventory"}
                    </button>
                    <button on:click={openSelectedModPage}>Open Page</button>
                </div>

                <div class="detail-link-actions" aria-label="Nexus page sections">
                    <span class="detail-link-heading">Nexus surfaces</span>
                    {#each nexusSurfaceLinks as surface (surface.key)}
                        <button
                            title={surface.title}
                            on:click={() => surface.key === "files" ? openSelectedDownloadPage() : openSelectedModTab(surface.key)}
                        >
                            <span>{surface.label}</span>
                            <small>{surface.status}</small>
                        </button>
                    {/each}
                </div>
            </div>
        </section>
    </div>
{/if}

<style>
    .nexus-page {
        --nexus-card-min-height: clamp(138px, 17vh, 178px);
        --nexus-catalog-target-height: 360px;
        --nexus-scroll-bottom-guard: max(var(--app-bottom-safe-area, 42px), clamp(56px, 6.2vh, 84px));
        --nexus-scroller-target-height: 280px;
        --nexus-thumb-width: clamp(145px, 18vw, 230px);
        box-sizing: border-box;
        gap: clamp(0.55em, 1vh, 0.9em);
        height: 100%;
        justify-content: flex-start;
        min-height: 0;
        overflow: hidden;
    }

    .nexus-page.nexus-catalog-tight {
        gap: clamp(0.35em, 0.7vh, 0.55em);
    }

    .nexus-page.nexus-embedded-store-preview {
        --nexus-card-min-height: clamp(96px, 10vh, 118px);
        --nexus-scroll-bottom-guard: clamp(18px, 2.5vh, 30px);
        --nexus-thumb-width: clamp(112px, 13vw, 150px);
        gap: clamp(0.3em, 0.55vh, 0.42em);
    }

    .nexus-embedded-store-preview .account-panel {
        padding: 0.38em 0.65em;
    }

    .nexus-embedded-store-preview.nexus-session-connected .account-panel {
        display: none;
    }

    .nexus-embedded-store-preview .account-actions,
    .nexus-embedded-store-preview .rate-row,
    .nexus-embedded-store-preview .vortex-summary,
    .nexus-embedded-store-preview .catalog-toolbar,
    .nexus-embedded-store-preview .nexus-filter-row,
    .nexus-embedded-store-preview .action-queue-empty {
        display: none;
    }

    .nexus-embedded-store-preview.nexus-embedded-controls-expanded .catalog-toolbar {
        display: flex;
    }

    .nexus-embedded-store-preview.nexus-embedded-controls-expanded .nexus-filter-row {
        display: grid;
    }

    .nexus-embedded-store-preview .panel-title {
        font-size: 0.98em;
    }

    .nexus-embedded-store-preview .panel-subtitle {
        font-size: 0.76em;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .nexus-embedded-store-preview .catalog-panel,
    .nexus-embedded-store-preview .nexus-scroller {
        gap: 0.34em;
    }

    .nexus-embedded-store-preview .nexus-scroller {
        scroll-snap-type: y mandatory;
    }

    .nexus-embedded-store-preview .api-note {
        padding: 0.28em 0.55em;
    }

    .nexus-embedded-store-preview .cat-btn,
    .nexus-embedded-store-preview .refresh-btn,
    .nexus-embedded-store-preview .catalog-mode-buttons button {
        height: 2.2em;
    }

    .nexus-embedded-store-preview .nexus-filter-row select,
    .nexus-embedded-store-preview .nexus-filter-row .key-input {
        min-height: 2.2em;
        padding-bottom: 0.28em;
        padding-top: 0.28em;
    }

    .nexus-embedded-store-preview .nexus-body {
        align-items: center;
        display: grid;
        gap: 0.26em;
        grid-template-areas:
            "head actions"
            "description actions";
        grid-template-columns: minmax(0, 1fr) auto;
        padding: 0.42em 0.52em;
    }

    .nexus-embedded-store-preview .card-head {
        grid-area: head;
    }

    .nexus-embedded-store-preview .description-content {
        -webkit-line-clamp: 1;
        grid-area: description;
        line-clamp: 1;
        min-height: auto;
    }

    .nexus-embedded-store-preview .facts {
        display: none;
    }

    .nexus-embedded-store-preview .nexus-card-footer {
        align-items: center;
        align-self: start;
        gap: 0.35em;
        grid-area: actions;
        justify-self: end;
        margin-top: 0;
    }

    .nexus-embedded-store-preview .match-detail,
    .nexus-embedded-store-preview .nexus-enable {
        display: none;
    }

    .nexus-embedded-store-preview .button-row {
        gap: 0.28em;
        justify-content: flex-end;
        margin-left: 0;
        max-width: min(36vw, 25em);
    }

    .nexus-embedded-store-preview .button-row .track-btn,
    .nexus-embedded-store-preview .button-row .endorse-btn,
    .nexus-embedded-store-preview .button-row .endorsed-btn {
        display: none;
    }

    .nexus-embedded-store-preview .button-row button {
        font-size: 0.68em;
        min-width: 5.6em;
        padding: 0.34em 0.5em;
    }

    .nexus-embedded-store-preview .button-row .vortex-install-btn {
        min-width: 8.7em;
    }

    .nexus-page.nexus-catalog-tight .account-panel {
        padding: 0.45em 0.75em;
    }

    .nexus-page.nexus-catalog-tight .account-actions {
        gap: 0.38em;
    }

    .nexus-page.nexus-catalog-tight .account-actions button,
    .nexus-page.nexus-catalog-tight .auto-endorse-control {
        font-size: 0.72em;
        min-height: 2.2em;
        padding: 0.26em 0.58em;
    }

    .nexus-page.nexus-catalog-tight .auto-endorse-label small {
        display: none;
    }

    .nexus-page.nexus-catalog-tight .panel-title {
        font-size: 0.98em;
    }

    .nexus-page.nexus-catalog-tight .panel-subtitle {
        font-size: 0.78em;
    }

    .nexus-page.nexus-catalog-tight .rate-row,
    .nexus-page.nexus-catalog-tight .vortex-summary,
    .nexus-page.nexus-catalog-tight .catalog-panel,
    .nexus-page.nexus-catalog-tight .action-queue,
    .nexus-page.nexus-catalog-tight .nexus-filter-row,
    .nexus-page.nexus-catalog-tight .nexus-scroller {
        gap: 0.38em;
    }

    .nexus-page.nexus-catalog-tight .rate-meter {
        gap: 0.2em;
        padding: 0.32em 0.5em;
    }

    .nexus-page.nexus-catalog-tight .summary-card {
        gap: 0;
        padding: 0.38em 0.5em;
    }

    .nexus-page.nexus-catalog-tight .summary-note,
    .nexus-page.nexus-catalog-tight .queue-chip small {
        display: none;
    }

    .nexus-page.nexus-catalog-tight .api-note {
        padding: 0.34em 0.6em;
    }

    .nexus-page.nexus-catalog-tight .queue-chip {
        min-height: 2.35em;
        padding: 0.3em 0.5em;
    }

    .nexus-page.nexus-catalog-tight .queue-chip b {
        font-size: 1em;
    }

    .nexus-page.nexus-catalog-tight .action-queue-empty {
        min-height: 1.9em;
        padding: 0.26em 0.55em;
    }

    .nexus-page.nexus-catalog-tight .nexus-filter-row select,
    .nexus-page.nexus-catalog-tight .nexus-filter-row .key-input {
        min-height: 2.28em;
        padding-bottom: 0.32em;
        padding-top: 0.32em;
    }

    .nexus-page.nexus-catalog-tight .cat-btn,
    .nexus-page.nexus-catalog-tight .refresh-btn,
    .nexus-page.nexus-catalog-tight .catalog-mode-buttons button {
        height: 2.3em;
    }

    .nexus-page.nexus-catalog-tight .nexus-card {
        grid-template-columns: var(--nexus-thumb-width) minmax(0, 1fr);
        min-height: var(--nexus-card-min-height);
    }

    .nexus-page.nexus-catalog-tight .thumbnail-frame,
    .nexus-page.nexus-catalog-tight .thumbnail-button,
    .nexus-page.nexus-catalog-tight .nexus-img {
        min-height: var(--nexus-card-min-height);
    }

    .nexus-page.nexus-catalog-tight .nexus-body {
        gap: 0.32em;
        padding: 0.5em 0.65em;
    }

    .nexus-page.nexus-catalog-tight .mod-title {
        font-size: 0.98em;
    }

    .nexus-page.nexus-catalog-tight .description-content {
        -webkit-line-clamp: 1;
        line-clamp: 1;
        min-height: auto;
    }

    .nexus-page.nexus-catalog-tight .facts,
    .nexus-page.nexus-catalog-tight .inventory-facts {
        font-size: 0.72em;
    }

    .nexus-page.nexus-catalog-tight .nexus-card-footer {
        align-items: center;
        gap: 0.45em;
    }

    .nexus-page.nexus-catalog-tight .button-row {
        gap: 0.35em;
    }

    .nexus-page.nexus-catalog-tight .button-row button {
        font-size: 0.72em;
        min-width: 5.8em;
        padding: 0.45em 0.62em;
    }

    .nexus-page.nexus-catalog-tight .button-row .vortex-install-btn {
        min-width: 9.2em;
    }

    .nexus-page.nexus-catalog-very-tight .description-content {
        -webkit-line-clamp: 1;
        line-clamp: 1;
    }

    .nexus-page.nexus-catalog-very-tight .facts,
    .nexus-page.nexus-catalog-very-tight .inventory-facts {
        grid-template-columns: repeat(3, minmax(0, 1fr));
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
        display: flex;
        flex-wrap: wrap;
        font-size: 0.85em;
        gap: 0.35em;
        align-items: center;
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

    .account-actions button {
        font-size: 0.82em;
        margin: 0;
        min-height: 2.55em;
        min-width: 8.2em;
        padding: 0.42em 0.8em;
        white-space: nowrap;
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

    .inventory-refresh-btn {
        background: rgba(120, 217, 244, 0.075);
        border-color: rgba(120, 217, 244, 0.24);
        box-shadow: none;
        color: #bdeefa;
    }

    .account-actions .inventory-refresh-btn {
        font-size: 0.8em;
        min-height: 2.6em;
        min-width: 9.8em;
        padding: 0.3em 0.7em;
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

    .account-summary-card {
        border-color: rgba(120, 217, 244, 0.2);
    }

    .premium-summary {
        border-color: rgba(244, 210, 109, 0.32);
        box-shadow: inset 0 0 0 1px rgba(244, 210, 109, 0.08);
    }

    .premium-summary .summary-value {
        color: #f4d26d;
    }

    .supporter-summary {
        border-color: rgba(120, 217, 244, 0.32);
        box-shadow: inset 0 0 0 1px rgba(120, 217, 244, 0.08);
    }

    .supporter-summary .summary-value {
        color: #78d9f4;
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
        flex: 1 1 var(--nexus-catalog-target-height);
        flex-direction: column;
        gap: 0.65em;
        min-height: 0;
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

    .action-queue-empty {
        align-items: center;
        background: rgba(15, 15, 15, 0.62);
        border: 1px solid rgba(98, 240, 155, 0.18);
        box-sizing: border-box;
        color: #8fa2b2;
        display: flex;
        flex: 0 0 auto;
        gap: 0.7em;
        justify-content: space-between;
        min-height: 2.25em;
        min-width: 0;
        padding: 0.35em 0.7em;
        text-transform: uppercase;
    }

    .action-queue-empty b {
        color: #62f09b;
        flex: 0 0 auto;
        font-size: 0.78em;
        font-weight: 900;
        letter-spacing: 0.04em;
        white-space: nowrap;
    }

    .action-queue-empty span {
        font-size: 0.72em;
        font-weight: 800;
        min-width: 0;
        overflow: hidden;
        text-align: right;
        text-overflow: ellipsis;
        white-space: nowrap;
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

    .nexus-filter-row select:disabled {
        color: rgba(232, 232, 232, 0.46);
        cursor: not-allowed;
        opacity: 0.74;
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
        flex: 1 1 var(--nexus-scroller-target-height);
        flex-direction: column;
        gap: 0.65em;
        height: var(--nexus-scroller-target-height);
        max-height: var(--nexus-scroller-target-height);
        min-height: min(var(--nexus-scroller-target-height), 100%);
        overscroll-behavior-y: auto;
        overflow-y: auto;
        padding: 0 0.45em var(--nexus-scroll-bottom-guard) 0;
        scroll-padding-bottom: var(--nexus-scroll-bottom-guard);
        scroll-padding-top: 0.3em;
        scroll-snap-type: y proximity;
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
        scroll-margin-top: 0.3em;
        scroll-snap-align: start;
        scroll-snap-stop: always;
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
        align-items: center;
        color: #8d99a5;
        display: flex;
        gap: 0.35em;
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

    .button-row .preview-load-btn {
        color: #78d9f4;
        min-width: 7.4em;
    }

    .button-row .track-btn,
    .detail-actions .track-btn {
        color: #fdc66d;
        min-width: 6.6em;
    }

    .button-row .conflict-review-btn,
    .detail-actions .conflict-review-btn,
    .detail-conflict-head .conflict-review-btn {
        color: #fdc66d;
        min-width: 9.6em;
    }

    .detail-actions .dependency-review-btn,
    .detail-actions .file-review-btn {
        color: #fdc66d;
        min-width: 10.4em;
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

    .empty-nexus-panel {
        align-items: start;
        display: grid;
        gap: 0.65em;
    }

    .empty-nexus-copy {
        display: grid;
        gap: 0.25em;
        min-width: 0;
    }

    .empty-nexus-copy > span {
        color: #eefcff;
        font-size: 1.02em;
        font-weight: 900;
    }

    .empty-nexus-copy small {
        color: #aab8c5;
        line-height: 1.45;
    }

    .empty-nexus-chips {
        display: flex;
        flex-wrap: wrap;
        gap: 0.35em;
        min-width: 0;
    }

    .empty-nexus-chips span {
        background: rgba(120, 217, 244, 0.1);
        border: 1px solid rgba(120, 217, 244, 0.18);
        color: #cdeef7;
        font-size: 0.72em;
        font-weight: 900;
        line-height: 1.2;
        max-width: 100%;
        overflow-wrap: anywhere;
        padding: 0.28em 0.5em;
    }

    .empty-nexus-clear {
        justify-self: start;
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
        max-width: min(97vw, var(--app-content-max-width, 1680px));
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

    .thumbnail-nav button,
    .thumbnail-count button {
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

    .thumbnail-count button {
        background: rgba(128, 219, 180, 0.12);
        border-color: rgba(128, 219, 180, 0.28);
    }

    .thumbnail-count button:hover,
    .thumbnail-count button:focus-visible {
        background: rgba(128, 219, 180, 0.2);
        border-color: rgba(128, 219, 180, 0.5);
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

    .category-source-badge {
        border: 1px solid rgba(128, 219, 180, 0.45);
        color: #9fdabf;
        flex: 0 0 auto;
        font-size: 0.78em;
        line-height: 1;
        padding: 0.14em 0.42em;
        text-transform: uppercase;
    }

    .category-source-inferred {
        border-color: rgba(231, 190, 107, 0.52);
        color: #e7be6b;
    }

    .byline-divider {
        color: rgba(158, 176, 191, 0.62);
        flex: 0 0 auto;
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
        pointer-events: auto;
        text-overflow: ellipsis;
        top: 0.45em;
        white-space: nowrap;
        z-index: 2;
    }

    .thumbnail-count-action {
        gap: 0.32em;
        padding: 0.2em;
    }

    .thumbnail-preview-fetch {
        padding: 0.18em;
    }

    .thumbnail-preview-fetch button {
        flex: 1 1 auto;
        gap: 0.28em;
        height: auto;
        justify-content: flex-start;
        min-height: 2.15em;
        padding: 0.24em 0.48em;
        width: 100%;
    }

    .thumbnail-preview-fetch button span {
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .thumbnail-preview-fetch button:disabled {
        cursor: progress;
        opacity: 0.82;
    }

    .thumbnail-count-label {
        align-items: center;
        display: inline-flex;
        gap: 0.28em;
        min-width: 0;
        padding: 0 0.16em;
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
        grid-template-columns: minmax(10em, 0.65fr) minmax(26em, 1.35fr) minmax(18em, auto);
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

    .deployment-checklist {
        display: grid;
        gap: 0.38em;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        min-width: 0;
    }

    .deployment-check-item {
        background: rgba(255, 255, 255, 0.035);
        border: 1px solid rgba(255, 255, 255, 0.1);
        box-shadow: none;
        display: flex;
        flex-direction: column;
        gap: 0.18em;
        line-height: 1.12;
        margin: 0;
        min-height: 3.15em;
        min-width: 0;
        padding: 0.42em 0.5em;
        text-align: left;
        -webkit-mask-image: none;
        mask-image: none;
    }

    .deployment-check-item small,
    .deployment-check-item b {
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .deployment-check-item small {
        color: #8d99a5;
        font-size: 0.62em;
        font-weight: 900;
        letter-spacing: 0.07em;
        text-transform: uppercase;
    }

    .deployment-check-item b {
        color: #dce4ea;
        font-size: 0.74em;
        font-weight: 900;
    }

    .deployment-check-ready {
        border-color: rgba(98, 240, 155, 0.24);
    }

    .deployment-check-ready b {
        color: #62f09b;
    }

    .deployment-check-review {
        border-color: rgba(253, 198, 109, 0.34);
    }

    .deployment-check-review b {
        color: #fdc66d;
    }

    .deployment-check-blocked {
        border-color: rgba(253, 155, 157, 0.38);
    }

    .deployment-check-blocked b {
        color: #fd9b9d;
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

    .detail-actions .inventory-refresh-btn {
        min-width: 9.2em;
    }

    .detail-link-actions {
        align-items: stretch;
        display: flex;
        flex-wrap: wrap;
        gap: 0.45em;
        grid-column: 1 / -1;
        justify-content: flex-end;
        min-width: 0;
    }

    .detail-link-heading {
        align-items: center;
        color: #8d99a5;
        display: flex;
        flex: 1 1 100%;
        font-size: 0.68em;
        font-weight: 900;
        justify-content: flex-end;
        line-height: 1;
        text-transform: uppercase;
    }

    .detail-link-actions button {
        align-items: center;
        display: grid;
        flex: 1 1 6.25em;
        gap: 0.08em;
        line-height: 1.12;
        margin: 0;
        min-height: 2.6em;
        min-width: 0;
        padding: 0.32em 0.45em;
    }

    .detail-link-actions button span,
    .detail-link-actions button small {
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .detail-link-actions button small {
        color: #8d99a5;
        font-size: 0.66em;
        font-weight: 900;
        text-transform: uppercase;
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

    .detail-side-dependencies-first .dependency-box {
        order: 3;
    }

    .detail-side-dependencies-first .selected-file-notes {
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
        max-width: min(17.5em, calc(100% - 1.3em));
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

    .detail-media-nav-single {
        max-width: min(8.25em, calc(100% - 1.3em));
    }

    .detail-media-nav span {
        align-items: center;
        color: #d6dde5;
        display: flex;
        flex: 1 1 auto;
        font-size: 0.72em;
        font-weight: 900;
        gap: 0.25em;
        justify-content: center;
        min-width: 4em;
        text-align: center;
        white-space: nowrap;
    }

    .detail-media-nav :global(.detail-media-icon) {
        display: block;
        flex: 0 0 auto;
        height: 1em;
        width: 1em;
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
    .changelog-compact-note,
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

    .nexus-rich-text :global(*) {
        box-sizing: border-box;
        max-width: 100%;
    }

    .nexus-rich-text :global(p) {
        margin: 0.55em 0 0;
        min-width: 0;
        overflow-wrap: anywhere;
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

    .nexus-rich-text :global(.nexus-rich-line-block-mono) {
        font-family: Consolas, "Courier New", monospace;
        font-size: 0.95em;
        overflow-x: auto;
        tab-size: 4;
        white-space: break-spaces;
    }

    .nexus-rich-text :global(p:first-child),
    .nexus-rich-text :global(ul:first-child),
    .nexus-rich-text :global(ol:first-child),
    .nexus-rich-text :global(blockquote:first-child),
    .nexus-rich-text :global(pre:first-child),
    .nexus-rich-text :global(.nexus-rich-line-block:first-child),
    .nexus-rich-text :global(.nexus-rich-table-wrap:first-child),
    .nexus-rich-text :global(.nexus-rich-definition-list:first-child),
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
        min-width: 0;
        overflow-wrap: anywhere;
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

    .nexus-rich-text :global(pre[data-nexus-label])::before {
        color: #78d9f4;
        content: attr(data-nexus-label);
        display: block;
        font-family: "Segoe UI", Arial, sans-serif;
        font-size: 0.78em;
        font-weight: 900;
        margin-bottom: 0.45em;
        text-transform: uppercase;
    }

    .nexus-rich-text :global(.nexus-rich-image) {
        border: 1px solid rgba(255, 255, 255, 0.12);
        box-sizing: border-box;
        display: block;
        height: auto;
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

    .nexus-rich-text :global(.nexus-rich-anchor:empty) {
        display: inline-block;
        height: 0;
        overflow: hidden;
        width: 0;
    }

    .nexus-rich-text :global(.nexus-rich-anchor:not(:empty)) {
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

    .nexus-rich-text :global(.nexus-rich-disclosure-details) {
        border-left: 3px solid rgba(120, 217, 244, 0.3);
    }

    .nexus-rich-text :global(.nexus-rich-disclosure-spoiler) {
        border-left: 3px solid rgba(253, 198, 109, 0.28);
    }

    .nexus-rich-text :global(.nexus-rich-disclosure) {
        padding: 0;
    }

    .nexus-rich-text :global(.nexus-rich-disclosure > summary) {
        align-items: center;
        color: #eefcff;
        cursor: pointer;
        display: flex;
        font-size: 0.82em;
        font-weight: 900;
        gap: 0.45em;
        list-style: none;
        min-height: 2.35em;
        padding: 0.55em 0.7em;
        text-transform: uppercase;
    }

    .nexus-rich-text :global(.nexus-rich-disclosure > summary::-webkit-details-marker) {
        display: none;
    }

    .nexus-rich-text :global(.nexus-rich-disclosure > summary::before) {
        border-color: transparent transparent transparent currentColor;
        border-style: solid;
        border-width: 0.32em 0 0.32em 0.48em;
        content: "";
        flex: 0 0 auto;
        transform-origin: 35% 50%;
        transition: transform 120ms ease;
    }

    .nexus-rich-text :global(.nexus-rich-disclosure[open] > summary::before) {
        transform: rotate(90deg);
    }

    .nexus-rich-text :global(.nexus-rich-disclosure > summary:focus-visible) {
        outline: 2px solid rgba(120, 217, 244, 0.55);
        outline-offset: -2px;
    }

    .nexus-rich-text :global(.nexus-rich-disclosure > div) {
        border-top: 1px solid rgba(255, 255, 255, 0.08);
        padding: 0 0.75em 0.7em;
    }

    .nexus-rich-text :global(.nexus-rich-disclosure > div > :first-child) {
        margin-top: 0.55em;
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
        min-width: 0;
        overflow-wrap: anywhere;
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
        min-width: 0;
        overflow-wrap: anywhere;
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
        overflow-wrap: anywhere;
        padding: 0.45em 0.55em;
        text-align: left;
        vertical-align: top;
        word-break: break-word;
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

    .nexus-rich-text :global(.nexus-rich-definition-list) {
        display: grid;
        gap: 0.55em;
        margin: 0.8em 0 0;
    }

    .nexus-rich-text :global(.nexus-rich-definition-list > div) {
        border-left: 2px solid rgba(120, 217, 244, 0.2);
        display: grid;
        gap: 0.22em;
        min-width: 0;
        padding-left: 0.7em;
    }

    .nexus-rich-text :global(.nexus-rich-definition-list dt) {
        color: #eefcff;
        font-weight: 900;
    }

    .nexus-rich-text :global(.nexus-rich-definition-list dd) {
        color: #c6d0d9;
        margin: 0;
        min-width: 0;
    }

    .nexus-rich-text :global(.nexus-rich-definition-list dd p:first-child) {
        margin-top: 0;
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

    .author-instructions {
        border-top: 1px solid rgba(255, 255, 255, 0.1);
        display: flex;
        flex-direction: column;
        gap: 0.45em;
        margin-top: 0.65em;
        padding-top: 0.55em;
    }

    .author-instructions-head {
        align-items: center;
        display: flex;
        gap: 0.65em;
        justify-content: space-between;
        min-width: 0;
    }

    .author-instructions-head b {
        background: rgba(120, 217, 244, 0.08);
        border: 1px solid rgba(120, 217, 244, 0.22);
        color: #bdeefa;
        flex: 0 0 auto;
        font-size: 0.72em;
        font-weight: 900;
        padding: 0.2em 0.5em;
        text-transform: uppercase;
    }

    .author-instruction-list {
        display: grid;
        gap: 0.38em;
        max-height: clamp(9em, 18vh, 14em);
        min-height: 0;
        overflow-y: auto;
    }

    .author-instruction-row {
        background: rgba(255, 255, 255, 0.035);
        border: 1px solid rgba(120, 217, 244, 0.16);
        box-shadow: none;
        display: grid;
        gap: 0.18em;
        line-height: 1.24;
        margin: 0;
        min-width: 0;
        padding: 0.48em 0.58em;
        text-align: left;
        text-transform: none;
        -webkit-mask-image: none;
        mask-image: none;
    }

    .author-instruction-row small {
        color: #8d99a5;
        font-size: 0.68em;
        font-weight: 900;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        text-transform: uppercase;
        white-space: nowrap;
    }

    .author-instruction-row span {
        color: #dce4ea;
        font-size: 0.78em;
        font-weight: 900;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .author-instruction-row p {
        color: #aeb9c2;
        font-size: 0.74em;
        font-weight: 700;
        margin: 0;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .author-instruction-warning {
        border-color: rgba(253, 198, 109, 0.35);
    }

    .author-instruction-requirement {
        border-color: rgba(98, 240, 155, 0.22);
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

    .detail-conflict-head > div {
        display: flex;
        flex: 1 1 auto;
        flex-direction: column;
        gap: 0.16em;
        min-width: 0;
    }

    .detail-conflict-head small {
        color: #9aa5af;
        font-size: 0.72em;
        font-weight: 700;
        line-height: 1.22;
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

    .file-picker-head {
        align-items: center;
        display: flex;
        gap: 0.65em;
        justify-content: space-between;
        min-width: 0;
    }

    .file-picker-head .detail-section-title {
        min-width: 0;
    }

    .use-recommended-file-btn {
        color: #62f09b;
        flex: 0 0 auto;
        font-size: 0.72em;
        margin: 0;
        min-height: 2.15em;
        min-width: 9.2em;
        padding: 0.35em 0.55em;
    }

    .dependency-readiness-grid,
    .file-readiness-grid {
        display: grid;
        gap: 0.45em;
        grid-template-columns: repeat(auto-fit, minmax(8.4em, 1fr));
    }

    .file-filter-row {
        display: grid;
        gap: 0.35em;
        grid-template-columns: repeat(4, minmax(0, 1fr));
    }

    .file-filter-row button {
        -webkit-mask-image: none;
        align-items: center;
        background: rgba(18, 18, 18, 0.78);
        border: 1px solid rgba(255, 255, 255, 0.12);
        box-shadow: none;
        color: #9fb0bf;
        display: inline-flex;
        font-size: 0.68em;
        font-weight: 900;
        justify-content: center;
        margin: 0;
        mask-image: none;
        min-height: 2.2em;
        min-width: 0;
        padding: 0.28em 0.45em;
        text-transform: uppercase;
        white-space: nowrap;
    }

    .file-filter-row button:disabled {
        cursor: default;
        opacity: 0.42;
    }

    .file-filter-row button:not(:disabled):hover,
    .file-filter-row .file-filter-selected {
        border-color: rgba(98, 240, 155, 0.42);
        color: #62f09b;
    }

    .file-filter-row b {
        color: inherit;
        font-size: 0.96em;
        margin-left: 0.35em;
    }

    .dependency-readiness-chip,
    .file-readiness-chip {
        align-content: start;
        background: rgba(255, 255, 255, 0.045);
        border: 1px solid rgba(255, 255, 255, 0.11);
        box-sizing: border-box;
        display: grid;
        gap: 0.16em;
        grid-template-columns: auto minmax(0, 1fr);
        min-height: 5.1em;
        min-width: 0;
        padding: 0.45em 0.5em;
    }

    .dependency-readiness-chip small,
    .file-readiness-chip small {
        color: #88939e;
        font-size: 0.68em;
        font-weight: 900;
        grid-column: 1 / -1;
        letter-spacing: 0;
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
        grid-column: 1 / -1;
        line-height: 1.25;
        min-width: 0;
        overflow-wrap: anywhere;
        white-space: normal;
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

    .changelog-compact-note {
        align-items: center;
        color: #8d99a5;
        display: flex;
        flex: 0 0 auto;
        gap: 0.65em;
        justify-content: space-between;
        min-height: 2.7em;
    }

    .changelog-compact-note .dependency-empty {
        margin: 0;
        text-align: right;
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

    .file-row-list {
        display: grid;
        gap: 0.45em;
        grid-template-columns: repeat(auto-fit, minmax(min(100%, 16.75em), 1fr));
        min-width: 0;
    }

    .file-row {
        background: rgba(44, 44, 44, 0.9);
        border: 1px solid rgba(255, 255, 255, 0.12);
        box-shadow: none;
        display: flex;
        flex-direction: column;
        gap: 0.16em;
        margin: 0;
        min-height: 3.8em;
        padding: 0.48em 0.6em;
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
        display: -webkit-box;
        font-weight: 900;
        line-height: 1.18;
        min-width: 0;
        overflow: hidden;
        overflow-wrap: anywhere;
        text-overflow: ellipsis;
        white-space: normal;
        line-clamp: 2;
        -webkit-box-orient: vertical;
        -webkit-line-clamp: 2;
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

    .author-requirement-detected {
        background: rgba(98, 240, 155, 0.055);
        border-color: rgba(98, 240, 155, 0.2);
    }

    .author-requirement-warning {
        background: rgba(253, 198, 109, 0.06);
        border-color: rgba(253, 198, 109, 0.24);
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

    .author-requirement-main .author-requirement-excerpt {
        color: #b9c6cf;
        font-size: 0.72em;
        font-style: italic;
        line-height: 1.25;
    }

    .author-requirement-detected .author-requirement-main span,
    .author-requirement-detected .author-requirement-main small {
        color: #9edeb9;
    }

    .author-requirement-warning .author-requirement-main span,
    .author-requirement-warning .author-requirement-main small {
        color: #f1cf92;
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

    .dependency-actions .dependency-primary-action {
        color: #78d9f4;
        min-width: 7.2em;
    }

    .dependency-actions .dependency-primary-warn {
        color: #fdc66d;
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

        .deployment-check-item {
            min-height: 2.8em;
            padding: 0.32em 0.45em;
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

        .deployment-checklist {
            grid-template-columns: repeat(2, minmax(0, 1fr));
        }

        .file-filter-row {
            grid-template-columns: repeat(2, minmax(0, 1fr));
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
