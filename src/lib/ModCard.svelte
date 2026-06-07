<script lang="ts">
  import { processProgress, processing } from './store';
    import { createEventDispatcher } from 'svelte';
    import { ModDatabase, modPreviewUrls, type Mod } from './mods';
    import StatusButton from './StatusButton.svelte';
    import * as dialog from "@tauri-apps/plugin-dialog"
    import LucideChevronLeft from "~icons/lucide/chevron-left";
    import LucideChevronRight from "~icons/lucide/chevron-right";
    import LucideImages from "~icons/lucide/images";

    export let mod: Mod;
    export let detailActionsOnly = false;
    export let showDetailsButton = true;

    let isLibrary = false;
    let isImageLoaded = false;
    let activePreviewModKey = "";
    let currentPreviewIndex = 0;
    let previewUrls: string[] = [];
    let currentPreviewUrl = "";
    let previewCountLabel = "";

    const fallbackPreviewUrl = "https://placehold.co/600x400/252525/FFF?text=No+Image";

    const dispatch = createEventDispatcher<{ refreshMods: void; details: Mod }>();

    $: isLibrary = mod.type === "Library" || mod.installedMod?.loaderType === "redloader-library";
    $: {
      const modKey = mod.mod_id || mod.slug || mod.name;
      if (activePreviewModKey !== modKey) {
        activePreviewModKey = modKey;
        currentPreviewIndex = 0;
        isImageLoaded = false;
      }
    }
    $: previewUrls = modPreviewUrls(mod);
    $: if (currentPreviewIndex >= previewUrls.length) {
      currentPreviewIndex = 0;
    }
    $: currentPreviewUrl = previewUrls[currentPreviewIndex] ?? fallbackPreviewUrl;
    $: previewCountLabel = previewUrls.length > 0 ? `${currentPreviewIndex + 1}/${previewUrls.length}` : "Local";

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

    function cyclePreview(direction: number) {
      if (previewUrls.length <= 1) {
        return;
      }

      currentPreviewIndex = (currentPreviewIndex + direction + previewUrls.length) % previewUrls.length;
      isImageLoaded = false;
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

    function showDetails() {
      dispatch("details", mod);
    }

    function openModPage() {
      ModDatabase.openModPage(mod);
    }
</script>

{#if detailActionsOnly}
  <div class="detail-action-surface">
    {#if mod.isInstalled && !isLibrary}
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

    <StatusButton isUpdateAvailable={mod.hasUpdate} isModInstalled={mod.isInstalled} update={update} uninstall={uninstall} install={install} />
  </div>
{:else}
<div class="feature-container description">
  <div class="mod-card-row">
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
        src={currentPreviewUrl}
        alt={`Preview for ${mod.name}`}
        on:load={onImageLoad}
      />
      <div
        class="preview-count"
        class:preview-count-fallback={previewUrls.length === 0}
        aria-label={previewUrls.length > 0 ? `${mod.name} preview image ${currentPreviewIndex + 1} of ${previewUrls.length}` : `${mod.name} uses the local fallback preview image`}
        title={previewUrls.length > 0 ? `${previewUrls.length} preview ${previewUrls.length === 1 ? "image" : "images"}` : "Local fallback preview"}
      >
        <LucideImages aria-hidden="true" />
        <span>{previewCountLabel}</span>
      </div>
      {#if previewUrls.length > 1}
        <div class="preview-controls" aria-label={`Cycle preview images for ${mod.name}`}>
          <button type="button" aria-label={`Previous preview image for ${mod.name}`} title="Previous preview image" on:click={() => cyclePreview(-1)}>
            <LucideChevronLeft aria-hidden="true" />
          </button>
          <button type="button" aria-label={`Next preview image for ${mod.name}`} title="Next preview image" on:click={() => cyclePreview(1)}>
            <LucideChevronRight aria-hidden="true" />
          </button>
        </div>
      {/if}
    </div>

    <div class="mod-info">
      <div class="title-line">
        <span class="mod-title">{mod.name}</span>
      </div>
      <div class="meta-row">
        <span class="source-pill source-{mod.installedMod?.installSource ?? 'online'}">{sourceLabel()}</span>
        <span class="source-pill">{loaderLabel()}</span>
        <span class="source-pill">{multiplayerLabel()}</span>
        {#if mod.installedMod?.vortexPackage}
          <span class="source-pill source-detail">{mod.installedMod.vortexPackage}</span>
        {/if}
      </div>
      <span class="description-content header-desc">{mod.shortDescription?mod.shortDescription:""}</span>
      <div class="fact-row">
        <span>Author <b class="update">{mod.user.name}</b></span>
        <span>Version <b class="update">{mod.latestVersion}</b></span>
        <span>Updated <b class="update">{mod.lastReleasedAt?formatDate(mod.lastReleasedAt):"-"}</b></span>
        <span>Category <b class="update">{mod.category?mod.category.name:"-"}</b></span>
      </div>
    </div>

    <div class="mod-actions">
      {#if mod.isInstalled && !isLibrary}
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

      <StatusButton isUpdateAvailable={mod.hasUpdate} isModInstalled={mod.isInstalled} update={update} uninstall={uninstall} install={install} />
      {#if showDetailsButton}
        <button class="details-button" type="button" on:click={showDetails}>Details</button>
      {/if}
      <button class="open-page-button" type="button" on:click={openModPage}>Open Page</button>
    </div>
  </div>
</div>
{/if}

<style>
  .mod-card-row {
    align-items: stretch;
    display: grid;
    gap: clamp(0.5em, 0.75vw, 0.72em);
    grid-template-columns: var(--sotf-thumb-width, clamp(136px, 15vw, 210px)) minmax(0, 1fr) minmax(7.8em, 10.4em);
    min-width: 0;
    width: 100%;
  }

  .mod-info,
  .mod-actions {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .mod-info {
    gap: 0.2em;
    justify-content: center;
  }

  .mod-actions {
    align-self: stretch;
    gap: 0.28em;
    justify-content: center;
  }

  .detail-action-surface {
    align-items: stretch;
    display: grid;
    gap: 0.45em;
    grid-template-columns: minmax(0, 1fr);
    min-width: 0;
    width: 100%;
  }

  .detail-action-surface :global(button) {
    margin: 0;
    min-height: 2.7em;
    width: 100%;
  }

  .mod-actions :global(button) {
    font-size: 0.78em;
    line-height: 1.1;
    margin: 0;
    min-height: 2.08em;
    padding: 0.32em 0.55em;
  }

  .mod-actions :global(.horizontal) {
    gap: 0.28em;
    width: 100%;
  }

  .description {
    padding: 0.5em;
    /* border-radius: 10px; */
    /* border: 2px solid #414141; */
    /* border-bottom: 2px solid #414141; */

    border-radius: 6px;
    border-bottom: 2px solid #333;
    background-color: #121212;

    margin: 0 0 0.36em;
  }

  .description-content {
    color: #767676;
    display: block;
    font-size: 0.9em;
    line-height: 1.26;
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
    text-align: left;
  }

  .title-line {
    align-items: baseline;
    display: flex;
    flex-wrap: wrap;
    gap: 0.45em;
    min-width: 0;
  }

  .mod-title {
    color: #a2a2a2;
    display: block;
    font-size: 1.02em;
    font-weight: bold;
    line-height: 1.12;
    margin-top: 0;
    min-width: 0;
    overflow-wrap: anywhere;
    text-align: left;
  }

  .meta-row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.32em;
    margin: 0;
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
    padding: 0.26em 0.48em;
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

  .fact-row {
    display: grid;
    gap: 0.18em 0.6em;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    min-width: 0;
  }

  .fact-row span {
    color: #7d7d7d;
    font-size: 0.8em;
    line-height: 1.25;
    min-width: 0;
    overflow: hidden;
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .details-button,
  .open-page-button {
    color: #d6dde5;
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
    justify-content: center;
    margin: 0;
    padding: 0.34em 0.54em;
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

  .image-container {
    align-self: center;
    background: rgba(8, 8, 8, 0.85);
    border-radius: 6px;
    height: var(--sotf-thumb-height, clamp(74px, 9vh, 112px));
    min-width: 0;
    overflow: hidden;
    position: relative;
    width: 100%;
  }
  
  .cover-img {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    border-radius: 6px;
    object-fit: cover;
    transition: opacity 0.3s ease-in-out;
  }
  
  .main-image {
    opacity: 0;
  }
  
  .main-image.isImageLoaded {
    opacity: 1;
  }

  .preview-count,
  .preview-controls {
    align-items: center;
    display: flex;
    gap: 0.2em;
    position: absolute;
    z-index: 2;
  }

  .preview-count {
    background: rgba(6, 10, 12, 0.82);
    border: 1px solid rgba(255, 255, 255, 0.14);
    color: #d6f3ff;
    font-size: 0.68em;
    font-weight: 900;
    left: 0.35em;
    padding: 0.18em 0.34em;
    text-transform: uppercase;
    top: 0.35em;
  }

  .preview-count :global(svg) {
    height: 1em;
    width: 1em;
  }

  .preview-count-fallback {
    color: #9aa5af;
  }

  .preview-controls {
    bottom: 0.35em;
    right: 0.35em;
  }

  .preview-controls button {
    align-items: center;
    background: rgba(10, 14, 18, 0.82);
    border: 1px solid rgba(255, 255, 255, 0.14);
    box-shadow: none;
    color: #62f09b;
    display: inline-flex;
    height: 1.8em;
    justify-content: center;
    margin: 0;
    min-width: 1.8em;
    padding: 0;
    -webkit-mask-image: none;
    mask-image: none;
  }

  .preview-controls button:hover {
    border-color: rgba(98, 240, 155, 0.4);
  }

  .preview-controls :global(svg) {
    height: 1.05em;
    width: 1.05em;
  }

  .header-desc {
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    display: -webkit-box;
    line-clamp: 2;
    max-height: 2.7em;
    overflow: hidden;
  }

  @media (max-width: 920px) {
    .mod-card-row {
      grid-template-columns: minmax(118px, var(--sotf-thumb-width, 168px)) minmax(0, 1fr);
    }

    .mod-actions {
      align-items: stretch;
      grid-column: 1 / -1;
      justify-content: flex-start;
    }
  }

  @media (max-width: 680px) {
    .mod-card-row,
    .fact-row {
      grid-template-columns: 1fr;
    }
  }
</style>
