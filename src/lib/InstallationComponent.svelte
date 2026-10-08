<script lang="ts">
  import { gameExePath, isPathValid, processName, processProgress, processing } from './store';
  import { message } from '@tauri-apps/plugin-dialog';
  import { InstallMode, type FeatureInstaller } from "./featureInstaller";
  import { onMount } from 'svelte';

  export let feature: FeatureInstaller;

  let currentMode = 'Install';
  let readyMode = InstallMode.Install;
  let readyPath: string | null = null;
  let readyFeature: FeatureInstaller | null = null;
  let shouldBeVisible = false;
  let featureLabel = '';
  let loading = true;
  let statusError: string | null = null;
  let mounted = false;
  let refreshGeneration = 0;

  $: currentClass = currentMode.toLowerCase();
  $: description = feature.description;
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
    readyPath = null;
    readyFeature = null;
    statusError = null;
    featureLabel = selectedFeature.getName();

    if (!loading) {
      return;
    }

    try {
      const mode = await selectedFeature.refreshMode();
      if (!isCurrentRefresh(generation, selectedFeature, selectedPath)) {
        return;
      }

      const label = await selectedFeature.getRemoteVersionString(true) ?? selectedFeature.getName();
      const visible = await selectedFeature.canDoAction(mode);
      if (!isCurrentRefresh(generation, selectedFeature, selectedPath)) {
        return;
      }

      currentMode = InstallMode[mode];
      readyMode = mode;
      featureLabel = label;
      shouldBeVisible = visible;
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
    if (loading || $processing || !shouldBeVisible || !$isPathValid
      || readyPath !== $gameExePath || readyFeature !== feature) {
      return;
    }

    const selectedFeature = feature;
    ++refreshGeneration;
    loading = true;
    shouldBeVisible = false;
    readyPath = null;
    processing.set(true);
    processProgress.set(0);

    try {
      await selectedFeature.handle(mode);
    } catch (error) {
      try {
        await message(`${error}`, { title: `${selectedFeature.getName()} operation failed`, kind: 'error' });
      } catch (dialogError) {
        console.error('Failed to show operation error', error, dialogError);
      }
    } finally {
      processing.set(false);
      processName.set('');
      processProgress.set(0);
      if (mounted) {
        await refreshStatus(feature, $gameExePath, $isPathValid);
      }
    }
  }
</script>

{#if loading}
  <div class="feature-container">
    <span class="description-content">Checking {feature.getName()} installation...</span>
  </div>
{:else if statusError}
  <div class="feature-container">
    <span class="description-content" role="alert">{statusError}</span>
    <button disabled={$processing || !$isPathValid} on:click={() => refreshStatus(feature, $gameExePath, $isPathValid)}>Retry {feature.getName()} check</button>
  </div>
{:else if shouldBeVisible && readyPath === $gameExePath && readyFeature === feature && $isPathValid}
  <div class="feature-container" class:description={description}>
    {#if description}
        <span class="description-content">{description}</span>
    {/if}
  {#if currentMode==="Update"}
    <div class="horizontal">
      <button disabled={$processing} on:click={() => handleAction(InstallMode.Update)} class="update btn-left">Update {featureLabel}</button>
      <button disabled={$processing} on:click={() => handleAction(InstallMode.Uninstall)} class="uninstall btn-right">Uninstall {feature.getName()}</button>
    </div>
  {:else}
    <button disabled={$processing} on:click={() => handleAction(readyMode)} class="{currentClass}">{currentMode} {featureLabel}</button>
  {/if}
  </div>
{/if}

<style>
  .horizontal {
    display: flex;
    flex-direction: row;
    align-items: center;
  }

  .horizontal > * {
    /* margin-right: 1em; */
    flex: 1;
  }

  .feature-container > * {
    width: 100%;
  }

  .description {
    padding: clamp(0.45em, 1.2vh, 0.65em);
    border-radius: 2px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    background: rgba(42, 42, 42, 0.58);
  }

  .description-content {
    margin-bottom: clamp(0.45em, 1.1vh, 0.8em);
    display: block;
    text-align: center;
    font-size: 0.9em;
    color: #a2a2a2;
  }

  .btn-left {
    border-radius: 2px 0 0 2px;
  }

  .btn-right {
    border-radius: 0 2px 2px 0;
  }

  .feature-container button {
    margin-bottom: 0;
  }

  @media (max-height: 780px) {
    .description-content {
      font-size: 0.82em;
      line-height: 1.25;
    }
  }

  @media (max-height: 720px) {
    .description {
      padding: 0.42em 0.55em;
    }

    .description-content {
      font-size: 0.76em;
      margin-bottom: 0.34em;
    }
  }
</style>
