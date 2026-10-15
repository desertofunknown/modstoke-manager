import assert from "node:assert/strict";
import test from "node:test";
import {
    assertSiteLoaderManifest,
    configuredSiteIndexUrl,
    parseSiteReleaseIndex,
    selectedGuidedBundle,
    selectedSiteRelease,
    siteArtifactUrl
} from "../src/lib/siteReleaseIndex.ts";

const hash = "a".repeat(64);
const commit = "b".repeat(40);
const thumbprint = "c".repeat(40);
const checks = {
    cleanSource: "passed", licenses: "passed", build: "passed", install: "passed",
    game: "passed", multiplayer: "passed", hostedFetch: "passed"
};

function loader() {
    return {
        releaseId: "loader-preview-1", product: "loader", version: "0.4.1-preview.1", channel: "preview",
        publishedUtc: "2026-10-15T00:00:00Z", sourceCommit: commit,
        notesPath: "/releases/#loader-preview-1", validation: { ...checks },
        compatibility: { gameId: "sons-of-the-forest", testedGameBuilds: ["build-1"], minManagerVersion: "0.2.0" },
        bundled: {
            sdk: { assemblyPath: "OpenACAI/Runtime/OpenACAI.Endnight.Loader.Core.dll", sourceCommit: commit },
            client: { pluginId: "com.modstoke.sotf.client", version: "0.1.0", installPath: "BepInEx/plugins/ModstokeClient/ModstokeClient.dll", sourceCommit: commit }
        },
        artifacts: [{
            kind: "game-root-bundle", sitePath: "/downloads/loader/0.4.1-preview.1/bundle.zip",
            fileName: "bundle.zip", size: 123, sha256: hash, signatureStatus: "verified",
            signingEvidence: { publisher: "OpenACAI", verifiedUtc: "2026-10-15T00:00:00Z", files: [{ path: "OpenACAI/Runtime/OpenACAI.Endnight.Loader.Core.dll", sha256: hash, certificateThumbprint: thumbprint }] }
        }]
    };
}

function manager() {
    return {
        releaseId: "manager-preview-1", product: "manager", version: "0.2.1-preview.1", channel: "preview",
        publishedUtc: "2026-10-15T00:00:00Z", sourceCommit: commit,
        notesPath: "/releases/#manager-preview-1", validation: { ...checks },
        defaultLoaderReleaseId: "loader-preview-1",
        artifacts: [{
            kind: "manager-windows", sitePath: "/downloads/manager/0.2.1-preview.1/Manager.exe",
            fileName: "Manager.exe", size: 234, sha256: hash, signatureStatus: "verified",
            signingEvidence: { publisher: "OpenACAI", verifiedUtc: "2026-10-15T00:00:00Z", files: [{ path: "Manager.exe", sha256: hash, certificateThumbprint: thumbprint }] }
        }]
    };
}

function index() {
    return { schema: 1, latest: { manager: "manager-preview-1", loader: "loader-preview-1" }, releases: [manager(), loader()] };
}

function manifest(record = loader()) {
    return {
        schema: 1, version: record.version,
        package: { fileName: "bundle.zip", downloadUrl: siteArtifactUrl(record.artifacts[0]), sha256: hash },
        update: { manifestUrl: "https://modstoke.com/releases/loader/latest/OpenACAILoader.manifest.json", minManagerVersion: "0.2.0" },
        runtime: { targetFramework: "net11.0", dotnetRuntimeVersion: "11.0" },
        files: [
            { path: record.bundled.sdk.assemblyPath, sha256: hash, size: 1 },
            { path: record.bundled.client.installPath, sha256: hash, size: 1 }
        ]
    };
}

test("empty website feed advertises no installable bundle", () => {
    const empty = parseSiteReleaseIndex({ schema: 1, latest: { manager: null, loader: null }, releases: [] });
    assert.equal(selectedGuidedBundle(empty), null);
});

