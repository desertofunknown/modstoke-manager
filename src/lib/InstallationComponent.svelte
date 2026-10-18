<script lang="ts">
  import { gameExePath, isPathValid, processName, processProgress, processing } from './store';
  import { confirm, message } from '@tauri-apps/plugin-dialog';
  import { InstallMode, type FeatureInstaller } from "./featureInstaller";
  import { onMount } from 'svelte';

  export let feature: FeatureInstaller;

  let currentMode = 'Install';
  let readyMode = InstallMode.Install;
  let readyPath: string | null = null;
  let readyFeature: FeatureInstaller | null = null;
  let shouldBeVisible = false;
  let actionAvailable = false;
  let featureLabel = '';
  let loading = true;
  let statusError: string | null = null;
  let releaseWarning: string | null = null;
  let operationError: string | null = null;
  let actionBusy = false;
  let mounted = false;
  let refreshGeneration = 0;

  $: currentClass = currentMode.toLowerCase();
  $: description = feature.description;
  $: {
    feature;
    $gameExePath;
    operationError = null;
  }
  $: if (mounted) {
    void refreshStatus(feature, $gameExePath, $isPathValid);
  }

  onMount(() => {
    mounted = true;
    return () => {
      mounted = false;
      ++refreshGeneration;
    };
  });

  function isCurrentRefresh(generation: number, selectedFeature: FeatureInstaller, selectedPath: string) {
    return mounted && generation === refreshGeneration && selectedFeature === feature
      && selectedPath === $gameExePath && $isPathValid;
  }

  async function refreshStatus(selectedFeature: FeatureInstaller, selectedPath: string, pathValid: boolean) {
    const generation = ++refreshGeneration;
    loading = pathValid && !!selectedPath;
    shouldBeVisible = false;
    actionAvailable = false;
    readyPath = null;
    readyFeature = null;
    statusError = null;
    releaseWarning = null;
    featureLabel = selectedFeature.getName();

    if (!loading) {
      return;
    }

    try {
      const mode = await selectedFeature.refreshMode();
      if (!isCurrentRefresh(generation, selectedFeature, selectedPath)) {
        return;
      }

      const label = mode === InstallMode.Uninstall
        ? selectedFeature.getName()
        : await selectedFeature.getRemoteVersionString(true) ?? selectedFeature.getName();
      const available = await selectedFeature.canDoAction(mode);
      if (!isCurrentRefresh(generation, selectedFeature, selectedPath)) {
        return;
      }

      currentMode = InstallMode[mode];
      readyMode = mode;
      featureLabel = label;
      releaseWarning = selectedFeature.releaseWarning;
      actionAvailable = available;
      shouldBeVisible = available || !!releaseWarning;
      readyPath = selectedPath;
      readyFeature = selectedFeature;
    } catch (error) {
      if (isCurrentRefresh(generation, selectedFeature, selectedPath)) {
        statusError = `Could not check ${selectedFeature.getName()} installation: ${error}`;
      }
    } finally {
      if (isCurrentRefresh(generation, selectedFeature, selectedPath)) {
        loading = false;
      }
    }
  }

  async function handleAction(mode: InstallMode) {
    if (loading || actionBusy || $processing || !actionAvailable || !$isPathValid
      || readyPath !== $gameExePath || readyFeature !== feature
      || (mode !== readyMode && !(readyMode === InstallMode.Update && mode === InstallMode.Uninstall))) {
      return;
    }

    const selectedFeature = feature;
    const selectedPath = $gameExePath;
    const generation = refreshGeneration;
    let operationStarted = false;
    actionBusy = true;
    operationError = null;

    try {
      if (mode === InstallMode.Uninstall) {
        const approved = await confirm(`Uninstall ${selectedFeature.getName()} from this installation?\n\n${selectedPath}`, {
          title: `Uninstall ${selectedFeature.getName()}`,
          kind: 'warning',
          okLabel: 'Uninstall',
          cancelLabel: 'Cancel'
        });
        if (!approved) {
          return;
        }
      }

      if (!isCurrentRefresh(generation, selectedFeature, selectedPath) || $processing
        || loading || !actionAvailable || readyPath !== selectedPath || readyFeature !== selectedFeature) {
        return;
      }

      ++refreshGeneration;
      loading = true;
      shouldBeVisible = false;
      actionAvailable = false;
      readyPath = null;
      processing.set(true);
      processProgress.set(0);
      operationStarted = true;
      await selectedFeature.handle(mode);
    } catch (error) {
      if (mounted && selectedFeature === feature && selectedPath === $gameExePath) {
        operationError = `${selectedFeature.getName()} operation failed: ${error}`;
      }
      try {
        await message(`${error}`, { title: `${selectedFeature.getName()} operation failed`, kind: 'error' });
      } catch (dialogError) {
        console.error('Failed to show operation error', error, dialogError);
      }
    } finally {
      if (operationStarted) {
        processing.set(false);
        processName.set('');
        processProgress.set(0);
        if (mounted) {
          await refreshStatus(feature, $gameExePath, $isPathValid);
        }
      }
      actionBusy = false;
    }
  }
