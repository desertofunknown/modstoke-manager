import * as path from "@tauri-apps/api/path";
import * as fs from "@tauri-apps/plugin-fs";
import type { SiteLoaderRelease } from "./siteReleaseIndex";

export type SiteBundleInventory = {
    installedVersion: string | null;
    loaderCorePresent: boolean;
    sdkAssemblyPresent: boolean;
    clientCompanionPresent: boolean | null;
};

// Presence is a quick local inventory, not an integrity result. The install
// flow separately compares every file with the published manifest hash.
export async function readSiteBundleInventory(gameRoot: string, release: SiteLoaderRelease | null): Promise<SiteBundleInventory> {
    const loaderManifest = await path.join(gameRoot, "BepInEx/plugins/OpenACAILoader/openacai-loader.manifest.json");
    let installedVersion: string | null = null;
    try {
        const parsed: unknown = JSON.parse(await fs.readTextFile(loaderManifest));
        if (parsed && typeof parsed === "object" && "version" in parsed && typeof parsed.version === "string") {
            installedVersion = parsed.version;
        }
    } catch {
        // The loader may not be installed yet.
    }
    const core = "OpenACAI/Runtime/OpenACAI.Endnight.Loader.Core.dll";
    return {
        installedVersion,
        loaderCorePresent: await fs.exists(await path.join(gameRoot, core)),
        sdkAssemblyPresent: await fs.exists(await path.join(gameRoot, release?.bundled.sdk.assemblyPath ?? core)),
        clientCompanionPresent: release
            ? await fs.exists(await path.join(gameRoot, release.bundled.client.installPath))
            : null
    };
}
