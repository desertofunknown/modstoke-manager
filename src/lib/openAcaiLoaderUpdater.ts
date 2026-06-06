import * as path from "@tauri-apps/api/path";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { invoke } from "@tauri-apps/api/core";
import { download } from "@tauri-apps/plugin-upload";
import semver from "semver";
import { BaseZipInstaller } from "./baseZipInstaller";
import { getDirectoryPath, processName, processProgress } from "./store";
import { TempFileCache } from "./tempFileCache";
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

const LOADER_MANIFEST_URL = "https://github.com/desertofunknown/openacai-loader/releases/latest/download/OpenACAILoader.manifest.json";
const INSTALLED_MANIFEST_PATH = "BepInEx/plugins/OpenACAILoader/openacai-loader.manifest.json";
const CACHED_LATEST_MANIFEST_PATH = "BepInEx/plugins/OpenACAILoader/openacai-manager-latest-loader-manifest.json";

export class OpenAcaiLoaderInstaller extends BaseZipInstaller {
    private manifest: LoaderUpdateManifest | null = null;

    constructor() {
        super("Endnight Loader");
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
            console.log("Failed to get latest Endnight Loader version", error);
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
            throw new Error(`Failed to fetch Endnight Loader manifest: ${response.status}`);
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

    processName.set(`Verifying Endnight Loader files...`);
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

export async function ensureLatestOpenAcaiLoader(): Promise<LoaderIntegrityReport> {
    const manifest = await fetchLatestLoaderManifest();
    const report = await verifyInstalledLoader(manifest);
    if (!report.needsUpdate) {
        return report;
    }

    await downloadVerifyAndInstallLoader(manifest);
    return await verifyInstalledLoader(manifest);
}

export async function downloadVerifyAndInstallLoader(manifest: LoaderUpdateManifest): Promise<void> {
    const gameRoot = await getDirectoryPath();
    const tempPath = await TempFileCache.createFile();
    const existingBepInExConfig = await readExistingBepInExConfig(gameRoot);

    try {
        processName.set(`Downloading Endnight Loader ${manifest.version}...`);
        processProgress.set(0);

        let downloadProgress = 0;
        await download(
            manifest.package.downloadUrl,
            tempPath,
            ({ progress, total }) => {
                downloadProgress += progress;
                if (total > 0) {
                    processProgress.set(downloadProgress / total * 100);
                }
            }
        );

        processName.set("Verifying Endnight Loader package...");
        const packageHash = await hashFile(tempPath);
        if (packageHash.toUpperCase() !== manifest.package.sha256.toUpperCase()) {
            throw new Error(`Downloaded loader package hash mismatch. Expected ${manifest.package.sha256}, got ${packageHash}.`);
        }

        const inspection = await invoke<{ is_valid: boolean; errors: string[] }>("inspect_openacai_loader_zip", {
            source: tempPath
        });
        if (!inspection.is_valid) {
            throw new Error(inspection.errors.join("\n") || "Downloaded loader package failed validation.");
        }

        processName.set(`Installing Endnight Loader ${manifest.version}...`);
        await thisUnzip(tempPath, gameRoot);
        await restoreBepInExConfig(gameRoot, existingBepInExConfig);
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
    if (!manifest.version || !manifest.package?.downloadUrl || !manifest.package?.sha256 || !Array.isArray(manifest.files)) {
        throw new Error("Endnight Loader manifest is missing required update metadata.");
    }

    return {
        ...manifest,
        source
    };
}

async function writeCachedLatestManifest(manifest: LoaderUpdateManifest): Promise<void> {
    try {
        const gameRoot = await getDirectoryPath();
        const cacheDir = await path.join(gameRoot, "BepInEx/plugins/OpenACAILoader");
        const cachePath = await path.join(gameRoot, CACHED_LATEST_MANIFEST_PATH);
        const cacheableManifest = { ...manifest };
        delete cacheableManifest.source;

        await fs.mkdir(cacheDir, { recursive: true });
        await fs.writeTextFile(cachePath, JSON.stringify(cacheableManifest, null, 2));
    } catch (error) {
        console.log("Failed to cache latest Endnight Loader manifest", error);
    }
}

async function readCachedLatestManifest(): Promise<LoaderUpdateManifest | null> {
    try {
        const cachePath = await path.join(await getDirectoryPath(), CACHED_LATEST_MANIFEST_PATH);
        if (!await fs.exists(cachePath)) {
            return null;
        }

        return validateLoaderManifest(JSON.parse(await fs.readTextFile(cachePath)) as LoaderUpdateManifest, "cache");
    } catch (error) {
        console.log("Failed to read cached Endnight Loader manifest", error);
        return null;
    }
}

async function thisUnzip(sourcePath: string, destinationPath: string): Promise<void> {
    await invoke("unzip_handler", {
        source: sourcePath.replace(/\\/g, "/"),
        destination: destinationPath.replace(/\\/g, "/")
    });
}

async function readExistingBepInExConfig(gameRoot: string): Promise<string | null> {
    try {
        const configPath = await path.join(gameRoot, "BepInEx/config/BepInEx.cfg");
        return await fs.exists(configPath) ? await fs.readTextFile(configPath) : null;
    } catch {
        return null;
    }
}

async function restoreBepInExConfig(gameRoot: string, config: string | null): Promise<void> {
    if (config === null) {
        return;
    }

    const configPath = await path.join(gameRoot, "BepInEx/config/BepInEx.cfg");
    await fs.writeTextFile(configPath, config);
}
