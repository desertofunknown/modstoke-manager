<script lang="ts">
    import { onMount } from "svelte";
    import { isPathValid } from "../lib/store";
    import ModCard from "../lib/ModCard.svelte";
    import type { Mod, ModCategory } from "../lib/mods";
    import { ModDatabase, Sorting } from "../lib/mods";
    import InfiniteScroll from "../lib/InfiniteScroll.svelte";
    import { debounce } from "lodash";
    import SvgSpinnersBlocksWave from '~icons/svg-spinners/blocks-wave'

    let filtered: Mod[] = [];
    let filterTerm: string = "";
    let selectedCategory = "all";
    let selectedType = "all";
    let selectedCompatibility = "all";
    let categories: ModCategory[] = [];

    let onlineSelected = true;
    let installedSelected = false;

    let isGrid = false;

    let page = 1;
	let newBatch: Mod[] = [];
    let isLoading: boolean = false;
    let hasLoadedOnce = false;
    let catalogError: string = "";
    let installedInventoryWarning = "";

    $: visibleMods = filtered.filter(matchesClientFilters);

    async function fetchData() {
        //processing.set(true);
        //processProgress.set(0);
        //processName.set("Loading mods...");
        isLoading = true;
        catalogError = "";
        try {
            let res = await ModDatabase.fetchMods(page, Sorting.newest, true, false, filterTerm, selectedCategory, selectedType);
            let mods = res.data;
            await ModDatabase.initModList(mods);
		    newBatch = mods;
            filtered = [...filtered, ...newBatch];
            hasLoadedOnce = true;
        } catch (error) {
            newBatch = [];
            catalogError = `Failed to load the mod catalog: ${error}`;
        } finally {
            isLoading = false;
        }
        //processing.set(false);
	};

    // $: filtered = [
	// 	...filtered,
    //     ...newBatch
    // ];

    onMount(async () => {
        //processing.set(true);
        //processProgress.set(0);
        //processName.set("Loading mods...");
        //await ModDatabase.refreshAll(false);

        //await filter();
        //processing.set(false);
        
        await loadCategories();
        await fetchData();

        try {
            await ModDatabase.initDatabase();
            ModDatabase.initModList(filtered);
            filtered = [...filtered];
            installedInventoryWarning = "";
        } catch (error) {
            installedInventoryWarning = `Installed mod scan failed: ${error}`;
            console.log(installedInventoryWarning);
        }
        

        isGrid = window.innerWidth > 1000;
    });

    // async function filter() {
    //     let modBucket: Mod[] = [];
    //     if(filterTerm.startsWith("unapproved:"))
    //     {
    //         modBucket = await ModDatabase.getUnapprovedMods();
    //     }
    //     else if(filterTerm.startsWith("nsfw:"))
    //     {
    //         modBucket = await ModDatabase.getNsfwMods();
    //     }
    //     else
    //     {
    //         modBucket = await ModDatabase.getMods();
    //     }

    //     let split =  filterTerm.split(":");
    //     let term = split[split.length - 1];

    //     filtered = modBucket.filter((mod) => {
    //         let passesTerm = mod.name.toLowerCase().includes(term.toLowerCase());
    //         let passesScope = (onlineSelected && !mod.isInstalled) || (installedSelected && mod.isInstalled);
    //         return passesTerm && passesScope;
    //     });
    // }

    const handleSearchInput = debounce(async e => {
        filterTerm = e.target.value;
        await reloadOnline();
    }, 600);

    async function loadCategories() {
        try {
            categories = await ModDatabase.fetchCategories();
        } catch (error) {
            console.log("failed to fetch SOTF categories", error);
        }
    }

    async function reloadOnline() {
        onlineSelected = true;
        installedSelected = false;
        page = 1;
        filtered = [];
        newBatch = [];
        hasLoadedOnce = false;
        await fetchData();
    }

    async function toggleOnline() {
        // onlineSelected = !onlineSelected;

        await reloadOnline();
        //await filter();
    }

    async function toggleInstalled() {
        // installedSelected = !installedSelected;

        onlineSelected = false;
        installedSelected = true;
        isLoading = true;
        filtered = [];
        try {
            filtered = await ModDatabase.getInstalledMods();
            catalogError = "";
        } catch (error) {
            catalogError = `Failed to scan installed mods: ${error}`;
        } finally {
            isLoading = false;
        }
        // page = 1;
        // filtered = [];
        // await fetchData();
        //await filter();
    }

    async function refreshMods() {
        try {
            await ModDatabase.loadInstalledMods();
            installedInventoryWarning = "";
        } catch (error) {
            installedInventoryWarning = `Installed mod scan failed: ${error}`;
            console.log(installedInventoryWarning);
        }
        
        if (onlineSelected) {
            await toggleOnline();
            return;
        }

        await toggleInstalled();
    }

    function matchesClientFilters(mod: Mod): boolean {
        if (selectedCompatibility === "mp-compatible" && !mod.isMultiplayerCompatible) {
            return false;
        }

        if (selectedCompatibility === "all-players" && !mod.requiresAllPlayers) {
            return false;
        }

        if (selectedCompatibility === "client" && mod.modSide !== "client") {
            return false;
        }

        return true;
    }

