import { invoke } from '@tauri-apps/api/core';
import { exists } from '@tauri-apps/plugin-fs';
import { processName, processProgress } from './store';
import { TempFileCache } from './tempFileCache';
import { download } from '@tauri-apps/plugin-upload';
import * as dialog from "@tauri-apps/plugin-dialog"

export async function unzip(sourcePath: string, destinationPath: string) {
    const src = sourcePath.replace(/\\/g, '/');
    const dest = destinationPath.replace(/\\/g, '/');
    if(!await exists(src)) throw new Error('Source file does not exist!');
    if(!await exists(dest)) throw new Error('Destination folder does not exist!');
    await invoke('unzip_handler', { source: src, destination: dest });
}

export async function downloadAndInstall(
    destination: string,
    downloadUrl: string,
    downloadName: string,
    installArchive?: (sourcePath: string) => Promise<void>): Promise<void> {
    processName.set(`Downloading ${downloadName}...`);

    if (!downloadUrl) {
      throw new Error(`Couldn't find download URL for ${downloadName}!`);
    }

    const tempPath = await TempFileCache.createFile();
    console.log(`Downloading ${downloadUrl} to ${tempPath}`);

    try {
      let downloadProgress = 0;
      
      await download(
        downloadUrl,
        tempPath,
        ({ progress, total }) => {
          downloadProgress += progress;
          if (total > 0) {
            processProgress.set(downloadProgress / total * 100);
          }
        }
      );

      processName.set(`Installing ${downloadName}...`);
      if (installArchive) {
        await installArchive(tempPath);
      } else {
        await unzip(tempPath, destination);
      }
    } finally {
      await TempFileCache.clearCache();
    }
  }

  export async function showMessageBox(title: string, message: string): Promise<void> {
    await dialog.message(message, { title: title});
  }
