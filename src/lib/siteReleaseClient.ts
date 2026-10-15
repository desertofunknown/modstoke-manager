import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import {
    assertSiteLoaderManifest,
    configuredSiteIndexUrl,
    parseSiteReleaseIndex,
    SITE_LOADER_MANIFEST_PATH,
    SITE_ORIGIN,
    type SiteLoaderRelease,
    type SiteReleaseIndex
} from "./siteReleaseIndex";
import type { LoaderUpdateManifest } from "./openAcaiLoaderUpdater";

const MAX_INDEX_TEXT = 1024 * 1024;
const MAX_MANIFEST_TEXT = 4 * 1024 * 1024;

export function siteIndexUrl(): string | null {
    // The production origin must be deliberately enabled by the release build.
    return configuredSiteIndexUrl(import.meta.env.VITE_MODSTOKE_RELEASE_INDEX_URL);
}

async function readSiteJson(url: string, maxLength: number): Promise<unknown> {
    const response = await tauriFetch(url, { method: "GET", headers: { "Cache-Control": "no-cache" } });
    if (!response.ok) {
        throw new Error(`Modstoke website returned HTTP ${response.status}.`);
    }
    const contentLength = Number(response.headers.get("content-length"));
    if (contentLength > maxLength) {
        throw new Error("Modstoke release metadata is too large.");
    }
    const body = await response.text();
    if (body.length > maxLength) {
        throw new Error("Modstoke release metadata is too large.");
    }
    try {
        return JSON.parse(body);
    } catch {
        throw new Error("Modstoke release metadata is not valid JSON.");
    }
}

export async function fetchSiteReleaseIndex(): Promise<SiteReleaseIndex | null> {
    const url = siteIndexUrl();
    if (!url) return null;
    return parseSiteReleaseIndex(await readSiteJson(url, MAX_INDEX_TEXT));
}

export async function fetchSiteLoaderManifest(loader: SiteLoaderRelease): Promise<LoaderUpdateManifest> {
    if (!siteIndexUrl()) {
        throw new Error("Website releases are not enabled in this Manager build.");
    }
    const raw = await readSiteJson(`${SITE_ORIGIN}${SITE_LOADER_MANIFEST_PATH}`, MAX_MANIFEST_TEXT);
    assertSiteLoaderManifest(raw, loader);
    return raw as LoaderUpdateManifest;
}
