import { writable, get } from 'svelte/store';
import * as path from "@tauri-apps/api/path";
import * as fs from "@tauri-apps/plugin-fs"

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

export async function getModsDir(): Promise<string> {
    let modsDir = await path.join(await getDirectoryPath(), "Mods");
    if(!await fs.exists(modsDir)) {
        await fs.mkdir(modsDir);
    }

    return modsDir;
}

export async function getLibsDir(): Promise<string> {
    let modsDir = await path.join(await getDirectoryPath(), "Libs");
    if(!await fs.exists(modsDir)) {
        await fs.mkdir(modsDir);
    }

    return modsDir;
}
