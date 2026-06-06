<script lang="ts">
    import { onMount } from "svelte";
    import SOTFMods from "./Mods.svelte";
    import NexusVortex from "./NexusVortex.svelte";
    import LucideCloudDownload from "~icons/lucide/cloud-download";
    import LucideStore from "~icons/lucide/store";

    type ModHubSource = "sotf" | "nexus";

    const MOD_HUB_SOURCE_KEY = "openacai-mod-hub-source";

    let activeSource: ModHubSource = "sotf";

    onMount(() => {
        const savedSource = localStorage.getItem(MOD_HUB_SOURCE_KEY);
        if (savedSource === "sotf" || savedSource === "nexus") {
            activeSource = savedSource;
        }
    });

    function selectSource(source: ModHubSource) {
        activeSource = source;
        localStorage.setItem(MOD_HUB_SOURCE_KEY, source);
    }
</script>

<div class="mod-hub">
    <div class="source-switch" aria-label="Mod source">
        <button
            type="button"
            class:source-selected={activeSource === "sotf"}
            aria-pressed={activeSource === "sotf"}
            on:click={() => selectSource("sotf")}
            title="Browse SOTF Mods"
        >
            <LucideStore aria-hidden="true" />
            <span>SOTF Mods</span>
        </button>
        <button
            type="button"
            class:source-selected={activeSource === "nexus"}
            aria-pressed={activeSource === "nexus"}
            on:click={() => selectSource("nexus")}
            title="Browse Nexus and Vortex"
        >
            <LucideCloudDownload aria-hidden="true" />
            <span>Nexus / Vortex</span>
        </button>
    </div>

    <div class="source-panel">
        {#if activeSource === "sotf"}
            <SOTFMods />
        {:else}
            <NexusVortex />
        {/if}
    </div>
</div>

<style>
    .mod-hub {
        box-sizing: border-box;
        display: flex;
        flex-direction: column;
        gap: clamp(0.45em, 0.8vh, 0.7em);
        height: 100%;
        min-height: 0;
        overflow: hidden;
        width: 100%;
    }

    .source-switch {
        align-items: stretch;
        display: grid;
        flex: 0 0 auto;
        gap: 0.45em;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        width: 100%;
    }

    .source-switch button {
        align-items: center;
        color: #aeb6bb;
        display: inline-flex;
        gap: 0.48em;
        height: clamp(2.25em, 4.8vh, 2.75em);
        justify-content: center;
        margin: 0;
        min-width: 0;
        padding: 0 0.8em;
        white-space: nowrap;
        width: 100%;
    }

    .source-switch button :global(svg) {
        display: block;
        flex: 0 0 auto;
        font-size: 1.08em;
        stroke-width: 2.3;
    }

    .source-switch button span {
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    .source-switch button.source-selected {
        background:
            linear-gradient(180deg, rgba(50, 64, 58, 0.95), rgba(17, 28, 23, 0.96)),
            rgba(12, 22, 17, 0.94);
        border-color: rgba(98, 240, 155, 0.46);
        color: #62f09b;
        text-shadow: -0.55px 0 rgba(255, 64, 64, 0.58), 0.55px 0 rgba(66, 232, 255, 0.58);
    }

    .source-panel {
        display: flex;
        flex: 1 1 auto;
        flex-direction: column;
        min-height: 0;
        overflow: hidden;
        width: 100%;
    }

    @media (max-width: 620px) {
        .source-switch {
            gap: 0.35em;
        }

        .source-switch button {
            font-size: 0.82em;
            padding: 0 0.55em;
        }
    }

    @media (max-height: 720px) {
        .mod-hub {
            gap: 0.35em;
        }

        .source-switch button {
            height: 2.05em;
        }
    }
</style>
