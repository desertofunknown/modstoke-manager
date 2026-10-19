import { DebugInstaller } from "./debugInstaller";
import type { BaseInstaller } from "./baseInstaller";
import { getDirectoryPath, processName } from "./store";
import { BaseUninstaller } from "./baseUninstaller";
import { get } from "svelte/store";
import { GithubInstaller } from "./githubInstaller";
import { redLoaderInfo, unityExplorerInfo } from "./githubInfo";
import { invoke } from "@tauri-apps/api/core";
import * as path from "@tauri-apps/api/path";
import { gameExePath } from "./store";
import semver from "semver";
import { OpenAcaiLoaderInstaller } from "./openAcaiLoaderUpdater";
import * as fs from "@tauri-apps/plugin-fs"

export enum InstallMode {
    Install,
    Uninstall,
    Update
}

export enum VersionResult {
    Equal,
    Greater,
    Lesser,
}

export class FeatureInstaller {
    private _installer: BaseInstaller | null;
    private _uninstaller: BaseUninstaller;
    private _versionCheckPath: string | null = null;
    private _modeRefreshGeneration = 0;
    private _remoteVersion: string | null = null;
    private _installAvailable = false;
    private _isInstalled = false;

    public currentMode: InstallMode = InstallMode.Install;
    public currentModeState: string = "Install";
    public description: string | null = null;
    public displayName: string | null = null;
    public releaseWarning: string | null = null;

    public expectedMode: InstallMode | null = null;
    public additionalFoldersToCreate: string[] | null = null;

    constructor(installer: BaseInstaller | null, uninstaller: BaseUninstaller, versionCheckPath: string | null = null) {
        this._installer = installer;
        this._uninstaller = uninstaller;
        this._versionCheckPath = versionCheckPath;
    }

    public async install(): Promise<void> {
        if(!this._installer)
        {
            return;
        }

        processName.set("Preparing " + this.getName() + "...");
        if (!await this._installer.prepare()) {
            processName.set(this.getName() + " installation canceled.");
            return;
        }

        processName.set("Installing " + this.getName() + "...");
        await this._installer.install();

        if(this.additionalFoldersToCreate)
        {
            for (const folder of this.additionalFoldersToCreate) {
                let dir = await path.join(await getDirectoryPath(), folder);
                if(!await fs.exists(dir)) {
                    await fs.mkdir(dir);
                }
            }
        }
    }

    public async uninstall(): Promise<void> {
        processName.set("Removing " + this.getName() + "...");
        await this._uninstaller.uninstall();
    }

    public async handle(mode: InstallMode): Promise<void> {
        try {
            switch (mode) {
                case InstallMode.Install:
                case InstallMode.Update:
                    await this.install();
                    break;
                case InstallMode.Uninstall:
                    await this.uninstall();
                    break;
            }
        } catch (error) {
            // A failed operation may still have changed the installed files.
            try {
                await this.refreshMode();
            } catch (refreshError) {
                console.error("Failed to refresh installation state", refreshError);
            }
            throw error;
        }

        await this.refreshMode();
    }

    public async handleCurrentMode(): Promise<void> {
        await this.handle(this.getMode());
    }

    public getName(): string {
        if (this.displayName) {
            return this.displayName;
        }

        if(this._installer)
        {
            return this._installer.getName();
        }

        return this._uninstaller.getName();
    }

    public getMode(): InstallMode {
        return this.currentMode;
    }

    private setMode(mode: InstallMode){
        this.currentMode = mode;
        this.currentModeState = this.getModeString();
    }

    public getModeString(): string {
        switch (this.getMode()) {
            case InstallMode.Install:
                return "Install";
            case InstallMode.Update:
                return "Update";
            case InstallMode.Uninstall:
                return "Uninstall";
        }
    }

    public async refreshMode(): Promise<InstallMode> {
        const generation = ++this._modeRefreshGeneration;
        const exePath = get(gameExePath);
        const gameRoot = await path.dirname(exePath);
        const installed = await this._uninstaller.isInstalled(gameRoot);
        let mode = installed ? InstallMode.Uninstall : InstallMode.Install;
        let remoteVersion: string | null = null;
        let installAvailable = false;
        let releaseWarning: string | null = null;

        if (this._installer) {
            try {
                remoteVersion = await this._installer.getTargetVersion();
                // Local debug installers do not have a release version.
                installAvailable = !!remoteVersion || this._installer instanceof DebugInstaller;
                if (!installAvailable) {
                    releaseWarning = `Latest ${this.getName()} release information is unavailable.`;
                } else if (installed && await this.checkRemoteVersion(gameRoot, remoteVersion) === VersionResult.Greater) {
                    mode = InstallMode.Update;
                }
            } catch (error) {
                releaseWarning = `Could not check ${this.getName()} releases: ${error}`;
            }
        }

        // Instances survive component remounts, so only the latest scan may publish mode.
        if (generation === this._modeRefreshGeneration && exePath === get(gameExePath)) {
            this._isInstalled = installed;
            this._remoteVersion = remoteVersion;
            this._installAvailable = installAvailable;
            this.releaseWarning = releaseWarning;
            this.setMode(mode);
        }

        return mode;
    }

