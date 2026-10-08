<script lang="ts">
  import { processName, processProgress, processing } from './store';
  import { message } from '@tauri-apps/plugin-dialog';
  import { InstallMode, type FeatureInstaller } from "./featureInstaller";
    import { onMount } from 'svelte';

  export let feature: FeatureInstaller;

  $: currentMode = feature.currentModeState;
  $: currentClass = currentMode.toLowerCase();
  $: shouldBeVisible = false;
  $: description = feature.description;

  $: featureLabel = feature.getName();
  $: loading = true;

  async function handleWrapper(callback: () => Promise<void>) {
    processing.set(true);
    processProgress.set(0);

    try {
      await callback();
    } catch (error) {
      await message(`${error}`, { title: `${feature.getName()} operation failed`, kind: 'error' });
    } finally {
      processing.set(false);
      processName.set('');
      processProgress.set(0);
      currentMode = feature.currentModeState;
      await refreshVisibility();
    }
  }

  async function handleDefault() {
    await handleWrapper(async () => {
      await feature.handleCurrentMode();
    });
  }  

  async function handleUpdate() {
    await handleWrapper(async () => {
      await feature.handle(InstallMode.Update);
    });
  }

  async function handleUninstall() {
    await handleWrapper(async () => {
      await feature.handle(InstallMode.Uninstall);
    });
  }

  onMount( async () => {
    await feature.refreshMode();
    currentMode = feature.currentModeState;
    featureLabel = await feature.getRemoteVersionString(true)??feature.getName();
    await refreshVisibility();
    loading = false;
  });

  async function refreshVisibility()
  {
    shouldBeVisible = await feature.canDoAction() as boolean;
  }
</script>

{#if loading}
  <div class="feature-container">
    <span class="description-content">Checking {feature.getName()} installation...</span>
  </div>
{/if}

{#if shouldBeVisible}
  <div class="feature-container" class:description={description}>
    {#if description}
        <span class="description-content">{description}</span>
    {/if}
  {#if currentMode==="Update"}
    <div class="horizontal">
      <button on:click={handleUpdate} class="update btn-left">Update {featureLabel}</button>
      <button on:click={handleUninstall} class="uninstall btn-right">Uninstall {feature.getName()}</button>
    </div>
  {:else}
    <button on:click={handleDefault} class="{currentClass}">{currentMode} {featureLabel}</button>
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
