import { normalizeMatchKey, type InstalledInventoryEntry, type InstallSource, type LoaderType } from "./modInventory";
import type { Mod } from "./mods";

export const MAX_PLAN_PACKAGES = 128;
const MAX_DEPENDENCIES = 64;
const MAX_EDGES = 1024;

export type PackageSource = {
    provider: "sotf-mods" | "installed" | "modstoke-native-local";
    author?: string;
    slug?: string;
    url?: string;
};

export type ProfilePackage = {
    id: string;
    name: string;
    version: string;
    dependencies: string[];
    dependencyBasis: "catalog-current-identities" | "modstoke-native-required-versions";
    source: PackageSource;
    ownership: InstallSource;
    enabled: boolean;
    archiveSha256?: string;
    native?: import("./nativePackage").NativePackagePreview["archive"];
};

export type InstallObservation = {
    id: string;
    version: string;
    enabled: boolean;
    ownership: InstallSource;
    loaderType: LoaderType | "modstoke-native-mod";
    manifestPath?: string;
    packagePath?: string;
    assemblyPath?: string;
};

export type TargetReceipt = {
    relativePath: string;
    originalSha256: string | null;
    replacementSha256: string | null;
};

export type InstallStep = {
    package: ProfilePackage;
    action: "install" | "update" | "preserve";
    before: InstallObservation | null;
    status: "planned" | "downloaded" | "applying" | "applied";
    targets?: TargetReceipt[];
};

export type InstallPlan = {
    id: string;
    requestedRoots: string[];
    steps: InstallStep[];
    nativePack?: import("./nativePack").NativePackDocument;
};

function text(value: unknown, label: string): string {
    if (typeof value !== "string" || !value.trim() || value.length > 512 || /[\x00-\x1f]/.test(value)) {
        throw new Error(`Missing or invalid ${label}. Refresh the catalog before installing.`);
    }
    return value.trim();
}

export function catalogPackage(mod: Mod): ProfilePackage {
    if (mod.dependencyMetadataError) {
        throw new Error(mod.dependencyMetadataError);
    }
    if (!Array.isArray(mod.dependencies) || mod.dependencies.length > MAX_DEPENDENCIES) {
        throw new Error("Missing or oversized dependency metadata. Refresh the catalog before installing.");
    }
    const id = text(mod.mod_id, "mod identity");
    const author = text(mod.user?.slug, "mod author coordinate");
    const slug = text(mod.slug, "mod coordinate");
    const version = text(mod.latestVersion, `version for ${id}`);
    return {
        id,
        name: text(mod.name, "mod name"),
        version,
        dependencies: mod.dependencies.map(dependency => text(dependency, `dependency for ${id}`)),
        dependencyBasis: "catalog-current-identities",
        source: {
            provider: "sotf-mods", author, slug,
            url: `https://sotf-mods.com/mods/${encodeURIComponent(author)}/${encodeURIComponent(slug)}/download/${encodeURIComponent(version)}`
        },
        ownership: "native",
        enabled: true
    };
}

export function observeInstall(entry: InstalledInventoryEntry): InstallObservation {
    return {
        id: entry.id,
        version: text(entry.version, `installed version for ${entry.id}`),
        enabled: entry.enabled,
        ownership: entry.installSource,
        loaderType: entry.loaderType,
        manifestPath: entry.manifestPath,
        packagePath: entry.packagePath,
        assemblyPath: entry.assemblyPath
    };
}

function identity(value: string): string { return value.trim().toLowerCase(); }

export function matchingInstall(pkg: ProfilePackage, inventory: InstalledInventoryEntry[]): InstalledInventoryEntry | null {
    const keys = [pkg.id, pkg.name, pkg.source.slug].map(normalizeMatchKey).filter(Boolean);
    const candidates = inventory.filter(entry => identity(entry.id) === identity(pkg.id)
        || entry.matchKeys.some(key => keys.includes(key)));
    const exact = candidates.filter(entry => identity(entry.id) === identity(pkg.id));
    if (candidates.length > 1 || (candidates.length === 1 && exact.length !== 1)) {
        throw new Error(`Cannot unambiguously match installed files for ${pkg.id}. Resolve the conflicting identity before installing.`);
    }
    return exact[0] ?? null;
}

export function sameObservation(a: InstallObservation | null, b: InstallObservation | null): boolean {
    const canonical = (value: InstallObservation | null) => value && {
        ...value,
        manifestPath: value.manifestPath?.replace(/\\/g, "/").toLowerCase(),
        packagePath: value.packagePath?.replace(/\\/g, "/").toLowerCase(),
        assemblyPath: value.assemblyPath?.replace(/\\/g, "/").toLowerCase()
    };
    return JSON.stringify(canonical(a)) === JSON.stringify(canonical(b));
}

