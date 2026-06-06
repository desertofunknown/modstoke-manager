import { DebugInstaller } from "./debugInstaller";
import type { BaseInstaller } from "./baseInstaller";
import { getDirectoryPath, processName } from "./store";
import { BaseUninstaller } from "./baseUninstaller";
import { writable, get } from "svelte/store";
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

    public currentMode: InstallMode = InstallMode.Install;
    public currentModeState: string = "Install";
    public description: string | null = null;

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

        await this.uninstall();
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
        switch (mode) {
            case InstallMode.Install:
                await this.install();
                break;
            case InstallMode.Update:
                await this.install();
                break;
            case InstallMode.Uninstall:
                await this.uninstall();
                break;
        }

        await this.refreshMode();
    }

    public async handleCurrentMode(): Promise<void> {
        await this.handle(this.getMode());
    }

    public getName(): string {
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

    public async refreshMode() : Promise<void> {
        if(await this._uninstaller.isInstalled()){
            if(await this.checkRemoteVersion() === VersionResult.Greater) {
                this.setMode(InstallMode.Update);
            }else {
                this.setMode(InstallMode.Uninstall);                
            }
        }else {
            this.setMode(InstallMode.Install);
        }
    }

    public async checkRemoteVersion(): Promise<VersionResult | null> {
        if(!this._versionCheckPath)
        {
            return null;
        }

        let localVersion = await this.getLocalVersion();
        if(!localVersion)
        {
            return null;
        }

        let remoteVersion = await this._installer?.getTargetVersion();
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

    async getLocalVersion(): Promise<string | null> {
        if(!this._versionCheckPath)
        {
            return null;
        }

        try {
            const exeDir = await path.dirname(get(gameExePath));
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

    public async canDoAction(): Promise<Boolean>{
        if(!this.expectedMode)
        {
            return true;
        }

        if(this.expectedMode !== this.getMode()){
            return false;
        }

        return true;
    }

    public async getRemoteVersionString(withPrefix: boolean): Promise<string | null> {
        let version = await this._installer?.getTargetVersion();
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
], "OpenACAI Endnight Loader");
openAcaiLoaderUninstaller.overrideCheckFiles = [
    "BepInEx\\plugins\\OpenACAILoader\\openacai-loader-state.json",
    "BepInEx\\plugins\\OpenACAILoader\\openacai-loader.manifest.json",
    "BepInEx\\plugins\\OpenACAILoader\\OpenACAI.Endnight.Loader.Core.dll"
];

export let openAcaiLoaderFeature = new FeatureInstaller(openAcaiLoaderInstaller, openAcaiLoaderUninstaller, "BepInEx\\plugins\\OpenACAILoader\\openacai-loader.manifest.json");
openAcaiLoaderFeature.additionalFoldersToCreate = ["Mods"];
openAcaiLoaderFeature.description = "Installs or repairs the BepInEx-first OpenACAI Endnight Loader package from the public GitHub release manifest, then verifies installed file hashes.";

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
