<script lang="ts">
    import { onDestroy, onMount } from "svelte";
    import SvgSpinnersBlocksWave from "~icons/svg-spinners/blocks-wave";
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

    type CatalogMode = "online" | "installed";
    type InstallFilter = "all" | "attention" | "installed" | "missing" | "updates" | "disabled" | "vortex" | "native" | "manual" | "tracked" | "conflicts";
    type NexusSortMode = "attention" | "updated" | "downloads" | "endorsements" | "name" | "version";
    type InstalledSortMode = "attention" | "name" | "source" | "location" | "state" | "version";
    type DependencyStatus = "installed" | "missing" | "version-mismatch" | "review";
    type InstallPlanTone = "ready" | "review" | "blocked";
    type ResolvedDependency = NexusModDependency & {
        match: InstalledInventoryEntry | null;
        status: DependencyStatus;
    };
    type LocalConflict = {
        key: string;
        label: string;
        entries: InstalledInventoryEntry[];
    };
    type InstallPlan = {
        action: string;
        target: string;
        tone: InstallPlanTone;
        notes: string[];
    };

    let session: NexusSession = { is_connected: false };
    let apiKey = "";
    let mods: NexusMod[] = [];
    let nexusCategories: NexusCategory[] = [];
    let endorsements: NexusEndorsement[] = [];
    let endorsementsLoaded = false;
    let trackedMods: NexusTrackedMod[] = [];
    let trackedModsLoaded = false;
    let inventory: InstalledInventoryEntry[] = [];
    let selectedView: NexusView = "all";
    let catalogMode: CatalogMode = "online";
    let nexusSearchTerm = "";
    let selectedNexusCategory = "all";
    let selectedInstallFilter: InstallFilter = "all";
    let selectedNexusSort: NexusSortMode = "attention";
    let selectedInstalledSort: InstalledSortMode = "attention";
    let isLoading = false;
    let isDetailLoading = false;
    let status = "";
    let vortexStagingPath: string | null = null;
    let selectedMod: NexusMod | null = null;
    let selectedModDetails: NexusMod | null = null;
    let selectedModFiles: NexusModFile[] = [];
    let selectedFileId: number | null = null;
    let selectedDependencies: NexusModDependency[] = [];
    let selectedChangelogs: NexusModChangelog[] = [];
    let selectedInstallMatch: InstalledInventoryEntry | null = null;
    let selectedInstallConflict: LocalConflict | null = null;
    let selectedInstallPlan: InstallPlan = {
        action: "Choose a file",
        target: "-",
        tone: "blocked",
        notes: ["Select a Nexus file before handing it to Vortex."]
    };
    let selectedInstallActionButtonLabel = "Choose File";
    let activeNexusActionId: number | null = null;
    let activeNexusEndorseId: number | null = null;
    let activeNexusTrackId: number | null = null;
    let autoEndorseDownloadedMods = false;
    let autoEndorseAttemptedIds: number[] = [];
    let ssoSocket: WebSocket | null = null;
    let ssoTimeout: number | null = null;

    const NEXUS_SSO_URL = "wss://sso.nexusmods.com";
    const NEXUS_SSO_APPLICATION_SLUG = "openacai-mod-manager";
    const NEXUS_SSO_PROTOCOL = 2;
    const NEXUS_SSO_UUID_KEY = "openacai-nexus-sso-request-id";
    const NEXUS_SSO_TOKEN_KEY = "openacai-nexus-sso-connection-token";
    const NEXUS_AUTO_ENDORSE_KEY = "openacai-nexus-auto-endorse-downloaded";
    const NEXUS_AUTO_ENDORSE_ATTEMPTED_KEY = "openacai-nexus-auto-endorse-attempted";
    const MAX_AUTO_ENDORSE_PER_REFRESH = 3;
    const NEXUS_MANUAL_REFRESH_COOLDOWN_MS = 60_000;
    let nextManualRefreshAt = 0;

    $: nexusCategoryOptions = buildNexusCategoryOptions(nexusCategories, mods);
    $: if (selectedNexusCategory !== "all" && !nexusCategoryOptions.includes(selectedNexusCategory)) {
        selectedNexusCategory = "all";
    }
    $: visibleNexusMods = sortNexusMods(mods.filter(matchesNexusFilters));
    $: visibleInstalledEntries = sortInstalledEntries(inventory.filter(matchesInstalledFilters));
    $: installedCount = inventory.length;
    $: vortexCount = inventory.filter(entry => entry.installSource === "vortex").length;
    $: nativeCount = inventory.filter(entry => entry.installSource === "native").length;
    $: manualCount = inventory.filter(entry => entry.installSource === "manual").length;
    $: updateCount = inventory.filter(hasInventoryUpdate).length;
    $: disabledCount = inventory.filter(entry => !entry.enabled).length;
    $: onlineAttentionCount = mods.filter(nexusNeedsAttention).length;
    $: installedAttentionCount = inventory.filter(inventoryNeedsAttention).length;
    $: trackedCount = trackedMods.length;
    $: selectedNexusFile = selectedModFiles.find(file => file.file_id === selectedFileId) ?? null;
    $: recommendedNexusFileId = pickRecommendedNexusFile(selectedModFiles)?.file_id ?? null;
    $: displayedSelectedModFiles = sortNexusFilesForDisplay(selectedModFiles, recommendedNexusFileId);
    $: localConflicts = findLocalConflicts(inventory);
    $: localConflictEntryKeys = new Set(localConflicts.flatMap(conflict => conflict.entries.map(inventoryEntryKey)));
    $: conflictCount = localConflictEntryKeys.size;
    $: resolvedDependencies = selectedDependencies.map(resolveDependencyStatus);
    $: dependencyIssueCount = resolvedDependencies.filter(dependency =>
        dependency.status === "missing" || dependency.status === "version-mismatch"
    ).length;
    $: dependencyReviewCount = resolvedDependencies.filter(dependency => dependency.status === "review").length;
    $: selectedInstallMatch = selectedMod
        ? findMatchingInstall(inventory, selectedMod.name, selectedMod.mod_id, [
            selectedMod.author ?? "",
            selectedMod.uploaded_by ?? ""
        ])
        : null;
    $: selectedInstallConflict = conflictForEntry(selectedInstallMatch);
    $: selectedInstallPlan = buildInstallPlan(
        selectedModDetails ?? selectedMod,
        selectedNexusFile,
        selectedInstallMatch,
        selectedInstallConflict,
        resolvedDependencies,
        vortexStagingPath
    );
    $: selectedInstallActionButtonLabel = selectedInstallPlan.tone === "blocked"
        ? "Choose File"
        : selectedInstallPlan.tone === "review"
            ? `${selectedInstallPlan.action} Anyway`
            : selectedInstallPlan.action;
    $: selectedInstallFileLabel = selectedNexusFile
        ? `${fileChoiceCategoryLabel(selectedNexusFile)} - ${selectedNexusFile.name}`
        : "No file selected";

    onMount(async () => {
        loadEndorsementPreferences();
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
        nexusCategories = [];
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
            nexusCategories = response.categories;
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

    function toggleAutoEndorse(event: Event) {
        autoEndorseDownloadedMods = (event.currentTarget as HTMLInputElement).checked;
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
        selectedInstallFilter = "conflicts";
    }

    async function openModPage(mod: NexusMod) {
        await shell.open(getNexusModPageUrl(mod));
    }

    async function openDownloadPage(mod: NexusMod) {
        await shell.open(getNexusModDownloadUrl(mod));
    }

    async function openInventoryLocation(entry: InstalledInventoryEntry) {
        const target = entry.packagePath ?? entry.assemblyPath ?? await getDirectoryPath();
        await openExternalTarget(target);
    }

    async function openModDetails(mod: NexusMod) {
        selectedMod = mod;
        selectedModDetails = mod;
        selectedModFiles = [];
        selectedFileId = null;
        selectedDependencies = [];
        selectedChangelogs = [];
        isDetailLoading = true;

        try {
            const [details, fileResponse, changelogResponse] = await Promise.all([
                fetchNexusModDetails(mod.mod_id).catch(() => mod),
                fetchNexusModFiles(mod.mod_id),
                fetchNexusModChangelogs(mod.mod_id).catch(() => ({ changelogs: [], rate_limit: session.rate_limit ?? {} }))
            ]);
            selectedModDetails = { ...mod, ...details };
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

    function closeModDetails() {
        selectedMod = null;
        selectedModDetails = null;
        selectedModFiles = [];
        selectedFileId = null;
        selectedDependencies = [];
        selectedChangelogs = [];
    }

    async function selectNexusFile(fileId: number) {
        selectedFileId = fileId;
        await loadDependenciesForFile(fileId);
    }

    async function loadDependenciesForFile(fileId: number) {
        selectedDependencies = [];
        try {
            const response = await fetchNexusFileDependencies(fileId);
            selectedDependencies = response.dependencies;
            session = {
                ...session,
                rate_limit: response.rate_limit
            };
        } catch {
            selectedDependencies = [];
        }
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

        if (dependency.version && !match.version) {
            return { ...dependency, match, status: "review" };
        }

        if (dependency.version && versionsDiffer(match.version, dependency.version)) {
            return { ...dependency, match, status: "version-mismatch" };
        }

        return { ...dependency, match, status: "installed" };
    }

    function dependencySummaryLabel(): string {
        if (resolvedDependencies.length === 0) {
            return "None listed";
        }

        if (dependencyIssueCount > 0) {
            return `${dependencyIssueCount} need attention`;
        }

        if (dependencyReviewCount > 0) {
            return `${dependencyReviewCount} review`;
        }

        return "Ready";
    }

    function dependencyStatusLabel(dependency: ResolvedDependency): string {
        switch (dependency.status) {
            case "installed":
                return dependency.match ? `Installed: ${describeInstallSource(dependency.match)}` : "Installed";
            case "version-mismatch":
                return `Version mismatch: installed ${dependency.match?.version ?? "unknown"}; wants ${dependency.version}`;
            case "review":
                return `Installed: ${describeInstallSource(dependency.match)}; version unknown`;
            default:
                return "Missing from this game folder";
        }
    }

    function buildInstallPlan(
        mod: NexusMod | null,
        file: NexusModFile | null,
        match: InstalledInventoryEntry | null,
        conflict: LocalConflict | null,
        dependencies: ResolvedDependency[],
        stagingPath: string | null): InstallPlan {
        if (!mod || !file) {
            return {
                action: "Choose a file",
                target: "-",
                tone: "blocked",
                notes: ["Select a Nexus file before handing it to Vortex."]
            };
        }

        const target = match?.expectedLocation ?? inferNexusInstallTarget(mod, file);
        const notes: string[] = [];
        const missingCount = dependencies.filter(dependency => dependency.status === "missing").length;
        const mismatchCount = dependencies.filter(dependency => dependency.status === "version-mismatch").length;
        const reviewCount = dependencies.filter(dependency => dependency.status === "review").length;

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

        if (conflict) {
            notes.push(`Local conflict detected across ${conflict.entries.length} matching installs.`);
        }

        if (match && !match.enabled) {
            notes.push("The local match is currently disabled.");
        }

        return {
            action: installPlanAction(mod, match),
            target,
            tone: notes.length > 0 ? "review" : "ready",
            notes: notes.length > 0 ? notes : ["No local blockers detected from API dependency data."]
        };
    }

    function installPlanAction(mod: NexusMod, match: InstalledInventoryEntry | null): string {
        if (!match) {
            return "Install with Vortex";
        }

        if (versionsDiffer(match.version, mod.version)) {
            return "Update with Vortex";
        }

        return match.installSource === "vortex" ? "Repair/redeploy in Vortex" : "Import/update via Vortex";
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

    async function openDependencyLocation(dependency: ResolvedDependency) {
        if (!dependency.match) {
            return;
        }

        await openInventoryLocation(dependency.match);
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
            entry.loaderType
        ].some(value => value.toLowerCase().includes(search))) {
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

        if (selectedInstallFilter === "conflicts" && !isConflictedEntry(entry)) {
            return false;
        }

        if (selectedInstallFilter === "attention" && !inventoryNeedsAttention(entry)) {
            return false;
        }

        return true;
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
            return versionsDiffer(match.version, mod.version) ? "Update in Vortex" : "Reinstall in Vortex";
        }

        return "Get Vortex File";
    }

    function nexusUpdateLabel(mod: NexusMod): string {
        const match = installedMatch(mod);
        if (!match) {
            return isNexusModTracked(mod.mod_id) ? "Tracked" : "Not installed";
        }

        if (versionsDiffer(match.version, mod.version)) {
            return "Update available";
        }

        return match.version && mod.version ? "Current" : "Installed";
    }

    function selectedModUpdateLabel(): string {
        return selectedMod ? nexusUpdateLabel(selectedMod) : "-";
    }

    function selectedModUpdateTone(): "update" | "current" | "tracked" | "neutral" {
        return updateTone(selectedModUpdateLabel());
    }

    function inventoryUpdateLabel(entry: InstalledInventoryEntry): string {
        const onlineMod = findOnlineModForEntry(entry);
        if (!onlineMod) {
            return entry.installSource === "vortex" ? "Refresh Nexus" : "Local only";
        }

        if (versionsDiffer(entry.version, onlineMod.version)) {
            return "Update available";
        }

        return entry.version && onlineMod.version ? "Current" : "Installed";
    }

    function findOnlineModForEntry(entry: InstalledInventoryEntry): NexusMod | null {
        const entryNexusId = numericNexusId(entry.nexusModId);
        return mods.find(mod =>
            (entryNexusId && mod.mod_id === entryNexusId)
            || findMatchingInstall([entry], mod.name, mod.mod_id, [
                mod.author ?? "",
                mod.uploaded_by ?? ""
            ]) !== null
        ) ?? null;
    }

    function updateTone(label: string): "update" | "current" | "tracked" | "neutral" {
        if (label === "Update available") {
            return "update";
        }

        if (label === "Current") {
            return "current";
        }

        if (label === "Tracked") {
            return "tracked";
        }

        return "neutral";
    }

    function versionsDiffer(left?: string, right?: string): boolean {
        if (!left || !right) {
            return false;
        }

        return left.trim().toLowerCase() !== right.trim().toLowerCase();
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

    function formatNumber(value?: number): string {
        return typeof value === "number" ? value.toLocaleString() : "-";
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

    function plainText(value?: string): string {
        return (value ?? "")
            .replace(/<br\s*\/?>/gi, "\n")
            .replace(/<\/p>/gi, "\n\n")
            .replace(/<[^>]*>/g, " ")
            .replace(/\[img[^\]]*\][\s\S]*?\[\/img\]/gi, " ")
            .replace(/\[url=([^\]]+)\]([\s\S]*?)\[\/url\]/gi, "$2 ($1)")
            .replace(/\[\*\]/g, "\n- ")
            .replace(/\[\/?(?:b|i|u|s|size|color|font|center|left|right|list|quote|spoiler|code)[^\]]*\]/gi, "")
            .replace(/\[\/?[a-z0-9_-]+[^\]]*\]/gi, "")
            .replace(/&#92;/g, "\\")
            .replace(/&amp;/g, "&")
            .replace(/&lt;/g, "<")
            .replace(/&gt;/g, ">")
            .replace(/[ \t]+/g, " ")
            .replace(/\n\s+/g, "\n")
            .replace(/\n{3,}/g, "\n\n")
            .trim();
    }
</script>

<div class="column nexus-page">
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
                    <span>Auto-endorse Vortex downloads</span>
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
            <span>Hourly {session.rate_limit.hourly_remaining ?? "-"} / {session.rate_limit.hourly_limit ?? "-"}</span>
            <span>Daily {session.rate_limit.daily_remaining ?? "-"} / {session.rate_limit.daily_limit ?? "-"}</span>
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
            <button type="button" class="summary-card summary-action" on:click={showInstalledInventory}>
                <span class="summary-label">Installed</span>
                <span class="summary-value">{installedCount}</span>
            </button>
            <button type="button" class="summary-card summary-action" class:attention-summary={(catalogMode === "online" ? onlineAttentionCount : installedAttentionCount) > 0} on:click={showCurrentAttention}>
                <span class="summary-label">Attention</span>
                <span class="summary-value">{catalogMode === "online" ? onlineAttentionCount : installedAttentionCount}</span>
                <span class="summary-note">{catalogMode === "online" ? "Tracked missing, updates, conflicts" : "Updates, disabled, conflicts"}</span>
            </button>
            <div class="summary-card">
                <span class="summary-label">Vortex / Native / Manual</span>
                <span class="summary-value">{vortexCount} / {nativeCount} / {manualCount}</span>
            </div>
            <button type="button" class="summary-card summary-action" class:update-summary={updateCount > 0 || disabledCount > 0} on:click={showUpdatesOrDisabled}>
                <span class="summary-label">Updates / Disabled</span>
                <span class="summary-value">{updateCount} / {disabledCount}</span>
                <span class="summary-note">Local deployment state</span>
            </button>
            <button type="button" class="summary-card summary-action" class:conflict-summary={conflictCount > 0} on:click={showLocalConflicts}>
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
                <button class="cat-btn refresh-btn" disabled={isLoading} on:click={() => loadMods(selectedView, true)}>Refresh</button>
            </div>

            <div class="notice api-note">
                <span>Nexus requests are cached locally for {NEXUS_CACHE_TTL_MINUTES} minutes.</span>
                {#if catalogMode === "online"}
                    <span>{visibleNexusMods.length} shown from {mods.length} loaded. {onlineAttentionCount > 0 ? `${onlineAttentionCount} need attention.` : ""} {trackedModsLoaded ? `${trackedCount} tracked.` : ""} {conflictCount > 0 ? `${conflictCount} in conflicts.` : ""}</span>
                {:else}
                    <span>{visibleInstalledEntries.length} shown from {installedCount} installed. {installedAttentionCount > 0 ? `${installedAttentionCount} need attention.` : ""} {trackedModsLoaded ? `${trackedCount} tracked.` : ""} {conflictCount > 0 ? `${conflictCount} in conflicts.` : ""}</span>
                {/if}
            </div>

            <div class="nexus-filter-row">
                <input class="generic-input key-input" bind:value={nexusSearchTerm} placeholder="Search Nexus" />
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
                    <option value="conflicts">Conflicts</option>
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
            </div>

            <div class="nexus-scroller" aria-live="polite">
                {#if catalogMode === "online"}
                    {#each visibleNexusMods as mod}
                        {@const match = installedMatch(mod)}
                        {@const updateLabel = nexusUpdateLabel(mod)}
                        {@const updateToneValue = updateTone(updateLabel)}
                        {@const conflict = conflictForEntry(match)}
                        <article class="nexus-card" class:nexus-installed={!!match} class:nexus-conflict={!!conflict}>
                            <button class="thumbnail-button" aria-label={`Open ${mod.name} details`} on:click={() => openModDetails(mod)}>
                                <img
                                    class="nexus-img"
                                    src={mod.picture_url ?? "https://placehold.co/320x180/252525/FFF?text=Nexus"}
                                    alt=""
                                />
                            </button>

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
                                    <span>State <b class:update-state-update={updateToneValue === "update"} class:update-state-current={updateToneValue === "current"} class:update-state-tracked={updateToneValue === "tracked"}>{updateLabel}</b></span>
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
                        {@const inventoryUpdate = inventoryUpdateLabel(entry)}
                        {@const inventoryTone = updateTone(inventoryUpdate)}
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
                                    <span>Update <b class:update-state-update={inventoryTone === "update"} class:update-state-current={inventoryTone === "current"}>{inventoryUpdate}</b></span>
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
                <button class="close-detail" on:click={closeModDetails}>Close</button>
            </div>

            {#if isDetailLoading}
                <div class="loading-line">
                    <SvgSpinnersBlocksWave />
                    <span>Loading mod details...</span>
                </div>
            {/if}

            <div class="detail-grid">
                <div class="detail-main">
                    <img
                        class="detail-img"
                        src={selectedModDetails?.picture_url ?? selectedMod.picture_url ?? "https://placehold.co/640x360/252525/FFF?text=Nexus"}
                        alt=""
                    />

                    <div class="detail-text">
                        <span class="detail-section-title">Directions / Description</span>
                        <p>{plainText(selectedModDetails?.description ?? selectedModDetails?.summary ?? selectedMod.summary) || "No directions or description are available through the Nexus API for this mod. Open the Nexus page to review author instructions before installing."}</p>
                    </div>

                    <div class="changelog-box">
                        <span class="detail-section-title">Changelog</span>
                        {#if selectedChangelogs.length === 0}
                            <span class="dependency-empty">No API-listed changelog entries were returned for this mod.</span>
                        {:else}
                            {#each selectedChangelogs as changelog}
                                <div class="changelog-row">
                                    <span>{changelog.version}</span>
                                    <p>{plainText(changelog.changes)}</p>
                                </div>
                            {/each}
                        {/if}
                    </div>
                </div>

                <div class="detail-side">
                    <div class="detail-facts">
                        <span>Version <b>{selectedModDetails?.version ?? "-"}</b></span>
                        <span>Type <b>{selectedModDetails?.loader_type ?? "Unknown"}</b></span>
                        <span>Update <b class:update-state-update={selectedModUpdateTone() === "update"} class:update-state-current={selectedModUpdateTone() === "current"} class:update-state-tracked={selectedModUpdateTone() === "tracked"}>{selectedModUpdateLabel()}</b></span>
                        <span>Tracked <b>{isNexusModTracked(selectedMod.mod_id) ? "Yes" : "No"}</b></span>
                        <span>Updated <b>{formatTimestamp(selectedModDetails?.updated_timestamp, selectedModDetails?.updated_time)}</b></span>
                        <span>Downloads <b>{formatNumber(selectedModDetails?.mod_downloads)}</b></span>
                        <span>Installed <b>{describeInstallSource(installedMatch(selectedMod))}</b></span>
                        <span>Dependencies <b class:update-state-update={dependencyIssueCount > 0} class:update-state-tracked={dependencyReviewCount > 0 && dependencyIssueCount === 0} class:update-state-current={resolvedDependencies.length > 0 && dependencyIssueCount === 0 && dependencyReviewCount === 0}>{dependencySummaryLabel()}</b></span>
                    </div>

                    <div
                        class="install-plan"
                        class:install-plan-ready={selectedInstallPlan.tone === "ready"}
                        class:install-plan-review={selectedInstallPlan.tone === "review"}
                        class:install-plan-blocked={selectedInstallPlan.tone === "blocked"}
                    >
                        <div class="install-plan-head">
                            <span class="detail-section-title">Install Plan</span>
                            <b>{installPlanToneLabel(selectedInstallPlan.tone)}</b>
                        </div>
                        <div class="install-plan-facts">
                            <span>Action <b>{selectedInstallPlan.action}</b></span>
                            <span>Target <b>{selectedInstallPlan.target}</b></span>
                            <span class="install-plan-file-fact">File <b>{selectedInstallFileLabel}</b></span>
                        </div>
                        <div class="install-plan-notes">
                            {#each selectedInstallPlan.notes as note}
                                <span>{note}</span>
                            {/each}
                        </div>
                    </div>

                    <div class="file-picker">
                        <span class="detail-section-title">Files</span>
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
                                <span class="file-meta">{fileChoiceCategoryLabel(file)} · v{file.version ?? file.mod_version ?? "-"} · {formatSizeKb(file.size)}</span>
                            </button>
                        {/each}
                    </div>

                    <div class="dependency-box">
                        <span class="detail-section-title">Dependencies</span>
                        {#if resolvedDependencies.length === 0}
                            <span class="dependency-empty">No API-listed dependencies for the selected file. Still review the author directions for manual requirements.</span>
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
                                    <small>{dependency.file_name ?? dependency.group_name ?? "Candidate file"} {dependency.version ? `· v${dependency.version}` : ""}</small>
                                    <small>{dependencyStatusLabel(dependency)}</small>
                                    <div class="dependency-actions">
                                        {#if dependency.mod_id}
                                            <button on:click={() => openDependencyPage(dependency)}>Nexus</button>
                                        {/if}
                                        {#if dependency.match}
                                            <button on:click={() => openDependencyLocation(dependency)}>Open Folder</button>
                                        {/if}
                                    </div>
                                </div>
                            {/each}
                        {/if}
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
                        <button on:click={openSelectedModPage}>Open Nexus Page</button>
                        <button on:click={openSelectedDownloadPage}>Open Files Page</button>
                    </div>
                </div>
            </div>
        </section>
    </div>
{/if}

<style>
    .nexus-page {
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
        padding: 0 0.7em;
        text-transform: uppercase;
        white-space: nowrap;
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
        flex: 0 0 auto;
        font-size: 0.78em;
        justify-content: flex-end;
        line-height: 1;
        margin-top: -0.15em;
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

    .summary-label {
        color: #8d99a5;
        font-size: 0.68em;
        font-weight: 900;
        letter-spacing: 0.1em;
        overflow: hidden;
        text-overflow: ellipsis;
        text-transform: uppercase;
        white-space: nowrap;
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
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
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
        flex: 1 1 auto;
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

    .nexus-filter-row {
        display: grid;
        flex: 0 0 auto;
        gap: 0.6em;
        grid-template-columns: minmax(16em, 1fr) repeat(3, minmax(10em, 0.55fr));
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
        height: 2.55em;
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
        display: flex;
        flex: 1 1 auto;
        flex-direction: column;
        gap: 0.65em;
        min-height: 0;
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
        grid-template-columns: clamp(145px, 18vw, 230px) minmax(0, 1fr);
        min-height: clamp(138px, 17vh, 178px);
        overflow: hidden;
    }

    .nexus-installed {
        border-left: 3px solid #62f09b;
    }

    .nexus-conflict {
        border-left: 3px solid #fdc66d;
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
    }

    .nexus-img {
        background: #252525;
        display: block;
        height: 100%;
        min-height: clamp(138px, 17vh, 178px);
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
        background:
            linear-gradient(180deg, rgba(18, 18, 18, 0.96), rgba(6, 6, 6, 0.94)),
            rgba(0, 0, 0, 0.92);
        border: 1px solid rgba(255, 255, 255, 0.18);
        box-sizing: border-box;
        display: flex;
        flex-direction: column;
        gap: 0.8em;
        max-height: min(86vh, 820px);
        max-width: min(92vw, 1180px);
        min-height: min(70vh, 720px);
        padding: clamp(1em, 2vh, 1.35em);
        width: 100%;
    }

    .detail-header,
    .detail-actions {
        align-items: center;
        display: flex;
        gap: 0.75em;
        justify-content: space-between;
    }

    .detail-title {
        display: flex;
        flex-direction: column;
        min-width: 0;
    }

    .close-detail {
        color: #fd9b9d;
        min-width: 7em;
    }

    .detail-grid {
        display: grid;
        flex: 1 1 auto;
        gap: 1em;
        grid-template-columns: minmax(0, 1fr) minmax(300px, 0.45fr);
        min-height: 0;
    }

    .detail-main,
    .detail-side {
        display: flex;
        flex-direction: column;
        gap: 0.75em;
        min-height: 0;
        min-width: 0;
    }

    .detail-img {
        background: #151515;
        border: 1px solid rgba(255, 255, 255, 0.12);
        height: clamp(170px, 28vh, 300px);
        object-fit: cover;
        width: 100%;
    }

    .detail-text,
    .changelog-box,
    .file-picker,
    .dependency-box,
    .install-plan,
    .detail-facts {
        background: rgba(18, 18, 18, 0.88);
        border: 1px solid rgba(255, 255, 255, 0.12);
        box-sizing: border-box;
        padding: 0.75em;
    }

    .detail-text {
        flex: 1 1 auto;
        min-height: 0;
        overflow-y: auto;
    }

    .detail-text p {
        color: #b8c0c8;
        font-size: 0.9em;
        line-height: 1.45;
        margin: 0.55em 0 0;
        white-space: pre-wrap;
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

    .install-plan-facts {
        color: #8d99a5;
        display: grid;
        font-size: 0.76em;
        gap: 0.35em 0.75em;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        margin-top: 0.55em;
    }

    .install-plan-facts span,
    .install-plan-facts b,
    .install-plan-notes span {
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
    }

    .install-plan-ready .install-plan-notes span {
        color: #98d9af;
    }

    .file-picker,
    .dependency-box {
        flex: 1 1 0;
        min-height: 0;
        overflow-y: auto;
    }

    .changelog-box {
        flex: 0 1 auto;
        max-height: clamp(120px, 20vh, 220px);
        min-height: 0;
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

    .changelog-row p {
        color: #b8c0c8;
        font-size: 0.82em;
        line-height: 1.35;
        margin: 0.3em 0 0;
        white-space: pre-wrap;
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

    .detail-actions {
        flex: 0 0 auto;
        justify-content: flex-end;
    }

    @media (max-width: 1120px) {
        .vortex-summary {
            grid-template-columns: repeat(2, minmax(0, 1fr));
        }

        .facts {
            grid-template-columns: repeat(2, minmax(0, 1fr));
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
        .vortex-summary {
            grid-template-columns: 1fr;
        }

        .nexus-card {
            grid-template-columns: 1fr;
        }

        .nexus-img {
            height: clamp(130px, 22vh, 190px);
        }

        .source-pill {
            max-width: 100%;
        }

        .button-row {
            margin-left: 0;
            width: 100%;
        }

        .detail-grid {
            grid-template-columns: 1fr;
            overflow-y: auto;
        }

        .detail-panel {
            max-height: 92vh;
        }
    }
</style>
