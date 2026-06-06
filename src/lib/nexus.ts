import { invoke } from "@tauri-apps/api/core";

export type NexusUser = {
    user_id?: number;
    name?: string;
    profile_url?: string;
    is_premium?: boolean;
    is_supporter?: boolean;
};

export type NexusRateLimit = {
    hourly_limit?: string;
    hourly_remaining?: string;
    hourly_reset?: string;
    daily_limit?: string;
    daily_remaining?: string;
    daily_reset?: string;
};

export type NexusSession = {
    is_connected: boolean;
    user?: NexusUser;
    rate_limit?: NexusRateLimit;
    error?: string;
};

export type NexusMod = {
    mod_id: number;
    name: string;
    summary?: string;
    description?: string;
    version?: string;
    author?: string;
    uploaded_by?: string;
    picture_url?: string;
    category_name?: string;
    endorsement_count?: number;
    mod_downloads?: number;
    mod_unique_downloads?: number;
    created_timestamp?: number;
    updated_timestamp?: number;
    created_time?: string;
    updated_time?: string;
    loader_type?: string;
};

export type NexusModFile = {
    file_id: number;
    name: string;
    version?: string;
    mod_version?: string;
    category_name?: string;
    description?: string;
    changelog_html?: string;
    size?: number;
    is_primary?: boolean;
    uploaded_timestamp?: number;
    uploaded_time?: string;
};

export type NexusModDependency = {
    id: string;
    mod_id?: number;
    file_id?: number;
    mod_name: string;
    file_name?: string;
    version?: string;
    group_name?: string;
};

type RawNexusModsResponse = {
    mods: Array<NexusMod & Record<string, unknown>> | { data?: Array<NexusMod & Record<string, unknown>> };
    rate_limit: NexusRateLimit;
};

type NexusModsResponse = {
    mods: NexusMod[];
    rate_limit: NexusRateLimit;
};

type RawNexusModDetailsResponse = {
    details: NexusMod & Record<string, unknown>;
    rate_limit: NexusRateLimit;
};

type RawNexusModFilesResponse = {
    files: Array<NexusModFile & Record<string, unknown>> | { data?: Array<NexusModFile & Record<string, unknown>> };
    rate_limit: NexusRateLimit;
};

type NexusModFilesResponse = {
    files: NexusModFile[];
    rate_limit: NexusRateLimit;
};

type RawNexusModDependenciesResponse = {
    dependencies: unknown;
    rate_limit: NexusRateLimit;
};

type NexusModDependenciesResponse = {
    dependencies: NexusModDependency[];
    rate_limit: NexusRateLimit;
};

export type NexusView = "all" | "trending" | "latest_added" | "latest_updated";

export const NEXUS_CACHE_TTL_MINUTES = 10;

const NEXUS_CACHE_TTL_MS = NEXUS_CACHE_TTL_MINUTES * 60 * 1000;
const ALL_NEXUS_FEEDS = ["trending", "latest_added", "latest_updated", "updated"] as const;

let sessionCache: { value: NexusSession; cachedAt: number } | null = null;
let sessionRequest: Promise<NexusSession> | null = null;
const modsCache = new Map<string, { value: NexusModsResponse; cachedAt: number }>();
const modsRequests = new Map<string, Promise<NexusModsResponse>>();
const detailsCache = new Map<number, { value: NexusMod; cachedAt: number }>();
const detailsRequests = new Map<number, Promise<NexusMod>>();
const filesCache = new Map<number, { value: NexusModFilesResponse; cachedAt: number }>();
const filesRequests = new Map<number, Promise<NexusModFilesResponse>>();
const dependencyCache = new Map<number, { value: NexusModDependenciesResponse; cachedAt: number }>();
const dependencyRequests = new Map<number, Promise<NexusModDependenciesResponse>>();

function cacheFresh(cachedAt: number): boolean {
    return Date.now() - cachedAt < NEXUS_CACHE_TTL_MS;
}

export function clearNexusClientCache(): void {
    sessionCache = null;
    sessionRequest = null;
    modsCache.clear();
    modsRequests.clear();
    detailsCache.clear();
    detailsRequests.clear();
    filesCache.clear();
    filesRequests.clear();
    dependencyCache.clear();
    dependencyRequests.clear();
}

export async function getNexusSession(options: { force?: boolean } = {}): Promise<NexusSession> {
    if (!options.force && sessionCache && cacheFresh(sessionCache.cachedAt)) {
        return sessionCache.value;
    }

    if (!options.force && sessionRequest) {
        return await sessionRequest;
    }

    sessionRequest = invoke<NexusSession>("nexus_get_session")
        .then((session) => {
            sessionCache = { value: session, cachedAt: Date.now() };
            return session;
        })
        .finally(() => {
            sessionRequest = null;
        });

    return await sessionRequest;
}

