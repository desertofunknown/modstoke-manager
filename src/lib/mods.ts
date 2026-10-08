import * as path from "@tauri-apps/api/path";
import { invoke } from '@tauri-apps/api/core';
import { gameExePath, getDirectoryPath, getLibsDir, getModsDir, isPathValid, processName, processProgress } from './store';
import { get } from 'svelte/store';
import { downloadAndInstall, showMessageBox, unzip } from './utils';
import { assertCurrentGameRoot, scanInstalledInventory, type InstallSource, type LoaderType } from './modInventory';
import * as fs from "@tauri-apps/plugin-fs"
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";

export type ModCategory = {
    id?: number;
    name: string;
    slug: string;
}

export type ModAuthor = {
    name: string;
    slug: string;
    imageUrl?: string;
    isTrusted?: boolean;
}

export type ModImage = {
    url?: string;
    isPrimary?: boolean;
    isThumbnail?: boolean;
}

export type ModVersion = {
    id?: number;
    version: string;
    isLatest?: boolean;
    changelog?: string;
    filename?: string;
    extension?: string;
    createdAt?: string;
    updatedAt?: string;
    _count?: {
        downloads?: number;
    };
}

export type Mod = {
    name: string;
    slug: string;
    mod_id: string;
    shortDescription: string;
    description?: string;
    isApproved: boolean;
    // category_name: string;
    // category_slug: string;
    category: ModCategory;
    // user_name: string;
    // user_slug: string;
    user: ModAuthor;
    imageUrl: string;
    images?: ModImage[];
    latestVersion: string;
    lastReleasedAt: string;
    type: string;
    modSide?: string | null;
    isMultiplayerCompatible?: boolean;
    requiresAllPlayers?: boolean;
    lastWeekDownloads?: number;
    downloads?: number;
    commentsCount?: number;
    favoritesCount?: number;
    reviewsCount?: number;
    averageRating?: number;
    sourceUrl?: string | null;
    dependencies: string[];
    versions?: ModVersion[];

    isInstalled: boolean;
    installedMod?: InstalledMod;
    hasUpdate: boolean;
}

export function modPreviewUrls(mod: Mod): string[] {
    const urls = [
        mod.imageUrl,
        ...(mod.images ?? []).map((image) => image.url)
    ];

    return Array.from(new Set(urls
        .map((url) => typeof url === "string" ? url.trim() : "")
        .filter((url) => url.length > 0 && !url.includes("placehold.co"))));
}

export function modDependencies(mod: Pick<Mod, "dependencies">): string[] {
    return normalizeModDependencies((mod as { dependencies?: unknown }).dependencies);
}

export function normalizeModDependencies(value: unknown): string[] {
    const rawDependencies = Array.isArray(value)
        ? value
        : typeof value === "string"
            ? value.split(/[,\n;]+/)
            : value == null
                ? []
                : [value];
    const dependencies = rawDependencies
        .map(dependencyIdentifier)
        .filter((dependency): dependency is string => Boolean(dependency));

    return Array.from(new Set(dependencies));
}

function dependencyIdentifier(value: unknown): string | null {
    if (typeof value === "string") {
        const trimmed = value.trim();
        return trimmed.length > 0 ? trimmed : null;
    }

    if (typeof value === "number") {
        return String(value);
    }

    if (!value || typeof value !== "object") {
        return null;
    }

    const record = value as Record<string, unknown>;
    for (const key of ["mod_id", "modId", "slug", "id", "name", "title"]) {
        const candidate = record[key];
        if (typeof candidate === "string" && candidate.trim().length > 0) {
            return candidate.trim();
        }

        if (typeof candidate === "number") {
            return String(candidate);
        }
    }

    return null;
}

type RequestMeta = {
    limit: number;
    next_page: number;
    page: number;
    pages: number;
    prev_page: number;
    total: number;
}

type EndpointResponse = {
    status?: boolean;
    meta: RequestMeta;
    data: Mod[];
}

type CategoriesResponse = {
    status?: boolean;
    data: ModCategory[];
}

export type ModManifest = {
    id: string;
    author: string;
    version: string;
    type: string;
}

export type InstalledMod = {
    gameRoot: string;
    modName: string;
    isEnabled: boolean;
    manifest: ModManifest;
    loaderType?: LoaderType;
    installSource?: InstallSource;
    store?: string;
    vortexPackage?: string;
    nexusModId?: string;
    expectedLocation?: string;
    matchKeys?: string[];
    assemblyPath?: string;
    enabledAssemblyPath?: string;
    disabledAssemblyPath?: string;
    packagePath?: string;
    enabledPackagePath?: string;
    disabledPackagePath?: string;
}

