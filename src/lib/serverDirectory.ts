import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ServerRequirements } from "./serverRequirements";

export type DirectoryPackage = { id: string; version: string };
export type CommunityServer = {
    id: string;
    name: string;
    description: string;
    hostLabel: string;
    region: string;
    kind: "dedicated" | "player-hosted";
    online: boolean;
    firstSeen: string | null;
    lastSeen: string | null;
    playersCurrent: number | null;
    playersMax: number | null;
    components: DirectoryPackage[];
    requiredMods: DirectoryPackage[];
    gameBuild: string | null;
    settings: { difficulty: string | null; pvp: boolean | null; friendlyFire: boolean | null; passwordRequired: boolean | null };
    hostMedianLatencyMs: number | null;
    join: { kind: "lobby"; lobbyId: string } | { kind: "dedicated"; address: string; port: number } | null;
};
export type CommunityDirectory = { schemaVersion: 1; generatedAt: string; servers: CommunityServer[] };
export type CommunityHistory = { schemaVersion: 1; serverId: string; samples: { at: string; players: number | null; latencyMs: number | null }[] };

export const listCommunityServers = () => invoke<CommunityDirectory>("list_community_servers");
export const getCommunityServer = (serverId: string) => invoke<CommunityServer>("get_community_server", { serverId });
export const getCommunityHistory = (serverId: string) => invoke<CommunityHistory>("get_community_server_history", { serverId });
export const joinCommunityServer = (expectedServer: CommunityServer, gameRoot: string) =>
    invoke<string>("join_community_server", { expectedServer, gameRoot });

export function communityRequirements(server: CommunityServer): ServerRequirements {
    return {
        format: "modstoke.server-requirements",
        schemaVersion: 1,
        name: server.name,
        packages: server.requiredMods.map(item => ({ loader: "modstoke", ...item })),
    };
}

/** Subscribe first, then consume the native pending slot so startup links cannot be missed. */
export async function receiveCommunityLinks(onServer: (id: string) => void): Promise<() => void> {
    let active = true;
    let draining = false;
    let queued = false;
    const drain = async () => {
        queued = true;
        if (draining) return;
        draining = true;
        try {
            while (active && queued) {
                queued = false;
                const id = await invoke<string | null>("take_pending_community_join");
                if (active && id && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(id)) onServer(id);
            }
        } finally { draining = false; }
    };
    const stop = await listen("community-join-pending", () => { void drain().catch(console.error); });
    try { await drain(); }
    catch (error) { active = false; stop(); throw error; }
    return () => { active = false; stop(); };
}

export function knownCount(value: number | null): string {
    return value === null ? "Unknown" : value.toLocaleString(undefined, { maximumFractionDigits: 1 });
}

export function seenAt(value: string | null): string {
    if (!value) return "Unknown";
    const date = new Date(value);
    return Number.isNaN(date.getTime()) ? "Unknown" : date.toLocaleString();
}
