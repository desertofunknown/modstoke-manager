import { invoke } from "@tauri-apps/api/core";

export type BackupSummary = {
    id: string;
    revision: string;
    createdUtc: string;
    label: string;
    files: number;
    bytes: number;
};

export type WorldState = {
    installationId: string;
    dataDirectory: string;
    saveDirectory: string;
    saveSlot: number;
    worldRevision: string | null;
    files: number;
    bytes: number;
    backups: BackupSummary[];
    pending: { revision: string; detail: string } | null;
    problem: string | null;
};

export type WorldRequest = {
    executable: string;
    installationId: string;
    expectedRevision: string;
    expectedConfigRevision: string;
    saveDirectory: string;
};

export function readServerWorld(request: WorldRequest): Promise<WorldState> {
    return invoke("read_server_world", { ...request });
}

export function createServerWorldBackup(request: WorldRequest, expectedWorldRevision: string): Promise<WorldState> {
    return invoke("create_server_world_backup", { ...request, expectedWorldRevision });
}

export function restoreServerWorldBackup(
    request: WorldRequest, expectedWorldRevision: string, backupId: string, expectedBackupRevision: string
): Promise<WorldState> {
    return invoke("restore_server_world_backup", { ...request, expectedWorldRevision, backupId, expectedBackupRevision });
}

export function recoverServerWorldRestore(
    request: WorldRequest, expectedPendingRevision: string, direction: "finish" | "undo"
): Promise<WorldState> {
    return invoke("recover_server_world_restore", { ...request, expectedPendingRevision, direction });
}