</script>

<svelte:window on:resize={() => isGrid = window.innerWidth > 1000} />
<div class="column mods-page">
    {#if $isPathValid}
        <div class="row-center">
            <input class="generic-input search-input" placeholder="Search" type="text" on:input={handleSearchInput} />
            <button class="btn-left cat-btn" class:cat-btn-selected={onlineSelected} on:click={toggleOnline}>Online</button>
            <button class="btn-right cat-btn" class:cat-btn-selected={installedSelected} on:click={toggleInstalled}>Installed</button>
        </div>

        <div class="filter-row">
            <label>
                <span>Category</span>
                <select bind:value={selectedCategory} on:change={reloadOnline}>
                    <option value="all">All categories</option>
                    {#each categories as category}
                        <option value={category.slug}>{category.name}</option>
                    {/each}
                </select>
            </label>
            <label>
                <span>Type</span>
                <select bind:value={selectedType} on:change={reloadOnline}>
                    <option value="all">All types</option>
                    <option value="Mod">Mods</option>
                    <option value="Library">Libraries</option>
                    <option value="Build">Builds</option>
                </select>
            </label>
            <label>
                <span>Compatibility</span>
                <select bind:value={selectedCompatibility}>
                    <option value="all">All compatibility</option>
                    <option value="mp-compatible">Multiplayer compatible</option>
                    <option value="all-players">Requires all players</option>
                    <option value="client">Client-side</option>
                </select>
            </label>
        </div>

        {#if catalogError}
            <div class="catalog-error">
                <span>{catalogError}</span>
                <button on:click={() => fetchData()}>Retry</button>
            </div>
        {/if}

        {#if installedInventoryWarning && onlineSelected}
            <div class="catalog-error subtle warning-note">{installedInventoryWarning}</div>
        {/if}

        <div class="scroller" class:grid={isGrid}>
            {#each visibleMods as mod}
                <ModCard mod={mod} isGrid={isGrid} on:refreshMods={refreshMods}/>
            {/each}

            {#if isLoading}
            <SvgSpinnersBlocksWave style="font-size: 2em; color: #38d68d; position: absolute; bottom: 40px;" />
            {/if}

            <InfiniteScroll
                hasMore={newBatch.length !== 0 && !installedSelected}
                threshold={100}
                on:loadMore={() => {page++; fetchData()}} />
        </div>

        {#if hasLoadedOnce && !isLoading && !catalogError && visibleMods.length === 0}
            <div class="catalog-error subtle">No mods are available for the current filter.</div>
        {/if}

    {:else}
        <b>Set the correct path in the main tab to start browsing mods.</b>
    {/if}
    
</div>

<style>
    .mods-page {
        height: 100%;
        justify-content: flex-start;
        min-height: 0;
    }

    .scroller {
        flex: 1 1 auto;
        height: auto;
        min-height: 0;
        overflow-y: scroll;
        overflow-x: hidden;
        padding-bottom: 1em;
        position: relative;
        width: 100%;
    }

    .grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
        grid-gap: 1em;
    }

    .search-input {
        flex: 1;
        margin-right: 0.5em;
    }

    .filter-row {
        display: grid;
        gap: 0.6em;
        grid-template-columns: repeat(3, minmax(0, 1fr));
        margin: 0 0 0.8em;
        width: 100%;
    }

    .filter-row label {
        align-items: flex-start;
        display: flex;
        flex-direction: column;
        gap: 0.2em;
        text-align: left;
    }

    .filter-row span {
        color: #a2a2a2;
        font-size: 0.74em;
        font-weight: 800;
        letter-spacing: 0.12em;
        text-transform: uppercase;
    }

    .filter-row select {
        background: rgba(18, 18, 18, 0.92);
        border: 1px solid rgba(255, 255, 255, 0.16);
        color: #e8e8e8;
        font-family: inherit;
        font-weight: 700;
        min-height: 2.6em;
        padding: 0.45em 0.7em;
        text-transform: uppercase;
        width: 100%;
    }

    .cat-btn {
        padding: 0;
        margin-top: -4px;
        height: 2.7em;
        width: 6em;
        color: #a2a2a2;
    }

    .cat-btn-selected {
        background-color: #111;
        color: #38d68d;
    }

    .catalog-error {
        align-items: center;
        background: rgba(44, 44, 44, 0.88);
        border: 1px solid rgba(255, 255, 255, 0.15);
        color: #fdc66d;
        display: flex;
        font-weight: 700;
        gap: 1em;
        justify-content: space-between;
        margin: 0.5em 0 0.8em;
        padding: 0.8em 1em;
        text-align: left;
        width: 100%;
    }

    .catalog-error button {
        flex: 0 0 auto;
        margin: 0;
        min-width: 8em;
    }

    .catalog-error.subtle {
        color: #a2a2a2;
        justify-content: center;
    }

    .warning-note {
        color: #fdc66d;
        justify-content: flex-start;
    }

    @media (max-width: 850px) {
        .filter-row {
            grid-template-columns: 1fr;
        }
    }
</style>
