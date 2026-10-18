<script lang="ts">
    import { onMount } from "svelte";
    import { Command } from "@tauri-apps/plugin-shell";
    import { get } from "svelte/store";
    import { gameExePath } from "../lib/store";
    import * as path from "@tauri-apps/api/path";
    import { processing, processName, processProgress } from "../lib/store";
    import FeatureList from "../lib/FeatureList.svelte";
    import { ueFeature } from "../lib/featureInstaller";
    import * as dialog from "@tauri-apps/plugin-dialog"

    let isTemplateInstalled = false;
    let isCheckingTemplate = false;
    let templateStatus = "Modstoke SDK templates are under construction.";
    let modName = "MyMod";
    let appendComments = true;
    const templateTemporarilyDisabled = true;

    async function checkForTemplate() {
        isCheckingTemplate = true;
        templateStatus = "Checking for the Sons mod template...";

        try {
            let cmd = Command.create("dotnet-template-check", ["new", "sotfmod", "-h"]);
            let result = await cmd.execute();
            isTemplateInstalled = result.code === 0 && !result.stdout.includes("No templates or subcommands found");
            templateStatus = isTemplateInstalled ? "Template ready." : "Template not installed.";
        } catch (err) {
            console.log(err);
            isTemplateInstalled = false;
            templateStatus = "Template check failed. You can install the template below.";
        } finally {
            isCheckingTemplate = false;
        }
    }

    async function installTemplate() {
        try {
            processing.set(true);
            processName.set("Installing template...");
            processProgress.set(15);

            let cmd = Command.create("dotnet-install-template", ["new", "--install", "RedLoader.Templates"]);
            await cmd.execute();
            processProgress.set(85);
        } catch (err) {
            console.log(err);
            await dialog.message(`${err}`, {
                title: "Template install failed",
                kind: "error"
            });
        } finally {
            processing.set(false);
            processName.set("");
            processProgress.set(0);
        }

        await checkForTemplate();
    }

    async function createProject() {
        try {
            let targetLocation = "";
            let gameDir = await path.dirname(get(gameExePath));

            const diares = await dialog.open({
                directory: true,
                multiple: false,
            });

            if (typeof diares === "string" && diares.length > 0) {
                const dir = diares as string;
                targetLocation = await path.join(dir, modName);
            }

            if (!targetLocation) {
                return;
            }

            processing.set(true);
            processName.set("Creating project...");
            processProgress.set(20);

            console.log(modName);
            console.log(targetLocation);
            console.log(gameDir);

            let argList = ["new", "sotfmod", "-n", modName, "-g", gameDir, "-o", targetLocation];
            if (!appendComments) {
                argList.push("-c", "false");
            }

            console.log(argList);
            console.log(`Checked ${appendComments}`);
    
            let cmd = Command.create("dotnet-create-project", argList);
            let result = await cmd.execute();
            processProgress.set(80);
            console.log(result.stdout);
            console.log(result.stderr);

            cmd = Command.create("open-explorer", [targetLocation]);
            await cmd.execute();
            processProgress.set(100);
        } catch (err) {
            console.log(err);
            await dialog.message(`${err}`, {
                title: "Project creation failed",
                kind: "error"
            });
        } finally {
            processing.set(false);
            processName.set("");
            processProgress.set(0);
        }
    }

    onMount(async () => {
        if (!templateTemporarilyDisabled) {
            await checkForTemplate();
        }
    });
</script>

