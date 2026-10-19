import { invoke } from "@tauri-apps/api/core";
import { derived, get, writable } from "svelte/store";
import { gameExePath, isPathValid, processing, processName } from "./store";

export type InstallationTarget = {
    id: string;
    name: string;
    executable: string;
    kind: "player" | "server";
};

export type InspectedInstallation = {
    target: InstallationTarget;
    available: boolean;
    problem: string | null;
    root: string | null;
};

export type InstallationRegistry = {
    schemaVersion: 1;
    selectedId: string | null;
    targets: InstallationTarget[];
};

export type InstallationRegistryState = {
    registry: InstallationRegistry;
    revision: string | null;
    selected: InspectedInstallation | null;
};

export const installationProfiles = writable<InstallationRegistryState | null>(null);
export const selectedInstallation = derived(installationProfiles, state => state?.selected ?? null);
export const installationProfileError = writable("");
export const profilesReady = writable(false);

function publish(state: InstallationRegistryState | null): void {
    // Existing installers and the Steam launch path belong to player installs.
    // Server selection has its own inventory/recovery view and never supplies them a root.
    isPathValid.set(false);
    gameExePath.set("");
    installationProfiles.set(state);
    if (state?.selected?.available && state.selected.target.kind === "player") {
        gameExePath.set(state.selected.target.executable);
        isPathValid.set(true);
    }
}

async function readRegistry(): Promise<InstallationRegistryState> {
    return invoke("read_installation_profiles");
}

async function writeRegistry(state: InstallationRegistryState, registry: InstallationRegistry): Promise<InstallationRegistryState> {
    return invoke("write_installation_profiles", { expectedRevision: state.revision, registry });
}

export async function initializeInstallationProfiles(): Promise<void> {
    try {
        let state = await readRegistry();
        // Discover the existing Steam player installation only on first use.
        // An intentionally empty or unreadable registry must not be replaced.
        if (state.revision === null && state.registry.targets.length === 0) {
            const executable = await invoke<string | null>("get_steam_path");
            if (executable) {
                const inspected = await invoke<InspectedInstallation>("inspect_installation_target", { executable });
                if (inspected.available && inspected.target.kind === "player") {
                    state = await writeRegistry(state, {
                        schemaVersion: 1,
                        selectedId: inspected.target.id,
                        targets: [inspected.target]
                    });
                }
            }
        }
        publish(state);
        installationProfileError.set("");
    } catch (error) {
        publish(null);
        installationProfileError.set(String(error));
    } finally {
        profilesReady.set(true);
    }
}

async function changeProfile(change: (state: InstallationRegistryState) => Promise<InstallationRegistryState>): Promise<void> {
    if (get(processing)) throw new Error("Wait for the current operation before changing installation profiles.");
    processing.set(true);
    processName.set("Updating installation profiles...");
    try {
        const state = get(installationProfiles) ?? await readRegistry();
        publish(await change(state));
        installationProfileError.set("");
    } catch (error) {
        // A write may have reached disk before its reply failed. Reload before
        // exposing an installation to other operations; never reuse a stale revision.
        try { publish(await readRegistry()); }
        catch { publish(null); }
        installationProfileError.set(String(error));
        throw error;
    } finally {
        processing.set(false);
        processName.set("");
        profilesReady.set(true);
    }
}

function copyRegistry(state: InstallationRegistryState): InstallationRegistry {
    return { ...state.registry, targets: state.registry.targets.map(target => ({ ...target })) };
}

export async function refreshInstallationProfiles(): Promise<void> {
    await changeProfile(async () => readRegistry());
}

export async function addInstallationProfile(executable: string, expectedKind?: InstallationTarget["kind"]): Promise<void> {
    await changeProfile(async state => {
        const inspected = await invoke<InspectedInstallation>("inspect_installation_target", { executable });
        if (!inspected.available) throw new Error(inspected.problem ?? "The selected installation is unavailable.");
        if (expectedKind && inspected.target.kind !== expectedKind) {
            throw new Error(expectedKind === "server"
                ? "Choose SonsOfTheForestDS.exe for a dedicated server."
                : "Choose SonsOfTheForest.exe for a player installation.");
        }
        const registry = copyRegistry(state);
        const existing = registry.targets.find(target => target.id === inspected.target.id);
        if (existing && existing.kind !== inspected.target.kind) {
            throw new Error("This folder already has a profile of another type. Keep player and dedicated-server installations in separate folders.");
        }
        if (!existing) registry.targets.push(inspected.target);
        registry.selectedId = inspected.target.id;
        return writeRegistry(state, registry);
    });
}

export async function selectInstallationProfile(id: string): Promise<void> {
    await changeProfile(async state => {
        if (!state.registry.targets.some(target => target.id === id)) throw new Error("This profile is no longer available. Refresh the profile list.");
        const registry = copyRegistry(state);
        registry.selectedId = id;
        return writeRegistry(state, registry);
    });
}

export async function renameInstallationProfile(id: string, name: string): Promise<void> {
    await changeProfile(async state => {
        const registry = copyRegistry(state);
        const target = registry.targets.find(item => item.id === id);
        if (!target) throw new Error("This profile is no longer available. Refresh the profile list.");
        target.name = name.trim();
        return writeRegistry(state, registry);
    });
}

export async function removeInstallationProfile(id: string): Promise<void> {
    await changeProfile(async state => {
        const registry = copyRegistry(state);
        registry.targets = registry.targets.filter(target => target.id !== id);
        if (registry.selectedId === id) registry.selectedId = null;
        return writeRegistry(state, registry);
    });
}
