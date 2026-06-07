import { invoke } from "@tauri-apps/api/core";

export type NexusUser = {
    user_id?: number;
    name?: string;
    profile_url?: string;
    membership_tier?: string;
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
    image_urls?: string[];
    category_id?: number;
    category_name?: string;
    category_source?: NexusCategorySource;
    endorsement_count?: number;
    mod_downloads?: number;
    mod_unique_downloads?: number;
    created_timestamp?: number;
    updated_timestamp?: number;
    created_time?: string;
    updated_time?: string;
    loader_type?: string;
};

export type NexusCategorySource = "api" | "local" | "inferred";

export type NexusCategory = {
    category_id?: number;
    name: string;
    parent_category_id?: number;
    source: NexusCategorySource;
};

export type NexusModFile = {
    file_id: number;
    nexus_file_id?: string;
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
    nexus_file_id?: string;
    mod_name: string;
    file_name?: string;
    version?: string;
    version_requirement?: string;
    group_name?: string;
};

export type NexusModChangelog = {
    version: string;
    changes: string;
    updated_at?: string;
};

export type NexusEndorsement = {
    mod_id: number;
    game_domain_name?: string;
    status?: string;
    endorsed_at?: string;
};

export type NexusTrackedMod = {
    mod_id: number;
    name?: string;
    game_domain_name?: string;
};

type RawNexusModsResponse = {
    mods: Array<NexusMod & Record<string, unknown>> | { data?: Array<NexusMod & Record<string, unknown>> };
    rate_limit: NexusRateLimit;
};

type RawNexusGameInfoResponse = {
    game: unknown;
    rate_limit: NexusRateLimit;
};

