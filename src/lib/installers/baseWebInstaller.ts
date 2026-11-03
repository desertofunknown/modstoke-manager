import { BaseZipInstaller } from "./baseZipInstaller";
import { getDirectoryPath } from "../store"
import { downloadAndInstall } from "../utils";

export abstract class BaseWebInstaller extends BaseZipInstaller {
  protected constructor(name: string) {
    super(name);
  }

  public getLocalVersion(): string | null {
    return null;
  }

  public async install(): Promise<void> {
    const gameRoot = await getDirectoryPath();

    const selectedVersion = await this.getTargetVersion();
    if (!selectedVersion) {
      throw new Error(`Couldn't find a target version for ${this.getName()}!`);
    }

    await downloadAndInstall(gameRoot, await this.getDownloadUrl(selectedVersion), this.getName());
  }

  protected abstract getDownloadUrl(version: string): Promise<string>;
}