export async function saveNexusApiKey(apiKey: string): Promise<NexusSession> {
    clearNexusClientCache();
    const session = await invoke<NexusSession>("nexus_save_api_key", { apiKey });
    sessionCache = { value: session, cachedAt: Date.now() };
    return session;
}

export async function clearNexusApiKey(): Promise<void> {
    clearNexusClientCache();
    await invoke("nexus_clear_api_key");
}

export async function fetchNexusSotfMods(view: NexusView, options: { force?: boolean } = {}): Promise<NexusModsResponse> {
    if (view === "all") {
        const cacheKey = "all";
        const cached = modsCache.get(cacheKey);
        if (!options.force && cached && cacheFresh(cached.cachedAt)) {
            return cached.value;
        }

        const existingRequest = modsRequests.get(cacheKey);
        if (!options.force && existingRequest) {
            return await existingRequest;
        }

        const request = Promise.allSettled(ALL_NEXUS_FEEDS.map(feed => fetchSingleNexusSotfMods(feed, options)))
            .then((responses) => {
                const successful = responses
                    .filter((response): response is PromiseFulfilledResult<NexusModsResponse> => response.status === "fulfilled")
                    .map(response => response.value);

                if (successful.length === 0) {
                    const firstError = responses.find((response): response is PromiseRejectedResult => response.status === "rejected");
                    throw new Error(firstError ? `${firstError.reason}` : "No Nexus feed returned mods.");
                }

                const merged = {
                    mods: mergeNexusMods(successful.flatMap(response => response.mods)),
                    rate_limit: successful[successful.length - 1].rate_limit
                };
                modsCache.set(cacheKey, { value: merged, cachedAt: Date.now() });
                return merged;
            })
            .finally(() => {
                modsRequests.delete(cacheKey);
            });

        modsRequests.set(cacheKey, request);
        return await request;
    }

    return await fetchSingleNexusSotfMods(view, options);
}

async function fetchSingleNexusSotfMods(view: string, options: { force?: boolean } = {}): Promise<NexusModsResponse> {
    const cached = modsCache.get(view);
    if (!options.force && cached && cacheFresh(cached.cachedAt)) {
        return cached.value;
    }

    const existingRequest = modsRequests.get(view);
    if (!options.force && existingRequest) {
        return await existingRequest;
    }

    const request = invoke<RawNexusModsResponse>("nexus_fetch_sotf_mods", { view })
        .then((response) => {
            const rawMods = Array.isArray(response.mods) ? response.mods : response.mods.data ?? [];
            const normalized = {
                ...response,
                mods: mergeNexusMods(rawMods.map(normalizeNexusMod).filter(mod => mod.mod_id > 0))
            };
            modsCache.set(view, { value: normalized, cachedAt: Date.now() });
            return normalized;
        })
        .finally(() => {
            modsRequests.delete(view);
        });

    modsRequests.set(view, request);
    return await request;
}

export async function fetchNexusModDetails(modId: number, options: { force?: boolean } = {}): Promise<NexusMod> {
    const cached = detailsCache.get(modId);
    if (!options.force && cached && cacheFresh(cached.cachedAt)) {
        return cached.value;
    }

    const existingRequest = detailsRequests.get(modId);
    if (!options.force && existingRequest) {
        return await existingRequest;
    }

    const request = invoke<RawNexusModDetailsResponse>("nexus_fetch_mod_details", { modId })
        .then((response) => {
            const details = normalizeNexusMod(response.details);
            detailsCache.set(modId, { value: details, cachedAt: Date.now() });
            return details;
        })
        .finally(() => {
            detailsRequests.delete(modId);
        });

    detailsRequests.set(modId, request);
    return await request;
}

export async function fetchNexusModFiles(modId: number, options: { force?: boolean } = {}): Promise<NexusModFilesResponse> {
    const cached = filesCache.get(modId);
    if (!options.force && cached && cacheFresh(cached.cachedAt)) {
        return cached.value;
    }

    const existingRequest = filesRequests.get(modId);
    if (!options.force && existingRequest) {
        return await existingRequest;
    }

    const request = invoke<RawNexusModFilesResponse>("nexus_fetch_mod_files", { modId })
        .then((response) => {
            const rawFiles = Array.isArray(response.files) ? response.files : response.files.data ?? [];
            const normalized = {
                ...response,
                files: rawFiles.map(normalizeNexusFile).filter(file => file.file_id > 0)
            };
            filesCache.set(modId, { value: normalized, cachedAt: Date.now() });
            return normalized;
        })
        .finally(() => {
            filesRequests.delete(modId);
        });

    filesRequests.set(modId, request);
    return await request;
}

