<script lang="ts">
  import { processProgress, processing } from './store';
    import { createEventDispatcher } from 'svelte';
    import { ModDatabase, type Mod } from './mods';
    import StatusButton from './StatusButton.svelte';
    import * as dialog from "@tauri-apps/plugin-dialog"

    export let mod: Mod;
    export let isGrid: boolean = false;

    let isLibrary = false;
    let isImageLoaded = false;

    const dispatch = createEventDispatcher();

    $: isLibrary = mod.type === "Library" || mod.installedMod?.loaderType === "redloader-library";

    async function update() {
      if (!mod.installedMod) {
        return;
      }

      if (isVortexManaged()) {
        await showVortexManagedMessage("update");
        return;
      }

      await uninstall();
      await install();

      dispatch("refreshMods");
    }

    async function uninstall() {

      if (!mod.installedMod) {
        return;
      }

      if (isVortexManaged()) {
        await showVortexManagedMessage("uninstall");
        return;
      }

      processing.set(true);
      processProgress.set(0);

      try {
        await ModDatabase.uninstallMod(mod.installedMod);
        await refresh();
      } finally {
        processing.set(false);
      }

      dispatch("refreshMods");
    }

    async function install() {
      processing.set(true);
      processProgress.set(0);

      try {
        await ModDatabase.installMod(mod);
        await refresh();
      } finally {
        processing.set(false);
      }

      dispatch("refreshMods");
    }

    async function enableMod() {
      if (!mod.installedMod) {
        return;
      }

      if (isVortexManaged()) {
        await showVortexManagedMessage("enable");
        return;
      }

      await ModDatabase.toggleMod(mod.installedMod, true);
      await refresh();
    }

    async function disableMod() {
      if (!mod.installedMod) {
        return;
      }

      if (isVortexManaged()) {
        await showVortexManagedMessage("disable");
        return;
      }

      await ModDatabase.toggleMod(mod.installedMod, false);
      await refresh();
    }

    async function handleEnabledChange(event: Event) {
      const checked = (event.currentTarget as HTMLInputElement).checked;
      if (checked) {
        await enableMod();
        return;
      }

      await disableMod();
    }

    async function refresh() {
      mod = mod;
      //installedMod = ModDatabase.getInstalledMod(mod.mod_id);
      //isModInstalled = installedMod !== undefined;

      //isUpdateAvailable = false;
    }

    function formatDate(dateString: string) {
      const date = new Date(dateString);
      return date.toLocaleDateString("en-US", {
        year: "numeric",
        month: "2-digit",
        day: "2-digit",
      });
    };
    
    function onImageLoad() {
      isImageLoaded = true;
    }

    function isVortexManaged() {
      return mod.installedMod?.installSource === "vortex";
    }

    async function showVortexManagedMessage(action: string) {
      await dialog.message(
        `${mod.name} is currently managed by Vortex/Nexus. Use Vortex to ${action} it so deployment metadata stays consistent.`,
        {
          title: "Managed by Vortex",
          kind: "info"
        }
      );
    }

    function sourceLabel() {
      if (!mod.installedMod) {
        return "Online";
      }

      if (mod.installedMod.installSource === "vortex") {
        return "Vortex";
      }

      if (mod.installedMod.installSource === "native") {
        return "Native";
      }

      return "Manual";
    }

    function loaderLabel() {
      if (mod.installedMod?.loaderType === "bepinex-plugin") {
        return "BepInEx";
      }

      if (mod.installedMod?.loaderType === "redloader-library" || mod.type === "Library") {
        return "RedLoader Library";
      }

      return "RedLoader Mod";
    }

    function multiplayerLabel() {
      if (mod.requiresAllPlayers) {
        return "All players";
      }

      if (mod.isMultiplayerCompatible) {
        return "MP compatible";
      }

      if (mod.modSide === "client") {
        return "Client-side";
      }

      return "Compatibility unknown";
    }
</script>

