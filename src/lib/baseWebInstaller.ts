import { BaseZipInstaller } from "./baseZipInstaller";
import { getDirectoryPath, processName, processProgress } from "./store"
import { TempFileCache } from "./tempFileCache";
import { download } from "@tauri-apps/plugin-upload";


export abstract class BaseWebInstaller extends BaseZipInstaller {
  protected constructor(name: string) {
    super(name);
  }

  public getLocalVersion(): string | null {
    return null;
  }

  public async install(): Promise<void> {
    const exeDir = await getDirectoryPath();

    const selectedVersion = await this.getTargetVersion();
    if (!selectedVersion) {
      throw new Error(`Couldn't find a target version for ${this.getName()}!`);
    }

    await this.newInstall(exeDir, selectedVersion);
  }

  protected abstract getDownloadUrl(version: string): Promise<string>;

  private async newInstall(gamePath: string, version: string): Promise<void> {
    await this.downloadAndInstall(gamePath, version);
  }

  private async downloadAndInstall(destination: string, selectedVersion: string): Promise<void> {
    processName.set(`Downloading ${this.getName()}...`);
    const downloadUrl = await this.getDownloadUrl(selectedVersion);

    if (!downloadUrl) {
      throw new Error(`Couldn't find a download URL for ${this.getName()}!`);
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

      processName.set(`Extracting ${this.getName()}...`);
      await this.unzip(tempPath, destination);
    } finally {
      await TempFileCache.clearCache();
    }
  }
}

