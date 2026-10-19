import { invoke } from "@tauri-apps/api/core";

export type NativePackageState = "candidate" | "library" | "metadata-only" | "disabled" | "incomplete" | "ambiguous";

export type NativePackage = {
    manifestPath: string;
    packagePath: string;
    id: string;
    name: string;
    version: string | null;
    author: string | null;
    packageType: "mod" | "library";
    entryType: string | null;
    entryMethod: string | null;
    assemblyPath: string | null;
    assemblyCandidates: string[];
    state: NativePackageState;
    details: string[];
};

export type NativeInventory = {
    root: string;
    packages: NativePackage[];
    notices: string[];
};

export async function readNativeInventory(gameRoot: string): Promise<NativeInventory> {
    return invoke("read_native_mod_inventory", { gameRoot });
}

export function nativePackageStateLabel(state: NativePackageState): string {
    switch (state) {
        case "candidate": return "Load candidate";
        case "library": return "Library files";
        case "metadata-only": return "Metadata only";
        case "disabled": return "Disabled path";
        case "incomplete": return "Needs attention";
        case "ambiguous": return "Conflicting files";
    }
}
