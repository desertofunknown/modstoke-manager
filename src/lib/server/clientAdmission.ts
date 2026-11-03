import { invoke } from "@tauri-apps/api/core";

export type ClientAdmissionState = {
    installationId: string;
    root: string;
    configPath: string;
    state: "missing" | "configured" | "unreadable";
    enabled: boolean | null;
    revision: string | null;
    configProblem: string | null;
    clientAvailable: boolean;
    clientVersion: string | null;
    clientProblem: string | null;
    processState: "stopped" | "running" | "unknown";
    processProblem: string | null;
    canWrite: boolean;
    writeProblem: string | null;
};

export function readClientAdmission(executable: string, installationId: string): Promise<ClientAdmissionState> {
    return invoke("read_client_admission", { executable, installationId });
}

export function writeClientAdmission(
    executable: string, installationId: string, expectedRevision: string | null, enabled: boolean
): Promise<ClientAdmissionState> {
    return invoke("write_client_admission", { executable, installationId, expectedRevision, enabled });
}
