import { writable, get } from 'svelte/store';
import * as path from "@tauri-apps/api/path";

export const gameExePath = writable('');
export const isPathValid = writable(false);
export const isDotnetInstalled = writable(false);

export const processing = writable(false);
export const processName = writable('');
export const processProgress = writable(0);

export async function getDirectoryPath(): Promise<string> {
    const exe = get(gameExePath);
    return await path.dirname(exe);
}
