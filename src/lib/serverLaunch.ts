import { invoke } from "@tauri-apps/api/core";

export type ServerLaunchState = {
    installationId: string;
    root: string;
    dataDirectory: string | null;
    revision: string | null;
    configPath: string | null;
    configRevision: string | null;
    config: { serverName: string; saveMode: string; saveSlot: number; maxPlayers: number; logFilesEnabled: boolean } | null;
    configProblem: string | null;
    processState: "stopped" | "running" | "stopping" | "other" | "unknown";
    processId: number | null;
    lastExit: { success: boolean; code: number | null } | null;
    processProblem: string | null;
    restartPending: boolean;
    restartProblem: string | null;
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

export type ServerConfigChange = {
    changed: boolean;
    configRevision: string;
    backupPath: string | null;
};

export function writeServerIdentity(
    executable: string, installationId: string, expectedRevision: string, expectedConfigRevision: string,
    serverName: string
): Promise<ServerConfigChange> {
    return invoke("write_server_identity", {
        executable, installationId, expectedRevision, expectedConfigRevision, serverName
    });
}

export function writeServerLogFiles(
    executable: string, installationId: string, expectedRevision: string, expectedConfigRevision: string,
    enabled: boolean
): Promise<ServerConfigChange> {
    return invoke("write_server_log_files", {
        executable, installationId, expectedRevision, expectedConfigRevision, enabled
    });
}

export function launchDedicatedServer(
    executable: string, installationId: string, expectedRevision: string, expectedConfigRevision: string
): Promise<ServerLaunchState> {
    return invoke("launch_dedicated_server", { executable, installationId, expectedRevision, expectedConfigRevision });
}

export function stopDedicatedServer(
    executable: string, installationId: string, force: boolean, restart: boolean
): Promise<ServerLaunchState> {
    return invoke("stop_dedicated_server", { executable, installationId, force, restart });
}
