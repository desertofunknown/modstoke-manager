import semver from "semver";

interface GithubRelease {
    tag_name: string;
    draft: boolean;
    prerelease: boolean;
    assets: { name: string }[];
}

function isGithubRelease(value: unknown): value is GithubRelease {
    if (typeof value !== "object" || value === null) return false;

    const release = value as Record<string, unknown>;
    return typeof release.tag_name === "string"
        && typeof release.draft === "boolean"
        && typeof release.prerelease === "boolean"
        && Array.isArray(release.assets)
        && release.assets.every((asset: unknown) => typeof asset === "object"
            && asset !== null
            && "name" in asset
            && typeof asset.name === "string");
}

export class GithubInfo {
    private readonly _repo: string;
    private readonly _githubFileName: string;
    private _releases: string[] = [];
    private _initialised = false;
    private _fetchPromise: Promise<void> | null = null;

    constructor(repo: string, githubFileName: string) {
        this._repo = repo;
        this._githubFileName = githubFileName;
    }

    get apiPath(): string {
        return `https://api.github.com/repos/${this._repo}/releases`;
    }

    get downloadPath(): string {
        return `https://github.com/${this._repo}/releases/download`;
    }

    public getDownloadLink(version: string): string {
        return `${this.downloadPath}/${version}/${this._githubFileName}`;
    }

    public async fetch(): Promise<void> {
        if (this.isInitialised()) return;
        if (this._fetchPromise) return this._fetchPromise;

        this._fetchPromise = this.fetchReleases();
        try {
            await this._fetchPromise;
        } finally {
            this._fetchPromise = null;
        }
    }

    private async fetchReleases(): Promise<void> {
        const headers = new Headers();
        headers.append("User-Agent", "Tauri App");

        const response = await fetch(this.apiPath, { headers });
        if (!response.ok) {
            throw new Error(`Failed to fetch releases for ${this._repo} (HTTP ${response.status}).`);
        }

        let data: unknown;
        try {
            data = await response.json();
        } catch {
            throw new Error(`GitHub returned invalid release data for ${this._repo}.`);
        }

        if (!Array.isArray(data) || !data.every(isGithubRelease)) {
            throw new Error(`GitHub returned invalid release data for ${this._repo}.`);
        }

        const releases = data.filter(release => {
            if (release.draft || release.prerelease) return false;
            if (!release.assets.some(asset => asset.name === this._githubFileName)) return false;

            const version = semver.parse(release.tag_name);
            return version !== null && version.prerelease.length === 0;
        }).map(release => release.tag_name);

        // Preserve the original tag for GitHub's download URL.
        this._releases = [...new Set(releases)].sort(semver.rcompare);
        this._initialised = true;
    }

    public async getLatest(): Promise<string | null> {
        await this.fetch();
        if (this._releases.length <= 0) return null;
        return this._releases[0];
    }

    public async getReleases(): Promise<string[]> {
        await this.fetch();
        return [...this._releases];
    }

    public isInitialised(): boolean {
        return this._initialised;
    }
}

export let redLoaderInfo = new GithubInfo("ToniMacaroni/RedLoader", "RedLoader.zip");
export let unityExplorerInfo = new GithubInfo("ToniMacaroni/UnityExplorer_Sons", "UnityExplorer.zip");
