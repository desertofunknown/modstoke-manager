// The website publishes this index only after its release checks pass. Keep this
// parser independent of Tauri so malformed or stale records can be tested here.

export const SITE_ORIGIN = "https://modstoke.com";
export const SITE_INDEX_PATH = "/release-index.json";
export const SITE_LOADER_MANIFEST_PATH = "/releases/loader/latest/OpenACAILoader.manifest.json";

export type SiteProduct = "manager" | "loader";
export type SiteCheck = "passed" | "not-applicable" | "pending";

export type SiteArtifact = {
    kind: "manager-windows" | "game-root-bundle";
    sitePath: string;
    fileName: string;
    size: number;
    sha256: string;
    signatureStatus: "verified" | "unsigned-preview";
    unsignedReason?: string;
    signingEvidence: {
        publisher: string;
        verifiedUtc: string;
        files: { path: string; sha256: string; certificateThumbprint: string }[];
    } | null;
};

type CommonRelease = {
    releaseId: string;
    product: SiteProduct;
    version: string;
    channel: "stable" | "preview";
    publishedUtc: string;
    sourceCommit: string;
    notesPath: string;
    validation: Record<"cleanSource" | "licenses" | "build" | "install" | "game" | "multiplayer" | "hostedFetch", SiteCheck>;
    artifacts: [SiteArtifact];
};

export type SiteManagerRelease = CommonRelease & {
    product: "manager";
    defaultLoaderReleaseId: string | null;
};

export type SiteLoaderRelease = CommonRelease & {
    product: "loader";
    compatibility: {
        gameId: "sons-of-the-forest";
        testedGameBuilds: string[];
        minManagerVersion: string;
    };
    bundled: {
        sdk: { assemblyPath: string; sourceCommit: string };
        client: { pluginId: string; version: string; installPath: string; sourceCommit: string };
    };
};

export type SiteReleaseIndex = {
    schema: 1;
    latest: { manager: string | null; loader: string | null };
    releases: (SiteManagerRelease | SiteLoaderRelease)[];
};

const slug = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const versionText = /^[0-9]+\.[0-9]+\.[0-9]+(?:[-+][A-Za-z0-9.-]+)?$/;
const sha256 = /^[a-f0-9]{64}$/i;
const commit = /^[a-f0-9]{40,64}$/i;
const fileName = /^[A-Za-z0-9][A-Za-z0-9._-]*$/;
const coreChecks = ["cleanSource", "licenses", "build", "install", "hostedFetch"] as const;

function fail(detail: string): never {
    throw new Error(`Invalid Modstoke release index: ${detail}`);
}

function object(value: unknown): value is Record<string, unknown> {
    return value !== null && typeof value === "object" && !Array.isArray(value);
}

function filled(value: unknown): value is string {
    return typeof value === "string" && value.trim() === value && value.length > 0;
}

function utc(value: unknown): value is string {
    return filled(value) && /Z$/.test(value) && !Number.isNaN(Date.parse(value));
}

export function safeGameRelativePath(value: unknown): value is string {
    return filled(value) && !value.includes("\\") && !value.startsWith("/") &&
        value.split("/").every(part => part.length > 0 && part !== "." && part !== ".." && !part.includes(":"));
}

export function configuredSiteIndexUrl(value: string | null | undefined): string | null {
    if (!value) return null;
    let url: URL;
    try {
        url = new URL(value);
    } catch {
        return fail("configured website URL is not a URL");
    }
    if (url.origin !== SITE_ORIGIN || url.pathname !== SITE_INDEX_PATH || url.search || url.hash || url.username || url.password) {
        fail(`configured website URL must be ${SITE_ORIGIN}${SITE_INDEX_PATH}`);
    }
    return url.href;
}

export function siteArtifactUrl(artifact: SiteArtifact): string {
    return new URL(artifact.sitePath, SITE_ORIGIN).href;
}

