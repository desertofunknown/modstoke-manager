import { invoke } from "@tauri-apps/api/core";

export type ServerLaunchState = {
    installationId: string;
    root: string;
    dataDirectory: string | null;
    revision: string | null;
    configPath: string | null;
    configRevision: string | null;
    config: { serverName: string; saveMode: string; saveSlot: number; maxPlayers: number } | null;
    configProblem: string | null;
    processState: "stopped" | "running" | "other" | "unknown";
    processId: number | null;
    lastExit: { success: boolean; code: number | null } | null;
    processProblem: string | null;
    canSave: boolean;
    saveProblem: string | null;
    canLaunch: boolean;
    launchProblem: string | null;
};

export function readServerLaunch(executable: string, installationId: string): Promise<ServerLaunchState> {
    return invoke("read_server_launch", { executable, installationId });
}

export function writeServerLaunch(
    executable: string, installationId: string, expectedRevision: string | null, dataDirectory: string
): Promise<ServerLaunchState> {
    return invoke("write_server_launch", { executable, installationId, expectedRevision, dataDirectory });
}

export type ServerCapacityChange = {
    changed: boolean;
    maxPlayers: number;
    configRevision: string;
    backupPath: string | null;
};

export function writeServerCapacity(
    executable: string, installationId: string, expectedRevision: string, expectedConfigRevision: string,
    maxPlayers: number, acknowledgeExperimental: boolean
): Promise<ServerCapacityChange> {
    return invoke("write_server_capacity", {
        executable, installationId, expectedRevision, expectedConfigRevision, maxPlayers, acknowledgeExperimental
    });
}

export type ServerIdentityChange = {
    changed: boolean;
    configRevision: string;
    backupPath: string | null;
};

export function writeServerIdentity(
    executable: string, installationId: string, expectedRevision: string, expectedConfigRevision: string,
    serverName: string
): Promise<ServerIdentityChange> {
    return invoke("write_server_identity", {
        executable, installationId, expectedRevision, expectedConfigRevision, serverName
    });
}

export function launchDedicatedServer(
    executable: string, installationId: string, expectedRevision: string, expectedConfigRevision: string
): Promise<ServerLaunchState> {
    return invoke("launch_dedicated_server", { executable, installationId, expectedRevision, expectedConfigRevision });
}
