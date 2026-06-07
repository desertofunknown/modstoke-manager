<script lang="ts">
    import { onMount } from "svelte";
    import SOTFMods from "./Mods.svelte";
    import NexusVortex from "./NexusVortex.svelte";
    import LucideCloudDownload from "~icons/lucide/cloud-download";
    import LucideLayoutGrid from "~icons/lucide/layout-grid";
    import LucideMaximize2 from "~icons/lucide/maximize-2";
    import LucideSearch from "~icons/lucide/search";
    import LucideStore from "~icons/lucide/store";
    import LucideX from "~icons/lucide/x";

    type ModHubSource = "all" | "sotf" | "nexus";

    const MOD_HUB_SOURCE_KEY = "openacai-mod-hub-source";
    const MOD_HUB_SEARCH_KEY = "openacai-mod-hub-search";

    let activeSource: ModHubSource = "all";
    let sharedSearchTerm = "";
    let sharedSearchVersion = 0;

    onMount(() => {
        const savedSource = localStorage.getItem(MOD_HUB_SOURCE_KEY);
        if (savedSource === "all" || savedSource === "sotf" || savedSource === "nexus") {
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
                class:source-selected={activeSource === "all"}
                aria-pressed={activeSource === "all"}
                on:click={() => selectSource("all")}
                title="Browse all stores"
            >
                <LucideLayoutGrid aria-hidden="true" />
                <span>All Stores</span>
            </button>
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

    <div class="source-panel" class:all-source-panel={activeSource === "all"}>
        {#if activeSource === "sotf"}
            <SOTFMods
                sharedSearchTerm={sharedSearchTerm}
                sharedSearchVersion={sharedSearchVersion}
                showEmbeddedSearch={false}
                on:searchChange={(event) => setSharedSearchTerm(event.detail)}
            />
        {:else if activeSource === "all"}
            <section class="source-section">
                <div class="source-section-head">
                    <span><LucideStore aria-hidden="true" /> SOTF Mods</span>
                    <button type="button" on:click={() => selectSource("sotf")} title="Open SOTF Mods source" aria-label="Open SOTF Mods source">
                        <LucideMaximize2 aria-hidden="true" />
                    </button>
                </div>
                <div class="source-section-body">
                    <SOTFMods
                        sharedSearchTerm={sharedSearchTerm}
                        sharedSearchVersion={sharedSearchVersion}
                        showEmbeddedSearch={false}
                        on:searchChange={(event) => setSharedSearchTerm(event.detail)}
                    />
                </div>
            </section>

            <section class="source-section">
                <div class="source-section-head">
                    <span><LucideCloudDownload aria-hidden="true" /> Nexus / Vortex</span>
                    <button type="button" on:click={() => selectSource("nexus")} title="Open Nexus / Vortex source" aria-label="Open Nexus / Vortex source">
                        <LucideMaximize2 aria-hidden="true" />
                    </button>
                </div>
                <div class="source-section-body nexus-source-section-body">
                    <NexusVortex
                        sharedSearchTerm={sharedSearchTerm}
                        sharedSearchVersion={sharedSearchVersion}
                        showEmbeddedSearch={false}
                        on:searchChange={(event) => setSharedSearchTerm(event.detail)}
                    />
                </div>
            </section>
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
        grid-template-columns: repeat(3, minmax(0, 1fr));
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

    .all-source-panel {
        gap: 0.55em;
        overflow-y: auto;
        padding-right: 0.28em;
        scrollbar-gutter: stable;
    }

    .all-source-panel .source-section {
        padding: 0.52em;
        scroll-margin-top: 0.5em;
    }

    .all-source-panel .source-section-body {
        height: clamp(23em, 43vh, 33em);
    }

    .all-source-panel .nexus-source-section-body {
        height: clamp(34em, 58vh, 45em);
    }

    .source-section {
        background: rgba(10, 10, 10, 0.34);
        border: 1px solid rgba(255, 255, 255, 0.1);
        box-sizing: border-box;
        display: flex;
        flex: 0 0 auto;
        flex-direction: column;
        gap: 0.5em;
        min-height: 0;
        padding: 0.65em;
        width: 100%;
    }

    .source-section-head {
        align-items: center;
        display: flex;
        flex: 0 0 auto;
        gap: 0.65em;
        justify-content: space-between;
        min-width: 0;
    }

    .source-section-head span {
        align-items: center;
        color: #d8e3ea;
        display: inline-flex;
        gap: 0.45em;
        font-size: 0.82em;
        font-weight: 900;
        letter-spacing: 0.08em;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        text-transform: uppercase;
        white-space: nowrap;
    }

    .source-section-head :global(svg) {
        color: #62f09b;
        flex: 0 0 auto;
        font-size: 1.02em;
        stroke-width: 2.35;
    }

    .source-section-head button {
        align-items: center;
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.12);
        box-shadow: none;
        color: #aeb6bb;
        display: grid;
        height: 2.15em;
        justify-content: center;
        margin: 0;
        min-height: 0;
        min-width: 2.35em;
        padding: 0;
        width: 2.35em;
        -webkit-mask-image: none;
        mask-image: none;
    }

    .source-section-head button:hover {
        border-color: rgba(98, 240, 155, 0.44);
        color: #62f09b;
    }

    .source-section-body {
        height: clamp(31em, 62vh, 46em);
        min-height: 0;
        overflow: hidden;
    }

    .nexus-source-section-body {
        height: clamp(44em, 80vh, 58em);
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

        .source-section {
            padding: 0.5em;
        }

        .source-section-body,
        .nexus-source-section-body {
            height: clamp(34em, 86vh, 54em);
        }

        .all-source-panel .source-section-body,
        .all-source-panel .nexus-source-section-body {
            height: clamp(32em, 82vh, 52em);
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

        .all-source-panel {
            gap: 0.42em;
        }

        .all-source-panel .source-section {
            padding: 0.44em;
        }

        .all-source-panel .source-section-body {
            height: clamp(20em, 39vh, 28em);
        }

        .all-source-panel .nexus-source-section-body {
            height: clamp(29em, 53vh, 38em);
        }
    }
</style>