<div class="feature-container description {isGrid?'grid-thing':''}">
  <span class="mod-title">{mod.name} (<a href={ModDatabase.getModPageUrl(mod)} target="_blank" rel="noreferrer" class="site-link">view on site</a>)</span>
  <div class="meta-row">
    <span class="source-pill source-{mod.installedMod?.installSource ?? 'online'}">{sourceLabel()}</span>
    <span class="source-pill">{loaderLabel()}</span>
    <span class="source-pill">{multiplayerLabel()}</span>
    {#if mod.installedMod?.vortexPackage}
      <span class="source-pill source-detail">{mod.installedMod.vortexPackage}</span>
    {/if}
  </div>
  <span class="description-content header-desc">{mod.shortDescription?mod.shortDescription:""}</span>
  <div class="mod-card-horizontal">
    <!-- <img class="cover-img" src="{mod.imageUrl?mod.imageUrl:"https://placehold.co/600x400/252525/FFF?text=No+Image"}" /> -->
    <div class="image-container">
      <img
        class="cover-img main-image"
        class:isImageLoaded={!isImageLoaded}
        src="https://placehold.co/600x400/252525/FFF?text=Loading"
        alt="Loading..."
      />
      
      <img
        class="cover-img main-image"
        class:isImageLoaded
        src={mod.imageUrl?mod.imageUrl:"https://placehold.co/600x400/252525/FFF?text=No+Image"}
        alt="Mod cover..."
        on:load={onImageLoad}
      />
    </div>
    <div class="vertical">
      {#if mod.isInstalled && !isLibrary && !isGrid}
        <label class="enable-control" class:vortex-disabled={isVortexManaged()}>
          <input
            type="checkbox"
            checked={!!mod.installedMod?.isEnabled}
            disabled={isVortexManaged()}
            on:change={handleEnabledChange}
          />
          <span>{mod.installedMod?.isEnabled ? "Enabled" : "Disabled"}</span>
        </label>
      {/if}
      <span class="description-content">Author: <b class="update">{mod.user.name}</b></span>
      <span class="description-content">Version: <b class="update">{mod.latestVersion}</b></span>
      <span class="description-content">Updated: <b class="update">{mod.lastReleasedAt?formatDate(mod.lastReleasedAt):"-"}</b></span>
      <span class="description-content">Category: <b class="update">{mod.category?mod.category.name:"-"}</b></span>
    </div>
  </div>

  {#if mod.isInstalled && !isLibrary && isGrid}
    <label class="enable-control grid-enable-control" class:vortex-disabled={isVortexManaged()}>
      <input
        type="checkbox"
        checked={!!mod.installedMod?.isEnabled}
        disabled={isVortexManaged()}
        on:change={handleEnabledChange}
      />
      <span>{mod.installedMod?.isEnabled ? "Enabled" : "Disabled"}</span>
    </label>
  {/if}

  <div class="bottom-container">
    <StatusButton isUpdateAvailable={mod.hasUpdate} isModInstalled={mod.isInstalled} update={update} uninstall={uninstall} install={install} />
  </div>
</div>

<style>
  .mod-card-horizontal {
    display: flex;
    flex-direction: row;
    align-items: center;
  }

  .vertical {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    margin-left: 0.7em;
  }
  
  .mod-card-horizontal > * {
    /* margin-right: 1em; */
    flex: 1;
  }

  .feature-container > * {
    width: 100%;
  }

  .feature-container {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    position: relative;
  }

  .header-desc {
    height: 3em;
  }

  .description {
    padding: 10px;
    /* border-radius: 10px; */
    /* border: 2px solid #414141; */
    /* border-bottom: 2px solid #414141; */

    border-radius: 10px;
    border-bottom: 2px solid #333;
    background-color: #121212;

    margin-bottom: 20px;
    margin-right: 0.4em;
  }

  .description-content {
    display: block;
    text-align: left;
    font-size: 0.9em;
    color: #767676;
    margin-bottom: 1em;
  }

  .description-content > b {
    font-weight: 500;
  }

  .mod-title {
    margin-top: 0.2em;
    display: block;
    font-size: 1.2em;
    font-weight: bold;
    color: #a2a2a2;
    text-align: left;
  }

  .meta-row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4em;
    margin: 0.4em 0 0.8em;
  }

  .source-pill {
    background: #202832;
    border: 1px solid rgba(252, 252, 252, 0.08);
    border-radius: 6px;
    color: #bfc8d2;
    display: inline-block;
    font-size: 0.74em;
    font-weight: 700;
    line-height: 1.2;
    max-width: 100%;
    overflow: hidden;
    padding: 0.35em 0.55em;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .source-vortex {
    color: #78d9f4;
  }

  .source-native {
    color: #38d68d;
  }

  .source-manual {
    color: #fdc66d;
  }

  .source-detail {
    flex: 1 1 12em;
    text-align: left;
  }

  .site-link {
    font-weight: 700;
    font-size: 0.65em;
    text-transform: lowercase;
  }

  .grid-toggle-button {
    position: absolute;
    bottom: 1.4em;
    left: 1em;
    bottom: 7em;
  }

  .toggle-button {
    padding: 0;
    height: 2em;
    width: 7em;
    font-size: 0.9em;
    font-weight: 400;
    background-color: rgb(25, 25, 25);
    align-self: flex-start;
  }

  .enable-control {
    align-items: center;
    background: rgba(18, 18, 18, 0.88);
    border: 1px solid rgba(255, 255, 255, 0.14);
    color: #62f09b;
    display: inline-flex;
    font-size: 0.85em;
    font-weight: 800;
    gap: 0.55em;
    margin: 0 0 0.7em;
    padding: 0.42em 0.62em;
    text-transform: uppercase;
  }

  .enable-control input[type="checkbox"] {
    appearance: none;
    background: rgba(8, 8, 8, 0.96);
    border: 1px solid rgba(255, 255, 255, 0.35);
    display: grid;
    float: none;
    height: 1.2em;
    margin: 0;
    padding: 0;
    place-content: center;
    transform: none;
    width: 1.2em;
  }

  .enable-control input[type="checkbox"]::before {
    box-shadow: inset 1em 1em #62f09b;
    content: "";
    height: 0.68em;
    transform: scale(0);
    transition: transform 120ms ease-in-out;
    width: 0.68em;
  }

  .enable-control input[type="checkbox"]:checked::before {
    transform: scale(1);
  }

  .enable-control:not(:has(input:checked)) {
    color: #fd9b9d;
  }

  .vortex-disabled {
    color: #78d9f4;
    opacity: 0.72;
  }

  .grid-enable-control {
    bottom: 7em;
    left: 1em;
    position: absolute;
  }

  .grid-thing {
    max-height: 25em;
  }

  .image-container {
    position: relative;
    width: 100%;
    /* height: 12.5em; */
    padding-bottom: 29%;
    margin-bottom: 1em;
  }
  
  .cover-img {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    border-radius: 10px;
    object-fit: cover;
    transition: opacity 0.3s ease-in-out;
  }
  
  .main-image {
    opacity: 0;
  }
  
  .main-image.isImageLoaded {
    opacity: 1;
  }
</style>
