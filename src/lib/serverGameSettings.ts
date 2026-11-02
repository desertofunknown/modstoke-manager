import { invoke } from "@tauri-apps/api/core";
import type { ServerConfigChange } from "./serverLaunch";

/** A key the dedicated server reads; `choices` is null for true/false settings, and a null value means the game default. */
export type GameSetting = { key: string; choices: string[] | null; value: unknown };

export type GameSettingsState = {
    configRevision: string;
    gameMode: unknown;
    saveInterval: unknown;
    idleDayCycleSpeed: unknown;
    idleTargetFramerate: unknown;
    activeTargetFramerate: unknown;
    lanOnly: unknown;
    skipNetworkAccessibilityTest: unknown;
    gameSettings: GameSetting[];
    customSettings: GameSetting[];
    ignoredKeys: string[];
};

export type GameSettingsChange = {
    gameMode?: string;
    saveInterval?: number;
    idleDayCycleSpeed?: number;
    idleTargetFramerate?: number;
    activeTargetFramerate?: number;
    lanOnly?: boolean;
    skipNetworkAccessibilityTest?: boolean;
    gameSettings?: Record<string, boolean | string | null>;
    customSettings?: Record<string, boolean | string | null>;
};

export function readServerGameSettings(executable: string, installationId: string): Promise<GameSettingsState> {
    return invoke("read_server_game_settings", { executable, installationId });
}

export function writeServerGameSettings(
    executable: string, installationId: string, expectedRevision: string, expectedConfigRevision: string,
    change: GameSettingsChange
): Promise<ServerConfigChange> {
    return invoke("write_server_game_settings", { executable, installationId, expectedRevision, expectedConfigRevision, change });
}