type FsDirEntry = {
    name?: string;
    path?: string;
}

export type ModList = {
    mods: any[];
}

export enum Sorting {
    newest = "newest",
}

const MOD_REPOSITORY_API = "https://api.sotf-mods.com/api/";
const MOD_REPOSITORY_WEB = "https://sotf-mods.com";
const MAX_MANIFEST_SCAN_DEPTH = 4;

export class ModDatabase {

    private static mods: Mod[] = [];
    private static unapprovedMods: Mod[] | null;
    private static nsfwMods: Mod[] | null;
    private static installedMods: InstalledMod[] = [];
    private static installedModsExePath: string | null = null;
    private static installedModsRequest: { exePath: string; promise: Promise<void> } | null = null;
    private static installedScanGeneration = 0;

    public static async fetchMods(
            page: number, 
            sorting: Sorting = Sorting.newest,
            approved: boolean = true, 
            nsfw: boolean = false,
            searchTerm: string | null = null,
            categorySlug: string | null = null,
            typeFilter: string | null = null): Promise<EndpointResponse> {

        let url = `${MOD_REPOSITORY_API}mods?&approved=${approved}&orderby=${sorting}&page=${page}&nsfw=${nsfw}`;
        if(searchTerm) searchTerm = searchTerm.trim();
        if(searchTerm && searchTerm !== "")
        {
            url += "&search=" + encodeURIComponent(searchTerm);
        }
        if (categorySlug && categorySlug !== "all") {
            url += "&category=" + encodeURIComponent(categorySlug);
        }
        if (typeFilter && typeFilter !== "all") {
            url += "&type=" + encodeURIComponent(typeFilter);
        }

        const result = await this.fetchJson<EndpointResponse>(url);
        result.data = this.normalizeMods(result.data);
        return result;
    }

    public static async fetchCategories(): Promise<ModCategory[]> {
        const result = await this.fetchJson<CategoriesResponse>(`${MOD_REPOSITORY_API}categories`);
        return Array.isArray(result.data) ? result.data : [];
    }

    public static async fetchMod(id: string): Promise<Mod | null> {
        let resultData = await this.fetchJson<any>(MOD_REPOSITORY_API + "mods/" + encodeURIComponent(id));
        if(resultData.status === false){
            return null;
        }

        return this.normalizeMod(resultData.data as Mod);
    }

    public static async fetchAllMods(sorting: Sorting = Sorting.newest, approved: boolean = true, nsfw: boolean = false): Promise<Mod[]> {
        processName.set("Getting initial mod page");
        processProgress.set(0);
        let result = await this.fetchMods(1, sorting, approved, nsfw);
        let meta = result.meta;
        let mods = result.data as Mod[];

        if(meta.pages > 1) {
            for(let i = 2; i <= meta.pages; i++) {
                try {
                    processName.set(`Getting mod page ${i}/${meta.pages}`);
                    let pageResult = await this.fetchMods(i, sorting, approved, nsfw);
                    mods = mods.concat(pageResult.data);
                    processProgress.set(i / meta.pages * 100);
                } catch (error) {
                    showMessageBox("Error", `Failed to load mods page ${i}: ${error}!`);
                    break;
                }
            }
        }

        return mods;
    }

    public static getMods(): Mod[] {
        return this.mods;
    }

    public static async getInstalledMods(): Promise<Mod[]> {
        await this.initDatabase();
        return await Promise.all(this.currentInstalledMods().map(async m=>{
            let remoteMod: Mod | null = null;
            try {
                remoteMod = await this.fetchMod(m.modName);
            } catch (error) {
                console.log(`failed to fetch remote mod metadata for ${m.modName}`, error);
            }

            if( remoteMod ) {
                remoteMod.isInstalled = true;
                remoteMod.installedMod = m;
                return remoteMod;
            }


            // return {
            //     name: m.modName,
            //     mod_id: m.manifest.id,
            //     user_name: m.manifest.author,
            //     latestVersion: m.manifest.version,
            //     isInstalled: true,
            //     installedMod: m
            // } as Mod
            return {
                name: m.modName,
                mod_id: m.manifest.id,
                user: {
                    name: m.manifest.author,
                    slug: m.manifest.author
                } as ModAuthor,
                latestVersion: m.manifest.version,
                type: m.manifest.type,
                isInstalled: true,
                installedMod: m
            } as Mod
        }));
    }

