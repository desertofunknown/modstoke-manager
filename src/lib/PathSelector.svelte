<script lang="ts">
    import { gameExePath, isPathValid } from "./store";
    import * as dialog from "@tauri-apps/plugin-dialog"
    import LucideFolderOpen from "~icons/lucide/folder-open";

    async function selectPath() {
        try {
            const result = await dialog.open({
                multiple: false,
                filters: [{ name: "Executable", extensions: ["exe"] }],
            });
            const exe = Array.isArray(result) ? result[0] : result;
            if (!exe) {
                return;
            }

            if (isSonsExecutable(exe)) {
                gameExePath.set(exe);
                isPathValid.set(true);
                return;
            }

            await dialog.message("Select SonsOfTheForest.exe from the Sons Of The Forest install folder.", {
                title: "Game path",
                kind: "warning"
            });
        } catch (error) {
            console.error("Error selecting path:", error);
        }
    }

    function isSonsExecutable(exe: string) {
        return exe.split(/[\\/]/).pop()?.toLowerCase() === "sonsoftheforest.exe";
    }
</script>

<div id="path-selector">
    <input class="path-input" type="text" bind:value={$gameExePath} placeholder="Select SonsOfTheForest.exe" readonly />
    <button class="select-btn" on:click={selectPath} aria-label="Select SonsOfTheForest.exe" title="Select SonsOfTheForest.exe">
        <LucideFolderOpen aria-hidden="true" />
    </button>
</div>

<style>
    #path-selector {
        margin-bottom: clamp(0.45em, 1.4vh, 1em);
        display: flex;
        width: 100%;
        align-items: center;
    }

    .path-input {
        flex: 1; /* Takes up all available space */
        box-sizing: border-box;
        min-height: clamp(2.4em, 5vh, 3.1em);
        margin: 0; /* Removes default margin from input */
        border-radius: 2px 0 0 2px;
        width: 80%;
        color: #a2a2a2;
        text-align: center;
        /* font-family: "Roboto Mono", monospace; */
    }

    .select-btn {
        align-items: center;
        border-radius: 0 2px 2px 0;
        box-sizing: border-box;
        display: flex;
        justify-content: center;
        min-height: clamp(2.4em, 5vh, 3.1em);
        margin: 0; /* Removes default margin from button */
        padding: 0;
        width: 3.4em;
    }

    .select-btn :global(svg) {
        display: block;
        font-size: 1.1em;
        stroke-width: 2.25;
    }

    @media (max-height: 720px) {
        #path-selector {
            margin-bottom: 0.35em;
        }

        .path-input,
        .select-btn {
            min-height: 2.25em;
        }
    }
</style>