</script>

{#if loading || statusError || (shouldBeVisible && readyPath === $gameExePath && readyFeature === feature && $isPathValid)}
  <section class="feature-container installation-card" aria-label={feature.getName()}>
    <div class="feature-copy">
      <div class="feature-heading">
        <h3>{feature.getName()}</h3>
        {#if loading}
          <span class="feature-state checking" role="status">Checking installation…</span>
        {:else if statusError}
          <span class="feature-state state-error">Check failed</span>
        {:else}
          <span class="feature-state" class:state-installed={currentMode === "Uninstall"} class:state-update={currentMode === "Update"}>
            {currentMode === "Uninstall" ? "Installed" : currentMode === "Update" ? "Update available" : actionAvailable ? "Ready to install" : "Release unavailable"}
          </span>
        {/if}
      </div>
      {#if description}
        <p class="description-content">{description}</p>
      {/if}
      {#if !loading && !statusError && featureLabel !== feature.getName()}
        <p class="feature-version">Available release <span>{featureLabel}</span></p>
      {/if}
    </div>

    {#if !loading && !statusError && actionAvailable}
      <div class="feature-actions">
        {#if currentMode === "Update"}
          <button type="button" disabled={actionBusy || $processing} aria-label={`Update ${featureLabel}`} on:click={() => handleAction(InstallMode.Update)} class="update">Update</button>
          <button type="button" disabled={actionBusy || $processing} aria-label={`Uninstall ${feature.getName()}`} on:click={() => handleAction(InstallMode.Uninstall)} class="uninstall">Uninstall</button>
        {:else}
          <button type="button" disabled={actionBusy || $processing} aria-label={`${currentMode} ${featureLabel}`} on:click={() => handleAction(readyMode)} class={currentClass}>{currentMode}</button>
        {/if}
      </div>
    {/if}

    {#if statusError}
      <div class="feature-feedback error-feedback">
        <p role="alert">{statusError}</p>
        <button type="button" class="retry-check" aria-label={`Retry ${feature.getName()} check`} disabled={actionBusy || $processing || !$isPathValid} on:click={() => refreshStatus(feature, $gameExePath, $isPathValid)}>Retry check</button>
      </div>
    {:else if !loading && releaseWarning}
      <div class="feature-feedback warning-feedback">
        <p role="status">{releaseWarning}</p>
        <button type="button" class="retry-check" aria-label={`Retry ${feature.getName()} check`} disabled={actionBusy || $processing} on:click={() => refreshStatus(feature, $gameExePath, $isPathValid)}>Retry check</button>
      </div>
    {/if}
  </section>
{/if}

{#if operationError}
  <div class="feature-container operation-feedback">
    <p class="operation-error" role="alert">{operationError}</p>
  </div>
{/if}

<style>
  .installation-card {
    align-items: center;
    background: var(--surface-panel, #171a21);
    border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.08));
    border-radius: 10px;
    display: flex;
    flex-wrap: wrap;
    gap: 14px 18px;
    min-width: 0;
    padding: 16px 18px;
    text-align: left;
  }

  .feature-copy {
    flex: 1 1 220px;
    min-width: 0;
  }

  .feature-heading {
    align-items: center;
    display: flex;
    flex-wrap: wrap;
    gap: 7px 10px;
    min-width: 0;
  }

  .feature-heading h3 {
    color: var(--text-primary, #eef0f4);
    font-size: 15px;
    font-weight: 650;
    line-height: 1.4;
    margin: 0;
    overflow-wrap: anywhere;
  }

  .feature-state {
    background: var(--surface-raised, #20242c);
    border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.08));
    border-radius: 5px;
    color: var(--text-secondary, #9da6b6);
    display: inline-flex;
    font-size: 11px;
    font-weight: 550;
    line-height: 1.35;
    padding: 3px 7px;
  }

  .state-installed {
    background: rgba(123, 184, 151, 0.1);
    border-color: rgba(123, 184, 151, 0.13);
    color: #a4d3b8;
  }

  .state-update {
    background: var(--accent-soft, rgba(238, 182, 107, 0.12));
    border-color: rgba(238, 182, 107, 0.16);
    color: var(--accent, #eeb66b);
  }

  .state-error {
    background: rgba(214, 133, 133, 0.1);
    border-color: rgba(214, 133, 133, 0.16);
    color: #e6a4a4;
  }

  .description-content {
    color: var(--text-secondary, #9da6b6);
    font-size: 13px;
    line-height: 1.55;
    margin: 7px 0 0;
    overflow-wrap: anywhere;
    text-align: left;
  }

  .feature-version {
    color: #818c9e;
    font-size: 11px;
    line-height: 1.5;
    margin: 7px 0 0;
    overflow-wrap: anywhere;
  }

  .feature-version span {
    color: #b4bdcb;
    margin-left: 5px;
  }

  .feature-actions {
    align-items: center;
    display: flex;
    flex: 0 1 auto;
    flex-wrap: wrap;
    gap: 8px;
    margin-left: auto;
    min-width: 0;
  }

  .installation-card button {
    background: var(--surface-raised, #20242c);
    border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.08));
    border-radius: 7px;
    color: var(--text-primary, #eef0f4);
    flex: 0 0 auto;
    font-size: 12px;
    font-weight: 600;
    line-height: 1.3;
    margin: 0;
    min-height: 34px;
    padding: 7px 12px;
    width: auto;
  }

  .installation-card button.install,
  .installation-card button.update {
    background: var(--accent, #eeb66b);
    border-color: var(--accent, #eeb66b);
    color: #201b15;
  }

  .installation-card button.install:enabled:hover,
  .installation-card button.update:enabled:hover {
    background: #f3c58b;
    border-color: #f3c58b;
    color: #201b15;
  }

  .installation-card button.uninstall:enabled:hover {
    background: rgba(214, 133, 133, 0.1);
    border-color: rgba(214, 133, 133, 0.3);
    color: #e6a4a4;
  }

  .feature-feedback {
    align-items: center;
    border-top: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.08));
    display: flex;
    flex: 1 0 100%;
    flex-wrap: wrap;
    gap: 10px 14px;
    min-width: 0;
    padding-top: 13px;
  }

  .feature-feedback p {
    flex: 1 1 200px;
    font-size: 12px;
    line-height: 1.5;
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .warning-feedback {
    color: #d6b583;
  }

  .error-feedback {
    color: #e6a4a4;
  }

  .installation-card .retry-check {
    background: transparent;
    color: #bec6d3;
    margin-left: auto;
  }

  .installation-card .retry-check:enabled:hover {
    background: var(--surface-raised, #20242c);
    border-color: rgba(255, 255, 255, 0.18);
  }

  .operation-feedback {
    background: rgba(214, 133, 133, 0.08);
    border: 1px solid rgba(214, 133, 133, 0.16);
    border-radius: 8px;
    padding: 12px 16px;
  }

  .operation-error {
    color: #e6a4a4;
    font-size: 12px;
    line-height: 1.5;
    margin: 0;
    overflow-wrap: anywhere;
    text-align: left;
  }
</style>