    public static async getUnapprovedMods(force: boolean = false): Promise<Mod[]> {
        if(!this.unapprovedMods || this.unapprovedMods.length === 0 || force) {
            let mods = await this.fetchAllMods(Sorting.newest, false);
            this.unapprovedMods = mods;
        }

        return this.unapprovedMods;
    }

    public static async getNsfwMods(force: boolean = false): Promise<Mod[]> {
        if(!this.nsfwMods || this.nsfwMods.length === 0 || force) {
            let mods = await this.fetchAllMods(Sorting.newest, true, true);
            this.nsfwMods = mods;
        }

        return this.nsfwMods;
    }

    public static async loadMods(force: boolean = false): Promise<void> {
        if (!this.mods || this.mods.length === 0 || force) {
            this.mods = await this.fetchAllMods();
        }
    }

    public static openModPage(mod: Mod): void {
        window.open(this.getModPageUrl(mod));
    }

    public static getModPageUrl(mod: Mod): string {
        return `${MOD_REPOSITORY_WEB}/mods/${mod.user.slug}/${mod.slug}`;
    }

    private static async fetchJson<T>(url: string): Promise<T> {
        const result = await tauriFetch(url, { method: "GET" });
        if (!result.ok) {
            throw new Error(`Request failed: ${result.status} ${result.statusText}`);
        }

        return await result.json();
    }

    private static normalizeMods(mods: Mod[] | undefined): Mod[] {
        return Array.isArray(mods) ? mods.map(mod => this.normalizeMod(mod)) : [];
    }

    private static normalizeMod(mod: Mod): Mod {
        mod.dependencies = modDependencies(mod);
        return mod;
    }

    private static async initInstalledMod(
        gameRoot: string,
        folderPath: string,
        isEnabled: boolean,
        assemblyPath?: string,
        enabledAssemblyPath?: string,
        disabledAssemblyPath?: string,
        packagePath?: string,
        enabledPackagePath?: string,
        disabledPackagePath?: string): Promise<InstalledMod | null> {
        try {
            let modManifestPath = await path.join(folderPath, "manifest.json");
            let modManifest = await fs.readTextFile(modManifestPath);
            let manifest = JSON.parse(modManifest);
            return {
                gameRoot,
                manifest: manifest,
                isEnabled: isEnabled,
                modName: manifest.id ?? await path.basename(folderPath),
                assemblyPath,
                enabledAssemblyPath,
                disabledAssemblyPath,
                packagePath: packagePath ?? folderPath,
                enabledPackagePath: enabledPackagePath ?? folderPath,
                disabledPackagePath
            }
        } catch (error) {
            console.log(`failed to load mod ${await path.basename(folderPath)}`, error);
            return null;
        }
    }

    private static async readDirSafe(folderPath: string, logErrors: boolean = true): Promise<any[]> {
        try {
            if (!await fs.exists(folderPath)) {
                return [];
            }

            return await fs.readDir(folderPath);
        } catch (error) {
            if (logErrors) {
                console.log(`failed to read directory ${folderPath}`, error);
            }

            return [];
        }
    }

    private static async resolveDirEntryPath(parentPath: string, entry: FsDirEntry): Promise<string | null> {
        if (entry.path) {
            return entry.path;
        }

        if (entry.name) {
            return await path.join(parentPath, entry.name);
        }

        return null;
    }

    private static async findManifestFolders(folderPath: string, depth: number = 0): Promise<string[]> {
        if (depth > MAX_MANIFEST_SCAN_DEPTH) {
            return [];
        }

        let results: string[] = [];
        const files = await this.readDirSafe(folderPath);
        for (const file of files) {
            const filePath = await this.resolveDirEntryPath(folderPath, file);
            if (!filePath) {
                continue;
            }

            const name = file.name ?? await path.basename(filePath);
            if (name.toLowerCase() === "manifest.json") {
                results.push(await path.dirname(filePath));
                continue;
            }

            if (depth === MAX_MANIFEST_SCAN_DEPTH || name.toLowerCase().endsWith(".dll") || name.toLowerCase().endsWith(".disabled")) {
                continue;
            }

            const children = await this.readDirSafe(filePath, false);
            if (children.length !== 0) {
                results = results.concat(await this.findManifestFolders(filePath, depth + 1));
            }
        }

        return results;
    }

    private static normalizePath(value: string): string {
        return value.replace(/\\/g, "/").replace(/\/+$/g, "").toLowerCase();
    }

