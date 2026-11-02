import { invoke } from "@tauri-apps/api/core";

export type AdminRole = { id: string; name: string; rank: number; inherits: string[]; allow: string[]; deny: string[] };

export type AdminAssignment = {
    steamId: string;
    role?: string | null;
    allow?: string[];
    deny?: string[];
    note?: string | null;
    expiresUtc?: string | null;
    addedBy?: string | null;
    addedUtc?: string | null;
};

export type AdminPolicy = {
    format: string;
    schemaVersion: number;
    defaults: { remoteOperators: boolean; remoteMinimumRank: number; unassignedRole: string | null };
    gameRoleMap: { owner: string | null; admin: string | null; trusted: string | null; guest: string | null };
    roles: AdminRole[];
    assignments: AdminAssignment[];
};

/** One entry of the node catalog Admin writes to permission-nodes.json on every start. */
export type PermissionNode = { node: string; group: string; description: string; sensitive: boolean; hostOnly: boolean };

/** Admin's verdict on the file it last read, from permissions-status.json. */
export type AdminPermissionsStatus = {
    sha256: string | null;
    valid: boolean;
    error: string | null;
    warnings: string[];
    activeSha256: string | null;
    checkedUtc: string;
};

export type AdminPermissionsState = {
    installationId: string;
    folder: string;
    path: string;
    state: "no-folder" | "missing" | "valid" | "invalid" | "unreadable" | "unsupported";
    revision: string | null;
    policy: AdminPolicy | null;
    problems: string[];
    warnings: string[];
    nodes: PermissionNode[];
    nodesProblem: string | null;
    status: AdminPermissionsStatus | null;
    statusProblem: string | null;
    adminVersion: string | null;
};

export type AdminPolicyCheck = { problems: string[]; warnings: string[] };

export type AdminPermissionsSave = { changed: boolean; revision: string; backupPath: string | null; state: AdminPermissionsState };

export function readAdminPermissions(executable: string, installationId: string): Promise<AdminPermissionsState> {
    return invoke("read_admin_permissions", { executable, installationId });
}

export function checkAdminPermissions(executable: string, installationId: string, policy: AdminPolicy): Promise<AdminPolicyCheck> {
    return invoke("check_admin_permissions", { executable, installationId, policy });
}

export function writeAdminPermissions(
    executable: string, installationId: string, expectedRevision: string, policy: AdminPolicy
): Promise<AdminPermissionsSave> {
    return invoke("write_admin_permissions", { executable, installationId, expectedRevision, policy });
}