<div class="modding-page">
    <header class="page-heading">
        <span class="page-eyebrow">DEVELOPMENT</span>
        <h1>Creator tools</h1>
        <p>Build and explore your next mod for Sons of the Forest.</p>
    </header>

    <div class="tools-grid">
        <section class="tool-panel sdk-panel" aria-labelledby="sdk-heading">
            <div class="tool-heading">
                <span class="tool-icon sdk-icon" aria-hidden="true">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
                        <path d="m8 7-5 5 5 5m8-10 5 5-5 5m-3-14-2 18" />
                    </svg>
                </span>
                <div>
                    <span class="tool-category">PROJECT SETUP</span>
                    <h2 id="sdk-heading">Modstoke SDK</h2>
                </div>
            </div>

            {#if templateTemporarilyDisabled}
                <div class="sdk-state">
                    <span class="state-badge"><span aria-hidden="true"></span>In development</span>
                    <h3>Templates under construction</h3>
                    <p>Project templates are being rebuilt for Modstoke Loader. Project creation is currently unavailable.</p>
                </div>
                <div class="panel-footer">
                    <button class="project-button" disabled>
                        <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
                            <rect x="5" y="10" width="14" height="11" rx="2" />
                            <path d="M8 10V7a4 4 0 0 1 8 0v3m-4 5v2" />
                        </svg>
                        Create project
                    </button>
                </div>
            {:else if isCheckingTemplate}
                <div class="template-status" role="status">{templateStatus}</div>
            {:else if !isTemplateInstalled}
                <div class="template-content">
                    <p>Install the .NET template to create a new mod project.</p>
                    <button class="project-button" on:click={installTemplate}>Install template</button>
                </div>
            {:else}
                <div class="template-content">
                    <p>Create a project using the game installation selected in Overview.</p>
                    <label class="field-label" for="mod-project-name">Project name</label>
                    <input class="project-name" id="mod-project-name" type="text" bind:value={modName} />
                    <div class="comment-option">
                        <input type="checkbox" id="append-comments" bind:checked={appendComments}>
                        <div>
                            <label for="append-comments">Include code comments</label>
                            <p>Helpful explanations in the generated code. Recommended for your first mod.</p>
                        </div>
                    </div>
                    <button class="project-button" on:click={createProject}>Create project</button>
                </div>
            {/if}
        </section>

        <section class="tool-panel inspection-panel" aria-labelledby="inspection-heading">
            <div class="tool-heading">
                <span class="tool-icon" aria-hidden="true">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
                        <path d="m12 3 9 5-9 5-9-5 9-5Zm-9 9 9 5 9-5M3 16l9 5 9-5" />
                    </svg>
                </span>
                <div>
                    <span class="tool-category">IN-GAME TOOLS</span>
                    <h2 id="inspection-heading">Runtime inspection</h2>
                </div>
            </div>
            <div class="tool-feature">
                <FeatureList features={[ueFeature]} />
            </div>
        </section>
    </div>

    <p class="workspace-note">
        <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="9" />
            <path d="M12 11v5m0-9v.01" />
        </svg>
        Tools use the game installation selected in Overview.
    </p>
</div>

<style>
    .modding-page {
        display: flex;
        flex-direction: column;
        gap: 28px;
        min-height: 0;
        width: 100%;
        text-align: left;
    }

    .page-eyebrow {
        color: var(--accent, #eeb66b);
        font-size: 10px;
        font-weight: 700;
        letter-spacing: 0.16em;
    }

    .page-heading h1 {
        margin: 8px 0 9px;
        color: var(--text-primary, #eef0f4);
        font-size: clamp(27px, 2.7vw, 32px);
        font-weight: 650;
        line-height: 1.15;
        letter-spacing: -0.035em;
        text-align: left;
    }

    .page-heading p {
        margin: 0;
        color: var(--text-secondary, #9da6b6);
        font-size: 13px;
        line-height: 1.6;
    }

    .tools-grid {
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        align-items: start;
        gap: 20px;
    }

    .tool-panel {
        min-width: 0;
        overflow: hidden;
        border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.08));
        border-radius: 14px;
        background: var(--surface-panel, #171a21);
    }

    .sdk-panel {
        background:
            radial-gradient(ellipse at 100% 0, rgba(238, 182, 107, 0.045), transparent 65%),
            var(--surface-panel, #171a21);
    }

    .tool-heading {
        display: flex;
        align-items: center;
        gap: 14px;
        padding: 24px 24px 22px;
        border-bottom: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.08));
    }

    .tool-icon {
        display: grid;
        flex: 0 0 42px;
        width: 42px;
        height: 42px;
        place-items: center;
        border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.08));
        border-radius: 11px;
        color: #b9c4d6;
        background: var(--surface-raised, #20242c);
    }

    .tool-icon svg { width: 23px; height: 23px; }
    .sdk-icon { color: var(--accent, #eeb66b); background: var(--accent-soft, rgba(238, 182, 107, 0.12)); border-color: rgba(238, 182, 107, 0.13); }
    .tool-category { display: block; margin-bottom: 3px; color: var(--text-secondary, #9da6b6); font-size: 9px; font-weight: 700; line-height: 1.5; letter-spacing: 0.12em; }
    .tool-heading h2 { margin: 0; color: var(--text-primary, #eef0f4); font-size: 18px; font-weight: 600; line-height: 1.4; letter-spacing: -0.02em; }

    .sdk-state,
    .template-content { padding: 24px; }

    .state-badge {
        display: inline-flex;
        align-items: center;
        gap: 7px;
        padding: 4px 9px;
        border: 1px solid rgba(238, 182, 107, 0.14);
        border-radius: 5px;
        color: var(--accent, #eeb66b);
        background: rgba(238, 182, 107, 0.065);
        font-size: 10px;
        font-weight: 600;
        line-height: 1.5;
    }

    .state-badge span { width: 5px; height: 5px; border-radius: 50%; background: currentColor; }
    .sdk-state h3 { margin: 17px 0 8px; color: var(--text-primary, #eef0f4); font-size: 15px; font-weight: 550; line-height: 1.5; }
    .sdk-state p,
    .template-content > p { margin: 0; color: var(--text-secondary, #9da6b6); font-size: 12px; line-height: 1.8; }
    .template-content > p { margin-bottom: 21px; }
    .panel-footer { padding: 0 24px 24px; }

    .project-button {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        gap: 8px;
        min-height: 38px;
        margin: 0;
        padding: 8px 16px;
        border: 1px solid rgba(238, 182, 107, 0.22);
        border-radius: 7px;
        background: var(--accent-soft, rgba(238, 182, 107, 0.12));
        color: var(--accent, #eeb66b);
        font-size: 12px;
        font-weight: 600;
        line-height: 1.5;
    }

    .project-button svg { width: 14px; height: 14px; }
    .project-button:enabled:hover { border-color: rgba(238, 182, 107, 0.45); background: rgba(238, 182, 107, 0.2); }
    .project-button:disabled { color: #818997; background: rgba(255, 255, 255, 0.03); border-color: var(--border-subtle, rgba(255, 255, 255, 0.08)); box-shadow: none; cursor: not-allowed; opacity: 1; }

    .template-status { padding: 26px 24px; color: var(--text-secondary, #9da6b6); font-size: 13px; line-height: 1.7; }
    .field-label { display: block; margin-bottom: 8px; color: var(--text-primary, #eef0f4); font-size: 12px; font-weight: 500; }
    .project-name { box-sizing: border-box; width: 100%; margin: 0 0 20px; padding: 10px 12px; border-radius: 7px; border-color: var(--border-subtle, rgba(255, 255, 255, 0.08)); color: var(--text-primary, #eef0f4); background: var(--surface-base, #101217); font-size: 13px; text-align: left; }
    .comment-option { display: flex; align-items: flex-start; gap: 10px; margin-bottom: 22px; }
    .comment-option input { flex: 0 0 17px; float: none; width: 17px; height: 17px; margin: 2px 0 0; }
    .comment-option label { color: var(--text-primary, #eef0f4); font-size: 12px; font-weight: 500; line-height: 1.6; }
    .comment-option p { margin: 5px 0 0; color: var(--text-secondary, #9da6b6); font-size: 11px; line-height: 1.7; }

    .tool-feature { padding: 24px; }
    .tool-feature :global(.column) { width: 100%; text-align: left; }
    .workspace-note { display: flex; align-items: flex-start; gap: 9px; margin: -5px 0 0; color: #7f8999; font-size: 11px; line-height: 1.7; }
    .workspace-note svg { flex: 0 0 15px; width: 15px; height: 15px; margin-top: 2px; }

    @media (max-width: 1100px) {
        .modding-page { gap: 24px; }
        .tools-grid { gap: 16px; }
        .tool-heading { gap: 11px; padding: 20px 18px; }
        .sdk-state,
        .template-content,
        .tool-feature { padding: 20px 18px; }
        .panel-footer { padding: 0 18px 20px; }
        .tool-heading h2 { font-size: 16px; }
    }

    @media (max-width: 760px) {
        .tools-grid { grid-template-columns: minmax(0, 1fr); }
    }
</style>
