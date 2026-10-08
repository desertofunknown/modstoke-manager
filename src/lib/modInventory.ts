import * as path from "@tauri-apps/api/path";
import { gameExePath, getDirectoryPath, isPathValid } from "./store";
import { get } from "svelte/store";
import * as fs from "@tauri-apps/plugin-fs"

export type LoaderType = "redloader-mod" | "redloader-library" | "bepinex-plugin";
export type InstallSource = "native" | "vortex" | "manual";

export type VortexDeploymentFile = {
    relPath: string;
    source: string;
    target?: string;
};

export type VortexDeployment = {
    detected: boolean;
    stagingPath: string | null;
    byPath: Map<string, string>;
};

export type InstalledInventoryEntry = {
    gameRoot: string;
    id: string;
    name: string;
    version?: string;
    author?: string;
    loaderType: LoaderType;
    manifestType?: string;
    expectedLocation: "Mods" | "Libs" | "BepInEx/plugins";
    installSource: InstallSource;
    store: string;
    vortexPackage?: string;
    nexusModId?: string;
    enabled: boolean;
    manifestPath?: string;
    packagePath?: string;
    assemblyPath?: string;
    matchKeys: string[];
};

type ManifestData = {
    id?: string;
    name?: string;
    author?: string;
    version?: string;
    type?: string;
};

type FsDirEntry = {
    name?: string;
    path?: string;
};

const MAX_SCAN_DEPTH = 4;
const BepInExSupportFolders = new Set(["openacailoader", "redloaderbepinexcompat"]);

export async function scanInstalledInventory(root?: string): Promise<InstalledInventoryEntry[]> {
    const gameRoot = root ?? await getDirectoryPath();
    const vortex = await readVortexDeployment(gameRoot);
    const entries: InstalledInventoryEntry[] = [];

    entries.push(...await scanRedLoaderRoot(gameRoot, "Mods", vortex));
    entries.push(...await scanRedLoaderRoot(gameRoot, "Libs", vortex));
    entries.push(...await scanBepInExPlugins(gameRoot, vortex));

    entries.sort((a, b) => a.name.localeCompare(b.name));
    return entries;
}

export async function readVortexDeployment(gameRoot: string): Promise<VortexDeployment> {
    const deployment: VortexDeployment = {
        detected: false,
        stagingPath: null,
        byPath: new Map<string, string>()
    };

    try {
        const deploymentPath = await path.join(gameRoot, "vortex.deployment.json");
        if (!await fs.exists(deploymentPath)) {
            return deployment;
        }

        const raw = await fs.readTextFile(deploymentPath);
        const parsed = JSON.parse(raw);
        deployment.detected = true;
        deployment.stagingPath = typeof parsed.stagingPath === "string" ? parsed.stagingPath : null;

        for (const file of parsed.files ?? []) {
            if (typeof file.relPath === "string" && typeof file.source === "string") {
                deployment.byPath.set(normalizePath(file.relPath), file.source);
            }
        }
    } catch (error) {
        console.log("Failed to read Vortex deployment metadata", error);
    }

    return deployment;
}

export function normalizeMatchKey(value: string | null | undefined): string {
    return (value ?? "")
        .toLowerCase()
        .replace(/&/g, "and")
        .replace(/[^a-z0-9]+/g, "");
}

export function findMatchingInstall(
    entries: InstalledInventoryEntry[],
    name: string | null | undefined,
    id?: string | number | null,
    aliases: string[] = []): InstalledInventoryEntry | null {
    const candidateKeys = [
        normalizeMatchKey(name),
        normalizeMatchKey(id?.toString()),
        ...aliases.map(alias => normalizeMatchKey(alias))
    ].filter(key => key.length > 0);

    return entries.find(entry =>
        entry.matchKeys.some(key => candidateKeys.includes(key))
        || (!!id && entry.nexusModId === id.toString())) ?? null;
}