type NexusModsResponse = {
    mods: NexusMod[];
    categories: NexusCategory[];
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

type RawNexusModChangelogsResponse = {
    changelogs: unknown;
    rate_limit: NexusRateLimit;
};

type NexusModChangelogsResponse = {
    changelogs: NexusModChangelog[];
    rate_limit: NexusRateLimit;
};

type RawNexusEndorsementsResponse = {
    endorsements: unknown;
    rate_limit: NexusRateLimit;
};

type NexusEndorsementsResponse = {
    endorsements: NexusEndorsement[];
    rate_limit: NexusRateLimit;
};

type RawNexusTrackedModsResponse = {
    tracked_mods: unknown;
    rate_limit: NexusRateLimit;
};

type NexusTrackedModsResponse = {
    tracked_mods: NexusTrackedMod[];
    rate_limit: NexusRateLimit;
};

type NexusActionResponse = {
    result: unknown;
    rate_limit: NexusRateLimit;
};

export type NexusView = "all" | "trending" | "latest_added" | "latest_updated";

export const NEXUS_CACHE_TTL_MINUTES = 10;

const NEXUS_CACHE_TTL_MS = NEXUS_CACHE_TTL_MINUTES * 60 * 1000;
const ALL_NEXUS_FEEDS = ["trending", "latest_added", "latest_updated", "updated"] as const;
const LOCAL_SOTF_NEXUS_CATEGORIES: NexusCategory[] = [
    { category_id: 4, name: "Gameplay", source: "local" },
    { category_id: 2, name: "Miscellaneous", source: "local" },
    { category_id: 3, name: "Visuals", source: "local" }
];
const NEXUS_CATEGORY_ALIASES: Record<string, string> = {
    "game play": "Gameplay",
    "game mechanics": "Gameplay",
    gameplay: "Gameplay",
    mechanics: "Gameplay",
    qol: "Gameplay",
    "quality of life": "Gameplay",
    balance: "Gameplay",
    survival: "Gameplay",
    building: "Gameplay",
    combat: "Gameplay",
    graphic: "Visuals",
    graphics: "Visuals",
    lighting: "Visuals",
    model: "Visuals",
    models: "Visuals",
    reshade: "Visuals",
    shader: "Visuals",
    shaders: "Visuals",
    texture: "Visuals",
    textures: "Visuals",
    visual: "Visuals",
    visuals: "Visuals",
    misc: "Miscellaneous",
    miscellaneous: "Miscellaneous",
    save: "Miscellaneous",
    saves: "Miscellaneous",
    tool: "Miscellaneous",
    tools: "Miscellaneous",
    translation: "Miscellaneous",
    translations: "Miscellaneous",
    utilities: "Miscellaneous",
    utility: "Miscellaneous"
};
const DEPENDENCY_SOURCE_KEYS = [
    "dependencies",
    "dependency_definitions",
    "dependencyDefinitions",
    "dependency_ranges",
    "dependencyRanges",
    "items",
    "results",
    "nodes",
    "edges"
];
const DEPENDENCY_WRAPPER_KEYS = [
    "data",
    "result",
    "payload",
    "response",
    "modFileDependencyRangesResponse",
    "modFileDependencyMaterializedResponse",
    "mod_file_dependency_ranges_response",
    "mod_file_dependency_materialized_response"
];

let sessionCache: { value: NexusSession; cachedAt: number } | null = null;
let sessionRequest: Promise<NexusSession> | null = null;
let categoriesCache: { value: NexusCategory[]; cachedAt: number } | null = null;
let categoriesRequest: Promise<NexusCategory[]> | null = null;
const modsCache = new Map<string, { value: NexusModsResponse; cachedAt: number }>();
const modsRequests = new Map<string, Promise<NexusModsResponse>>();
const detailsCache = new Map<number, { value: NexusMod; cachedAt: number }>();
const detailsRequests = new Map<number, Promise<NexusMod>>();
const filesCache = new Map<number, { value: NexusModFilesResponse; cachedAt: number }>();
const filesRequests = new Map<number, Promise<NexusModFilesResponse>>();
const dependencyCache = new Map<string, { value: NexusModDependenciesResponse; cachedAt: number }>();
const dependencyRequests = new Map<string, Promise<NexusModDependenciesResponse>>();
const changelogCache = new Map<number, { value: NexusModChangelogsResponse; cachedAt: number }>();
const changelogRequests = new Map<number, Promise<NexusModChangelogsResponse>>();
let endorsementCache: { value: NexusEndorsementsResponse; cachedAt: number } | null = null;
let endorsementRequest: Promise<NexusEndorsementsResponse> | null = null;
let trackedModsCache: { value: NexusTrackedModsResponse; cachedAt: number } | null = null;
let trackedModsRequest: Promise<NexusTrackedModsResponse> | null = null;

function cacheFresh(cachedAt: number): boolean {
    return Date.now() - cachedAt < NEXUS_CACHE_TTL_MS;
}

export function clearNexusClientCache(): void {
    sessionCache = null;
    sessionRequest = null;
    categoriesCache = null;
    categoriesRequest = null;
    modsCache.clear();
    modsRequests.clear();
    detailsCache.clear();
    detailsRequests.clear();
    filesCache.clear();
    filesRequests.clear();
    dependencyCache.clear();
    dependencyRequests.clear();
    changelogCache.clear();
    changelogRequests.clear();
    endorsementCache = null;
    endorsementRequest = null;
    trackedModsCache = null;
    trackedModsRequest = null;
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

export async function fetchNexusSotfCategories(options: { force?: boolean } = {}): Promise<NexusCategory[]> {
    if (!options.force && categoriesCache && cacheFresh(categoriesCache.cachedAt)) {
        return categoriesCache.value;
    }

    if (!options.force && categoriesRequest) {
        return await categoriesRequest;
    }

    categoriesRequest = invoke<RawNexusGameInfoResponse>("nexus_fetch_sotf_game_info")
        .then((response) => {
            const categories = mergeNexusCategories([
                ...normalizeNexusCategories(response.game),
                ...LOCAL_SOTF_NEXUS_CATEGORIES
            ]);
            categoriesCache = { value: categories, cachedAt: Date.now() };
            return categories;
        })
        .catch(() => {
            const fallback = mergeNexusCategories(LOCAL_SOTF_NEXUS_CATEGORIES);
            categoriesCache = { value: fallback, cachedAt: Date.now() };
            return fallback;
        })
        .finally(() => {
            categoriesRequest = null;
        });

    return await categoriesRequest;
}

export async function fetchNexusSotfMods(view: NexusView, options: { force?: boolean } = {}): Promise<NexusModsResponse> {
    const categories = await fetchNexusSotfCategories(options);

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

        const request = Promise.allSettled(ALL_NEXUS_FEEDS.map(feed => fetchSingleNexusSotfMods(feed, categories, options)))
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
                    categories,
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

    return await fetchSingleNexusSotfMods(view, categories, options);
}

async function fetchSingleNexusSotfMods(view: string, categories: NexusCategory[], options: { force?: boolean } = {}): Promise<NexusModsResponse> {
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
                categories,
                mods: mergeNexusMods(rawMods.map(mod => normalizeNexusMod(mod, categories)).filter(mod => mod.mod_id > 0))
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

    const request = Promise.all([
            invoke<RawNexusModDetailsResponse>("nexus_fetch_mod_details", { modId }),
            fetchNexusSotfCategories(options)
        ])
        .then(([response, categories]) => {
            const details = normalizeNexusMod(response.details, categories);
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

export async function fetchNexusFileDependencies(fileId: number, options: { force?: boolean; nexusFileId?: string } = {}): Promise<NexusModDependenciesResponse> {
    const cacheKey = dependencyCacheKey(fileId, options.nexusFileId);
    const cached = dependencyCache.get(cacheKey);
    if (!options.force && cached && cacheFresh(cached.cachedAt)) {
        return cached.value;
    }

    const existingRequest = dependencyRequests.get(cacheKey);
    if (!options.force && existingRequest) {
        return await existingRequest;
    }

    const request = invoke<RawNexusModDependenciesResponse>("nexus_fetch_file_dependencies", {
        fileId,
        nexusFileId: options.nexusFileId
    })
        .then((response) => {
            const normalized = {
                ...response,
                dependencies: normalizeDependencies(response.dependencies)
            };
            dependencyCache.set(cacheKey, { value: normalized, cachedAt: Date.now() });
            return normalized;
        })
        .finally(() => {
            dependencyRequests.delete(cacheKey);
        });

    dependencyRequests.set(cacheKey, request);
    return await request;
}

export async function fetchNexusModChangelogs(modId: number, options: { force?: boolean } = {}): Promise<NexusModChangelogsResponse> {
    const cached = changelogCache.get(modId);
    if (!options.force && cached && cacheFresh(cached.cachedAt)) {
        return cached.value;
    }

    const existingRequest = changelogRequests.get(modId);
    if (!options.force && existingRequest) {
        return await existingRequest;
    }

    const request = invoke<RawNexusModChangelogsResponse>("nexus_fetch_mod_changelogs", { modId })
        .then((response) => {
            const normalized = {
                changelogs: normalizeChangelogs(response.changelogs),
                rate_limit: response.rate_limit
            };
            changelogCache.set(modId, { value: normalized, cachedAt: Date.now() });
            return normalized;
        })
        .finally(() => {
            changelogRequests.delete(modId);
        });

    changelogRequests.set(modId, request);
    return await request;
}

export async function fetchNexusUserEndorsements(options: { force?: boolean } = {}): Promise<NexusEndorsementsResponse> {
    if (!options.force && endorsementCache && cacheFresh(endorsementCache.cachedAt)) {
        return endorsementCache.value;
    }

    if (!options.force && endorsementRequest) {
        return await endorsementRequest;
    }

    endorsementRequest = invoke<RawNexusEndorsementsResponse>("nexus_fetch_user_endorsements")
        .then((response) => {
            const normalized = {
                endorsements: normalizeEndorsements(response.endorsements),
                rate_limit: response.rate_limit
            };
            endorsementCache = { value: normalized, cachedAt: Date.now() };
            return normalized;
        })
        .finally(() => {
            endorsementRequest = null;
        });

    return await endorsementRequest;
}

export async function fetchNexusUserTrackedMods(options: { force?: boolean } = {}): Promise<NexusTrackedModsResponse> {
    if (!options.force && trackedModsCache && cacheFresh(trackedModsCache.cachedAt)) {
        return trackedModsCache.value;
    }

    if (!options.force && trackedModsRequest) {
        return await trackedModsRequest;
    }

    trackedModsRequest = invoke<RawNexusTrackedModsResponse>("nexus_fetch_user_tracked_mods")
        .then((response) => {
            const normalized = {
                tracked_mods: normalizeTrackedMods(response.tracked_mods),
                rate_limit: response.rate_limit
            };
            trackedModsCache = { value: normalized, cachedAt: Date.now() };
            return normalized;
        })
        .finally(() => {
            trackedModsRequest = null;
        });

    return await trackedModsRequest;
}

export async function endorseNexusSotfMod(modId: number, version?: string): Promise<NexusActionResponse> {
    const response = await invoke<NexusActionResponse>("nexus_endorse_sotf_mod", {
        modId,
        version: version?.trim() || undefined
    });
    endorsementCache = null;
    endorsementRequest = null;
    return response;
}

export async function trackNexusSotfMod(modId: number): Promise<NexusActionResponse> {
    const response = await invoke<NexusActionResponse>("nexus_track_sotf_mod", { modId });
    trackedModsCache = null;
    trackedModsRequest = null;
    return response;
}

export async function untrackNexusSotfMod(modId: number): Promise<NexusActionResponse> {
    const response = await invoke<NexusActionResponse>("nexus_untrack_sotf_mod", { modId });
    trackedModsCache = null;
    trackedModsRequest = null;
    return response;
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
        if (!existing) {
            byId.set(mod.mod_id, mod);
            continue;
        }

        const merged = { ...mod, ...existing, ...emptyFieldsFrom(existing, mod) };
        merged.image_urls = uniqueImageUrls([
            ...(mod.image_urls ?? []),
            ...(existing.image_urls ?? []),
            mod.picture_url,
            existing.picture_url
        ]);
        merged.picture_url = merged.picture_url ?? merged.image_urls[0];
        byId.set(mod.mod_id, merged);
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

function canonicalNexusCategoryName(name: string): string {
    const normalized = name
        .trim()
        .replace(/[_-]+/g, " ")
        .replace(/\s+/g, " ");
    return NEXUS_CATEGORY_ALIASES[normalized.toLowerCase()] ?? normalized;
}

function nexusCategoryNameKey(name: string): string {
    return canonicalNexusCategoryName(name).toLowerCase();
}

function localNexusCategoryByName(name: string): NexusCategory | undefined {
    const key = nexusCategoryNameKey(name);
    return LOCAL_SOTF_NEXUS_CATEGORIES.find(category => nexusCategoryNameKey(category.name) === key);
}

function normalizeNexusCategories(raw: unknown): NexusCategory[] {
    const root = objectField(raw);
    const data = objectField(root?.data);
    const rawCategories = categoryRecords(root?.categories)
        ?? categoryRecords(data?.categories)
        ?? (Array.isArray(raw) ? raw : null)
        ?? [];

    return mergeNexusCategories(rawCategories
        .map(category => normalizeNexusCategory(category, "api"))
        .filter((category): category is NexusCategory => category !== null));
}

function normalizeNexusCategory(raw: unknown, source: NexusCategorySource): NexusCategory | null {
    if (typeof raw === "string") {
        return raw.trim().length > 0 ? { name: raw.trim(), source } : null;
    }

    const category = objectField(raw);
    if (!category) {
        return null;
    }

    const categoryId = numberField(category.category_id)
        ?? numberField(category.id)
        ?? numberField(category.game_category_id);
    const parentCategoryId = numberField(category.parent_category_id)
        ?? numberField(category.parent_id)
        ?? numberField(category.parent);
    const rawName = stringField(category.name)
        ?? stringField(category.category_name)
        ?? stringField(category.title);
    const name = rawName ? canonicalNexusCategoryName(rawName) : undefined;

    if (!name || isRootNexusCategory(name, categoryId)) {
        return null;
    }

    return {
        category_id: categoryId,
        name,
        parent_category_id: parentCategoryId,
        source
    };
}

function categoryRecords(raw: unknown): unknown[] | null {
    if (Array.isArray(raw)) {
        return raw;
    }

    const object = objectField(raw);
    if (object) {
        return Object.values(object);
    }

    return null;
}

function mergeNexusCategories(input: NexusCategory[]): NexusCategory[] {
    const byKey = new Map<string, NexusCategory>();
    const byName = new Map<string, NexusCategory>();

    for (const category of input) {
        if (!category.name || isRootNexusCategory(category.name, category.category_id)) {
            continue;
        }

        const key = category.category_id !== undefined
            ? `id:${category.category_id}`
            : `name:${nexusCategoryNameKey(category.name)}`;
        const existing = byKey.get(key);
        const merged = preferNexusCategory(existing, category);
        byKey.set(key, merged);
    }

    for (const category of byKey.values()) {
        const nameKey = nexusCategoryNameKey(category.name);
        byName.set(nameKey, preferNexusCategory(byName.get(nameKey), category));
    }

    return Array.from(byName.values()).sort(compareNexusCategories);
}

function preferNexusCategory(existing: NexusCategory | undefined, next: NexusCategory): NexusCategory {
    if (!existing) {
        return next;
    }

    if (categorySourceRank(next.source) < categorySourceRank(existing.source)) {
        return {
            ...next,
            category_id: next.category_id ?? existing.category_id,
            parent_category_id: next.parent_category_id ?? existing.parent_category_id
        };
    }

    return {
        ...existing,
        category_id: existing.category_id ?? next.category_id,
        parent_category_id: existing.parent_category_id ?? next.parent_category_id
    };
}

function compareNexusCategories(left: NexusCategory, right: NexusCategory): number {
    const leftLocalIndex = LOCAL_SOTF_NEXUS_CATEGORIES.findIndex(category => nexusCategoryNameKey(category.name) === nexusCategoryNameKey(left.name));
    const rightLocalIndex = LOCAL_SOTF_NEXUS_CATEGORIES.findIndex(category => nexusCategoryNameKey(category.name) === nexusCategoryNameKey(right.name));

    if (leftLocalIndex !== -1 || rightLocalIndex !== -1) {
        return (leftLocalIndex === -1 ? Number.MAX_SAFE_INTEGER : leftLocalIndex)
            - (rightLocalIndex === -1 ? Number.MAX_SAFE_INTEGER : rightLocalIndex);
    }

    return (left.category_id ?? Number.MAX_SAFE_INTEGER) - (right.category_id ?? Number.MAX_SAFE_INTEGER)
        || left.name.localeCompare(right.name);
}

function categorySourceRank(source: NexusCategorySource): number {
    switch (source) {
        case "api":
            return 0;
        case "local":
            return 1;
        default:
            return 2;
    }
}

function normalizeNexusMod(raw: NexusMod & Record<string, unknown>, categories: NexusCategory[] = LOCAL_SOTF_NEXUS_CATEGORIES): NexusMod {
    const modId = numberField(raw.mod_id) ?? numberField(raw.id) ?? 0;
    const rawSummary = stringField(raw.summary)
        ?? stringField(raw.description)
        ?? stringField(raw.short_description);
    const resolvedCategory = resolveNexusCategory(raw, categories);
    const imageUrls = imageUrlsFrom(raw);
    const pictureUrl = normalizeImageUrl(stringField(raw.picture_url))
        ?? normalizeImageUrl(stringField(raw.picture))
        ?? normalizeImageUrl(stringField(raw.thumbnail_url))
        ?? normalizeImageUrl(stringField(raw.screenshot_url))
        ?? normalizeImageUrl(stringField(raw.image_url))
        ?? imageUrls[0];

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
        picture_url: pictureUrl,
        image_urls: uniqueImageUrls([pictureUrl, ...imageUrls]),
        category_id: resolvedCategory.category_id,
        category_name: resolvedCategory.name,
        category_source: resolvedCategory.source,
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

function resolveNexusCategory(raw: Record<string, unknown>, categories: NexusCategory[]): NexusCategory {
    const nestedCategory = objectField(raw.category);
    const categoryId = numberField(raw.category_id)
        ?? numberField(raw.categoryId)
        ?? numberField(raw.cat)
        ?? numberField(raw.category)
        ?? numberField(nestedCategory?.category_id)
        ?? numberField(nestedCategory?.id);
    const categoryById = categoryId !== undefined
        ? categories.find(category => category.category_id === categoryId)
        : undefined;
    const rawName = stringField(raw.category_name)
        ?? stringField(raw.categoryName)
        ?? stringField(nestedCategory?.name)
        ?? stringField(nestedCategory?.category_name)
        ?? stringField(raw.category);
    const categoryByName = rawName
        ? categories.find(category => nexusCategoryNameKey(category.name) === nexusCategoryNameKey(rawName))
        : undefined;

    if (categoryByName) {
        return categoryByName;
    }

    const canonicalRawName = rawName ? canonicalNexusCategoryName(rawName) : undefined;
    const localCategory = canonicalRawName ? localNexusCategoryByName(canonicalRawName) : undefined;

    if (canonicalRawName && !isRootNexusCategory(canonicalRawName, categoryId)) {
        return {
            category_id: categoryById?.category_id ?? localCategory?.category_id ?? categoryId,
            name: localCategory?.name ?? canonicalRawName,
            parent_category_id: categoryById?.parent_category_id ?? localCategory?.parent_category_id,
            source: categoryById?.source ?? localCategory?.source ?? "api"
        };
    }

    if (categoryById) {
        return categoryById;
    }

    const inferredName = inferNexusCategory(raw);
    const inferredCategory = localNexusCategoryByName(inferredName);
    return {
        category_id: inferredCategory?.category_id,
        name: inferredCategory?.name ?? inferredName,
        parent_category_id: inferredCategory?.parent_category_id,
        source: "inferred"
    };
}

function normalizeNexusFile(raw: NexusModFile & Record<string, unknown>): NexusModFile {
    const fileId = firstNumberFromFields(raw, [
        "file_id",
        "fileId",
        "game_scoped_id",
        "gameScopedId",
        "game_scoped_file_id",
        "gameScopedFileId",
        "id"
    ]) ?? 0;
    const nexusFileId = firstIdentifierFromFields(raw, [
        "nexus_file_id",
        "nexusFileId",
        "uid",
        "uuid",
        "file_uid",
        "fileUid",
        "file_uuid",
        "fileUuid",
        "id"
    ]);

    return {
        ...raw,
        file_id: fileId,
        nexus_file_id: nexusFileId,
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

export function normalizeDependencies(raw: unknown): NexusModDependency[] {
    const flattened: NexusModDependency[] = [];
    const seen = new Set<string>();

    for (const dependency of dependencySourceRecords(raw)) {
        for (const normalized of normalizeDependencyRecord(dependency)) {
            const key = [
                normalized.mod_id ?? "",
                normalized.file_id ?? "",
                normalized.mod_name,
                normalized.file_name ?? "",
                normalized.version ?? "",
                normalized.version_requirement ?? "",
                normalized.group_name ?? ""
            ].join("|");
            if (seen.has(key)) {
                continue;
            }

            seen.add(key);
            flattened.push(normalized);
        }
    }

    return flattened;
}

function dependencySourceRecords(raw: unknown): Array<Record<string, unknown>> {
    const records: Array<Record<string, unknown>> = [];
    const seen = new Set<Record<string, unknown>>();
    collectDependencySourceRecords(raw, records, seen, 0);
    return records;
}

function collectDependencySourceRecords(
    value: unknown,
    records: Array<Record<string, unknown>>,
    seen: Set<Record<string, unknown>>,
    depth: number
) {
    if (depth > 4 || value === undefined || value === null) {
        return;
    }

    if (Array.isArray(value)) {
        for (const item of value) {
            collectDependencySourceRecords(graphRecord(item), records, seen, depth + 1);
        }
        return;
    }

    const object = graphRecord(value);
    if (!object) {
        return;
    }

    if (isDependencyRecord(object) && !seen.has(object)) {
        seen.add(object);
        records.push(object);
    }

    for (const key of DEPENDENCY_SOURCE_KEYS) {
        collectDependencySourceRecords(object[key], records, seen, depth + 1);
    }

    for (const key of DEPENDENCY_WRAPPER_KEYS) {
        collectDependencySourceRecords(object[key], records, seen, depth + 1);
    }
}

function isDependencyRecord(record: Record<string, unknown>): boolean {
    return recordHasAnyField(record, [
        "candidate_groups",
        "candidateGroups",
        "ranges",
        "dependency_ranges",
        "dependencyRanges",
        "target_group",
        "targetGroup",
        "mod_id",
        "modId",
        "game_scoped_mod_id",
        "gameScopedModId",
        "game_scoped_id",
        "gameScopedId",
        "file_id",
        "fileId",
        "game_scoped_file_id",
        "gameScopedFileId",
        "nexus_file_id",
        "nexusFileId",
        "mod",
        "file",
        "target",
        "target_file",
        "targetFile",
        "min_version_id",
        "minVersionId",
        "max_version_id",
        "maxVersionId"
    ]);
}

function normalizeDependencyRecord(dependency: Record<string, unknown>): NexusModDependency[] {
    const rows: NexusModDependency[] = [];
    const candidateGroups = objectArrayFromFields(dependency, [
        "candidate_groups",
        "candidateGroups",
        "candidate_update_groups",
        "candidateUpdateGroups",
        "groups"
    ]);

    for (const group of candidateGroups) {
        const row = dependencyFromCandidateGroup(dependency, group);
        if (row) {
            rows.push(row);
        }
    }

    const ranges = objectArrayFromFields(dependency, [
        "ranges",
        "dependency_ranges",
        "dependencyRanges",
        "range_definitions",
        "rangeDefinitions"
    ]);
    for (const range of ranges) {
        const row = dependencyFromRange(dependency, range);
        if (row) {
            rows.push(row);
        }
    }

    if (rows.length > 0) {
        return rows;
    }

    const directRange = firstObjectFromFields(dependency, ["target_group", "targetGroup", "group", "update_group", "updateGroup"])
        && firstObjectFromFields(dependency, ["min_version", "minVersion", "minimum_version", "minimumVersion"])
        ? dependencyFromRange(dependency, dependency)
        : null;
    if (directRange) {
        return [directRange];
    }

    const flat = dependencyFromFlatRecord(dependency);
    return flat ? [flat] : [];
}

function dependencyFromCandidateGroup(dependency: Record<string, unknown>, group: Record<string, unknown>): NexusModDependency | null {
    const mod = firstObjectFromFields(group, ["mod", "target_mod", "targetMod", "game_mod", "gameMod"])
        ?? firstObjectFromFields(dependency, ["mod", "target_mod", "targetMod"]);
    const versions = objectArrayFromFields(group, [
        "candidate_versions",
        "candidateVersions",
        "versions",
        "files",
        "candidate_files",
        "candidateFiles"
    ]);
    const version = versions[0];
    const file = firstObjectFromFields(version, ["file", "mod_file", "modFile"])
        ?? firstObjectFromFields(group, ["file", "mod_file", "modFile"])
        ?? (version && recordHasAnyField(version, ["game_scoped_id", "gameScopedId", "file_id", "fileId", "id", "name", "file_name", "fileName"]) ? version : undefined);
    const modId = firstNumberFromFields(mod, ["game_scoped_id", "gameScopedId", "mod_id", "modId", "game_scoped_mod_id", "gameScopedModId", "id"])
        ?? firstNumberFromFields(group, ["mod_id", "modId", "game_scoped_mod_id", "gameScopedModId"])
        ?? firstNumberFromFields(dependency, ["mod_id", "modId", "game_scoped_mod_id", "gameScopedModId"]);
    const fileId = firstNumberFromFields(file, ["game_scoped_id", "gameScopedId", "file_id", "fileId", "game_scoped_file_id", "gameScopedFileId", "id"])
        ?? firstNumberFromFields(version, ["game_scoped_id", "gameScopedId", "file_id", "fileId", "game_scoped_file_id", "gameScopedFileId", "id"])
        ?? firstNumberFromFields(group, ["file_id", "fileId", "game_scoped_file_id", "gameScopedFileId"])
        ?? firstNumberFromFields(dependency, ["file_id", "fileId", "game_scoped_file_id", "gameScopedFileId"]);
    const nexusFileId = firstIdentifierFromFields(file, ["nexus_file_id", "nexusFileId", "uid", "uuid", "file_uid", "fileUid", "file_uuid", "fileUuid", "id"])
        ?? firstIdentifierFromFields(version, ["nexus_file_id", "nexusFileId", "uid", "uuid", "file_uid", "fileUid", "file_uuid", "fileUuid", "id"])
        ?? firstIdentifierFromFields(group, ["nexus_file_id", "nexusFileId"])
        ?? firstIdentifierFromFields(dependency, ["nexus_file_id", "nexusFileId"]);
    const modName = firstStringFromFields(mod, ["name", "mod_name", "modName", "title"])
        ?? firstStringFromFields(group, ["mod_name", "modName", "name", "title"])
        ?? firstStringFromFields(dependency, ["mod_name", "modName", "name", "title"]);

    if (!modName && !modId) {
        return null;
    }

    return {
        id: stringField(dependency.id) ?? stringField(group.id) ?? `${modId ?? "mod"}:${fileId ?? "file"}`,
        mod_id: modId,
        file_id: fileId,
        nexus_file_id: nexusFileId,
        mod_name: modName ?? "Unknown dependency",
        file_name: firstStringFromFields(file, ["name", "file_name", "fileName", "logical_filename", "logicalFilename"])
            ?? firstStringFromFields(version, ["name", "file_name", "fileName", "logical_filename", "logicalFilename"]),
        version: firstStringFromFields(file, ["version", "file_version", "fileVersion", "mod_version", "modVersion"])
            ?? firstStringFromFields(version, ["version", "file_version", "fileVersion", "name"]),
        group_name: firstStringFromFields(group, ["name", "group_name", "groupName", "label"])
    };
}

function dependencyFromRange(dependency: Record<string, unknown>, range: Record<string, unknown>): NexusModDependency | null {
    const targetGroup = firstObjectFromFields(range, ["target_group", "targetGroup", "group", "update_group", "updateGroup"])
        ?? firstObjectFromFields(dependency, ["target_group", "targetGroup", "group", "update_group", "updateGroup"]);
    const mod = firstObjectFromFields(targetGroup, ["mod", "target_mod", "targetMod"])
        ?? firstObjectFromFields(range, ["mod", "target_mod", "targetMod"])
        ?? firstObjectFromFields(dependency, ["mod", "target_mod", "targetMod"]);
    const minVersion = firstObjectFromFields(range, ["min_version", "minVersion", "minimum_version", "minimumVersion"])
        ?? firstObjectFromFields(dependency, ["min_version", "minVersion", "minimum_version", "minimumVersion"]);
    const maxVersion = firstObjectFromFields(range, ["max_version", "maxVersion", "maximum_version", "maximumVersion"])
        ?? firstObjectFromFields(dependency, ["max_version", "maxVersion", "maximum_version", "maximumVersion"]);
    const minFile = firstObjectFromFields(minVersion, ["file", "mod_file", "modFile"])
        ?? (minVersion && recordHasAnyField(minVersion, ["game_scoped_id", "gameScopedId", "file_id", "fileId", "id", "name", "file_name", "fileName"]) ? minVersion : undefined);
    const maxFile = firstObjectFromFields(maxVersion, ["file", "mod_file", "modFile"])
        ?? (maxVersion && recordHasAnyField(maxVersion, ["game_scoped_id", "gameScopedId", "file_id", "fileId", "id", "name", "file_name", "fileName"]) ? maxVersion : undefined);
    const representativeFile = maxFile ?? minFile;
    const modId = firstNumberFromFields(mod, ["game_scoped_id", "gameScopedId", "mod_id", "modId", "game_scoped_mod_id", "gameScopedModId", "id"])
        ?? firstNumberFromFields(targetGroup, ["mod_id", "modId", "game_scoped_mod_id", "gameScopedModId"]);
    const fileId = firstNumberFromFields(representativeFile, ["game_scoped_id", "gameScopedId", "file_id", "fileId", "game_scoped_file_id", "gameScopedFileId", "id"]);
    const nexusFileId = firstIdentifierFromFields(representativeFile, ["nexus_file_id", "nexusFileId", "uid", "uuid", "file_uid", "fileUid", "file_uuid", "fileUuid", "id"])
        ?? firstIdentifierFromFields(range, ["nexus_file_id", "nexusFileId"])
        ?? firstIdentifierFromFields(dependency, ["nexus_file_id", "nexusFileId"]);
    const modName = firstStringFromFields(mod, ["name", "mod_name", "modName", "title"])
        ?? firstStringFromFields(targetGroup, ["name", "group_name", "groupName", "label"]);
    const minLabel = dependencyVersionLabel(minVersion, minFile);
    const maxLabel = dependencyVersionLabel(maxVersion, maxFile);
    const exactVersion = minLabel && maxLabel && normalizedDependencyVersion(minLabel) === normalizedDependencyVersion(maxLabel)
        ? minLabel
        : undefined;

    if (!modName && !modId) {
        return null;
    }

    return {
        id: stringField(range.id) ?? stringField(dependency.id) ?? `${modId ?? "mod"}:${fileId ?? "file"}`,
        mod_id: modId,
        file_id: fileId,
        nexus_file_id: nexusFileId,
        mod_name: modName ?? "Unknown dependency",
        file_name: firstStringFromFields(representativeFile, ["name", "file_name", "fileName", "logical_filename", "logicalFilename"]),
        version: exactVersion,
        version_requirement: dependencyRequirementLabel(minLabel, maxLabel),
        group_name: firstStringFromFields(targetGroup, ["name", "group_name", "groupName", "label"])
    };
}

function dependencyFromFlatRecord(dependency: Record<string, unknown>): NexusModDependency | null {
    const mod = firstObjectFromFields(dependency, ["mod", "target_mod", "targetMod"]);
    const file = firstObjectFromFields(dependency, ["file", "mod_file", "modFile", "target_file", "targetFile", "target"]);
    const group = firstObjectFromFields(dependency, ["group", "target_group", "targetGroup", "update_group", "updateGroup"]);
    const groupMod = firstObjectFromFields(group, ["mod", "target_mod", "targetMod"]);
    const modId = firstNumberFromFields(dependency, ["mod_id", "modId", "game_scoped_mod_id", "gameScopedModId"])
        ?? firstNumberFromFields(mod, ["game_scoped_id", "gameScopedId", "mod_id", "modId", "id"])
        ?? firstNumberFromFields(groupMod, ["game_scoped_id", "gameScopedId", "mod_id", "modId", "id"]);
    const fileId = firstNumberFromFields(dependency, ["file_id", "fileId", "game_scoped_file_id", "gameScopedFileId"])
        ?? firstNumberFromFields(file, ["game_scoped_id", "gameScopedId", "file_id", "fileId", "id"]);
    const nexusFileId = firstIdentifierFromFields(file, ["nexus_file_id", "nexusFileId", "uid", "uuid", "file_uid", "fileUid", "file_uuid", "fileUuid", "id"])
        ?? firstIdentifierFromFields(dependency, ["nexus_file_id", "nexusFileId"]);
    const modName = firstStringFromFields(dependency, ["mod_name", "modName", "name", "title"])
        ?? firstStringFromFields(mod, ["name", "mod_name", "modName", "title"])
        ?? firstStringFromFields(groupMod, ["name", "mod_name", "modName", "title"])
        ?? firstStringFromFields(group, ["name", "group_name", "groupName", "label"]);

    if (!modName && !modId) {
        return null;
    }

    return {
        id: stringField(dependency.id) ?? `${modId ?? "mod"}:${fileId ?? "file"}`,
        mod_id: modId,
        file_id: fileId,
        nexus_file_id: nexusFileId,
        mod_name: modName ?? "Unknown dependency",
        file_name: firstStringFromFields(dependency, ["file_name", "fileName", "logical_filename", "logicalFilename"])
            ?? firstStringFromFields(file, ["name", "file_name", "fileName", "logical_filename", "logicalFilename"]),
        version: firstStringFromFields(dependency, ["version", "file_version", "fileVersion", "required_version", "requiredVersion"])
            ?? firstStringFromFields(file, ["version", "file_version", "fileVersion"]),
        version_requirement: firstStringFromFields(dependency, ["version_requirement", "versionRequirement", "requirement", "version_range", "versionRange", "required_version_range", "requiredVersionRange"]),
        group_name: firstStringFromFields(dependency, ["group_name", "groupName"]) ?? firstStringFromFields(group, ["name", "group_name", "groupName", "label"])
    };
}

function dependencyVersionLabel(version: Record<string, unknown> | undefined, file: Record<string, unknown> | undefined): string | undefined {
    return firstStringFromFields(file, ["version", "file_version", "fileVersion"])
        ?? firstStringFromFields(version, ["version", "file_version", "fileVersion"])
        ?? firstStringFromFields(file, ["name", "file_name", "fileName", "logical_filename", "logicalFilename"])
        ?? firstStringFromFields(version, ["name"]);
}

function dependencyRequirementLabel(minVersion?: string, maxVersion?: string): string | undefined {
    if (minVersion && maxVersion) {
        return normalizedDependencyVersion(minVersion) === normalizedDependencyVersion(maxVersion)
            ? `Requires ${minVersion}`
            : `Requires ${minVersion} - ${maxVersion}`;
    }

    if (minVersion) {
        return `Requires ${minVersion} or newer`;
    }

    if (maxVersion) {
        return `Requires ${maxVersion} or older`;
    }

    return undefined;
}

function normalizedDependencyVersion(value?: string): string {
    return value?.trim().toLowerCase().replace(/^v\s*/, "") ?? "";
}

function dependencyCacheKey(fileId: number, nexusFileId?: string): string {
    return `${fileId}:${nexusFileId?.trim() ?? ""}`;
}

function normalizeChangelogs(raw: unknown): NexusModChangelog[] {
    const root = objectField(raw);
    const source = Array.isArray(raw)
        ? raw
        : Array.isArray(root?.data)
            ? root.data
            : Array.isArray(root?.changelogs)
                ? root.changelogs
                : root
                    ? Object.entries(root).map(([version, value]) => ({ version, value }))
                    : [];

    return source
        .map((record, index) => {
            const changelog = objectField(record);
            const nestedValue = changelog?.value;
            const nested = objectField(nestedValue);
            const version = stringField(changelog?.version)
                ?? stringField(changelog?.mod_version)
                ?? stringField(changelog?.name)
                ?? stringField(nested?.version)
                ?? stringField(nested?.mod_version)
                ?? `Changelog ${index + 1}`;
            const changes = stringField(changelog?.changes)
                ?? stringField(changelog?.changelog)
                ?? stringField(changelog?.description)
                ?? stringField(changelog?.body)
                ?? stringField(nestedValue)
                ?? stringField(nested?.changes)
                ?? stringField(nested?.changelog)
                ?? stringField(nested?.description);

            return {
                version,
                changes: cleanSummary(changes) ?? "",
                updated_at: stringField(changelog?.updated_at)
                    ?? stringField(changelog?.created_at)
                    ?? stringField(changelog?.date)
                    ?? stringField(nested?.updated_at)
            };
        })
        .filter(changelog => changelog.changes.length > 0);
}

function normalizeEndorsements(raw: unknown): NexusEndorsement[] {
    const root = objectField(raw);
    const source = Array.isArray(raw)
        ? raw
        : Array.isArray(root?.data)
            ? root.data
            : Array.isArray(root?.endorsements)
                ? root.endorsements
                : [];

    return source
        .map((record) => {
            const endorsement = objectField(record);
            const mod = objectField(endorsement?.mod);
            const game = objectField(endorsement?.game);
            const modId = numberField(endorsement?.mod_id)
                ?? numberField(endorsement?.id)
                ?? numberField(mod?.mod_id)
                ?? numberField(mod?.id)
                ?? 0;

            return {
                mod_id: modId,
                game_domain_name: stringField(endorsement?.game_domain_name)
                    ?? stringField(endorsement?.domain_name)
                    ?? stringField(game?.domain_name),
                status: stringField(endorsement?.status)
                    ?? stringField(endorsement?.endorsement_status),
                endorsed_at: stringField(endorsement?.endorsed_at)
                    ?? stringField(endorsement?.date)
                    ?? stringField(endorsement?.created_at)
            };
        })
        .filter(endorsement => endorsement.mod_id > 0);
}

function normalizeTrackedMods(raw: unknown): NexusTrackedMod[] {
    const root = objectField(raw);
    const source = Array.isArray(raw)
        ? raw
        : Array.isArray(root?.data)
            ? root.data
            : Array.isArray(root?.tracked_mods)
                ? root.tracked_mods
                : [];

    return source
        .map((record) => {
            const tracked = objectField(record);
            const mod = objectField(tracked?.mod);
            const game = objectField(tracked?.game);
            const modId = numberField(tracked?.mod_id)
                ?? numberField(tracked?.id)
                ?? numberField(mod?.mod_id)
                ?? numberField(mod?.id)
                ?? 0;

            return {
                mod_id: modId,
                name: stringField(tracked?.name)
                    ?? stringField(tracked?.mod_name)
                    ?? stringField(mod?.name),
                game_domain_name: stringField(tracked?.game_domain_name)
                    ?? stringField(tracked?.domain_name)
                    ?? stringField(game?.domain_name)
            };
        })
        .filter(tracked => tracked.mod_id > 0);
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

    if (/texture|graphic|visual|lighting|shader|hud|ui|radio|music|sound|audio/.test(text)) {
        return "Visuals";
    }

    if (/translation|language|chinese|japanese|korean|spanish|french|german|russian|save|backup|autosave|preset/.test(text)) {
        return "Miscellaneous";
    }

    if (/admin|cheat|trainer|debug|console|command|menu|performance|fps|logging|crash|fix|patch|build|cave|survival|item|weapon|enemy|npc|golf|vehicle|gameplay/.test(text)) {
        return "Gameplay";
    }

    return "Miscellaneous";
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

function imageUrlsFrom(raw: Record<string, unknown>): string[] {
    const urls: Array<string | undefined> = [
        stringField(raw.picture_url),
        stringField(raw.picture),
        stringField(raw.thumbnail_url),
        stringField(raw.screenshot_url),
        stringField(raw.image_url)
    ];

    for (const key of ["images", "image_urls", "screenshots", "screenshot_urls", "pictures", "media", "gallery", "mod_media"]) {
        collectImageUrls(raw[key], urls);
    }

    return uniqueImageUrls(urls);
}

function collectImageUrls(value: unknown, urls: Array<string | undefined>) {
    if (Array.isArray(value)) {
        for (const item of value) {
            collectImageUrls(item, urls);
        }
        return;
    }

    const text = stringField(value);
    if (text) {
        urls.push(text);
        return;
    }

    const object = objectField(value);
    if (!object) {
        return;
    }

    for (const key of ["url", "uri", "image_url", "imageUrl", "picture_url", "pictureUrl", "thumbnail_url", "thumbnailUrl", "screenshot_url", "screenshotUrl", "full", "original", "large", "medium", "small", "src"]) {
        urls.push(stringField(object[key]));
    }

    for (const key of ["image", "thumbnail", "picture", "screenshot"]) {
        collectImageUrls(object[key], urls);
    }
}

function uniqueImageUrls(values: Array<string | undefined>): string[] {
    const seen = new Set<string>();
    const urls: string[] = [];

    for (const value of values) {
        const url = normalizeImageUrl(value);
        if (!url || seen.has(url)) {
            continue;
        }

        seen.add(url);
        urls.push(url);
    }

    return urls.slice(0, 12);
}

function normalizeImageUrl(value?: string): string | undefined {
    const trimmed = decodeHtmlEntities(value ?? "").trim();
    const candidate = trimmed.startsWith("//")
        ? `https:${trimmed}`
        : /^www\./i.test(trimmed)
            ? `https://${trimmed}`
            : trimmed;

    try {
        const url = new URL(candidate);
        return url.protocol === "https:" || url.protocol === "http:" ? url.toString() : undefined;
    } catch {
        return undefined;
    }
}

function stringField(value: unknown): string | undefined {
    return typeof value === "string" && value.trim().length > 0 ? value.trim() : undefined;
}

function objectField(value: unknown): Record<string, unknown> | undefined {
    return typeof value === "object" && value !== null && !Array.isArray(value)
        ? value as Record<string, unknown>
        : undefined;
}

function graphRecord(value: unknown): Record<string, unknown> | undefined {
    const object = objectField(value);
    if (!object) {
        return undefined;
    }

    return objectField(object.node) ?? objectField(object.value) ?? object;
}

function objectArrayFromFields(source: Record<string, unknown> | undefined, keys: string[]): Array<Record<string, unknown>> {
    if (!source) {
        return [];
    }

    const records: Array<Record<string, unknown>> = [];
    for (const key of keys) {
        records.push(...objectArrayFromValue(source[key]));
    }

    return records;
}

function objectArrayFromValue(value: unknown, depth = 0): Array<Record<string, unknown>> {
    if (depth > 2 || value === undefined || value === null) {
        return [];
    }

    if (Array.isArray(value)) {
        return value
            .map(record => graphRecord(record))
            .filter((record): record is Record<string, unknown> => Boolean(record));
    }

    const object = graphRecord(value);
    if (!object) {
        return [];
    }

    for (const key of ["data", "nodes", "edges", "items", "results"]) {
        const nested = objectArrayFromValue(object[key], depth + 1);
        if (nested.length > 0) {
            return nested;
        }
    }

    return [object];
}

function firstObjectFromFields(source: Record<string, unknown> | undefined, keys: string[]): Record<string, unknown> | undefined {
    return objectArrayFromFields(source, keys)[0];
}

function firstStringFromFields(source: Record<string, unknown> | undefined, keys: string[]): string | undefined {
    if (!source) {
        return undefined;
    }

    for (const key of keys) {
        const value = stringField(source[key]);
        if (value) {
            return value;
        }
    }

    return undefined;
}

function firstIdentifierFromFields(source: Record<string, unknown> | undefined, keys: string[]): string | undefined {
    if (!source) {
        return undefined;
    }

    for (const key of keys) {
        const value = source[key];
        if (typeof value === "string" && value.trim().length > 0) {
            return value.trim();
        }

        if (typeof value === "number" && Number.isFinite(value)) {
            return `${value}`;
        }
    }

    return undefined;
}

function firstNumberFromFields(source: Record<string, unknown> | undefined, keys: string[]): number | undefined {
    if (!source) {
        return undefined;
    }

    for (const key of keys) {
        const value = numberField(source[key]);
        if (value !== undefined) {
            return value;
        }
    }

    return undefined;
}

function recordHasAnyField(source: Record<string, unknown> | undefined, keys: string[]): boolean {
    return Boolean(source && keys.some(key => source[key] !== undefined && source[key] !== null));
}

function isRootNexusCategory(name: string, categoryId?: number): boolean {
    return categoryId === 0 || /^(sons\s+of\s+the\s+forest|sonsoftheforest)$/i.test(name);
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

    const cleaned = decodeHtmlEntities(value)
        .replace(/<[^>]*>/g, " ")
        .replace(/\s+/g, " ")
        .trim();

    return cleaned || undefined;
}

function decodeHtmlEntities(value: string): string {
    let decoded = value;
    for (let pass = 0; pass < 2; pass += 1) {
        const next = decoded.replace(/&(#x[0-9a-f]+|#\d+|[a-z]+);/gi, (match, entity: string) => {
            const lower = entity.toLowerCase();
            if (lower.startsWith("#x")) {
                return decodeCodePoint(Number.parseInt(lower.slice(2), 16), match);
            }

            if (lower.startsWith("#")) {
                return decodeCodePoint(Number.parseInt(lower.slice(1), 10), match);
            }

            switch (lower) {
                case "amp":
                    return "&";
                case "lt":
                    return "<";
                case "gt":
                    return ">";
                case "quot":
                    return "\"";
                case "apos":
                case "rsquo":
                case "lsquo":
                    return "'";
                case "nbsp":
                case "ensp":
                case "emsp":
                    return " ";
                case "ndash":
                    return "-";
                case "mdash":
                    return "--";
                case "hellip":
                    return "...";
                case "copy":
                    return "(c)";
                case "reg":
                    return "(r)";
                case "trade":
                    return "(tm)";
                default:
                    return match;
            }
        });

        if (next === decoded) {
            break;
        }

        decoded = next;
    }

    return decoded;
}

function decodeCodePoint(codePoint: number, fallback: string): string {
    if (!Number.isFinite(codePoint) || codePoint <= 0 || codePoint > 0x10ffff) {
        return fallback;
    }

    if (codePoint < 32 && codePoint !== 9 && codePoint !== 10 && codePoint !== 13) {
        return " ";
    }

    try {
        return String.fromCodePoint(codePoint);
    } catch {
        return fallback;
    }
}
