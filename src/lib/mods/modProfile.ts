import { invoke } from "@tauri-apps/api/core";
import * as fs from "@tauri-apps/plugin-fs";
import { assertCurrentGameRoot, scanInstalledInventory } from "./modInventory";
import { assertPlanInventory, type InstallPlan, type InstallStep, type ProfilePackage } from "./modInstallPlan";

export type ArchivePin = { id: string; version: string; url: string; sha256: string };
export type PackCheckpoint = {
    planId: string;
    rootName: string;
    workspace: string;
    phase: "available" | "restoring" | "discarding";
    beforePackages: ProfilePackage[];
    beforeRequestedRoots: string[];
    receipts: {
        stepIndex: number;
        packageId: string;
        files: {
            targets: {
                relativePath: string;
                originalSha256: string | null;
                replacementSha256: string | null;
                backupPath: string | null;
            }[];
        };
        restored: boolean;
    }[];
};
export type ModProfile = {
    schemaVersion: 1 | 2;
    gameRoot: string;
    requestedRoots: string[];
    packages: ProfilePackage[];
    archives: ArchivePin[];
    pendingPlan: InstallPlan | null;
    rollback?: PackCheckpoint | null;
};
export type ProfileState = { profile: ModProfile | null; revision: string | null };
type ExpectedFile = { path: string; sha256: string | null };

export async function readModProfile(gameRoot: string): Promise<ProfileState> {
    await assertCurrentGameRoot(gameRoot);
    // The native read also recovers a known interrupted package transaction.
    return invoke("read_mod_profile", { gameRoot });
}

export async function saveModProfile(gameRoot: string, state: ProfileState, profile: ModProfile): Promise<ProfileState> {
    await assertCurrentGameRoot(gameRoot);
    return invoke("write_mod_profile", { gameRoot, expectedRevision: state.revision, profile });
}

export async function rollbackModProfile(gameRoot: string, state: ProfileState): Promise<ProfileState> {
    const planId = state.profile?.rollback?.planId;
    if (!planId) throw new Error("No pack recovery point is available. Refresh the recovery status.");
    await assertCurrentGameRoot(gameRoot);
    return invoke("rollback_mod_profile", { gameRoot, expectedRevision: state.revision, planId });
}

export async function discardModProfileRollback(gameRoot: string, state: ProfileState): Promise<ProfileState> {
    const planId = state.profile?.rollback?.planId;
    if (!planId) throw new Error("No pack recovery point is available. Refresh the recovery status.");
    await assertCurrentGameRoot(gameRoot);
    return invoke("discard_mod_profile_rollback", { gameRoot, expectedRevision: state.revision, planId });
}

export function completedPackage(profile: ModProfile, pkg: ProfilePackage): void {
    const index = profile.packages.findIndex(entry => entry.id.toLowerCase() === pkg.id.toLowerCase());
    if (index < 0) profile.packages.push({ ...pkg });
    else profile.packages[index] = { ...pkg };
}

export function mergeRequestedRoots(existing: string[], added: string[]): string[] {
    const roots = new Map(existing.map(id => [id.toLowerCase(), id]));
    for (const id of added) roots.set(id.toLowerCase(), id);
    return [...roots.values()];
}

export function pinArchive(profile: ModProfile, step: InstallStep, sha256: string): void {
    const pkg = step.package;
    // This is observed download integrity, not a publisher signature. Changing
    // the catalog spelling or URL must not reset an exact version's known bytes.
    const known = profile.archives.find(pin => pin.id.toLowerCase() === pkg.id.toLowerCase() && pin.version === pkg.version);
    if (known && known.sha256 !== sha256) {
        throw new Error(`The bytes downloaded for ${pkg.id} ${pkg.version} differ from the profile's recorded SHA-256. The pending plan was preserved; review the changed download before proceeding.`);
    }
    if (!profile.archives.some(pin => pin.id.toLowerCase() === pkg.id.toLowerCase() && pin.version === pkg.version && pin.url === pkg.source.url)) {
        profile.archives.push({ id: pkg.id, version: pkg.version, url: pkg.source.url!, sha256 });
    }
    pkg.archiveSha256 = sha256;
    step.status = "downloaded";
}

export async function freshPlanInventory(gameRoot: string, plan: InstallPlan) {
    await assertCurrentGameRoot(gameRoot);
    const inventory = await scanInstalledInventory(gameRoot);
    await assertCurrentGameRoot(gameRoot);
    assertPlanInventory(plan, inventory);
    return inventory;
}

export async function applyProfilePackage(gameRoot: string, state: ProfileState, step: InstallStep, source: string): Promise<ProfileState> {
    const plan = state.profile!.pendingPlan!;
    const inventory = await freshPlanInventory(gameRoot, plan);
    const expectedFiles: ExpectedFile[] = [];
    const protectedPaths: { path: string; directory: boolean }[] = [];
    const paths = new Set<string>();
    // Hash relevant identity and assembly files again immediately before native
    // commit. The native command verifies these observations under the root lock.
    for (const entry of inventory) {
        if (entry.manifestPath) paths.add(entry.manifestPath);
        if (entry.assemblyPath) {
            paths.add(entry.assemblyPath);
            paths.add(entry.assemblyPath.replace(/\.(dll|disabled)$/i, entry.enabled ? ".disabled" : ".dll"));
        }
        if (entry.id.toLowerCase() !== step.package.id.toLowerCase()) {
            if (entry.packagePath) protectedPaths.push({ path: entry.packagePath, directory: true });
            if (entry.assemblyPath) protectedPaths.push({ path: entry.assemblyPath, directory: false });
        }
    }
    for (const file of paths) {
        expectedFiles.push({ path: file, sha256: await fs.exists(file) ? await invoke<string>("sha256_file", { path: file }) : null });
    }
    await freshPlanInventory(gameRoot, plan);
    await assertCurrentGameRoot(gameRoot);
    return invoke("apply_mod_profile_package", {
        gameRoot, expectedRevision: state.revision, packageId: step.package.id, source, expectedFiles, protectedPaths
    });
}
