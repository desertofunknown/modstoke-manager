import { BaseZipInstaller } from "./baseZipInstaller";
import { getDirectoryPath, gameExePath, processName, processProgress } from "./store"
import { get } from 'svelte/store'
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
      console.log(`Couldn't find a target version for ${this.getName()}!`);
      return;
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
      console.log(`Couldn't find download url for ${this.getName()}!`);
      return;
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

      // let client = await http.getClient();
      // const response = await client.get<Uint8Array>(downloadUrl, {
      //   timeout: 30,
      //   responseType: http.ResponseType.Binary,
      // });
    
      // const binaryData = response.data;
    
      // await fs.writeBinaryFile(tempPath, binaryData);
    } catch (error) {
      console.log(error);
      return;
    }

    //processProgress.set(50);

    processName.set(`Extracting ${this.getName()}...`);

    try {
      await this.unzip(tempPath, destination);
    } catch (error) {
      console.log(error);
      return;
    }

    await TempFileCache.clearCache();
  }
}

