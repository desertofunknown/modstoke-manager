<script lang="ts">
    import { gameExePath, isPathValid } from "./store";
    import * as dialog from "@tauri-apps/plugin-dialog"

    async function selectPath() {
        try {
            const result = await dialog.open({
                multiple: false,
                filters: [{ name: "Executable", extensions: ["exe"] }],
            });
            if (result && result.length > 0) {
                const exe = result as string;
                if (exe.endsWith("SonsOfTheForest.exe")) {
                    gameExePath.set(result as string);
                    isPathValid.set(true);
                }
            }
        } catch (error) {
            console.error("Error selecting path:", error);
        }
    }
</script>

<div id="path-selector">
    <input class="path-input" type="text" bind:value={$gameExePath} readonly />
    <button class="select-btn" on:click={selectPath}>...</button>
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
        border-radius: 0 2px 2px 0;
        min-height: clamp(2.4em, 5vh, 3.1em);
        margin: 0; /* Removes default margin from button */
        box-sizing: border-box;
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