    private static isDisabledPackagePath(folderPath: string): boolean {
        const normalized = this.normalizePath(folderPath);
        return normalized.includes("/_disabled/") || normalized.endsWith("/_disabled");
    }

    private static addInstalledMod(mods: InstalledMod[], mod: InstalledMod | null): void {
        if (!mod) {
            return;
        }

        const existing = mods.find(existingMod =>
            existingMod.manifest.id === mod.manifest.id || existingMod.modName === mod.modName);
        if (!existing) {
            mods.push(mod);
        }
    }

    public static async loadInstalledMods(): Promise<void> {
        const exePath = get(gameExePath);
        if (!exePath || !get(isPathValid)) {
            this.invalidateInstalledMods();
            return;
        }
        if (this.installedModsRequest?.exePath === exePath) {
            return this.installedModsRequest.promise;
        }

        const generation = ++this.installedScanGeneration;
        const promise = (async () => {
            try {
                const gameRoot = await path.dirname(exePath);
                const mods = await this.readInstalledMods(gameRoot);
                if (generation === this.installedScanGeneration
                    && exePath === get(gameExePath) && get(isPathValid)) {
                    this.installedMods = mods;
                    this.installedModsExePath = exePath;
                }
            } finally {
                if (generation === this.installedScanGeneration) {
                    this.installedModsRequest = null;
                }
            }
        })();
        this.installedModsRequest = { exePath, promise };
        return promise;
    }

    public static invalidateInstalledMods(): void {
        this.installedMods = [];
        this.installedModsExePath = null;
        this.installedModsRequest = null;
        this.installedScanGeneration += 1;
    }

    private static currentInstalledMods(): InstalledMod[] {
        return get(isPathValid) && this.installedModsExePath === get(gameExePath)
            ? this.installedMods
            : [];
    }

    private static async readInstalledMods(gameRoot: string): Promise<InstalledMod[]> {
        const mods: InstalledMod[] = [];

        const inventory = await scanInstalledInventory(gameRoot);
        for (const entry of inventory) {
            if (entry.loaderType === "bepinex-plugin") {
                continue;
            }

            const packageName = entry.packagePath ? await path.basename(entry.packagePath) : entry.id;
            const redLoaderRoot = await path.join(gameRoot, entry.loaderType === "redloader-library" ? "Libs" : "Mods");
            const enabledPackagePath = entry.packagePath && this.isDisabledPackagePath(entry.packagePath)
                ? await path.join(redLoaderRoot, packageName)
                : entry.packagePath;
            const disabledPackagePath = await path.join(redLoaderRoot, "_Disabled", packageName);
            const enabledAssemblyPath = entry.assemblyPath
                ? entry.assemblyPath.replace(/\.disabled$/i, ".dll")
                : undefined;
            const disabledAssemblyPath = entry.assemblyPath
                ? entry.assemblyPath.replace(/\.dll$/i, ".disabled")
                : undefined;

            this.addInstalledMod(mods, {
                gameRoot,
                modName: entry.id,
                isEnabled: entry.enabled,
                manifest: {
                    id: entry.id,
                    author: entry.author ?? "Unknown",
                    version: entry.version ?? "",
                    type: entry.manifestType ?? (entry.loaderType === "redloader-library" ? "Library" : "Mod")
                },
                loaderType: entry.loaderType,
                installSource: entry.installSource,
                store: entry.store,
                vortexPackage: entry.vortexPackage,
                nexusModId: entry.nexusModId,
                expectedLocation: entry.expectedLocation,
                matchKeys: entry.matchKeys,
                assemblyPath: entry.assemblyPath,
                enabledAssemblyPath,
                disabledAssemblyPath,
                packagePath: entry.packagePath,
                enabledPackagePath,
                disabledPackagePath
            });
        }

        if (mods.length !== 0) {
            return mods;
        }

        const modPath = await path.join(gameRoot, "Mods");
        const libPath = await path.join(gameRoot, "Libs");
        const scannedManifestFolders = new Set<string>();

        for (const rootPath of [modPath, libPath]) {
            const files = await this.readDirSafe(rootPath);
            for (const file of files) {
                const filePath = await this.resolveDirEntryPath(rootPath, file);
                if (!filePath) {
                    continue;
                }

                const fileName = file.name ?? await path.basename(filePath);
                if(fileName.endsWith(".dll") || fileName.endsWith(".disabled")){
                    let isEnabled = fileName.endsWith(".dll");
                    let folderName = fileName.replace(".dll", "").replace(".disabled", "");
                    let folderPath = await path.join(rootPath, folderName);
                    let enabledAssemblyPath = await path.join(rootPath, folderName + ".dll");
                    let disabledAssemblyPath = await path.join(rootPath, folderName + ".disabled");

                    if(!(await fs.exists(folderPath)))
                    {
                        console.log("folder does not exist for mod", folderName);
                        continue;
                    }

                    scannedManifestFolders.add(this.normalizePath(folderPath));
                    let mod = await this.initInstalledMod(
                        gameRoot,
                        folderPath,
                        isEnabled,
                        filePath,
                        enabledAssemblyPath,
                        disabledAssemblyPath,
                        folderPath,
                        folderPath);
                    this.addInstalledMod(mods, mod);
                }
            }
        }

        for (const rootPath of [modPath, libPath]) {
            const manifestFolders = await this.findManifestFolders(rootPath);
            for (const manifestFolder of manifestFolders) {
                const key = this.normalizePath(manifestFolder);
                if (scannedManifestFolders.has(key)) {
                    continue;
                }

                const packageName = await path.basename(manifestFolder);
                const disabled = this.isDisabledPackagePath(manifestFolder);
                const disabledPath = await path.join(rootPath, "_Disabled", packageName);
                const enabledPath = disabled
                    ? await path.join(rootPath, packageName)
                    : manifestFolder;

                let mod = await this.initInstalledMod(
                    gameRoot,
                    manifestFolder,
                    !disabled,
                    undefined,
                    undefined,
                    undefined,
                    manifestFolder,
                    enabledPath,
                    disabled ? manifestFolder : disabledPath);
                this.addInstalledMod(mods, mod);
            }
        }
        return mods;
    }

