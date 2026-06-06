<script lang="ts">
    import { onMount } from "svelte";
    import SOTFMods from "./Mods.svelte";
    import NexusVortex from "./NexusVortex.svelte";
    import LucideCloudDownload from "~icons/lucide/cloud-download";
    import LucideSearch from "~icons/lucide/search";
    import LucideStore from "~icons/lucide/store";
    import LucideX from "~icons/lucide/x";

    type ModHubSource = "sotf" | "nexus";

    const MOD_HUB_SOURCE_KEY = "openacai-mod-hub-source";
    const MOD_HUB_SEARCH_KEY = "openacai-mod-hub-search";

    let activeSource: ModHubSource = "sotf";
    let sharedSearchTerm = "";
    let sharedSearchVersion = 0;

    onMount(() => {
        const savedSource = localStorage.getItem(MOD_HUB_SOURCE_KEY);
        if (savedSource === "sotf" || savedSource === "nexus") {
            activeSource = savedSource;
        }

        const savedSearch = localStorage.getItem(MOD_HUB_SEARCH_KEY) ?? "";
        if (savedSearch) {
            sharedSearchTerm = savedSearch;
            sharedSearchVersion += 1;
        }
    });

    function selectSource(source: ModHubSource) {
        activeSource = source;
        localStorage.setItem(MOD_HUB_SOURCE_KEY, source);
    }

    function setSharedSearchTerm(value: string) {
        sharedSearchTerm = value;
        sharedSearchVersion += 1;
        localStorage.setItem(MOD_HUB_SEARCH_KEY, value);
    }

    function handleSharedSearchInput(event: Event) {
        setSharedSearchTerm((event.currentTarget as HTMLInputElement).value);
    }

    function clearSharedSearch() {
        setSharedSearchTerm("");
    }
</script>

<div class="mod-hub">
    <div class="hub-toolbar">
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

        <label class="hub-search">
            <LucideSearch aria-hidden="true" />
            <input
                aria-label="Search mods"
                placeholder="Search mods"
                type="text"
                value={sharedSearchTerm}
                on:input={handleSharedSearchInput}
            />
            <button
                type="button"
                aria-label="Clear search"
                disabled={!sharedSearchTerm}
                on:click={clearSharedSearch}
                title="Clear search"
            >
                <LucideX aria-hidden="true" />
            </button>
        </label>
    </div>

    <div class="source-panel">
        {#if activeSource === "sotf"}
            <SOTFMods
                sharedSearchTerm={sharedSearchTerm}
                sharedSearchVersion={sharedSearchVersion}
                showEmbeddedSearch={false}
                on:searchChange={(event) => setSharedSearchTerm(event.detail)}
            />
        {:else}
            <NexusVortex
                sharedSearchTerm={sharedSearchTerm}
                sharedSearchVersion={sharedSearchVersion}
                showEmbeddedSearch={false}
                on:searchChange={(event) => setSharedSearchTerm(event.detail)}
            />
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

    .hub-toolbar {
        align-items: stretch;
        display: grid;
        flex: 0 0 auto;
        gap: 0.5em;
        grid-template-columns: minmax(18em, 0.92fr) minmax(14em, 1fr);
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

    .hub-search {
        align-items: center;
        background: rgba(12, 12, 12, 0.82);
        border: 1px solid rgba(255, 255, 255, 0.13);
        box-sizing: border-box;
        display: grid;
        gap: 0.45em;
        grid-template-columns: auto minmax(0, 1fr) auto;
        min-width: 0;
        padding: 0 0.42em 0 0.7em;
    }

    .hub-search :global(svg) {
        color: #8d99a5;
        display: block;
        font-size: 1.05em;
        stroke-width: 2.35;
    }

    .hub-search input {
        background: transparent;
        border: 0;
        color: #e8eef3;
        font: inherit;
        font-size: 0.92em;
        font-weight: 800;
        height: 100%;
        min-height: clamp(2.25em, 4.8vh, 2.75em);
        min-width: 0;
        outline: none;
        padding: 0;
        width: 100%;
    }

    .hub-search input::placeholder {
        color: #77818b;
        opacity: 1;
        text-transform: uppercase;
    }

    .hub-search button {
        align-items: center;
        background: transparent;
        border: 0;
        box-shadow: none;
        color: #aeb6bb;
        display: grid;
        height: 2em;
        justify-content: center;
        margin: 0;
        min-height: 0;
        min-width: 2em;
        padding: 0;
        width: 2em;
        -webkit-mask-image: none;
        mask-image: none;
    }

    .hub-search button:not(:disabled):hover {
        color: #62f09b;
    }

    .hub-search button:disabled {
        opacity: 0.28;
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
        .hub-toolbar {
            grid-template-columns: 1fr;
        }

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

        .source-switch button,
        .hub-search input {
            height: 2.05em;
            min-height: 2.05em;
        }
    }
</style>
