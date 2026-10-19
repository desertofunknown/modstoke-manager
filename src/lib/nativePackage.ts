import { invoke } from "@tauri-apps/api/core";
import { get } from "svelte/store";
import { selectedInstallation, type InspectedInstallation } from "./installationProfiles";
import type { ProfileState } from "./modProfile";

export type NativePackagePreview = {
    archive: {
        id: string;
        name: string;
        version: string;
        packagePath: string;
        manifestPath: string;
        assemblyPath: string;
        required: { id: string; version: string }[];
        files: { path: string; sha256: string; size: number }[];
        sha256: string;
        size: number;
    };
    action: "install" | "update" | "resume";
    profileRevision: string | null;
    planId: string | null;
    notices: string[];
};

export function nativeInstallationKey(installation: InspectedInstallation | null): string {
    return installation ? JSON.stringify([
        installation.target.id, installation.target.kind, installation.root,
        installation.target.executable, installation.available
    ]) : "";
}

export function assertNativeInstallation(gameRoot: string, identity: string): void {
    const selected = get(selectedInstallation);
    if (!gameRoot || !selected?.available || selected.root !== gameRoot
        || nativeInstallationKey(selected) !== identity) {
        throw new Error("The selected installation changed or is unavailable. Select its ZIP again before installing.");
    }
}

export async function inspectNativePackage(gameRoot: string, source: string, identity: string): Promise<NativePackagePreview> {
    assertNativeInstallation(gameRoot, identity);
    const result = await invoke<NativePackagePreview>("inspect_native_package", { gameRoot, source });
    assertNativeInstallation(gameRoot, identity);
    return result;
}

export async function installNativePackage(
    gameRoot: string, source: string, preview: NativePackagePreview, identity: string
): Promise<ProfileState> {
    assertNativeInstallation(gameRoot, identity);
    const result = await invoke<ProfileState>("install_native_package", {
        gameRoot, source, expectedRevision: preview.profileRevision,
        expectedSha256: preview.archive.sha256, expectedPlanId: preview.planId
    });
    assertNativeInstallation(gameRoot, identity);
    return result;
}

export async function readNativePackageProfile(gameRoot: string, identity: string): Promise<ProfileState> {
    assertNativeInstallation(gameRoot, identity);
    const result = await invoke<ProfileState>("read_mod_profile", { gameRoot });
    assertNativeInstallation(gameRoot, identity);
    return result;
}