    public static async refreshAll(forceRefresh: boolean): Promise<void> {

        // making sure the mods and libs directory exists
        processName.set("Checking mods directory");
        await getModsDir();
        await getLibsDir();

        processName.set("Retrieving mods");
        await this.loadMods(forceRefresh);
        
        processName.set("Checking installed mods");
        await this.loadInstalledMods();

        processName.set("Updating mod list");
        this.initModList(this.mods);
    }

    public static async initDatabase(): Promise<void> {
        if (get(isPathValid) && this.installedModsExePath === get(gameExePath)) {
            return;
        }

        await this.loadInstalledMods();
    }

    public static initModList(modList: Mod[]): void {
        const installedMods = this.currentInstalledMods();
        modList.forEach(mod => {
            let installedMod = installedMods.find(installedMod => installedMod.manifest.id === mod.mod_id);
            mod.isInstalled = installedMod !== undefined;
            mod.installedMod = installedMod;
            mod.hasUpdate = mod.isInstalled && installedMod?.manifest.version !== mod.latestVersion;
        });
    }

    public static async loadPage(page: number): Promise<void> {
        await this.initDatabase();
        let res = await this.fetchMods(page);
        
    }

    public static getInstalledMod(modId: string): InstalledMod | undefined {
        return this.currentInstalledMods().find(mod => mod.modName === modId);
    }

    public static async installMod(mod: Mod): Promise<void> {
        const gameRoot = await getDirectoryPath();
        await this.installModAtRoot(mod, gameRoot, new Set<string>());
    }

    public static async updateMod(mod: Mod): Promise<void> {
        const installedMod = mod.installedMod;
        if (!installedMod) {
            throw new Error(`${mod.name} is not installed. Refresh the installed mods before updating it.`);
        }
        if (installedMod.installSource === "vortex") {
            throw new Error("This mod is managed by Vortex. Use Vortex to update it so deployment metadata stays consistent.");
        }

        const gameRoot = installedMod.gameRoot;
        await this.installModAtRoot(mod, gameRoot, new Set<string>(), installedMod);
    }