export function describeInstallSource(entry: InstalledInventoryEntry | null | undefined): string {
    if (!entry) {
        return "Not installed";
    }

    if (entry.installSource === "vortex") {
        return entry.vortexPackage ? `Vortex: ${entry.vortexPackage}` : "Vortex / Nexus";
    }

    if (entry.installSource === "native") {
        return "OpenACAI native store";
    }

    return "Manual / local";
}

export async function assertCurrentGameRoot(gameRoot: string): Promise<void> {
    const exePath = get(gameExePath);
    const selectedRoot = await getDirectoryPath();
    if (!get(isPathValid) || exePath !== get(gameExePath)
        || normalizePath(gameRoot) !== normalizePath(selectedRoot)) {
        throw new Error("The selected game folder has changed. Refresh the installed mods before changing this mod.");
    }
}

export async function setInventoryEntryEnabled(entry: InstalledInventoryEntry, shouldEnable: boolean): Promise<void> {
    await assertCurrentGameRoot(entry.gameRoot);
    const gameRoot = entry.gameRoot;
    if (entry.installSource === "vortex") {
        throw new Error("This mod is managed by Vortex. Use Vortex to enable or disable it so deployment metadata stays consistent.");
    }

    if (entry.loaderType === "bepinex-plugin" && entry.packagePath
        && normalizePath(entry.packagePath).includes("/_disabled/")) {
        if (!shouldEnable) {
            return;
        }

        const packageName = await path.basename(entry.packagePath);
        const enabledPackagePath = await path.join(gameRoot, "BepInEx", "plugins", packageName);
        const oldPackagePath = entry.packagePath;
        await fs.rename(oldPackagePath, enabledPackagePath);
        entry.packagePath = enabledPackagePath;
        if (entry.assemblyPath) {
            entry.assemblyPath = enabledPackagePath + entry.assemblyPath.slice(oldPackagePath.length);
        }
        entry.enabled = !isDisabledPath(entry.assemblyPath);
        if (entry.enabled) {
            return;
        }
    }

    if (entry.assemblyPath) {
        const enabledAssemblyPath = entry.assemblyPath.replace(/\.disabled$/i, ".dll");
        const disabledAssemblyPath = entry.assemblyPath.replace(/\.dll$/i, ".disabled");

        if (shouldEnable && await fs.exists(disabledAssemblyPath)) {
            await fs.rename(disabledAssemblyPath, enabledAssemblyPath);
            if (entry.packagePath === entry.assemblyPath) {
                entry.packagePath = enabledAssemblyPath;
            }
            entry.assemblyPath = enabledAssemblyPath;
            entry.enabled = true;
            return;
        }

        if (!shouldEnable && await fs.exists(enabledAssemblyPath)) {
            await fs.rename(enabledAssemblyPath, disabledAssemblyPath);
            if (entry.packagePath === entry.assemblyPath) {
                entry.packagePath = disabledAssemblyPath;
            }
            entry.assemblyPath = disabledAssemblyPath;
            entry.enabled = false;
            return;
        }
    }

    if (!entry.packagePath) {
        entry.enabled = shouldEnable;
        return;
    }

    const packageName = await path.basename(entry.packagePath);
    const rootPath = entry.expectedLocation === "Libs"
        ? await path.join(gameRoot, "Libs")
        : entry.expectedLocation === "BepInEx/plugins"
            ? await path.join(gameRoot, "BepInEx", "plugins")
            : await path.join(gameRoot, "Mods");
    const enabledPackagePath = isDisabledPath(entry.packagePath)
        ? await path.join(rootPath, packageName)
        : entry.packagePath;
    const disabledPackagePath = await path.join(rootPath, "_Disabled", packageName);

    if (shouldEnable && await fs.exists(disabledPackagePath)) {
        await fs.mkdir(rootPath, { recursive: true });
        await fs.rename(disabledPackagePath, enabledPackagePath);
        entry.packagePath = enabledPackagePath;
        entry.enabled = true;
        return;
    }

    if (!shouldEnable && await fs.exists(enabledPackagePath)) {
        const disabledParent = await path.dirname(disabledPackagePath);
        await fs.mkdir(disabledParent, { recursive: true });
        await fs.rename(enabledPackagePath, disabledPackagePath);
        entry.packagePath = disabledPackagePath;
        entry.enabled = false;
    }
}

