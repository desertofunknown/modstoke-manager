<script lang="ts">
    import InstallFeature from "../lib/InstallationComponent.svelte";
    import PathSelector from "../lib/PathSelector.svelte";
    import { openAcaiLoaderFeature, melonFeature } from "../lib/featureInstaller";
    import { gameExePath, isPathValid, isDotnetInstalled, getDirectoryPath, processName, processProgress, processing } from "../lib/store";
    import {
        ensureLatestOpenAcaiLoader,
        getInstalledLoaderVersion,
        verifyInstalledLoader,
        type LoaderIntegrityReport
    } from "../lib/openAcaiLoaderUpdater";
    import LucidePlay from "~icons/lucide/play";
    import LucideFolderOpen from "~icons/lucide/folder-open";
    import LucideShieldCheck from "~icons/lucide/shield-check";
    import LucideArrowUpRight from "~icons/lucide/arrow-up-right";
    import * as dialog from "@tauri-apps/plugin-dialog"
    import * as shell from "@tauri-apps/plugin-shell"
    import { Command } from "@tauri-apps/plugin-shell";

    let features = [melonFeature, openAcaiLoaderFeature];
    let loaderReport: LoaderIntegrityReport | null = null;
    let loaderStatus = "Not checked";
    let loaderStatusClass = "manual";
    let lastCheckedLoaderExePath = "";
    let loaderStatusRequestId = 0;
    let loaderProgressRequestId = 0;

    $: if ($isPathValid && $gameExePath && $gameExePath !== lastCheckedLoaderExePath) {
        loaderReport = null;
        loaderStatus = "Checking loader files...";
        loaderStatusClass = "manual";
        void refreshLoaderStatus(false);
    }

    $: if (!$isPathValid) {
        lastCheckedLoaderExePath = "";
        loaderReport = null;
        loaderStatus = "Not checked";
        loaderStatusClass = "manual";
    }

    async function openFolder() {
        await shell.open(await getDirectoryPath());
    }

    async function startGame() {
        if ($processing || !$isPathValid) return;
        const selectedExecutable = $gameExePath;
        processing.set(true);
        processName.set("Preparing Sons of the Forest...");
        processProgress.set(0);
        try {
            const installedVersion = await getInstalledLoaderVersion();
            if (installedVersion) {
                loaderReport = await ensureLatestOpenAcaiLoader();
                updateLoaderStatus(loaderReport);
            }
            if ($isPathValid && $gameExePath === selectedExecutable) await launchGame();
        } catch (error) {
            await dialog.message(`The game could not be prepared or started:\n${error}`, { title: "Start failed", kind: "error" });
        } finally {
            processing.set(false);
            processName.set("");
            processProgress.set(0);
        }
    }

    async function launchGame() {
        const steamUrl = "steam://rungameid/1326470";
        try {
            const cmd = Command.create("open-explorer", [steamUrl]);
            const result = await cmd.execute();
            if (result.code === 0) {
                return;
            }

            console.log("explorer steam launch failed, falling back to shell.open", result.stderr);
        } catch (error) {
            console.log("explorer steam launch failed, falling back to shell.open", error);
        }

        await shell.open(steamUrl);
    }

    async function refreshLoaderStatus(showProgress = true) {
        if (showProgress && ($processing || !$isPathValid)) return;
        const requestId = ++loaderStatusRequestId;
        const expectedExePath = $gameExePath;
        lastCheckedLoaderExePath = expectedExePath;

        if (showProgress) {
            loaderProgressRequestId = requestId;
            processing.set(true);
            processName.set("Checking Modstoke Loader files...");
            processProgress.set(0);
        }

        try {
            const report = await verifyInstalledLoader();
            if (!isCurrentLoaderStatusRequest(requestId, expectedExePath)) {
                return;
            }

            loaderReport = report;
            updateLoaderStatus(report);
        } catch (error) {
            if (!isCurrentLoaderStatusRequest(requestId, expectedExePath)) {
                return;
            }

            loaderStatus = `Loader update manifest unavailable: ${error}`;
            loaderStatusClass = "manual";
        } finally {
            if (showProgress && loaderProgressRequestId === requestId) {
                processing.set(false);
                processName.set("");
                processProgress.set(0);
            }
        }
    }

    function isCurrentLoaderStatusRequest(requestId: number, expectedExePath: string) {
        return requestId === loaderStatusRequestId && expectedExePath === $gameExePath;
    }

    async function repairLoader() {
        if ($processing || !$isPathValid) return;
        processing.set(true);
        processName.set("Preparing Modstoke Loader repair...");
        processProgress.set(0);

        try {
            loaderReport = await ensureLatestOpenAcaiLoader();
            updateLoaderStatus(loaderReport);
            await dialog.message("Modstoke Loader files are verified and current.", {
                title: "Loader verified",
                kind: "info"
            });
        } catch (error) {
            await dialog.message(`${error}`, {
                title: "Loader repair failed",
                kind: "error"
            });
        } finally {
            processing.set(false);
            processName.set("");
            processProgress.set(0);
        }
    }

    function updateLoaderStatus(report: LoaderIntegrityReport) {
        if (!report.installedVersion) {
            loaderStatus = `Latest available: ${report.manifest.version} (${report.manifest.runtime.targetFramework}, .NET ${report.manifest.runtime.dotnetRuntimeVersion})`;
            loaderStatusClass = "manual";
            return;
        }

        if (report.needsUpdate) {
            loaderStatus = `${report.verifiedFiles}/${report.totalFiles} files verified. ${report.issues.length} loader file issue${report.issues.length === 1 ? "" : "s"} detected. Latest: ${report.manifest.version}. Installed: ${report.installedVersion}.`;
            loaderStatusClass = "update";
            return;
        }

        loaderStatus = `Verified ${report.verifiedFiles}/${report.totalFiles} loader files for ${report.manifest.version} (${report.manifest.runtime.targetFramework}, .NET ${report.manifest.runtime.dotnetRuntimeVersion})`;
        loaderStatusClass = "install";
    }

    function loaderDetail(report: LoaderIntegrityReport | null): string | null {
        if (!report) {
            return null;
        }

        if (report.issues.length > 0) {
            return `${report.issues[0].path} - ${report.issues[0].reason}`;
        }

        const source = report.latestManifestSource === "cache" ? "cached release manifest" : "GitHub release manifest";
        const checkedAt = new Date(report.checkedAtUtc).toLocaleTimeString();
        return `Checked ${checkedAt} using ${source}. ${report.versionOutdated ? "Installed metadata is older, but files match the current manifest." : "No loader file drift detected."}`;
    }