    private static async installModAtRoot(
        mod: Mod,
        gameRoot: string,
        visited: Set<string>,
        previousInstall?: InstalledMod): Promise<void> {
        if (visited.has(mod.mod_id)) {
            return;
        }
        visited.add(mod.mod_id);

        await assertCurrentGameRoot(gameRoot);
        const modUrl = `${MOD_REPOSITORY_WEB}/mods/${mod.user.slug}/${mod.slug}/download/${mod.latestVersion}`;
        await downloadAndInstall(gameRoot, modUrl, mod.name, async (sourcePath) => {
            await assertCurrentGameRoot(gameRoot);
            if (previousInstall) {
                await invoke('update_native_mod', {
                    source: sourcePath,
                    destination: gameRoot,
                    previousInstall: {
                        assemblyPath: previousInstall.assemblyPath,
                        enabledAssemblyPath: previousInstall.enabledAssemblyPath,
                        packagePath: previousInstall.packagePath,
                        enabledPackagePath: previousInstall.enabledPackagePath,
                        isEnabled: previousInstall.isEnabled
                    }
                });
            } else {
                await unzip(sourcePath, gameRoot);
            }
        });

        for (const dependency of modDependencies(mod)) {
            if (visited.has(dependency)) {
                continue;
            }

            const dependencyMod = await this.fetchMod(dependency);
            if (dependencyMod && !visited.has(dependencyMod.mod_id)) {
                await showMessageBox("Installing dependency", `Installing dependency ${dependency} for mod ${mod.name} (a refresh may be needed to show the dependency as installed)`);
                await this.installModAtRoot(dependencyMod, gameRoot, visited);
            }
        }
    }

    private static async getPathsForMod(mod: InstalledMod): Promise<[string | undefined, string | undefined]> {
        await assertCurrentGameRoot(mod.gameRoot);
        if (mod.assemblyPath || mod.packagePath) {
            return [mod.assemblyPath, mod.packagePath];
        }

        const gamePath = mod.gameRoot;

        let subFolder = mod.manifest.type == "Mod" ? "Mods" : "Libs";
        let modDllPath = await path.join(gamePath, subFolder, mod.modName + (mod.isEnabled ? ".dll" : ".disabled"));
        let modFolder = await path.join(gamePath, subFolder, mod.modName);

        return [modDllPath, modFolder];
    }

    public static async uninstallMod(mod: InstalledMod): Promise<void> {
        let [modDllPath, modFolder] = await this.getPathsForMod(mod);

        if(modDllPath && await fs.exists(modDllPath)) await fs.remove(modDllPath);
        if(modFolder && await fs.exists(modFolder)) await fs.remove(modFolder, { recursive: true });

        //await this.refreshAll(false);
    }

    public static async toggleMod(mod: InstalledMod, shouldEnable: boolean): Promise<void> {
        await assertCurrentGameRoot(mod.gameRoot);
        if (mod.enabledAssemblyPath && mod.disabledAssemblyPath) {
            if(shouldEnable && (await fs.exists(mod.disabledAssemblyPath))) {
                await fs.rename(mod.disabledAssemblyPath, mod.enabledAssemblyPath);
                mod.isEnabled = true;
                mod.assemblyPath = mod.enabledAssemblyPath;
            } else if(!shouldEnable && (await fs.exists(mod.enabledAssemblyPath))) {
                await fs.rename(mod.enabledAssemblyPath, mod.disabledAssemblyPath);
                mod.isEnabled = false;
                mod.assemblyPath = mod.disabledAssemblyPath;
            }

            return;
        }

        if (mod.enabledPackagePath && mod.disabledPackagePath) {
            if(shouldEnable && (await fs.exists(mod.disabledPackagePath))) {
                const parentDir = await path.dirname(mod.enabledPackagePath);
                if (!await fs.exists(parentDir)) {
                    await fs.mkdir(parentDir, { recursive: true });
                }

                await fs.rename(mod.disabledPackagePath, mod.enabledPackagePath);
                mod.isEnabled = true;
                mod.packagePath = mod.enabledPackagePath;
            } else if(!shouldEnable && (await fs.exists(mod.enabledPackagePath))) {
                const parentDir = await path.dirname(mod.disabledPackagePath);
                if (!await fs.exists(parentDir)) {
                    await fs.mkdir(parentDir, { recursive: true });
                }

                await fs.rename(mod.enabledPackagePath, mod.disabledPackagePath);
                mod.isEnabled = false;
                mod.packagePath = mod.disabledPackagePath;
            }

            return;
        }

        const modPath = mod.gameRoot;
        let modDllPath = await path.join(modPath, "Mods", `${mod.modName}.dll`);
        let modDisabledPath = await path.join(modPath, "Mods", `${mod.modName}.disabled`);

        if(shouldEnable && (await fs.exists(modDisabledPath))) {
            await fs.rename(modDisabledPath, modDllPath);
            mod.isEnabled = true;
        } else if(!shouldEnable && (await fs.exists(modDllPath))) {
            await fs.rename(modDllPath, modDisabledPath);
            mod.isEnabled = false;
        }
    }
}
