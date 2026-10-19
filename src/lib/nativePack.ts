import { invoke } from "@tauri-apps/api/core";
import type { ProfileState } from "./modProfile";
import { assertNativeInstallation } from "./nativePackage";

export type NativePackDocument = {
    format: "modstoke.native-pack";
    schemaVersion: 1;
    name: string;
    packages: {
        id: string;
        name: string;
        version: string;
        folder: string;
        required: { id: string; version: string }[];
        sha256: string;
        size: number;
    }[];
};

export type NativePackPreparation = {
    document: NativePackDocument;
    totalArchiveBytes: number;
    totalDecodedBytes: number;
    fileCount: number;
};

export type NativePackInstallPreview = {
    document: NativePackDocument;
    packages: {
        id: string;
        name: string;
        version: string;
        action: "install" | "update";
        status: "planned" | "downloaded" | "applying" | "applied";
    }[];
    profileRevision: string | null;
    planId: string | null;
    notices: string[];
};

export function parseNativePack(text: string): Promise<NativePackDocument> {
    return invoke<NativePackDocument>("parse_native_pack", { text });
}

export function prepareNativePack(name: string, sources: string[], expected: string | null): Promise<NativePackPreparation> {
    return invoke<NativePackPreparation>("prepare_native_pack", { name, sources, expected });
}

export async function inspectNativePackInstall(
    gameRoot: string, sources: string[], document: NativePackDocument, identity: string
): Promise<NativePackInstallPreview> {
    assertNativeInstallation(gameRoot, identity);
    const result = await invoke<NativePackInstallPreview>("inspect_native_pack_install", {
        gameRoot, sources, document: JSON.stringify(document)
    });
    assertNativeInstallation(gameRoot, identity);
    return result;
}

export async function installNativePack(
    gameRoot: string, sources: string[], preview: NativePackInstallPreview, identity: string
): Promise<ProfileState> {
    assertNativeInstallation(gameRoot, identity);
    const result = await invoke<ProfileState>("install_native_pack", {
        gameRoot, sources, document: JSON.stringify(preview.document),
        expectedRevision: preview.profileRevision, expectedPlanId: preview.planId
    });
    assertNativeInstallation(gameRoot, identity);
    return result;
}
