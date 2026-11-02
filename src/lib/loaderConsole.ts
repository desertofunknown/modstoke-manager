import { invoke } from "@tauri-apps/api/core";

export type LoaderConsoleEntry = { id: number; timestamp: string; level: string; source: string; message: string };

export type LoaderConsoleCommand = { name: string; description: string; enabled: boolean; requiresConfirmation: boolean; acceptsArguments: boolean };

export type LoaderConsoleSnapshot = {
    sessionId: string;
    phase: string;
    summary: string;
    ready: boolean;
    firstId: number;
    lastId: number;
    dropped: number;
    entries: LoaderConsoleEntry[];
    commands: LoaderConsoleCommand[];
    mods: string[];
    accepted: boolean;
    detail: string | null;
};

/** Either a live console's snapshot, or why the console is not available. */
export type LoaderConsoleRead = { processId: number | null; problem: string | null; snapshot: LoaderConsoleSnapshot | null };

export function readLoaderConsole(executable: string, installationId: string, after: number | null): Promise<LoaderConsoleRead> {
    return invoke("read_loader_console", { executable, installationId, after });
}

export function submitLoaderConsole(
    executable: string, installationId: string, processId: number, command: string, args: string, confirmed: boolean, after: number
): Promise<LoaderConsoleSnapshot> {
    return invoke("submit_loader_console", { executable, installationId, processId, command, arguments: args, confirmed, after });
}
