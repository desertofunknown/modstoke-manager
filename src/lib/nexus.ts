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
};

type RawNexusModsResponse = {
    mods: Array<NexusMod & Record<string, unknown>> | { data?: Array<NexusMod & Record<string, unknown>> };
    rate_limit: NexusRateLimit;
};

type NexusModsResponse = {
    mods: NexusMod[];
    rate_limit: NexusRateLimit;
};

export type NexusView = "trending" | "latest_added" | "latest_updated";

export const NEXUS_CACHE_TTL_MINUTES = 10;

const NEXUS_CACHE_TTL_MS = NEXUS_CACHE_TTL_MINUTES * 60 * 1000;

let sessionCache: { value: NexusSession; cachedAt: number } | null = null;
let sessionRequest: Promise<NexusSession> | null = null;
const modsCache = new Map<NexusView, { value: NexusModsResponse; cachedAt: number }>();
const modsRequests = new Map<NexusView, Promise<NexusModsResponse>>();

function cacheFresh(cachedAt: number): boolean {
    return Date.now() - cachedAt < NEXUS_CACHE_TTL_MS;
}

export function clearNexusClientCache(): void {
    sessionCache = null;
    sessionRequest = null;
    modsCache.clear();
    modsRequests.clear();
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
                mods: rawMods.map(normalizeNexusMod).filter(mod => mod.mod_id > 0)
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

export function getNexusModPageUrl(mod: NexusMod): string {
    return `https://www.nexusmods.com/sonsoftheforest/mods/${mod.mod_id}`;
}

export function getNexusModDownloadUrl(mod: NexusMod): string {
    return `${getNexusModPageUrl(mod)}?tab=files`;
}

function normalizeNexusMod(raw: NexusMod & Record<string, unknown>): NexusMod {
    const modId = numberField(raw.mod_id) ?? numberField(raw.id) ?? 0;
    const rawSummary = stringField(raw.summary)
        ?? stringField(raw.description)
        ?? stringField(raw.short_description);

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
        category_name: stringField(raw.category_name) ?? stringField(raw.category),
        endorsement_count: numberField(raw.endorsement_count),
        mod_downloads: numberField(raw.mod_downloads) ?? numberField(raw.downloads),
        mod_unique_downloads: numberField(raw.mod_unique_downloads) ?? numberField(raw.unique_downloads),
        created_timestamp: numberField(raw.created_timestamp),
        updated_timestamp: numberField(raw.updated_timestamp) ?? numberField(raw.latest_file_update),
        created_time: stringField(raw.created_time),
        updated_time: stringField(raw.updated_time)
    };
}

function stringField(value: unknown): string | undefined {
    return typeof value === "string" && value.trim().length > 0 ? value.trim() : undefined;
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
