import * as path from "@tauri-apps/api/path";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { invoke } from "@tauri-apps/api/core";
import semver from "semver";
import { BaseZipInstaller } from "./baseZipInstaller";
import { getDirectoryPath, processName, processProgress } from "./store";
import { TempFileCache } from "./tempFileCache";
import { downloadToCache, unzip } from "./utils";
import * as fs from "@tauri-apps/plugin-fs"

export type LoaderManifestFile = {
    path: string;
    sha256: string;
    size: number;
};

export type LoaderUpdateManifest = {
    schema: number;
    name: string;
    version: string;
    generatedUtc: string;
    source?: LoaderManifestSource;
    package: {
        fileName: string;
        downloadUrl: string;
        sha256: string;
    };
    update: {
        manifestUrl: string;
        minManagerVersion: string;
    };
    runtime: {
        targetFramework: string;
        dotnetRuntimeVersion: string;
        supportPolicy: string;
        redLoaderCompatibilityTarget: string;
    };
    files: LoaderManifestFile[];
};

export type LoaderManifestSource = "remote" | "cache";

export type LoaderIntegrityIssue = {
    path: string;
    reason: "missing" | "hash-mismatch";
    expectedSha256?: string;
    actualSha256?: string;
};

export type LoaderIntegrityReport = {
    manifest: LoaderUpdateManifest;
    installedVersion: string | null;
    needsUpdate: boolean;
    versionOutdated: boolean;
    totalFiles: number;
    verifiedFiles: number;
    checkedAtUtc: string;
    latestManifestSource: LoaderManifestSource;
    issues: LoaderIntegrityIssue[];
};

const LOADER_MANIFEST_URL = "https://modstoke.com/releases/loader/latest/OpenACAILoader.manifest.json";
const INSTALLED_MANIFEST_PATH = "BepInEx/plugins/OpenACAILoader/openacai-loader.manifest.json";
// Kept with the downloads, where a mod archive installed into the game folder can't replace it.
const CACHED_LATEST_MANIFEST_FILE = "loader-manifest.json";

export class OpenAcaiLoaderInstaller extends BaseZipInstaller {
    private manifest: LoaderUpdateManifest | null = null;

    constructor() {
        super("Modstoke Loader");
    }

    public async prepare(): Promise<boolean> {
        this.manifest = await fetchLatestLoaderManifest();
        return true;
    }

    public async install(): Promise<void> {
        const manifest = this.manifest ?? await fetchLatestLoaderManifest();
        await downloadVerifyAndInstallLoader(manifest);
        this.manifest = null;
    }

    public async getTargetVersion(): Promise<string | null> {
        try {
            return (this.manifest ?? await fetchLatestLoaderManifest()).version;
        } catch (error) {
            console.log("Failed to get latest Modstoke Loader version", error);
            return null;
        }
    }
}

export async function fetchLatestLoaderManifest(): Promise<LoaderUpdateManifest> {
    try {
        const response = await tauriFetch(`${LOADER_MANIFEST_URL}?t=${Date.now()}`, {
            method: "GET"
        });
        if (response.status < 200 || response.status >= 300) {
            throw new Error(`Failed to fetch Modstoke Loader manifest: ${response.status}`);
        }

        const manifest = validateLoaderManifest(await response.json() as LoaderUpdateManifest, "remote");
        await writeCachedLatestManifest(manifest);
        return manifest;
    } catch (error) {
        const cachedManifest = await readCachedLatestManifest();
        if (cachedManifest) {
            return cachedManifest;
        }

        throw error;
    }
}

export async function verifyInstalledLoader(manifest?: LoaderUpdateManifest): Promise<LoaderIntegrityReport> {
    manifest = manifest ?? await fetchLatestLoaderManifest();
    const gameRoot = await getDirectoryPath();
    const installedVersion = await getInstalledLoaderVersion();
    const issues: LoaderIntegrityIssue[] = [];
    const totalFiles = manifest.files.length;
    let verifiedFiles = 0;

    processName.set(`Verifying Modstoke Loader files...`);
    processProgress.set(0);

    for (const [index, file] of manifest.files.entries()) {
        const installedPath = await path.join(gameRoot, file.path);
        if (!await fs.exists(installedPath)) {
            issues.push({
                path: file.path,
                reason: "missing",
                expectedSha256: file.sha256
            });
            processProgress.set(((index + 1) / Math.max(totalFiles, 1)) * 100);
            continue;
        }

        const actualSha256 = await hashFile(installedPath);
        if (actualSha256.toUpperCase() !== file.sha256.toUpperCase()) {
            issues.push({
                path: file.path,
                reason: "hash-mismatch",
                expectedSha256: file.sha256,
                actualSha256
            });
        } else {
            verifiedFiles += 1;
        }

        processProgress.set(((index + 1) / Math.max(totalFiles, 1)) * 100);
    }

    const versionOutdated = !!(installedVersion
        && semver.valid(installedVersion)
        && semver.valid(manifest.version)
        && semver.lt(installedVersion, manifest.version));

    const needsUpdate = !installedVersion || issues.length > 0;

    return {
        manifest,
        installedVersion,
        needsUpdate: !!needsUpdate,
        versionOutdated,
        totalFiles,
        verifiedFiles,
        checkedAtUtc: new Date().toISOString(),
        latestManifestSource: manifest.source ?? "remote",
        issues
    };
}