export async function fetchNexusFileDependencies(fileId: number, options: { force?: boolean } = {}): Promise<NexusModDependenciesResponse> {
    const cached = dependencyCache.get(fileId);
    if (!options.force && cached && cacheFresh(cached.cachedAt)) {
        return cached.value;
    }

    const existingRequest = dependencyRequests.get(fileId);
    if (!options.force && existingRequest) {
        return await existingRequest;
    }

    const request = invoke<RawNexusModDependenciesResponse>("nexus_fetch_file_dependencies", { fileId })
        .then((response) => {
            const normalized = {
                ...response,
                dependencies: normalizeDependencies(response.dependencies)
            };
            dependencyCache.set(fileId, { value: normalized, cachedAt: Date.now() });
            return normalized;
        })
        .finally(() => {
            dependencyRequests.delete(fileId);
        });

    dependencyRequests.set(fileId, request);
    return await request;
}

export function getNexusModPageUrl(mod: NexusMod): string {
    return `https://www.nexusmods.com/sonsoftheforest/mods/${mod.mod_id}`;
}

export function getNexusModDownloadUrl(mod: NexusMod): string {
    return `${getNexusModPageUrl(mod)}?tab=files`;
}

export function getNexusNxmUrl(mod: NexusMod, file: NexusModFile): string {
    return `nxm://sonsoftheforest/mods/${mod.mod_id}/files/${file.file_id}`;
}

export function pickRecommendedNexusFile(files: NexusModFile[]): NexusModFile | null {
    const installable = files.filter(file => !/^(old_version|archived|removed)$/i.test(file.category_name ?? ""));
    return installable.find(file => file.is_primary)
        ?? installable.find(file => /^main$/i.test(file.category_name ?? ""))
        ?? installable.sort((a, b) => (b.uploaded_timestamp ?? 0) - (a.uploaded_timestamp ?? 0))[0]
        ?? files[0]
        ?? null;
}

function mergeNexusMods(input: NexusMod[]): NexusMod[] {
    const byId = new Map<number, NexusMod>();
    for (const mod of input) {
        const existing = byId.get(mod.mod_id);
        byId.set(mod.mod_id, existing ? { ...mod, ...existing, ...emptyFieldsFrom(existing, mod) } : mod);
    }

    return Array.from(byId.values())
        .sort((a, b) => (b.updated_timestamp ?? 0) - (a.updated_timestamp ?? 0) || a.name.localeCompare(b.name));
}

function emptyFieldsFrom(preferred: NexusMod, fallback: NexusMod): Partial<NexusMod> {
    const result: Partial<NexusMod> = {};
    for (const key of Object.keys(fallback) as Array<keyof NexusMod>) {
        if (preferred[key] === undefined || preferred[key] === "") {
            (result as Record<string, unknown>)[key] = fallback[key];
        }
    }

    return result;
}

function normalizeNexusMod(raw: NexusMod & Record<string, unknown>): NexusMod {
    const modId = numberField(raw.mod_id) ?? numberField(raw.id) ?? 0;
    const rawSummary = stringField(raw.summary)
        ?? stringField(raw.description)
        ?? stringField(raw.short_description);
    const category = stringField(raw.category_name)
        ?? stringField(raw.category)
        ?? inferNexusCategory(raw);

    return {
        ...raw,
        mod_id: modId,
        name: stringField(raw.name)
            ?? stringField(raw.mod_name)
            ?? stringField(raw.title)
            ?? `Nexus Mod #${modId}`,
        summary: cleanSummary(rawSummary),
        version: stringField(raw.version) ?? stringField(raw.latest_version),
        author: stringField(raw.author) ?? stringField(raw.uploaded_by),
        uploaded_by: stringField(raw.uploaded_by),
        picture_url: stringField(raw.picture_url)
            ?? stringField(raw.picture)
            ?? stringField(raw.thumbnail_url)
            ?? stringField(raw.screenshot_url),
        category_name: category,
        endorsement_count: numberField(raw.endorsement_count),
        mod_downloads: numberField(raw.mod_downloads) ?? numberField(raw.downloads),
        mod_unique_downloads: numberField(raw.mod_unique_downloads) ?? numberField(raw.unique_downloads),
        created_timestamp: numberField(raw.created_timestamp),
        updated_timestamp: numberField(raw.updated_timestamp) ?? numberField(raw.latest_file_update),
        created_time: stringField(raw.created_time),
        updated_time: stringField(raw.updated_time),
        loader_type: inferLoaderType(raw)
    };
}

