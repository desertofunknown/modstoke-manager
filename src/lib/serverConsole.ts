import { invoke } from "@tauri-apps/api/core";

export type ConsoleSource = "game" | "loader";

export type ConsoleChunk = {
    file: string | null;
    offset: number;
    reset: boolean;
    lines: string[];
    problem: string | null;
};

export function readServerConsole(
    executable: string, installationId: string, source: ConsoleSource, file: string | null, offset: number | null
): Promise<ConsoleChunk> {
    return invoke("read_server_console", { executable, installationId, source, file, offset });
}