export function parseSiteReleaseIndex(raw: unknown): SiteReleaseIndex {
    if (!object(raw) || raw.schema !== 1 || !object(raw.latest) || !Array.isArray(raw.releases)) {
        fail("expected schema 1, latest, and releases");
    }
    if (Object.keys(raw.latest).sort().join(",") !== "loader,manager") {
        fail("latest must contain manager and loader only");
    }
    const byId = new Map<string, SiteManagerRelease | SiteLoaderRelease>();
    const artifactPaths = new Set<string>();

    for (const value of raw.releases) {
        if (!object(value) || !filled(value.releaseId) || !slug.test(value.releaseId) || byId.has(value.releaseId)) {
            fail("releaseId must be unique and URL-safe");
        }
        const id = value.releaseId;
        if (value.product !== "manager" && value.product !== "loader") fail(`${id}: invalid product`);
        if (!filled(value.version) || !versionText.test(value.version) ||
            (value.channel !== "stable" && value.channel !== "preview") ||
            !utc(value.publishedUtc) || !filled(value.sourceCommit) || !commit.test(value.sourceCommit) ||
            value.notesPath !== `/releases/#${id}`) {
            fail(`${id}: invalid version, channel, publication, source, or notes`);
        }
        const validation = value.validation;
        if (!object(validation) || coreChecks.some(key => validation[key] !== "passed") ||
            ([validation.game, validation.multiplayer].some(check =>
                check !== "passed" &&
                !(value.product === "manager" && check === "not-applicable") &&
                !(value.product === "loader" && value.channel === "preview" && check === "pending")))) {
            fail(`${id}: release validation has not passed`);
        }
        if (!Array.isArray(value.artifacts) || value.artifacts.length !== 1 || !object(value.artifacts[0])) {
            fail(`${id}: exactly one artifact is required`);
        }
        const artifact = value.artifacts[0];
        const expectedKind = value.product === "manager" ? "manager-windows" : "game-root-bundle";
        if (artifact.kind !== expectedKind || !filled(artifact.fileName) || !fileName.test(artifact.fileName) || artifact.fileName.includes("..") ||
            artifact.sitePath !== `/downloads/${value.product}/${value.version}/${artifact.fileName}` ||
            !Number.isSafeInteger(artifact.size) || (artifact.size as number) <= 0 ||
            !filled(artifact.sha256) || !sha256.test(artifact.sha256) ||
            (artifact.signatureStatus !== "verified" && artifact.signatureStatus !== "unsigned-preview") ||
            artifactPaths.has(artifact.sitePath as string)) {
            fail(`${id}: artifact identity, hash, or verification is invalid`);
        }
        if (value.product === "loader" && !artifact.fileName.toLowerCase().endsWith(".zip")) fail(`${id}: Loader artifact must be a ZIP`);
        if (value.product === "manager" && !/\.(exe|msi|zip)$/i.test(artifact.fileName)) fail(`${id}: unsupported Manager artifact`);
        artifactPaths.add(artifact.sitePath as string);

        if (artifact.signatureStatus === "unsigned-preview") {
            if (value.channel !== "preview" || artifact.signingEvidence !== null || !filled(artifact.unsignedReason)) {
                fail(`${id}: unsigned preview requires a reason and no signing claim`);
            }
        } else {
            const evidence = artifact.signingEvidence;
            if (!object(evidence) || !filled(evidence.publisher) || !utc(evidence.verifiedUtc) ||
                !Array.isArray(evidence.files) || evidence.files.length === 0 ||
                evidence.files.some((file: unknown) => !object(file) || !safeGameRelativePath(file.path) ||
                    !filled(file.sha256) || !sha256.test(file.sha256) || !filled(file.certificateThumbprint) ||
                    !/^[a-f0-9]{40,64}$/i.test(file.certificateThumbprint))) {
                fail(`${id}: signing evidence is incomplete`);
            }
            if (value.product === "manager" && !evidence.files.some((file: unknown) => object(file) &&
                file.path === artifact.fileName && typeof file.sha256 === "string" &&
                file.sha256.toLowerCase() === (artifact.sha256 as string).toLowerCase())) {
                fail(`${id}: Manager package is absent from signing evidence`);
            }
        }

        if (value.product === "manager") {
            if (value.defaultLoaderReleaseId !== null &&
                (!filled(value.defaultLoaderReleaseId) || !slug.test(value.defaultLoaderReleaseId))) {
                fail(`${id}: invalid default Loader reference`);
            }
        } else {
            const fit = value.compatibility;
            const bundle = value.bundled;
            if (!object(fit) || fit.gameId !== "sons-of-the-forest" || !Array.isArray(fit.testedGameBuilds) ||
                (fit.testedGameBuilds.length === 0 && validation.game !== "pending") ||
                (fit.testedGameBuilds.length > 0 && validation.game === "pending") ||
                fit.testedGameBuilds.some(build => !filled(build)) ||
                !filled(fit.minManagerVersion) || !versionText.test(fit.minManagerVersion) ||
                !object(bundle) || !object(bundle.sdk) || !safeGameRelativePath(bundle.sdk.assemblyPath) ||
                !filled(bundle.sdk.sourceCommit) || !commit.test(bundle.sdk.sourceCommit) ||
                !object(bundle.client) || !filled(bundle.client.pluginId) || bundle.client.pluginId === "com.openacai.sotf.admin" ||
                !filled(bundle.client.version) || !versionText.test(bundle.client.version) ||
                !safeGameRelativePath(bundle.client.installPath) || !filled(bundle.client.sourceCommit) ||
                !commit.test(bundle.client.sourceCommit)) {
                fail(`${id}: complete compatible SDK and client bundle is required`);
            }
        }
        byId.set(id, value as SiteManagerRelease | SiteLoaderRelease);
    }

    for (const product of ["manager", "loader"] as const) {
        const selected = raw.latest[product];
        if (selected !== null && (!filled(selected) || byId.get(selected)?.product !== product)) {
            fail(`latest.${product} must select a matching release`);
        }
    }
    for (const record of byId.values()) {
        if (record.product === "manager" && record.defaultLoaderReleaseId !== null &&
            byId.get(record.defaultLoaderReleaseId)?.product !== "loader") {
            fail(`${record.releaseId}: default Loader release is missing`);
        }
    }
    const selectedManagerId = raw.latest.manager;
    const selectedManager = typeof selectedManagerId === "string" ? byId.get(selectedManagerId) : null;
    if (selectedManager?.product === "manager" && selectedManager.defaultLoaderReleaseId !== null &&
        selectedManager.defaultLoaderReleaseId !== raw.latest.loader) {
        fail("selected Manager and Loader bundle do not agree");
    }
    return raw as SiteReleaseIndex;
}