function normalizeNexusFile(raw: NexusModFile & Record<string, unknown>): NexusModFile {
    const fileId = numberField(raw.file_id) ?? numberField(raw.id) ?? numberField(raw.game_scoped_id) ?? 0;

    return {
        ...raw,
        file_id: fileId,
        name: stringField(raw.name)
            ?? stringField(raw.file_name)
            ?? stringField(raw.logical_filename)
            ?? `Nexus File #${fileId}`,
        version: stringField(raw.version) ?? stringField(raw.mod_version),
        mod_version: stringField(raw.mod_version) ?? stringField(raw.version),
        category_name: stringField(raw.category_name) ?? stringField(raw.category) ?? stringField(raw.file_category) ?? "unknown",
        description: cleanSummary(stringField(raw.description)),
        changelog_html: stringField(raw.changelog_html),
        size: numberField(raw.size) ?? numberField(raw.file_size) ?? numberField(raw.size_kb),
        is_primary: booleanField(raw.is_primary) ?? booleanField(raw.primary_mod_manager_download),
        uploaded_timestamp: numberField(raw.uploaded_timestamp),
        uploaded_time: stringField(raw.uploaded_time)
    };
}

function normalizeDependencies(raw: unknown): NexusModDependency[] {
    const dependencies = Array.isArray(raw) ? raw : (raw as { dependencies?: unknown[] })?.dependencies ?? [];
    const flattened: NexusModDependency[] = [];

    for (const dependency of dependencies as Array<Record<string, unknown>>) {
        const groups = Array.isArray(dependency.candidate_groups) ? dependency.candidate_groups as Array<Record<string, unknown>> : [];
        for (const group of groups) {
            const mod = group.mod as Record<string, unknown> | undefined;
            const versions = Array.isArray(group.candidate_versions) ? group.candidate_versions as Array<Record<string, unknown>> : [];
            const version = versions[0];
            const file = version?.file as Record<string, unknown> | undefined;
            const modId = numberField(mod?.game_scoped_id) ?? numberField(mod?.id);
            const fileId = numberField(file?.game_scoped_id) ?? numberField(file?.id);

            flattened.push({
                id: stringField(dependency.id) ?? `${modId ?? "mod"}:${fileId ?? "file"}`,
                mod_id: modId,
                file_id: fileId,
                mod_name: stringField(mod?.name) ?? "Unknown dependency",
                file_name: stringField(file?.name) ?? stringField(file?.file_name),
                version: stringField(file?.version) ?? stringField(version?.version),
                group_name: stringField(group.name)
            });
        }
    }

    return flattened;
}

function inferNexusCategory(raw: Record<string, unknown>): string {
    const text = [
        stringField(raw.name),
        stringField(raw.mod_name),
        stringField(raw.title),
        stringField(raw.summary),
        stringField(raw.description),
        stringField(raw.short_description)
    ].filter(Boolean).join(" ").toLowerCase();

    if (/translation|language|chinese|japanese|korean|spanish|french|german|russian/.test(text)) {
        return "Translation";
    }

    if (/admin|cheat|trainer|debug|console|command|menu/.test(text)) {
        return "Admin / Tools";
    }

    if (/texture|graphic|visual|lighting|shader|hud|ui|radio|music|sound|audio/.test(text)) {
        return "Visual / Audio";
    }

    if (/save|backup|autosave|performance|fps|logging|crash|fix|patch/.test(text)) {
        return "Fixes / Utility";
    }

    if (/build|cave|survival|item|weapon|enemy|npc|golf|vehicle|gameplay/.test(text)) {
        return "Gameplay";
    }

    return "Uncategorized";
}

function inferLoaderType(raw: Record<string, unknown>): string {
    const text = [
        stringField(raw.name),
        stringField(raw.mod_name),
        stringField(raw.title),
        stringField(raw.summary),
        stringField(raw.description),
        stringField(raw.short_description)
    ].filter(Boolean).join(" ").toLowerCase();

    if (/bepinex|il2cpp/.test(text)) {
        return "BepInEx";
    }

    if (/redloader|red loader/.test(text)) {
        return "RedLoader";
    }

    return "Unknown";
}

function stringField(value: unknown): string | undefined {
    return typeof value === "string" && value.trim().length > 0 ? value.trim() : undefined;
}

function booleanField(value: unknown): boolean | undefined {
    return typeof value === "boolean" ? value : undefined;
}

function numberField(value: unknown): number | undefined {
    if (typeof value === "number" && Number.isFinite(value)) {
        return value;
    }

    if (typeof value === "string") {
        const parsed = Number(value);
        return Number.isFinite(parsed) ? parsed : undefined;
    }

    return undefined;
}

function cleanSummary(value: string | undefined): string | undefined {
    if (!value) {
        return undefined;
    }

    return value
        .replace(/<[^>]*>/g, " ")
        .replace(/\s+/g, " ")
        .trim();
}