</script>

<div class="main-page">
    <header class="page-heading">
        <div class="heading-copy">
            <p class="eyebrow">Game workspace</p>
            <h1>Sons of the Forest</h1>
            <p class="page-description">Your installation and modding tools.</p>
        </div>
        {#if $isPathValid}
            <div class="main-actions">
                <button class="folder-button" disabled={$processing} on:click={openFolder} title="Open game folder" aria-label="Open game folder">
                    <LucideFolderOpen aria-hidden="true" />
                </button>
                <button class="launch-button" disabled={$processing} on:click={startGame}>
                    <LucidePlay aria-hidden="true" />
                    <span>Launch game<small>through Steam</small></span>
                </button>
            </div>
        {/if}
    </header>

    <PathSelector />

    {#if $isPathValid}
        <div class="loader-workspace">
            <section class="loader-setup" aria-labelledby="loader-setup-heading">
                <div class="section-heading">
                    <div>
                        <h2 id="loader-setup-heading">Mod loaders</h2>
                        <p>Install and manage your modding tools.</p>
                    </div>
                </div>
                <div class="feature-list">
                    {#each features as feature}
                        <InstallFeature feature={feature} />
                    {/each}
                </div>
            </section>

            <section class="loader-health" aria-labelledby="loader-health-heading">
                <div class="health-heading">
                    <span class="health-icon"><LucideShieldCheck aria-hidden="true" /></span>
                    <div>
                        <h2 id="loader-health-heading">Loader verification</h2>
                        <span class="health-subtitle">Modstoke Loader</span>
                    </div>
                </div>
                <div class="health-result {loaderStatusClass}" role="status">
                    <span class="status-indicator" aria-hidden="true"></span>
                    <p>{loaderStatus}</p>
                </div>
                {#if loaderDetail(loaderReport)}
                    <p class="health-detail">{loaderDetail(loaderReport)}</p>
                {/if}
                <div class="health-actions">
                    <button class="tool-button" disabled={$processing} on:click={() => refreshLoaderStatus(true)}>Check files</button>
                    <button class="tool-button repair-button" disabled={$processing} on:click={repairLoader}>{loaderReport?.needsUpdate ? "Update / Repair" : "Verify / Repair"}</button>
                </div>
            </section>
        </div>
    {/if}

    {#if !$isDotnetInstalled}
        <p class="runtime-note"><a href="https://dotnet.microsoft.com/en-us/download/dotnet/11.0" target="_blank" rel="noreferrer">.NET 11</a> is recommended for current Modstoke Loader builds.</p>
    {/if}

    <footer class="workspace-footer">
        <span>Independent tools by OpenACAI. Not affiliated with Endnight Games.</span>
        <a href="https://github.com/desertofunknown/openacai-loader" target="_blank" rel="noreferrer">Modstoke Loader <LucideArrowUpRight aria-hidden="true" /></a>
    </footer>
</div>

<style>
    .main-page {
        display: flex;
        flex-direction: column;
        gap: 24px;
        width: 100%;
        min-width: 0;
        color: var(--text-primary, #eef0f4);
        text-align: left;
    }

    .page-heading {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 20px;
        min-width: 0;
        padding: 3px 0 1px;
    }

    .heading-copy { min-width: 0; }
    .eyebrow {
        margin: 0 0 9px;
        color: var(--accent, #eeb66b);
        font-size: 10px;
        font-weight: 700;
        letter-spacing: .14em;
        line-height: 1.4;
        text-transform: uppercase;
    }

    h1 {
        margin: 0;
        color: var(--text-primary, #eef0f4);
        font-size: clamp(26px, 2.5vw, 34px);
        font-weight: 650;
        letter-spacing: -.035em;
        line-height: 1.15;
    }

    .page-description {
        margin: 10px 0 0;
        color: var(--text-secondary, #9da6b6);
        font-size: 13px;
        line-height: 1.5;
    }

    .main-actions { display: flex; align-items: center; gap: 10px; flex-shrink: 0; }
    .main-actions button {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        height: 48px;
        margin: 0;
        border: 1px solid var(--border-subtle, #30353f);
        border-radius: 9px;
    }

    .folder-button { width: 46px; padding: 0; background: var(--surface-panel, #171a21); color: var(--text-secondary, #9da6b6); }
    .folder-button :global(svg) { width: 18px; height: 18px; }
    .launch-button { gap: 12px; padding: 0 20px; background: var(--accent, #eeb66b); color: #23190d; font-size: 14px; font-weight: 700; }
    .launch-button :global(svg) { width: 17px; height: 17px; fill: currentColor; stroke-width: 1.5; }
    .launch-button span { text-align: left; line-height: 1.1; }
    .launch-button small { display: block; margin-top: 4px; font-size: 10px; font-weight: 500; opacity: .72; }
    .launch-button:hover:not(:disabled) { background: #f5c78a; color: #23190d; }
    .folder-button:hover:not(:disabled) { background: var(--surface-raised, #20242c); color: var(--text-primary, #eef0f4); }

    .loader-workspace { display: grid; grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr); align-items: stretch; gap: 20px; }
    .loader-setup { min-width: 0; }
    .section-heading { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-bottom: 14px; }
    h2 { margin: 0; color: var(--text-primary, #eef0f4); font-size: 15px; font-weight: 650; line-height: 1.4; letter-spacing: -.01em; }
    .section-heading p { margin: 4px 0 0; color: var(--text-secondary, #9da6b6); font-size: 12px; line-height: 1.5; }
    .feature-list { display: flex; flex-direction: column; gap: 10px; min-width: 0; }
    .loader-health { display: flex; flex-direction: column; gap: 14px; min-width: 0; padding: 20px; background: var(--surface-panel, #171a21); border: 1px solid var(--border-subtle, #30353f); border-radius: 10px; }
    .health-heading { display: flex; align-items: center; gap: 11px; }
    .health-icon { display: grid; place-items: center; width: 36px; height: 36px; flex-shrink: 0; border: 1px solid var(--border-subtle, #30353f); background: var(--surface-raised, #20242c); border-radius: 9px; color: var(--text-secondary, #9da6b6); }
    .health-icon :global(svg) { width: 19px; height: 19px; }
    .health-subtitle { display: block; margin-top: 3px; color: var(--text-secondary, #9da6b6); font-size: 11px; line-height: 1.4; }
    .health-result { display: flex; align-items: flex-start; gap: 9px; color: #bac3d2; }
    .health-result p { margin: 0; font-size: 12px; line-height: 1.65; overflow-wrap: anywhere; }
    .status-indicator { width: 6px; height: 6px; flex-shrink: 0; margin-top: 7px; border-radius: 50%; background: currentColor; }
    .health-result.install { color: #9ac7ae; }
    .health-result.update { color: var(--accent, #eeb66b); }
    .health-detail { margin: -3px 0 0 15px; color: var(--text-secondary, #9da6b6); font-size: 11px; line-height: 1.65; overflow-wrap: anywhere; }
    .health-actions { display: flex; gap: 8px; flex-wrap: wrap; margin-top: auto; padding-top: 2px; }
    .tool-button { flex: 1 1 auto; min-height: 35px; margin: 0; padding: 8px 12px; border: 1px solid var(--border-subtle, #30353f); border-radius: 7px; color: var(--text-primary, #eef0f4); background: var(--surface-raised, #20242c); font-size: 12px; font-weight: 600; }
    .tool-button:hover:not(:disabled) { border-color: #515967; background: #282e38; }
    .repair-button { color: var(--accent, #eeb66b); }

    .runtime-note { margin: -5px 0 0; color: var(--text-secondary, #9da6b6); font-size: 12px; line-height: 1.5; }
    .runtime-note a { color: var(--accent, #eeb66b); }
    .workspace-footer { display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 12px; padding-top: 17px; border-top: 1px solid var(--border-subtle, #30353f); color: #7d8797; font-size: 10px; line-height: 1.5; }
    .workspace-footer a { display: inline-flex; align-items: center; gap: 4px; color: var(--text-secondary, #9da6b6); white-space: nowrap; }
    .workspace-footer a :global(svg) { width: 13px; height: 13px; }
    .workspace-footer a:hover { color: var(--accent, #eeb66b); }
    button:disabled { opacity: .45; }
    button:focus-visible, a:focus-visible { outline: 2px solid var(--accent, #eeb66b); outline-offset: 3px; }

    @media (max-width: 1100px) {
        .main-page { gap: 22px; }
        .loader-workspace { gap: 16px; }
        .loader-health { padding: 17px; }
        .launch-button { padding: 0 16px; }
    }

    @media (max-width: 820px) {
        .page-heading { align-items: flex-start; flex-wrap: wrap; }
        .loader-workspace { grid-template-columns: minmax(0, 1fr); }
        .health-actions { justify-content: flex-start; }
        .tool-button { flex: 0 1 auto; }
    }
</style>
