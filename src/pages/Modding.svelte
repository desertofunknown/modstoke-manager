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
    let templateStatus = "OpenACAI SDK templates are under construction.";
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

<div class="column modding-page">
    <b class="desc">
        Various tools for modders
    </b>
    {#if templateTemporarilyDisabled}
        <div class="description sdk-disabled">
            <span>
                OpenACAI SDK project templates are being rebuilt for the new OpenACAI Endnight Loader port.
            </span>
            <button class="generic-button" disabled>Template under construction</button>
        </div>
    {:else if isCheckingTemplate}
        <div class="template-status">{templateStatus}</div>
    {:else if !isTemplateInstalled}
        <div class="description">
            <span>
                Install the template to create new mod projects in dotnet
            </span>
            <button class="generic-button" on:click={installTemplate}>Install Template</button>
        </div>
    {:else}
        <div class="description">
            <span>
                Create a new mod project using the game path set in the "Main" tab
            </span>
            <input class="generic-input" type="text" bind:value={modName} />
            <div class="form-checkbox">
                <input type="checkbox" id="check" bind:checked={appendComments}>
                <label for="check">
                  Append Comments
                </label>
                <p class="note">
                  Append documenting comments to the generated code. Recommended if it's your first mod.
                </p>
            </div>
            <button class="generic-button" on:click={createProject}>Create Project</button>
        </div>
    {/if}
    <br />
    <FeatureList features={[ueFeature]} />
</div>

<style>
    .modding-page {
        height: 100%;
        justify-content: flex-start;
        min-height: 0;
    }

    .desc {
        margin-bottom: 1em;
        display: block;
        text-align: center;
        color: #a2a2a2;
    }

    .generic-button {
        color: #659cf0;
    }

    .generic-button:disabled {
        color: #a2a2a2;
        cursor: not-allowed;
        opacity: 0.72;
    }

    .description {
        background: rgba(42, 42, 42, 0.78);
        border: 1px solid rgba(255, 255, 255, 0.14);
        margin: 0 auto;
        padding: 1em;
        width: min(100%, 600px);
    }
    
    .description > * {
        width: 100%;
    }

    .description > span {
        margin-bottom: 1em;
        display: block;
        text-align: center;
        font-size: 0.9em;
        color: #a2a2a2;
    }

    .sdk-disabled {
        border-color: rgba(253, 198, 109, 0.28);
    }

    .template-status {
        background: rgba(44, 44, 44, 0.88);
        color: #cfcfcf;
        font-weight: 800;
        margin: 0 auto;
        padding: 0.9em 1em;
        text-transform: uppercase;
        width: min(100%, 600px);
    }
</style>