export function selectedSiteRelease(index: SiteReleaseIndex, product: "manager"): SiteManagerRelease | null;
export function selectedSiteRelease(index: SiteReleaseIndex, product: "loader"): SiteLoaderRelease | null;
export function selectedSiteRelease(index: SiteReleaseIndex, product: SiteProduct): SiteManagerRelease | SiteLoaderRelease | null {
    const id = index.latest[product];
    return (index.releases.find(release => release.releaseId === id) ?? null);
}

export function selectedGuidedBundle(index: SiteReleaseIndex): { manager: SiteManagerRelease | null; loader: SiteLoaderRelease } | null {
    const manager = selectedSiteRelease(index, "manager");
    const loader = selectedSiteRelease(index, "loader");
    if (!loader) return null;
    // A Loader may be published before the next Manager binary. Once a Manager
    // release is selected, its reviewed default bundle must agree with it.
    if (manager?.defaultLoaderReleaseId && manager.defaultLoaderReleaseId !== loader.releaseId) {
        fail("selected Manager and Loader bundle do not agree");
    }
    return { manager, loader };
}

export function assertSiteLoaderManifest(raw: unknown, loader: SiteLoaderRelease): void {
    if (!object(raw) || raw.schema !== 1 || raw.version !== loader.version ||
        !object(raw.package) || !object(raw.update) || !object(raw.runtime) || !Array.isArray(raw.files)) {
        fail("Loader compatibility manifest is incomplete");
    }
    const artifact = loader.artifacts[0];
    if (raw.package.fileName !== artifact.fileName || raw.package.downloadUrl !== siteArtifactUrl(artifact) ||
        typeof raw.package.sha256 !== "string" || raw.package.sha256.toLowerCase() !== artifact.sha256.toLowerCase() ||
        raw.update.manifestUrl !== `${SITE_ORIGIN}${SITE_LOADER_MANIFEST_PATH}` ||
        raw.update.minManagerVersion !== loader.compatibility.minManagerVersion ||
        !filled(raw.runtime.targetFramework) || !filled(raw.runtime.dotnetRuntimeVersion)) {
        fail("Loader manifest does not match the selected website artifact");
    }
    const paths = new Set<string>();
    for (const entry of raw.files) {
        if (!object(entry) || !safeGameRelativePath(entry.path) || !filled(entry.sha256) || !sha256.test(entry.sha256) ||
            !Number.isSafeInteger(entry.size) || (entry.size as number) <= 0 || paths.has(entry.path)) {
            fail("Loader manifest has an invalid or duplicate installed file");
        }
        paths.add(entry.path);
    }
    if (!paths.has(loader.bundled.sdk.assemblyPath) || !paths.has(loader.bundled.client.installPath)) {
        fail("Loader manifest does not include its SDK and client companion");
    }
}
