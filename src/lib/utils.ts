import { Channel, invoke } from '@tauri-apps/api/core';
import { exists } from '@tauri-apps/plugin-fs';
import { open as openInBrowser } from '@tauri-apps/plugin-shell';
import { processName, processProgress } from './store';
import { TempFileCache } from './tempFileCache';

// window.open does nothing in the Manager's webview, so buttons hand web links to the default browser.
export function openWebLink(url: string): void {
    let link: URL;
    try { link = new URL(url); } catch { return; }
    if (link.protocol !== 'https:' && link.protocol !== 'http:') return;
    openInBrowser(link.href).catch(error => console.warn('Could not open the link', error));
}

// Opens a local folder in Explorer. The Manager refuses anything that isn't an existing folder.
export async function openFolder(folder: string): Promise<void> {
    await invoke('open_folder', { path: folder });
}

export async function unzip(sourcePath: string, destinationPath: string) {
    const src = sourcePath.replace(/\\/g, '/');
    const dest = destinationPath.replace(/\\/g, '/');
    if(!await exists(src)) throw new Error('Source file does not exist!');
    if(!await exists(dest)) throw new Error('Destination folder does not exist!');
    await invoke('unzip_handler', { source: src, destination: dest });
}

// The Manager's downloader only fetches from the catalog and release hosts it knows, and checks the
// archive against sha256 when one is given.
export async function downloadToCache(url: string, sha256?: string): Promise<string> {
    const onProgress = new Channel<{ received: number; total: number | null }>();
    onProgress.onmessage = ({ received, total }) => {
        if (total) processProgress.set(Math.min(100, received / total * 100));
    };
    const filePath = await invoke<string>('download_to_cache', { url, sha256: sha256 ?? null, onProgress });
    TempFileCache.track(filePath);
    return filePath;
}

export async function downloadAndInstall(
    destination: string,
    downloadUrl: string,
    downloadName: string,
    installArchive?: (sourcePath: string) => Promise<void>): Promise<void> {
    processName.set(`Downloading ${downloadName}...`);
    processProgress.set(0);

    if (!downloadUrl) {
      throw new Error(`Couldn't find download URL for ${downloadName}!`);
    }

    try {
      const tempPath = await downloadToCache(downloadUrl);
      processName.set(`Installing ${downloadName}...`);
      processProgress.set(0);
      if (installArchive) {
        await installArchive(tempPath);
      } else {
        await unzip(tempPath, destination);
      }
    } finally {
      await TempFileCache.clearCache();
    }
  }