async function scanRedLoaderRoot(
    gameRoot: string,
    rootName: "Mods" | "Libs",
    vortex: VortexDeployment): Promise<InstalledInventoryEntry[]> {
    const rootPath = await path.join(gameRoot, rootName);
    const manifestFolders = await findManifestFolders(rootPath);
    const entries: InstalledInventoryEntry[] = [];

    for (const folderPath of manifestFolders) {
        if (isIgnoredPath(folderPath)) {
            continue;
        }

        const manifestPath = await path.join(folderPath, "manifest.json");
        const manifest = await readManifest(manifestPath);
        const manifestType = manifest.type ?? (rootName === "Libs" ? "Library" : "Mod");
        const loaderType: LoaderType = manifestType.toLowerCase() === "library"
            ? "redloader-library"
            : "redloader-mod";
        const assemblyPath = await findRedLoaderAssembly(folderPath);
        const source = await classifySource(gameRoot, folderPath, assemblyPath, loaderType, vortex);
        const packageName = await path.basename(folderPath);
        const id = manifest.id ?? packageName;
        const name = manifest.name ?? manifest.id ?? packageName;

        entries.push({
            gameRoot,
            id,
            name,
            version: manifest.version,
            author: manifest.author,
            loaderType,
            manifestType,
            expectedLocation: loaderType === "redloader-library" ? "Libs" : "Mods",
            installSource: source.installSource,
            store: source.store,
            vortexPackage: source.vortexPackage,
            nexusModId: source.nexusModId,
            enabled: !isDisabledPath(folderPath) && !isDisabledPath(assemblyPath),
            manifestPath,
            packagePath: folderPath,
            assemblyPath: assemblyPath ?? undefined,
            matchKeys: buildMatchKeys(id, name, packageName, assemblyPath, source.vortexPackage, source.nexusModId)
        });
    }

    return entries;
}

async function scanBepInExPlugins(gameRoot: string, vortex: VortexDeployment): Promise<InstalledInventoryEntry[]> {
    const pluginRoot = await path.join(gameRoot, "BepInEx", "plugins");
    const entries: InstalledInventoryEntry[] = [];

    if (!await fs.exists(pluginRoot)) {
        return entries;
    }

    for (const assemblyPath of await findFiles(pluginRoot, file => /\.(dll|disabled)$/i.test(file))) {
        if (await isBepInExSupportAssembly(pluginRoot, assemblyPath)) {
            continue;
        }

        const packagePath = await resolveBepInExPackagePath(pluginRoot, assemblyPath);
        const source = await classifySource(gameRoot, packagePath, assemblyPath, "bepinex-plugin", vortex);
        const assemblyName = stripExtension(await path.basename(assemblyPath));
        const packageName = await path.basename(packagePath);

        entries.push({
            gameRoot,
            id: assemblyName,
            name: assemblyName,
            loaderType: "bepinex-plugin",
            manifestType: "BepInExPlugin",
            expectedLocation: "BepInEx/plugins",
            installSource: source.installSource,
            store: source.store,
            vortexPackage: source.vortexPackage,
            nexusModId: source.nexusModId,
            enabled: !isDisabledPath(packagePath) && !isDisabledPath(assemblyPath),
            packagePath,
            assemblyPath,
            matchKeys: buildMatchKeys(assemblyName, packageName, undefined, assemblyPath, source.vortexPackage, source.nexusModId)
        });
    }

    return entries;
}

async function findManifestFolders(rootPath: string, depth: number = 0): Promise<string[]> {
    if (depth > MAX_SCAN_DEPTH || !await fs.exists(rootPath)) {
        return [];
    }

    const results: string[] = [];
    let files: any[] = [];
    try {
        files = await fs.readDir(rootPath);
    } catch {
        return results;
    }

    for (const file of files) {
        const filePath = await resolveDirEntryPath(rootPath, file);
        if (!filePath) {
            continue;
        }

        const name = file.name ?? await path.basename(filePath);
        if (name.toLowerCase() === "manifest.json") {
            results.push(await path.dirname(filePath));
            continue;
        }

        if (depth === MAX_SCAN_DEPTH || name.toLowerCase().startsWith(".") || name.toLowerCase().endsWith(".dll") || name.toLowerCase().endsWith(".disabled")) {
            continue;
        }

        results.push(...await findManifestFolders(filePath, depth + 1));
    }

    return results;
}