// From Start game, files that drifted from the installed version are repaired, but a different version
// is only installed when the player asks for it on the Loaders page. An older release is never installed
// over a newer one.
export async function ensureLatestOpenAcaiLoader(atLaunch = false): Promise<LoaderIntegrityReport> {
    const manifest = await fetchLatestLoaderManifest();
    const report = await verifyInstalledLoader(manifest);
    if (!report.needsUpdate) {
        return report;
    }

    const installed = report.installedVersion;
    if (installed && semver.valid(installed) && semver.valid(manifest.version) && semver.gt(installed, manifest.version)) {
        if (atLaunch) return report;
        throw new Error(`The published Modstoke Loader (${manifest.version}) is older than the installed one (${installed}). Nothing was changed.`);
    }
    if (atLaunch && installed !== manifest.version) {
        return report;
    }

    return await downloadVerifyAndInstallLoader(manifest);
}

export async function downloadVerifyAndInstallLoader(
    manifest: LoaderUpdateManifest
): Promise<LoaderIntegrityReport> {
    const gameRoot = await getDirectoryPath();

    try {
        processName.set(`Downloading Modstoke Loader ${manifest.version}...`);
        processProgress.set(0);
        // The downloader checks the package hash against the bytes it saved, so nothing is re-read here.
        const tempPath = await downloadToCache(manifest.package.downloadUrl, manifest.package.sha256);

        processName.set("Verifying Modstoke Loader package...");
        const inspection = await invoke<{ is_valid: boolean; errors: string[] }>("inspect_openacai_loader_zip", {
            source: tempPath
        });
        if (!inspection.is_valid) {
            throw new Error(inspection.errors.join("\n") || "Downloaded loader package failed validation.");
        }

        processName.set(`Installing Modstoke Loader ${manifest.version}...`);
        await unzip(tempPath, gameRoot);

        const report = await verifyInstalledLoader(manifest);
        if (report.needsUpdate) {
            const details = report.issues.map(issue => `${issue.path}: ${issue.reason}`);
            if (!report.installedVersion) {
                details.unshift("Installed loader version metadata is missing or unreadable.");
            }

            throw new Error(`Modstoke Loader verification failed after installation:\n${details.join("\n")}`);
        }

        return report;
    } finally {
        await TempFileCache.clearCache();
    }
}

export async function getInstalledLoaderVersion(): Promise<string | null> {
    try {
        const manifestPath = await path.join(await getDirectoryPath(), INSTALLED_MANIFEST_PATH);
        if (!await fs.exists(manifestPath)) {
            return null;
        }

        const manifest = JSON.parse(await fs.readTextFile(manifestPath));
        return typeof manifest.version === "string" ? manifest.version : null;
    } catch {
        return null;
    }
}

async function hashFile(filePath: string): Promise<string> {
    return await invoke<string>("sha256_file", { path: filePath });
}

function validateLoaderManifest(manifest: LoaderUpdateManifest, source: LoaderManifestSource): LoaderUpdateManifest {
    if (manifest.schema !== 1 || !manifest.version || !manifest.package?.downloadUrl || !manifest.package?.sha256
        || !Array.isArray(manifest.files) || manifest.files.length === 0) {
        throw new Error("Modstoke Loader manifest is missing required update metadata.");
    }

    // Verification joins these paths onto the game folder, so nothing may point outside it.
    const paths = new Set<string>();
    for (const file of manifest.files) {
        const parts = typeof file?.path === "string" ? file.path.split("/") : [];
        if (!parts.length || parts.some(part => !part || part === "." || part === ".." || /[\\:\u0000]/.test(part))
            || typeof file.sha256 !== "string" || !/^[a-f0-9]{64}$/i.test(file.sha256)
            || !Number.isSafeInteger(file.size) || file.size < 0) {
            throw new Error("Modstoke Loader manifest contains an invalid file record.");
        }

        const key = file.path.toLowerCase();
        if (paths.has(key)) {
            throw new Error("Modstoke Loader manifest contains duplicate file paths.");
        }
        paths.add(key);
    }

    return {
        ...manifest,
        source
    };
}

async function writeCachedLatestManifest(manifest: LoaderUpdateManifest): Promise<void> {
    try {
        const cacheDir = await path.join(await path.appCacheDir(), "downloads");
        const cachePath = await path.join(cacheDir, CACHED_LATEST_MANIFEST_FILE);
        const cacheableManifest = { ...manifest };
        delete cacheableManifest.source;

        await fs.mkdir(cacheDir, { recursive: true });
        await fs.writeTextFile(cachePath, JSON.stringify(cacheableManifest, null, 2));
    } catch (error) {
        console.log("Failed to cache latest Modstoke Loader manifest", error);
    }
}

async function readCachedLatestManifest(): Promise<LoaderUpdateManifest | null> {
    try {
        const cachePath = await path.join(await path.appCacheDir(), "downloads", CACHED_LATEST_MANIFEST_FILE);
        if (!await fs.exists(cachePath)) {
            return null;
        }

        return validateLoaderManifest(JSON.parse(await fs.readTextFile(cachePath)) as LoaderUpdateManifest, "cache");
    } catch (error) {
        console.log("Failed to read cached Modstoke Loader manifest", error);
        return null;
    }
}
