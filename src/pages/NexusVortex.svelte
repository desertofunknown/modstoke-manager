<script lang="ts">
    import { onDestroy, onMount } from "svelte";
    import SvgSpinnersBlocksWave from "~icons/svg-spinners/blocks-wave";
    import {
        clearNexusApiKey,
        endorseNexusSotfMod,
        fetchNexusFileDependencies,
        fetchNexusModDetails,
        fetchNexusModFiles,
        fetchNexusSotfMods,
        fetchNexusUserEndorsements,
        getNexusNxmUrl,
        getNexusModDownloadUrl,
        getNexusModPageUrl,
        getNexusSession,
        NEXUS_CACHE_TTL_MINUTES,
        pickRecommendedNexusFile,
        saveNexusApiKey,
        type NexusCategory,
        type NexusEndorsement,
        type NexusModDependency,
        type NexusModFile,
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
    import { Command } from "@tauri-apps/plugin-shell";

    type CatalogMode = "online" | "installed";

    let session: NexusSession = { is_connected: false };
    let apiKey = "";
    let mods: NexusMod[] = [];
    let nexusCategories: NexusCategory[] = [];
    let endorsements: NexusEndorsement[] = [];
    let endorsementsLoaded = false;
    let inventory: InstalledInventoryEntry[] = [];
    let selectedView: NexusView = "all";
    let catalogMode: CatalogMode = "online";
    let nexusSearchTerm = "";
    let selectedNexusCategory = "all";
    let selectedInstallFilter = "all";
    let isLoading = false;
    let isDetailLoading = false;
    let status = "";
    let vortexStagingPath: string | null = null;
    let selectedMod: NexusMod | null = null;
    let selectedModDetails: NexusMod | null = null;
    let selectedModFiles: NexusModFile[] = [];
    let selectedFileId: number | null = null;
    let selectedDependencies: NexusModDependency[] = [];
    let activeNexusActionId: number | null = null;
    let activeNexusEndorseId: number | null = null;
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
    $: visibleNexusMods = mods.filter(matchesNexusFilters);
    $: visibleInstalledEntries = inventory.filter(matchesInstalledFilters);
    $: installedCount = inventory.length;
    $: vortexCount = inventory.filter(entry => entry.installSource === "vortex").length;
    $: nativeCount = inventory.filter(entry => entry.installSource === "native").length;
    $: manualCount = inventory.filter(entry => entry.installSource === "manual").length;
    $: selectedNexusFile = selectedModFiles.find(file => file.file_id === selectedFileId) ?? null;

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
        isDetailLoading = true;

        try {
            const [details, fileResponse] = await Promise.all([
                fetchNexusModDetails(mod.mod_id).catch(() => mod),
                fetchNexusModFiles(mod.mod_id)
            ]);
            selectedModDetails = { ...mod, ...details };
            selectedModFiles = fileResponse.files;
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

        if (selectedInstallFilter === "missing") {
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
            <div class="summary-card">
                <span class="summary-label">Installed</span>
                <span class="summary-value">{installedCount}</span>
            </div>
            <div class="summary-card">
                <span class="summary-label">Vortex / Native / Manual</span>
                <span class="summary-value">{vortexCount} / {nativeCount} / {manualCount}</span>
            </div>
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
                    <span>{visibleNexusMods.length} shown from {mods.length} loaded.</span>
                {:else}
                    <span>{visibleInstalledEntries.length} shown from {installedCount} installed.</span>
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
                    <option value="installed">Installed</option>
                    <option value="missing">Not installed</option>
                    <option value="vortex">Vortex</option>
                    <option value="native">OpenACAI store</option>
                    <option value="manual">Manual</option>
                </select>
            </div>

            <div class="nexus-scroller" aria-live="polite">
                {#if catalogMode === "online"}
                    {#each visibleNexusMods as mod}
                        {@const match = installedMatch(mod)}
                        <article class="nexus-card" class:nexus-installed={!!match}>
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
                                    <span class="source-pill" class:source-vortex={match?.installSource === "vortex"} class:source-native={match?.installSource === "native"} class:source-manual={match?.installSource === "manual"}>
                                        {describeInstallSource(match)}
                                    </span>
                                </div>

                                <button class="description-content description-button" on:click={() => openModDetails(mod)}>{mod.summary ?? "No summary is available from Nexus for this mod."}</button>

                                <div class="facts">
                                    <span>Version <b>{mod.version ?? "-"}</b></span>
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
                                        <span class="match-detail">{loaderTypeLabel(match)} in {match.expectedLocation}</span>
                                    {:else}
                                        <span class="match-detail missing-match">Not installed in this game folder.</span>
                                    {/if}

                                    <div class="button-row">
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
                        <article class="nexus-card inventory-card" class:nexus-installed={entry.enabled}>
                            <div class="inventory-icon">
                                <span>{entry.loaderType === "bepinex-plugin" ? "BEP" : "RED"}</span>
                            </div>

                            <div class="nexus-body">
                                <div class="card-head">
                                    <div class="title-stack">
                                        <span class="mod-title">{entry.name}</span>
                                        <span class="mod-byline">{loaderTypeLabel(entry)} · {entry.author ?? "Unknown author"}</span>
                                    </div>
                                    <span class="source-pill" class:source-vortex={entry.installSource === "vortex"} class:source-native={entry.installSource === "native"} class:source-manual={entry.installSource === "manual"}>
                                        {describeInstallSource(entry)}
                                    </span>
                                </div>

                                <div class="facts inventory-facts">
                                    <span>Version <b>{entry.version ?? "-"}</b></span>
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
                                    <span class="match-detail">{entry.installSource === "vortex" ? "Managed by Vortex deployment metadata" : "Managed by OpenACAI Mod Manager"}</span>

                                    <div class="button-row">
                                        <button on:click={() => openInventoryLocation(entry)}>Open Folder</button>
                                        {#if entryNexusModId}
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
                </div>

                <div class="detail-side">
                    <div class="detail-facts">
                        <span>Version <b>{selectedModDetails?.version ?? "-"}</b></span>
                        <span>Type <b>{selectedModDetails?.loader_type ?? "Unknown"}</b></span>
                        <span>Updated <b>{formatTimestamp(selectedModDetails?.updated_timestamp, selectedModDetails?.updated_time)}</b></span>
                        <span>Downloads <b>{formatNumber(selectedModDetails?.mod_downloads)}</b></span>
                        <span>Installed <b>{describeInstallSource(installedMatch(selectedMod))}</b></span>
                    </div>

                    <div class="file-picker">
                        <span class="detail-section-title">Files</span>
                        {#if selectedModFiles.length === 0 && !isDetailLoading}
                            <div class="notice empty-nexus">No downloadable files were returned by Nexus.</div>
                        {/if}

                        {#each selectedModFiles as file}
                            <button class="file-row" class:file-row-selected={selectedFileId === file.file_id} on:click={() => selectNexusFile(file.file_id)}>
                                <span class="file-name">{file.name}</span>
                                <span class="file-meta">{file.category_name ?? "file"} · v{file.version ?? file.mod_version ?? "-"} · {formatSizeKb(file.size)}</span>
                            </button>
                        {/each}
                    </div>

                    <div class="dependency-box">
                        <span class="detail-section-title">Dependencies</span>
                        {#if selectedDependencies.length === 0}
                            <span class="dependency-empty">No API-listed dependencies for the selected file. Still review the author directions for manual requirements.</span>
                        {:else}
                            {#each selectedDependencies as dependency}
                                <div class="dependency-row">
                                    <span>{dependency.mod_name}</span>
                                    <small>{dependency.file_name ?? dependency.group_name ?? "Candidate file"} {dependency.version ? `· v${dependency.version}` : ""}</small>
                                </div>
                            {/each}
                        {/if}
                    </div>

                    <div class="detail-actions">
                        <button class="install" disabled={!selectedNexusFile} on:click={installSelectedFileWithVortex}>Install Selected With Vortex</button>
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
        grid-template-columns: repeat(4, minmax(0, 1fr));
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
        grid-template-columns: minmax(16em, 1fr) minmax(10em, 0.55fr) minmax(10em, 0.55fr);
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
        grid-template-columns: repeat(4, minmax(0, 1fr));
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
        grid-template-columns: repeat(4, minmax(0, 1fr));
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
    .file-picker,
    .dependency-box,
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

    .file-picker,
    .dependency-box {
        flex: 1 1 0;
        min-height: 0;
        overflow-y: auto;
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

    .file-name {
        color: #eefcff;
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
