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

<section id="path-selector" aria-labelledby="installation-label">
    <div class="selector-heading">
        <label id="installation-label" for="installation-path">Installation</label>
        <span class="path-caption">Game executable</span>
    </div>
    <div class="path-row">
        <input id="installation-path" class="path-input" type="text" bind:value={$gameExePath} title={$gameExePath} placeholder="Select SonsOfTheForest.exe" readonly />
        <button class="select-btn" on:click={selectPath} aria-label="Browse for SonsOfTheForest.exe" title="Select SonsOfTheForest.exe">
            <LucideFolderOpen aria-hidden="true" />
            Browse
        </button>
    </div>
</section>

<style>
    #path-selector {
        display: flex;
        flex-direction: column;
        gap: 12px;
        width: 100%;
        min-width: 0;
        margin: 0;
        padding: 18px 20px;
        border: 1px solid var(--border-subtle, #30353f);
        border-radius: 10px;
        background: var(--surface-panel, #171a21);
        box-sizing: border-box;
        text-align: left;
    }

    .selector-heading { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
    label { color: var(--text-primary, #eef0f4); font-size: 12px; font-weight: 600; line-height: 1.4; }
    .path-caption { color: var(--text-secondary, #9da6b6); font-size: 11px; line-height: 1.4; }
    .path-row { display: flex; align-items: center; gap: 10px; min-width: 0; }
    .path-input {
        flex: 1;
        min-width: 0;
        height: 40px;
        box-sizing: border-box;
        margin: 0;
        border: 1px solid #3a424f;
        border-radius: 7px;
        padding: 10px 12px;
        color: var(--text-secondary, #9da6b6);
        background: var(--surface-base, #101217);
        font-family: "Cascadia Code", Consolas, monospace;
        font-size: 11px;
        line-height: 1.6;
        text-overflow: ellipsis;
        text-align: left;
    }
    .select-btn { display: inline-flex; align-items: center; justify-content: center; gap: 8px; flex-shrink: 0; height: 40px; margin: 0; padding: 9px 15px; border: 1px solid var(--border-subtle, #30353f); border-radius: 7px; color: var(--text-primary, #eef0f4); background: var(--surface-raised, #20242c); font-size: 12px; font-weight: 500; line-height: 1.35; }
    .select-btn :global(svg) { width: 15px; height: 15px; }
    .select-btn:hover { border-color: #515967; background: #282e38; }
    button:focus-visible, input:focus-visible { outline: 2px solid var(--accent, #eeb66b); outline-offset: 3px; }

    @media (max-width: 850px) {
        #path-selector { padding: 16px; }
    }
</style>