    public async checkRemoteVersion(gameRoot?: string, targetVersion?: string | null): Promise<VersionResult | null> {
        if(!this._versionCheckPath)
        {
            return null;
        }

        let localVersion = await this.getLocalVersion(gameRoot);
        if(!localVersion)
        {
            return null;
        }

        let remoteVersion = targetVersion === undefined ? await this._installer?.getTargetVersion() : targetVersion;
        if(!remoteVersion)
        {
            return null;
        }

        console.log("Local version:",localVersion);
        console.log("Remote version:",remoteVersion);

        if(semver.gt(remoteVersion, localVersion)) {
            return VersionResult.Greater;
        }else if(semver.lt(remoteVersion, localVersion)) {
            return VersionResult.Lesser;
        }else {
            return VersionResult.Equal;
        }
    }

    async getLocalVersion(gameRoot?: string): Promise<string | null> {
        if(!this._versionCheckPath)
        {
            return null;
        }

        try {
            const exeDir = gameRoot ?? await getDirectoryPath();
            if (this._versionCheckPath.toLowerCase().endsWith(".json")) {
                const manifestPath = await path.join(exeDir, this._versionCheckPath);
                if (!await fs.exists(manifestPath)) {
                    return null;
                }

                const manifest = JSON.parse(await fs.readTextFile(manifestPath));
                return typeof manifest.version === "string" ? manifest.version : null;
            }

            return await invoke("get_file_version", {
                path: exeDir + "\\" + this._versionCheckPath
            });
        } catch (error) {
            console.log(error);
        }

        return null;
    }

    public async canDoAction(mode: InstallMode = this.getMode()): Promise<boolean>{
        if(this.expectedMode !== null && this.expectedMode !== mode){
            return false;
        }

        return mode === InstallMode.Uninstall ? this._isInstalled : this._installAvailable;
    }

    public async getRemoteVersionString(withPrefix: boolean): Promise<string | null> {
        const version = this._remoteVersion;
        if(!version)
        {
            return null;
        }

        return withPrefix ? this.getName() + " " + version : version;
    }
}

let loaderUninstaller = new BaseUninstaller([
    "_RedLoader"
], [
    "dobby.dll",
    "version.dll",
], "RedLoader");

let loaderDebugInstaller = new DebugInstaller("I:\\repos\\MelonLoader\\RedLoader.zip", "RedLoader");
let loaderInstaller = new GithubInstaller(redLoaderInfo, "RedLoader");
let openAcaiLoaderInstaller = new OpenAcaiLoaderInstaller();

let ueUninstaller = new BaseUninstaller([
    "Mods\\sinai-dev-UnityExplorer",
    "Mods\\UnityExplorer"
], [
    "Mods\\UnityExplorer.dll",
], "UnityExplorer");
let ueInstaller = new GithubInstaller(unityExplorerInfo, "UnityExplorer");

export let debugInstaller = new FeatureInstaller(loaderDebugInstaller, loaderUninstaller);

export let loaderFeature = new FeatureInstaller(loaderInstaller, loaderUninstaller, "_RedLoader\\net6\\RedLoader.dll");
loaderFeature.additionalFoldersToCreate = ["Mods"];

let openAcaiLoaderUninstaller = new BaseUninstaller([
    // Remove the pre-rebrand bridge folder so repaired installs cannot load both runtimes.
    "BepInEx\\plugins\\RedLoaderBepInExCompat",
    "BepInEx\\plugins\\OpenACAILoader",
    "_Redloader",
    "_RedLoader"
], [
    "dobby.dll",
    "version.dll"
], "Modstoke Loader");
openAcaiLoaderUninstaller.overrideCheckFiles = [
    "BepInEx\\plugins\\OpenACAILoader\\openacai-loader-state.json",
    "BepInEx\\plugins\\OpenACAILoader\\openacai-loader.manifest.json",
    "BepInEx\\plugins\\OpenACAILoader\\OpenACAI.Endnight.Loader.Core.dll"
];

export let openAcaiLoaderFeature = new FeatureInstaller(openAcaiLoaderInstaller, openAcaiLoaderUninstaller, "BepInEx\\plugins\\OpenACAILoader\\openacai-loader.manifest.json");
openAcaiLoaderFeature.displayName = "Modstoke Loader";
openAcaiLoaderFeature.additionalFoldersToCreate = ["Mods"];
openAcaiLoaderFeature.description = "Installs or repairs Modstoke Loader from a verified package while preserving existing user configuration.";

export let ueFeature = new FeatureInstaller(ueInstaller, ueUninstaller);
ueFeature.description = "UnityExplorer is a modding tool which lets you analyze and manipulate the game at runtime."

let bieUninstaller = new BaseUninstaller([
    "BepInEx"
], [
    "winhttp.dll"
], "BepInEx");
export let bieFeature = new FeatureInstaller(null, bieUninstaller);
bieFeature.expectedMode = InstallMode.Uninstall;

let melonUninstaller = new BaseUninstaller([
    "MelonLoader",
    "Mods",
    "UserData",
    "UserLibs"
], [
    "dobby.dll",
    "version.dll",
], "MelonLoader");
melonUninstaller.overrideCheckFiles = ["MelonLoader"];
export let melonFeature = new FeatureInstaller(null, melonUninstaller);
melonFeature.expectedMode = InstallMode.Uninstall;
