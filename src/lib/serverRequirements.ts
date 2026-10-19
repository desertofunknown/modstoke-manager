import { invoke } from "@tauri-apps/api/core";
import type { NativeInventory, NativePackage } from "./nativeInventory";

export type RequirementPackage = {
    loader: "modstoke";
    id: string;
    version: string;
};

export type ServerRequirements = {
    format: "modstoke.server-requirements";
    schemaVersion: 1;
    name: string;
    packages: RequirementPackage[];
};

export type RequirementStatus = "match" | "missing" | "version-mismatch" | "disabled" | "incomplete" | "ambiguous" | "unknown";

export type RequirementComparison = {
    requirement: RequirementPackage;
    status: RequirementStatus;
    detail: string;
    observedVersion: string | null;
};

export async function parseServerRequirements(text: string): Promise<ServerRequirements> {
    return invoke("parse_server_requirements", { text });
}

export async function createServerRequirements(name: string, packages: RequirementPackage[]): Promise<ServerRequirements> {
    return parseServerRequirements(JSON.stringify({
        format: "modstoke.server-requirements",
        schemaVersion: 1,
        name,
        packages,
    }));
}

function packagesById(inventory: NativeInventory): Map<string, NativePackage[]> {
    const groups = new Map<string, NativePackage[]>();
    for (const entry of inventory.packages) {
        const id = entry.id.toLowerCase();
        const group = groups.get(id) ?? [];
        group.push(entry);
        groups.set(id, group);
    }
    return groups;
}

// Match Rust's Unicode scalar, control-character and White_Space checks.
function validRequirementText(value: string | null, limit: number): value is string {
    return value !== null && value.length > 0 && [...value].length <= limit
        && !/[\p{Cc}\p{Cs}]/u.test(value)
        && !/^\p{White_Space}|\p{White_Space}$/u.test(value);
}

export function selectableNativePackages(inventory: NativeInventory): NativePackage[] {
    const groups = packagesById(inventory);
    return inventory.packages.filter(entry =>
        (entry.state === "candidate" || entry.state === "library")
        && validRequirementText(entry.id, 256)
        && validRequirementText(entry.version, 128)
        && groups.get(entry.id.toLowerCase())?.length === 1);
}

/** Compares observed package files only, never runtime loading or server compatibility. */
export function compareServerRequirements(document: ServerRequirements, inventory: NativeInventory): RequirementComparison[] {
    const groups = packagesById(inventory);
    return document.packages.map(requirement => {
        const entries = groups.get(requirement.id.toLowerCase()) ?? [];
        const result = (status: RequirementStatus, detail: string, observedVersion: string | null = null): RequirementComparison =>
            ({ requirement, status, detail, observedVersion });
        if (entries.length === 0) {
            return inventory.notices.length > 0
                ? result("unknown", "No matching ID was observed, but inventory notices prevent confirming that it is missing.")
                : result("missing", "No package with this ID was found in the inspected native package files.");
        }
        if (entries.length > 1) {
            return result("ambiguous", "Multiple inventory entries declare this ID; no package was selected.");
        }
        const entry = entries[0];
        const observed = entry.version;
        switch (entry.state) {
            case "ambiguous":
                return result("ambiguous", "The inventory reports conflicting package files.", observed);
            case "disabled":
                return result("disabled", "The matching package is in a disabled or pending path.", observed);
            case "metadata-only":
                return result("incomplete", "The matching package has metadata only and no executable entrypoint.", observed);
            case "incomplete":
                return result("incomplete", "The matching package files could not be inspected completely or need attention.", observed);
            case "candidate":
            case "library":
                if (observed === null || observed.length === 0) {
                    return result("unknown", "The matching package has no usable exact version in its metadata.", observed);
                }
                return observed === requirement.version
                    ? result("match", "Observed package files declare the requested exact version. Runtime loading and server compatibility are unverified.", observed)
                    : result("version-mismatch", "Observed package files declare a different exact version.", observed);
            default:
                return result("unknown", "The package inventory state is not recognized.", observed);
        }
    });
}