test("selected complete bundle and Manager download resolve", () => {
    const parsed = parseSiteReleaseIndex(index());
    assert.equal(selectedGuidedBundle(parsed)?.loader.releaseId, "loader-preview-1");
    assert.equal(selectedSiteRelease(parsed, "manager")?.releaseId, "manager-preview-1");
    assert.doesNotThrow(() => assertSiteLoaderManifest(manifest(), selectedGuidedBundle(parsed).loader));
});

test("a reviewed Loader may publish before a new Manager package", () => {
    const raw = index();
    raw.latest.manager = null;
    assert.equal(selectedGuidedBundle(parseSiteReleaseIndex(raw))?.loader.releaseId, "loader-preview-1");
});

test("a selected Manager with no default can precede the Loader rollout", () => {
    const raw = index();
    raw.releases[0].defaultLoaderReleaseId = null;
    assert.equal(selectedGuidedBundle(parseSiteReleaseIndex(raw))?.loader.releaseId, "loader-preview-1");
});

test("selected Manager must reference the selected Loader", () => {
    const raw = index();
    raw.latest.loader = null;
    assert.throws(() => parseSiteReleaseIndex(raw), /do not agree/);
});

test("unsafe artifact paths and incomplete bundles fail closed", () => {
    const badPath = index();
    badPath.releases[0].artifacts[0].sitePath = "https://evil.invalid/bundle.zip";
    assert.throws(() => parseSiteReleaseIndex(badPath), /artifact identity/);
    const absentClient = index();
    delete absentClient.releases[1].bundled.client;
    assert.throws(() => parseSiteReleaseIndex(absentClient), /complete compatible/);
});

test("compatibility manifest must bind the ZIP and both installed components", () => {
    const selected = selectedGuidedBundle(parseSiteReleaseIndex(index())).loader;
    const wrongHash = manifest();
    wrongHash.package.sha256 = "d".repeat(64);
    assert.throws(() => assertSiteLoaderManifest(wrongHash, selected), /does not match/);
    const missingClient = manifest();
    missingClient.files.pop();
    assert.throws(() => assertSiteLoaderManifest(missingClient, selected), /does not include/);
});

test("website origin is exact and opt-in", () => {
    assert.equal(configuredSiteIndexUrl(undefined), null);
    assert.equal(configuredSiteIndexUrl("https://modstoke.com/release-index.json"), "https://modstoke.com/release-index.json");
    assert.throws(() => configuredSiteIndexUrl("https://example.com/release-index.json"), /must be/);
    assert.throws(() => configuredSiteIndexUrl("https://modstoke.com.evil.invalid/release-index.json"), /must be/);
});

test("unsigned preview with pending game checks is explicit but installable for testing", () => {
    const raw = index();
    const record = raw.releases[1];
    record.validation.game = "pending";
    record.validation.multiplayer = "pending";
    record.compatibility.testedGameBuilds = [];
    record.artifacts[0].signatureStatus = "unsigned-preview";
    record.artifacts[0].signingEvidence = null;
    record.artifacts[0].unsignedReason = "Test package awaiting code signing";
    assert.equal(selectedGuidedBundle(parseSiteReleaseIndex(raw))?.loader.releaseId, "loader-preview-1");
    assert.doesNotThrow(() => assertSiteLoaderManifest(manifest(), record));
});

test("unsigned preview requires a reason and stable releases cannot be unsigned or pending", () => {
    const missingReason = index();
    missingReason.releases[1].artifacts[0].signatureStatus = "unsigned-preview";
    missingReason.releases[1].artifacts[0].signingEvidence = null;
    assert.throws(() => parseSiteReleaseIndex(missingReason), /unsigned preview/);

    const stable = index();
    stable.releases[1].channel = "stable";
    stable.releases[1].validation.game = "pending";
    stable.releases[1].compatibility.testedGameBuilds = [];
    assert.throws(() => parseSiteReleaseIndex(stable), /validation/);
});
