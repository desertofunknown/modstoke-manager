import { BaseZipInstaller } from "./baseZipInstaller";
import { getDirectoryPath } from "./store";
import { invoke } from "@tauri-apps/api/core";
import * as dialog from "@tauri-apps/plugin-dialog"

type LoaderZipInspection = {
    is_valid: boolean;
    has_manifest: boolean;
    has_bridge: boolean;
    has_bepinex_core: boolean;
    has_doorstop: boolean;
    has_private_admin_tool: boolean;
    entries: number;
    errors: string[];
};

export class ManualZipInstaller extends BaseZipInstaller {
    private readonly _dialogTitle: string;
    private _selectedZipPath: string | null = null;

    constructor(name: string, dialogTitle: string) {
        super(name);
        this._dialogTitle = dialogTitle;
    }

    public async prepare(): Promise<boolean> {
        const selected = await dialog.open({
            title: this._dialogTitle,
            multiple: false,
            directory: false,
            filters: [
                {
                    name: "Loader zip",
                    extensions: ["zip"]
                }
            ]
        });

        if (!selected || Array.isArray(selected)) {
            this._selectedZipPath = null;
            return false;
        }

        const inspection = await invoke<LoaderZipInspection>("inspect_openacai_loader_zip", {
            source: selected
        });

        if (!inspection.is_valid) {
            const details = inspection.errors.length > 0
                ? inspection.errors.join("\n")
                : "The selected zip is missing the Endnight Loader manifest or compatibility bridge.";

            await dialog.message(details, {
                title: "Invalid Endnight Loader package",
                kind: "error"
            });

            this._selectedZipPath = null;
            return false;
        }

        this._selectedZipPath = selected;
        return true;
    }

    public async install(): Promise<void> {
        if (!this._selectedZipPath) {
            return;
        }

        try {
            await this.unzip(this._selectedZipPath, await getDirectoryPath());
        } finally {
            this._selectedZipPath = null;
        }
    }

    public getTargetVersion(): Promise<string | null> {
        return Promise.resolve(null);
    }
}