async function findFiles(rootPath: string, predicate: (fileName: string) => boolean, depth: number = 0): Promise<string[]> {
    if (depth > MAX_SCAN_DEPTH || !await fs.exists(rootPath)) {
        return [];
    }

    const results: string[] = [];
    let files: any[] = [];
    try {
        files = await fs.readDir(rootPath);
    } catch {
        return results;
    }

    for (const file of files) {
        const filePath = await resolveDirEntryPath(rootPath, file);
        if (!filePath) {
            continue;
        }

        const name = file.name ?? await path.basename(filePath);
        if (predicate(name)) {
            results.push(filePath);
            continue;
        }

        if (!name.startsWith(".") && !name.toLowerCase().endsWith(".dll")) {
            results.push(...await findFiles(filePath, predicate, depth + 1));
        }
    }

    return results;
}

async function resolveDirEntryPath(parentPath: string, entry: FsDirEntry): Promise<string | null> {
    if (entry.path) {
        return entry.path;
    }

    if (entry.name) {
        return await path.join(parentPath, entry.name);
    }

    return null;
}

async function readManifest(manifestPath: string): Promise<ManifestData> {
    try {
        return JSON.parse(await fs.readTextFile(manifestPath));
    } catch (error) {
        console.log(`Failed to read manifest ${manifestPath}`, error);
        return {};
    }
}

async function findRedLoaderAssembly(packagePath: string): Promise<string | null> {
    const packageName = await path.basename(packagePath);
    const parent = await path.dirname(packagePath);
    const siblingDll = await path.join(parent, `${packageName}.dll`);
    const siblingDisabled = await path.join(parent, `${packageName}.disabled`);

    if (await fs.exists(siblingDll)) {
        return siblingDll;
    }

    if (await fs.exists(siblingDisabled)) {
        return siblingDisabled;
    }

    const nested = await findFiles(packagePath, file => file.toLowerCase().endsWith(".dll") || file.toLowerCase().endsWith(".disabled"), 0);
    return nested[0] ?? null;
}

async function classifySource(
    gameRoot: string,
    packagePath: string,
    assemblyPath: string | null,
    loaderType: LoaderType,
    vortex: VortexDeployment): Promise<{ installSource: InstallSource; store: string; vortexPackage?: string; nexusModId?: string }> {
    const vortexPackage = findVortexPackage(gameRoot, packagePath, vortex) ?? findVortexPackage(gameRoot, assemblyPath, vortex);
    if (vortexPackage || await hasVortexMarker(packagePath)) {
        const packageName = vortexPackage ?? "Vortex package";
        return {
            installSource: "vortex",
            store: "Nexus Mods / Vortex",
            vortexPackage: packageName,
            nexusModId: extractNexusModId(packageName)
        };
    }

    if (loaderType === "bepinex-plugin") {
        return {
            installSource: "manual",
            store: "Manual / local"
        };
    }

    return {
        installSource: "native",
        store: "OpenACAI native store"
    };
}

