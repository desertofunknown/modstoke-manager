import { getDirectoryPath } from './store';
import * as fs from "@tauri-apps/plugin-fs"

export class BaseUninstaller {
    private foldersToClear: string[];
    private filesToClear: string[];
    private name: string;

    public overrideCheckFiles: string[] | null = null;

    constructor(
        foldersToClear: string[],
        filesToClear: string[],
        name: string
    ) {
        this.foldersToClear = foldersToClear;
        this.filesToClear = filesToClear;
        this.name = name;
    }

    protected async getFilePath(fileName: string, gameRoot?: string): Promise<string> {
        const directoryPath = gameRoot ?? await getDirectoryPath();
        return `${directoryPath}/${fileName}`;
    }

    public get needsDialog(): boolean {
        return true;
    }

    public get customMessage(): string | null {
        return null;
    }

    public async uninstall(): Promise<void> {
        const gameRoot = await getDirectoryPath();
        for (const folder of this.foldersToClear) {
            const folderPath = await this.getFilePath(folder, gameRoot);
            if (await fs.exists(folderPath)) {
                await fs.remove(folderPath, { recursive: true });
            }
        }

        for (const file of this.filesToClear) {
            const filePath = await this.getFilePath(file, gameRoot);
            if (await fs.exists(filePath)) {
                await fs.remove(filePath);
            }
        }
    }

    public async isInstalled(gameRoot?: string): Promise<boolean> {
        const directoryPath = gameRoot ?? await getDirectoryPath();

        if (this.overrideCheckFiles) {
            for (const file of this.overrideCheckFiles) {
                const filePath = await this.getFilePath(file, directoryPath);
                if (await fs.exists(filePath)) {
                    return true;
                }
            }
            return false;
        }

        for (const folder of this.foldersToClear) {
            const folderPath = await this.getFilePath(folder, directoryPath);
            if (await fs.exists(folderPath)) {
                return true;
            }
        }

        for (const file of this.filesToClear) {
            const filePath = await this.getFilePath(file, directoryPath);
            if (await fs.exists(filePath)) {
                return true;
            }
        }

        return false;
    }

    public getName(): string {
        return this.name;
    }
}
