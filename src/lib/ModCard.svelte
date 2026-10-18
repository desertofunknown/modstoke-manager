<script lang="ts">
  import { gameExePath, isPathValid, processName, processProgress, processing } from './store';
    import { createEventDispatcher, onDestroy } from 'svelte';
    import { ModDatabase, modPreviewUrls, type Mod } from './mods';
    import { storeRichTextPlainText } from './richText';
    import { fallbackPreviewUrl } from './modPreview';
    import StatusButton from './StatusButton.svelte';
    import * as dialog from "@tauri-apps/plugin-dialog"
    import LucideChevronLeft from "~icons/lucide/chevron-left";
    import LucideChevronRight from "~icons/lucide/chevron-right";
    import LucideImages from "~icons/lucide/images";

    export let mod: Mod;
    export let actionsDisabled = false;
    export let detailActionsOnly = false;
    export let showDetailsButton = true;
    export let sortMetricLabel = "";
    export let sortMetricValue = "";

    let isLibrary = false;
    let isImageLoaded = false;
    let hasPreviewFailed = false;
    let activePreviewModKey = "";
    let currentPreviewIndex = 0;
    let previewUrls: string[] = [];
    let currentPreviewUrl = "";
    let previewCountLabel = "";
    let cardSummary = "";
    let sourceDisplayLabel = "";
    let loaderDisplayLabel = "";
    let multiplayerDisplayLabel = "";
    let downloadCountLabel = "";
    let isVortexManagedMod = false;
    let actionBusy = false;
    let operationError = "";
    let errorModKey = "";
    let errorPath = "";
    let destroyed = false;

    const dispatch = createEventDispatcher<{ refreshMods: void; details: Mod }>();
    onDestroy(() => { destroyed = true; });

    $: currentModKey = mod.mod_id || mod.slug || mod.name;
    $: if (currentModKey !== errorModKey || $gameExePath !== errorPath) {
      operationError = "";
      errorModKey = currentModKey;
      errorPath = $gameExePath;
    }

    $: isLibrary = mod.type === "Library" || mod.installedMod?.loaderType === "redloader-library";
    $: {
      const modKey = mod.mod_id || mod.slug || mod.name;
      if (activePreviewModKey !== modKey) {
        activePreviewModKey = modKey;
        currentPreviewIndex = 0;
      }
    }
    $: previewUrls = modPreviewUrls(mod);
    $: if (currentPreviewIndex >= previewUrls.length) {
      currentPreviewIndex = 0;
    }
    $: currentPreviewUrl = previewUrls[currentPreviewIndex] ?? fallbackPreviewUrl;
    $: {
      currentPreviewUrl;
      isImageLoaded = false;
      hasPreviewFailed = false;
    }
    $: previewCountLabel = previewUrls.length > 0 ? `${currentPreviewIndex + 1}/${previewUrls.length}` : "Local";
    $: cardSummary = storeRichTextPlainText(mod.shortDescription);
    $: sourceDisplayLabel = !mod.installedMod
      ? "Online"
      : mod.installedMod.installSource === "vortex"
        ? "Vortex"
        : mod.installedMod.installSource === "native"
          ? "Native"
          : "Manual";
    $: loaderDisplayLabel = mod.installedMod?.loaderType === "bepinex-plugin"
      ? "BepInEx"
      : mod.installedMod?.loaderType === "redloader-library" || mod.type === "Library"
        ? "RedLoader Library"
        : "RedLoader Mod";
    $: multiplayerDisplayLabel = mod.requiresAllPlayers
      ? "All players"
      : mod.isMultiplayerCompatible
        ? "MP compatible"
        : mod.modSide === "client"
          ? "Client-side"
          : "Compatibility unknown";
    $: downloadCountLabel = formatCount(mod.downloads ?? mod.lastWeekDownloads);
    $: isVortexManagedMod = mod.installedMod?.installSource === "vortex";

    async function update() {
      if (!mod.installedMod) return;
      await runOperation("update", selectedMod => ModDatabase.updateMod(selectedMod));
    }

    async function uninstall() {
      const installedMod = mod.installedMod;
      if (!installedMod) return;
      await runOperation("uninstall", () => ModDatabase.uninstallMod(installedMod));
    }

    async function install() {
      await runOperation("install", selectedMod => ModDatabase.installMod(selectedMod));
    }

    type ModAction = "install" | "update" | "uninstall" | "enable" | "disable";

    async function runOperation(action: ModAction, operation: (selectedMod: Mod) => Promise<void>) {
      if (destroyed || actionBusy || $processing || actionsDisabled || !$isPathValid || !$gameExePath) return;
      const selectedMod = mod;
      const installedMod = selectedMod.installedMod;
      const selectedPath = $gameExePath;
      let operationStarted = false;
      actionBusy = true;
      operationError = "";

      const isCurrentSelection = () => !destroyed && selectedMod === mod && selectedPath === $gameExePath;

      try {
        if (installedMod?.installSource === "vortex") {
          await showVortexManagedMessage(selectedMod, action);
          return;
        }

        if (action === "uninstall") {
          const confirmed = await dialog.confirm(
            `Uninstall ${selectedMod.name} from this installation?\n\n${selectedPath}`,
            { title: `Uninstall ${selectedMod.name}`, kind: "warning", okLabel: "Uninstall", cancelLabel: "Cancel" }
          );
          if (!confirmed) return;
        }

        if (!isCurrentSelection() || selectedMod.installedMod !== installedMod
          || selectedMod.installedMod?.installSource === "vortex" || !$isPathValid || $processing || actionsDisabled) return;

        operationStarted = true;
        processing.set(true);
        const actionLabel = { install: "Installing", update: "Updating", uninstall: "Removing", enable: "Enabling", disable: "Disabling" }[action];
        processName.set(`${actionLabel} ${selectedMod.name}...`);
        processProgress.set(0);
        await operation(selectedMod);
      } catch (error) {
        if (isCurrentSelection()) {
          operationError = `${selectedMod.name}: ${error instanceof Error ? error.message : String(error)}`;
          try {
            await dialog.message(operationError, { title: `${selectedMod.name} operation failed`, kind: "error" });
          } catch {
            // The inline error remains available if the native dialog cannot open.
          }
        }
      } finally {
        actionBusy = false;
        if (operationStarted) {
          processing.set(false);
          processName.set("");
          processProgress.set(0);
          if (isCurrentSelection()) {
            mod = mod;
            dispatch("refreshMods");
          }
        }
      }
    }

    async function handleEnabledChange(event: Event) {
      const input = event.currentTarget as HTMLInputElement;
      if (!mod.installedMod) {
        return;
      }

      const checked = input.checked;
      input.checked = mod.installedMod.isEnabled;
      const installedMod = mod.installedMod;
      await runOperation(checked ? "enable" : "disable", () => ModDatabase.toggleMod(installedMod, checked));
    }

    function formatDate(dateString: string) {
      const date = new Date(dateString);
      return date.toLocaleDateString("en-US", {
        year: "numeric",
        month: "2-digit",
        day: "2-digit",
      });
    };
    
    function onImageLoad(event: Event) {
      if ((event.currentTarget as HTMLImageElement).getAttribute("src") === currentPreviewUrl) {
        isImageLoaded = true;
      }
    }

    function onImageError(event: Event) {
      if ((event.currentTarget as HTMLImageElement).getAttribute("src") === currentPreviewUrl) {
        hasPreviewFailed = true;
        isImageLoaded = false;
      }
    }

    function cyclePreview(direction: number) {
      if (previewUrls.length <= 1) {
        return;
      }

      currentPreviewIndex = (currentPreviewIndex + direction + previewUrls.length) % previewUrls.length;
    }

    async function showVortexManagedMessage(selectedMod: Mod, action: string) {
      await dialog.message(
        `${selectedMod.name} is currently managed by Vortex/Nexus. Use Vortex to ${action} it so deployment metadata stays consistent.`,
        {
          title: "Managed by Vortex",
          kind: "info"
        }
      );
    }

    function formatCount(value?: number) {
      return typeof value === "number" ? value.toLocaleString() : "-";
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
      <label class="enable-control" class:vortex-disabled={isVortexManagedMod}>
        <input
          type="checkbox"
          checked={!!mod.installedMod?.isEnabled}
          aria-label={`Enable ${mod.name}`}
          disabled={isVortexManagedMod || actionsDisabled || actionBusy || $processing || !$isPathValid}
          on:change={handleEnabledChange}
        />
        <span>{mod.installedMod?.isEnabled ? "Enabled" : "Disabled"}</span>
      </label>
    {/if}

    <StatusButton subject={mod.name} disabled={actionsDisabled || actionBusy || $processing || !$isPathValid} isUpdateAvailable={mod.hasUpdate} isModInstalled={mod.isInstalled} update={update} uninstall={uninstall} install={install} />
    {#if operationError}<p class="operation-error" role="alert">{operationError}</p>{/if}
  </div>
{:else}
<article class="feature-container description mod-card">
  <div class="mod-card-row">
    <div class="image-container">
      <img
        class="cover-img main-image fallback-image"
        class:isImageLoaded={!isImageLoaded}
        src={fallbackPreviewUrl}
        alt={`Local fallback preview for ${mod.name}`}
      />
      {#key currentPreviewUrl}
        {#if currentPreviewUrl !== fallbackPreviewUrl && !hasPreviewFailed}
          <img
            class="cover-img main-image"
            class:isImageLoaded
            src={currentPreviewUrl}
            alt={`Preview for ${mod.name}`}
            on:load={onImageLoad}
            on:error={onImageError}
          />
        {/if}
      {/key}
      <div
        class="preview-count"
        class:preview-count-fallback={previewUrls.length === 0 || hasPreviewFailed}
        aria-label={hasPreviewFailed ? `${mod.name} preview image ${currentPreviewIndex + 1} of ${previewUrls.length} is unavailable; showing local fallback` : previewUrls.length > 0 ? `${mod.name} preview image ${currentPreviewIndex + 1} of ${previewUrls.length}` : `${mod.name} uses the local fallback preview image`}
        title={hasPreviewFailed ? "Preview unavailable; showing local fallback" : previewUrls.length > 0 ? `${previewUrls.length} preview ${previewUrls.length === 1 ? "image" : "images"}` : "Local fallback preview"}
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
        <h3 class="mod-title">{mod.name}</h3>
        {#if mod.isInstalled}
          <span class="installation-state" class:update-available={mod.hasUpdate}>{mod.hasUpdate ? "Update available" : "Installed"}</span>
        {/if}
        {#if sortMetricLabel && sortMetricValue}
          <span class="sort-metric-pill" title={`${sortMetricLabel}: ${sortMetricValue}`}>
            <span>{sortMetricLabel}</span>
            <b>{sortMetricValue}</b>
          </span>
        {/if}
      </div>
      <div class="author-line">by <strong>{mod.user.name}</strong></div>
      <p class="description-content header-desc">{cardSummary}</p>
      <div class="meta-row">
        <span class="source-pill source-{mod.installedMod?.installSource ?? 'online'}">{sourceDisplayLabel}</span>
        <span class="source-pill">{loaderDisplayLabel}</span>
        <span class="source-pill">{multiplayerDisplayLabel}</span>
        {#if mod.installedMod?.vortexPackage}
          <span class="source-pill source-detail">{mod.installedMod.vortexPackage}</span>
        {/if}
      </div>
      <div class="fact-row">
        <span>Version <b>{mod.latestVersion}</b></span>
        <span>Updated <b>{mod.lastReleasedAt?formatDate(mod.lastReleasedAt):"-"}</b></span>
        <span>Downloads <b>{downloadCountLabel}</b></span>
        <span>Category <b>{mod.category?mod.category.name:"-"}</b></span>
      </div>
    </div>

    <div class="mod-actions">
      {#if mod.isInstalled && !isLibrary}
        <label class="enable-control" class:vortex-disabled={isVortexManagedMod}>
          <input
            type="checkbox"
            checked={!!mod.installedMod?.isEnabled}
            aria-label={`Enable ${mod.name}`}
            disabled={isVortexManagedMod || actionsDisabled || actionBusy || $processing || !$isPathValid}
            on:change={handleEnabledChange}
          />
          <span>{mod.installedMod?.isEnabled ? "Enabled" : "Disabled"}</span>
        </label>
      {/if}

      <StatusButton subject={mod.name} disabled={actionsDisabled || actionBusy || $processing || !$isPathValid} isUpdateAvailable={mod.hasUpdate} isModInstalled={mod.isInstalled} update={update} uninstall={uninstall} install={install} />
      <div class="secondary-actions">
        {#if showDetailsButton}
          <button class="details-button" type="button" aria-label={`Details for ${mod.name}`} on:click={showDetails}>Details</button>
        {/if}
        <button class="open-page-button" type="button" aria-label={`Open Page for ${mod.name}`} on:click={openModPage}>Open page</button>
      </div>
      {#if operationError}<p class="operation-error" role="alert">{operationError}</p>{/if}
    </div>
  </div>
</article>
{/if}

<style>
  .mod-card {
    background: var(--surface-panel, #171a21);
    border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.08));
    border-radius: 10px;
    margin: 0 0 10px;
    padding: 14px;
    text-align: left;
    transition: border-color 160ms ease, background-color 160ms ease;
  }

  .mod-card:hover {
    background: #1a1e26;
    border-color: rgba(255, 255, 255, 0.14);
  }

  .mod-card-row {
    align-items: start;
    display: grid;
    gap: 12px 16px;
    grid-template-columns: var(--sotf-thumb-width, 160px) minmax(0, 1fr);
    min-width: 0;
    width: 100%;
  }

  .mod-info {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }

  .title-line {
    align-items: center;
    display: flex;
    flex-wrap: wrap;
    gap: 7px 10px;
    min-width: 0;
  }

  .mod-title {
    color: var(--text-primary, #eef0f4);
    font-size: 15px;
    font-weight: 650;
    line-height: 1.35;
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .author-line {
    color: var(--text-secondary, #9da6b6);
    font-size: 12px;
    line-height: 1.4;
    margin-top: -4px;
    overflow-wrap: anywhere;
  }

  .author-line strong {
    color: #b9c1cf;
    font-weight: 500;
  }

  .installation-state,
  .sort-metric-pill {
    align-items: center;
    border: 1px solid transparent;
    border-radius: 5px;
    display: inline-flex;
    flex: 0 1 auto;
    font-size: 11px;
    font-weight: 550;
    gap: 5px;
    line-height: 1.3;
    max-width: 100%;
    min-width: 0;
    padding: 3px 6px;
  }

  .installation-state {
    background: rgba(123, 184, 151, 0.1);
    color: #a4d3b8;
  }

  .installation-state.update-available,
  .sort-metric-pill {
    background: var(--accent-soft, rgba(238, 182, 107, 0.12));
    color: var(--accent, #eeb66b);
  }

  .sort-metric-pill {
    margin-left: auto;
  }

  .sort-metric-pill span,
  .sort-metric-pill b {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sort-metric-pill b {
    color: var(--text-primary, #eef0f4);
    font-weight: 600;
  }

  .description-content {
    color: var(--text-secondary, #9da6b6);
    display: block;
    font-size: 13px;
    line-height: 1.5;
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
    text-align: left;
  }

  .header-desc {
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    display: -webkit-box;
    line-clamp: 2;
    max-height: 3em;
    overflow: hidden;
  }

  .meta-row {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    min-width: 0;
  }

  .source-pill {
    background: var(--surface-raised, #20242c);
    border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.08));
    border-radius: 5px;
    color: #aeb7c6;
    display: inline-block;
    font-size: 11px;
    font-weight: 500;
    line-height: 1.4;
    max-width: 100%;
    overflow: hidden;
    padding: 2px 6px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .source-vortex {
    color: #b0c9dc;
  }

  .source-native {
    color: #a4d3b8;
  }

  .source-manual {
    color: #d4bd9d;
  }

  .source-detail {
    flex: 0 1 auto;
  }

  .fact-row {
    display: flex;
    flex-wrap: wrap;
    gap: 5px 14px;
    min-width: 0;
  }

  .fact-row span {
    color: #818c9e;
    font-size: 11px;
    line-height: 1.45;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .fact-row b {
    color: #b4bdcb;
    font-weight: 500;
    margin-left: 3px;
  }

  .mod-actions,
  .detail-action-surface,
  .secondary-actions {
    align-items: center;
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    min-width: 0;
  }

  .mod-actions {
    grid-column: 2;
    padding-top: 2px;
  }

  .detail-action-surface {
    width: 100%;
  }

  .secondary-actions {
    margin-left: auto;
  }

  .details-button,
  .open-page-button {
    background: transparent;
    border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.08));
    border-radius: 7px;
    color: #bec6d3;
    font-size: 12px;
    font-weight: 550;
    line-height: 1.25;
    margin: 0;
    min-height: 32px;
    padding: 7px 10px;
    width: auto;
  }

  .details-button:hover,
  .open-page-button:hover {
    background: var(--surface-raised, #20242c);
    border-color: rgba(255, 255, 255, 0.18);
    color: var(--text-primary, #eef0f4);
  }

  .enable-control {
    align-items: center;
    color: #b4d1c0;
    display: inline-flex;
    font-size: 12px;
    font-weight: 500;
    gap: 7px;
    line-height: 1.4;
    margin: 0 3px 0 0;
    min-height: 32px;
  }

  .enable-control input[type="checkbox"] {
    appearance: none;
    background: #303642;
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 12px;
    box-sizing: border-box;
    cursor: pointer;
    display: block;
    flex: 0 0 30px;
    float: none;
    height: 18px;
    margin: 0;
    padding: 2px;
    position: relative;
    transform: none;
    transition: background-color 140ms ease;
    width: 30px;
  }

  .enable-control input[type="checkbox"]::before {
    background: #bcc4d1;
    border-radius: 50%;
    content: "";
    display: block;
    height: 12px;
    transform: translateX(0);
    transition: transform 140ms ease;
    width: 12px;
  }

  .enable-control input[type="checkbox"]:checked {
    background: rgba(123, 184, 151, 0.35);
    border-color: rgba(123, 184, 151, 0.4);
  }

  .enable-control input[type="checkbox"]:checked::before {
    background: #b1d9c1;
    transform: translateX(12px);
  }

  .enable-control input[type="checkbox"]:disabled {
    cursor: not-allowed;
  }

  .enable-control:not(:has(input:checked)) {
    color: var(--text-secondary, #9da6b6);
  }

  .vortex-disabled {
    opacity: 0.65;
  }

  .operation-error {
    background: rgba(214, 133, 133, 0.08);
    border: 1px solid rgba(214, 133, 133, 0.16);
    border-radius: 7px;
    box-sizing: border-box;
    color: #e6a4a4;
    flex: 1 0 100%;
    font-size: 12px;
    line-height: 1.5;
    margin: 0;
    overflow-wrap: anywhere;
    padding: 9px 11px;
  }

  .image-container {
    background: var(--surface-base, #101217);
    border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.08));
    border-radius: 8px;
    grid-row: 1 / 3;
    height: var(--sotf-thumb-height, 108px);
    min-height: 88px;
    min-width: 0;
    overflow: hidden;
    position: relative;
    width: 100%;
  }

  .cover-img {
    height: 100%;
    left: 0;
    object-fit: cover;
    position: absolute;
    top: 0;
    transition: opacity 180ms ease;
    width: 100%;
  }

  .main-image {
    opacity: 0;
  }

  .fallback-image {
    box-sizing: border-box;
    object-fit: contain;
    padding: 10px;
  }

  .main-image.isImageLoaded {
    opacity: 1;
  }

  .preview-count,
  .preview-controls {
    align-items: center;
    display: flex;
    gap: 4px;
    position: absolute;
    z-index: 2;
  }

  .preview-count {
    background: rgba(16, 18, 23, 0.88);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 5px;
    color: #d0d6e0;
    font-size: 10px;
    font-weight: 550;
    left: 6px;
    line-height: 1.3;
    padding: 3px 5px;
    top: 6px;
  }

  .preview-count :global(svg) {
    height: 11px;
    width: 11px;
  }

  .preview-count-fallback {
    color: var(--text-secondary, #9da6b6);
  }

  .preview-controls {
    bottom: 6px;
    right: 6px;
  }

  .preview-controls button {
    align-items: center;
    background: rgba(16, 18, 23, 0.9);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 5px;
    box-shadow: none;
    color: var(--text-primary, #eef0f4);
    display: inline-flex;
    height: 25px;
    justify-content: center;
    margin: 0;
    min-height: 25px;
    min-width: 25px;
    padding: 0;
    width: 25px;
  }

  .preview-controls button:hover {
    background: var(--surface-raised, #20242c);
    border-color: rgba(255, 255, 255, 0.3);
  }

  .preview-controls :global(svg) {
    height: 14px;
    width: 14px;
  }

  @media (max-width: 760px) {
    .mod-card-row {
      gap: 12px;
      grid-template-columns: 112px minmax(0, 1fr);
    }

    .mod-actions {
      grid-column: 1 / -1;
    }

    .image-container {
      grid-row: auto;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .mod-card,
    .cover-img,
    .enable-control input[type="checkbox"],
    .enable-control input[type="checkbox"]::before {
      transition: none;
    }
  }
</style>