function findVortexPackage(gameRoot: string, absolutePath: string | null | undefined, vortex: VortexDeployment): string | null {
    if (!absolutePath || !vortex.detected) {
        return null;
    }

    const relative = relativeToRoot(gameRoot, absolutePath);
    if (!relative) {
        return null;
    }

    const normalized = normalizePath(relative);
    const enabledPath = normalized
        .replace(/^(bepinex\/plugins|mods|libs)\/_disabled\//, "$1/")
        .replace(/\.disabled$/, ".dll");
    const exact = vortex.byPath.get(normalized) ?? vortex.byPath.get(enabledPath);
    if (exact) {
        return exact;
    }

    const prefix = normalized.replace(/\/+$/g, "") + "/";
    const enabledPrefix = enabledPath.replace(/\/+$/g, "") + "/";
    const counts = new Map<string, number>();
    for (const [relPath, source] of vortex.byPath.entries()) {
        if (relPath.startsWith(prefix) || relPath.startsWith(enabledPrefix)) {
            counts.set(source, (counts.get(source) ?? 0) + 1);
        }
    }

    let winner: string | null = null;
    let winnerCount = 0;
    for (const [source, count] of counts.entries()) {
        if (count > winnerCount) {
            winner = source;
            winnerCount = count;
        }
    }

    return winner;
}

async function hasVortexMarker(packagePath: string | null | undefined, depth: number = 0): Promise<boolean> {
    if (!packagePath || depth > 3 || !await fs.exists(packagePath)) {
        return false;
    }

    try {
        const markerPath = await path.join(packagePath, "__folder_managed_by_vortex");
        if (await fs.exists(markerPath)) {
            return true;
        }

        for (const file of await fs.readDir(packagePath)) {
            const childPath = await resolveDirEntryPath(packagePath, file);
            if (!childPath) {
                continue;
            }

            const name = file.name ?? await path.basename(childPath);
            if (!name.startsWith(".") && await hasVortexMarker(childPath, depth + 1)) {
                return true;
            }
        }
    } catch {
        return false;
    }

    return false;
}

async function isBepInExSupportAssembly(pluginRoot: string, assemblyPath: string): Promise<boolean> {
    const relative = relativeToRoot(pluginRoot, assemblyPath);
    if (!relative) {
        return true;
    }

    const segments = relative.split(/[\\/]/);
    const packageSegment = segments[0] === "_disabled" ? segments[1] : segments[0];
    return BepInExSupportFolders.has(packageSegment);
}

async function resolveBepInExPackagePath(pluginRoot: string, assemblyPath: string): Promise<string> {
    const relative = relativeToRoot(pluginRoot, assemblyPath);
    if (!relative || !relative.includes("/")) {
        return assemblyPath;
    }

    const segments = relative.split(/[\\/]/);
    if (segments[0] === "_disabled") {
        return segments.length === 2
            ? assemblyPath
            : await path.join(pluginRoot, segments[0], segments[1]);
    }

    return await path.join(pluginRoot, segments[0]);
}

function buildMatchKeys(...values: Array<string | null | undefined>): string[] {
    const keys = values
        .flatMap(value => value ? [value, stripVortexPackageNoise(value), stripExtension(value.split(/[\\/]/).pop() ?? value)] : [])
        .map(normalizeMatchKey)
        .filter(key => key.length > 0);

    return Array.from(new Set(keys));
}

function extractNexusModId(packageName: string | null | undefined): string | undefined {
    const match = packageName?.match(/-(\d+)(?:-[^\\/-]+)+-\d{8,}$/);
    return match?.[1];
}

function stripVortexPackageNoise(value: string): string {
    return value
        .replace(/-\d+-[\d-]+-\d{8,}$/g, "")
        .replace(/\s+v?\d+(?:\.\d+){1,4}\s*$/i, "")
        .trim();
}

function stripExtension(value: string): string {
    return value.replace(/\.(dll|disabled)$/i, "");
}

function normalizePath(value: string): string {
    return value.replace(/\\/g, "/").replace(/^\/+/g, "").toLowerCase();
}

function relativeToRoot(root: string, absolutePath: string): string | null {
    const normalizedRoot = normalizePath(root).replace(/\/+$/g, "");
    const normalizedPath = normalizePath(absolutePath);

    if (!normalizedPath.startsWith(normalizedRoot + "/")) {
        return null;
    }

    return normalizedPath.slice(normalizedRoot.length + 1);
}

function isIgnoredPath(value: string): boolean {
    const normalized = normalizePath(value);
    return normalized.includes("/.vs/");
}

function isDisabledPath(value: string | null | undefined): boolean {
    const normalized = normalizePath(value ?? "");
    return normalized.includes("/_disabled/") || normalized.endsWith(".disabled") || normalized.endsWith(".pending");
}
