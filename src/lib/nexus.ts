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

export type NexusEndorsement = {
    mod_id: number;
    game_domain_name?: string;
    status?: string;
    endorsed_at?: string;
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

type RawNexusEndorsementsResponse = {
    endorsements: unknown;
    rate_limit: NexusRateLimit;
};

type NexusEndorsementsResponse = {
    endorsements: NexusEndorsement[];
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
const dependencyCache = new Map<number, { value: NexusModDependenciesResponse; cachedAt: number }>();
const dependencyRequests = new Map<number, Promise<NexusModDependenciesResponse>>();
let endorsementCache: { value: NexusEndorsementsResponse; cachedAt: number } | null = null;
let endorsementRequest: Promise<NexusEndorsementsResponse> | null = null;

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
    endorsementCache = null;
    endorsementRequest = null;
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

export async function endorseNexusSotfMod(modId: number, version?: string): Promise<NexusActionResponse> {
    const response = await invoke<NexusActionResponse>("nexus_endorse_sotf_mod", {
        modId,
        version: version?.trim() || undefined
    });
    endorsementCache = null;
    endorsementRequest = null;
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
    const name = stringField(category.name)
        ?? stringField(category.category_name)
        ?? stringField(category.title);

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
            : `name:${category.name.toLowerCase()}`;
        const existing = byKey.get(key);
        const merged = preferNexusCategory(existing, category);
        byKey.set(key, merged);
    }

    for (const category of byKey.values()) {
        const nameKey = category.name.toLowerCase();
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
    const leftLocalIndex = LOCAL_SOTF_NEXUS_CATEGORIES.findIndex(category => category.name === left.name);
    const rightLocalIndex = LOCAL_SOTF_NEXUS_CATEGORIES.findIndex(category => category.name === right.name);

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
        ? categories.find(category => category.name.toLowerCase() === rawName.toLowerCase())
        : undefined;

    if (categoryByName) {
        return categoryByName;
    }

    if (rawName && !isRootNexusCategory(rawName, categoryId)) {
        return {
            category_id: categoryById?.category_id ?? categoryId,
            name: rawName,
            parent_category_id: categoryById?.parent_category_id,
            source: categoryById?.source ?? "api"
        };
    }

    if (categoryById) {
        return categoryById;
    }

    return {
        name: inferNexusCategory(raw),
        source: "inferred"
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

function stringField(value: unknown): string | undefined {
    return typeof value === "string" && value.trim().length > 0 ? value.trim() : undefined;
}

function objectField(value: unknown): Record<string, unknown> | undefined {
    return typeof value === "object" && value !== null && !Array.isArray(value)
        ? value as Record<string, unknown>
        : undefined;
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

    return value
        .replace(/<[^>]*>/g, " ")
        .replace(/\s+/g, " ")
        .trim();
}
