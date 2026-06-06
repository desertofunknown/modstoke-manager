<script lang="ts">
    import InstallFeature from "../lib/InstallationComponent.svelte";
    import PathSelector from "../lib/PathSelector.svelte";
    import { openAcaiLoaderFeature, melonFeature } from "../lib/featureInstaller";
    import { isPathValid, isDotnetInstalled, getDirectoryPath, processProgress, processing } from "../lib/store";
    import {
        ensureLatestOpenAcaiLoader,
        getInstalledLoaderVersion,
        verifyInstalledLoader,
        type LoaderIntegrityReport
    } from "../lib/openAcaiLoaderUpdater";
    import openAcaiMark from "/openacai-mark.svg";
    import * as dialog from "@tauri-apps/plugin-dialog"
    import * as shell from "@tauri-apps/plugin-shell"
    import { Command } from "@tauri-apps/plugin-shell";

    let features = [melonFeature, openAcaiLoaderFeature];
    let loaderReport: LoaderIntegrityReport | null = null;
    let loaderStatus = "Not checked";
    let loaderStatusClass = "manual";
    let checkedLoader = false;

    $: if ($isPathValid && !checkedLoader) {
        checkedLoader = true;
        refreshLoaderStatus(false);
    }

    async function openFolder() {
        await shell.open(await getDirectoryPath());
    }

    async function startGame() {
        const installedVersion = await getInstalledLoaderVersion();
        if (installedVersion) {
            processing.set(true);
            processProgress.set(0);

            try {
                loaderReport = await ensureLatestOpenAcaiLoader();
                updateLoaderStatus(loaderReport);
            } catch (error) {
                await dialog.message(`OpenACAI Endnight Loader could not be verified or updated:\n${error}`, {
                    title: "Loader update failed",
                    kind: "error"
                });
                processing.set(false);
                return;
            }

            processing.set(false);
        }

        await launchGame();
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
        if (showProgress) {
            processing.set(true);
            processProgress.set(0);
        }

        try {
            loaderReport = await verifyInstalledLoader();
            updateLoaderStatus(loaderReport);
        } catch (error) {
            loaderStatus = `Loader update manifest unavailable: ${error}`;
            loaderStatusClass = "manual";
        } finally {
            if (showProgress) {
                processing.set(false);
            }
        }
    }

    async function repairLoader() {
        processing.set(true);
        processProgress.set(0);

        try {
            loaderReport = await ensureLatestOpenAcaiLoader();
            updateLoaderStatus(loaderReport);
            await dialog.message("OpenACAI Endnight Loader files are verified and current.", {
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

<div class="column main-page">
    <section class="brand-panel">
        <a class="brand-link" href="https://github.com/desertofunknown/openacai-loader" target="_blank" rel="noreferrer">
            <img class="brand-mark" src={openAcaiMark} alt="OpenACAI" />
            <span class="brand-copy">
                <span class="brand-title">OpenACAI Endnight Loader</span>
                <span class="brand-subtitle">OpenACAI Mod Manager</span>
            </span>
        </a>
    </section>
    <PathSelector />
    {#if $isPathValid}
        {#each features as feature}
            <InstallFeature feature={feature} />
        {/each}
        <section class="loader-health">
            <span class="description-content">OpenACAI Endnight Loader integrity</span>
            <span class="health-status {loaderStatusClass}">{loaderStatus}</span>
            {#if loaderDetail(loaderReport)}
                <span class="health-detail">{loaderDetail(loaderReport)}</span>
            {/if}
            <div class="health-actions">
                <button class="tool-button" on:click={() => refreshLoaderStatus(true)}>Check</button>
                <button class="tool-button install" on:click={repairLoader}>{loaderReport?.needsUpdate ? "Update / Repair" : "Verify / Repair"}</button>
            </div>
        </section>
    {/if}
    {#if $isPathValid}
        <div class="main-actions">
            <button class="tool-button" on:click={openFolder}>Open Game Folder</button>
            <button class="tool-button" on:click={startGame}>Start Game</button>
        </div>
    {/if}
    {#if !$isDotnetInstalled}
        <span class="runtime-note"><a href="https://dotnet.microsoft.com/en-us/download/dotnet/11.0" target="_blank">.NET 11</a> is recommended for current OpenACAI loader builds.</span>
    {/if}
</div>

<style>
    .main-page {
        gap: clamp(0.55em, 1.5vh, 0.95em);
        height: 100%;
        justify-content: flex-start;
        min-height: 0;
    }

    .brand-panel {
        display: flex;
        justify-content: center;
        margin: 0 auto clamp(0.35em, 1.2vh, 0.9em) auto;
        width: min(100%, 520px);
    }

    .brand-link {
        align-items: center;
        background: rgba(42, 42, 42, 0.74);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 2px;
        display: flex;
        gap: 0.9em;
        justify-content: center;
        padding: 0.75em 1em;
        width: 100%;
    }

    .brand-mark {
        display: block;
        height: clamp(58px, 8.2vh, 78px);
        width: clamp(58px, 8.2vh, 78px);
        object-fit: contain;
    }

    .brand-copy {
        display: flex;
        flex-direction: column;
        line-height: 1.1;
        text-align: left;
    }

    .brand-title {
        color: #f4f4f4;
        font-size: 1.48em;
        font-weight: 700;
        text-transform: uppercase;
    }

    .brand-subtitle {
        color: #b8b8b8;
        font-size: 0.82em;
        font-weight: 600;
        margin-top: 0.4em;
    }

    .tool-button {
        background-color: #1e1e1e;
        color: #c9c9c9;
    }

    .loader-health {
        background: rgba(42, 42, 42, 0.78);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 2px;
        display: flex;
        flex-direction: column;
        gap: 0.5em;
        margin: 0.25em auto;
        max-width: 520px;
        padding: clamp(0.6em, 1.4vh, 0.9em);
        text-align: left;
        width: min(100%, 520px);
    }

    .description-content {
        color: #a2a2a2;
        font-size: 0.9em;
        font-weight: 700;
    }

    .health-status {
        color: #b9c7d2;
        font-size: 0.9em;
        line-height: 1.35;
    }

    .health-detail {
        color: #c7c7c7;
        font-size: 0.8em;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .health-actions {
        display: flex;
        gap: 0.5em;
    }

    .health-actions > button {
        flex: 1;
        margin: 0;
    }

    .runtime-note {
        color: #a2a2a2;
        display: block;
        font-size: 0.86em;
        font-weight: 500;
        line-height: 1.3;
    }

    .main-actions {
        display: flex;
        gap: 0.6em;
        justify-content: center;
        margin: 0 auto;
        width: min(100%, 520px);
    }

    .main-actions > button {
        flex: 1;
        margin: 0;
    }

    @media (max-width: 700px) {
        .brand-link,
        .health-actions,
        .main-actions {
            flex-direction: column;
        }

        .brand-copy {
            text-align: center;
        }
    }

    @media (max-height: 780px) {
        .main-page {
            gap: 0.45em;
        }

        .brand-panel {
            margin-bottom: 0.25em;
        }

        .brand-link {
            padding: 0.55em 0.8em;
        }

        .brand-title {
            font-size: 1.25em;
        }

        .brand-subtitle,
        .health-status {
            font-size: 0.78em;
        }

        .health-detail {
            display: none;
        }

        .loader-health {
            gap: 0.35em;
            margin: 0.1em auto;
            padding: 0.55em 0.7em;
        }
    }

    @media (max-height: 720px) {
        .brand-mark {
            height: 44px;
            width: 44px;
        }

        .brand-title {
            font-size: 1.08em;
        }

        .description-content,
        .health-status,
        .runtime-note {
            font-size: 0.74em;
        }

        .health-actions,
        .main-actions {
            gap: 0.4em;
        }
    }
</style>
