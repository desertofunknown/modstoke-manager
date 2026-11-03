export class BaseInstaller {
    private _name: string;

    constructor(name: string) {
        this._name = name;
    }

    public async prepare(): Promise<boolean> {
        return true;
    }

    public async install(): Promise<void> {
    }

    public getName(): string {
        return this._name;
    }

    public getTargetVersion(): Promise<string | null> {
        return Promise.resolve("");
    }
}