// The catalog supplies dependency identities, not version ranges. Keeping an
// enabled installed version satisfies identity only; it is no compatibility claim.
export async function resolveInstallPlan(
    selected: Mod,
    inventory: InstalledInventoryEntry[],
    fetchMod: (id: string) => Promise<Mod | null>,
    updating: boolean): Promise<InstallPlan> {
    const selectedPackage = catalogPackage(selected);
    const nodes = new Map<string, { pkg: ProfilePackage; dependencies: string[] }>();
    const fetched = new Map<string, Mod>();
    const active = new Set<string>();
    const done = new Set<string>();
    const steps: InstallStep[] = [];
    let edges = 0;

    async function visit(identifier: string, depth: number, root = false): Promise<string> {
        if (depth > MAX_DEPENDENCIES || fetched.size >= MAX_PLAN_PACKAGES * 2) {
            throw new Error("Dependency graph exceeds the supported size (128 packages, 64 levels).");
        }
        let mod = fetched.get(identifier);
        if (!mod) {
            mod = await fetchMod(identifier) ?? undefined;
            if (!mod) throw new Error(`Dependency metadata is missing for ${identifier.slice(0, 512)}. No packages were installed.`);
            fetched.set(identifier, mod);
        }
        const pkg = catalogPackage(mod);
        if (root && (pkg.id !== selectedPackage.id || pkg.version !== selectedPackage.version
            || JSON.stringify(pkg.source) !== JSON.stringify(selectedPackage.source))) {
            throw new Error("The selected catalog version or coordinates changed. Refresh and choose the version again.");
        }
        const key = identity(pkg.id);
        const known = nodes.get(key);
        if (known && JSON.stringify(known.pkg) !== JSON.stringify(pkg)) {
            throw new Error(`Conflicting catalog metadata for ${pkg.id}. Refresh before retrying.`);
        }
        if (active.has(key)) throw new Error(`Dependency cycle includes ${pkg.id}. No packages were installed.`);
        if (done.has(key)) return known!.pkg.id;
        if (nodes.size >= MAX_PLAN_PACKAGES) throw new Error("Dependency graph exceeds 128 packages.");
        nodes.set(key, { pkg, dependencies: [] });
        active.add(key);
        const dependencies: string[] = [];
        for (const dependency of [...new Set(pkg.dependencies)].sort()) {
            if (++edges > MAX_EDGES) throw new Error("Dependency graph exceeds 1024 dependency edges.");
            dependencies.push(await visit(dependency, depth + 1));
        }
        active.delete(key);
        done.add(key);
        const resolved = { ...pkg };
        const installed = matchingInstall(pkg, inventory);
        const before = installed ? observeInstall(installed) : null;
        let action: InstallStep["action"] = "install";
        if (root && updating) {
            if (!before || before.ownership !== "native") {
                throw new Error(`The selected update is no longer a native install of ${pkg.id}. Refresh the installed mods.`);
            }
            action = "update";
            resolved.enabled = before.enabled;
        } else if (before) {
            if (!before.enabled) throw new Error(`Dependency ${pkg.id} is disabled. Enable it with its owning manager before retrying.`);
            action = "preserve";
            resolved.version = before.version;
            resolved.ownership = before.ownership;
            resolved.source = { provider: "installed" };
        }
        steps.push({ package: { ...resolved, dependencies: [...new Set(dependencies)].sort() }, action, before, status: "planned" });
        return pkg.id;
    }

    const root = await visit(selectedPackage.id, 0, true);
    return { id: crypto.randomUUID(), requestedRoots: [root], steps };
}

export function assertPlanInventory(plan: InstallPlan, inventory: InstalledInventoryEntry[]): void {
    for (const step of plan.steps) {
        const entry = matchingInstall(step.package, inventory);
        const current = entry ? observeInstall(entry) : null;
        if (step.status === "applied") {
            if (!current || current.version !== step.package.version || current.enabled !== step.package.enabled
                || current.ownership !== step.package.ownership) {
                throw new Error(`Installed state changed for ${step.package.id}. The pending plan was preserved; restore its recorded state before retrying.`);
            }
        } else if (!sameObservation(step.before, current)) {
            throw new Error(`Installed state changed for ${step.package.id}. The pending plan was preserved; restore its recorded state before retrying.`);
        }
    }
}
