<script lang="ts">
  import {
    gameExePath,
    isDotnetInstalled,
    isPathValid,
    processName,
    processProgress,
    processing,
  } from "./lib/store";
  import Page1 from "./pages/MainPage.svelte";
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { invoke } from "@tauri-apps/api/core";
  import { currentMonitor, getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";

  type TabId = "main" | "mods" | "modding";

  type Tab = {
    id: TabId;
    label: string;
  };

  let tabs: Tab[] = [
    { id: "main", label: "Main" },
    { id: "mods", label: "Mods" },
    { id: "modding", label: "Mod Creation" },
  ];

  let activeTab: TabId = "main";
  let tabButtons: Partial<Record<TabId, HTMLButtonElement>> = {};
  let modsPage: Promise<typeof import("./pages/ModHub.svelte")> | null = null;
  let moddingPage: Promise<typeof import("./pages/Modding.svelte")> | null = null;
  let isWindowMaximized = false;

  const MIN_WINDOW_WIDTH = 980;
  const MIN_WINDOW_HEIGHT = 680;
  const MAX_WINDOW_WIDTH = 1920;
  const MAX_WINDOW_HEIGHT = 1240;

  function selectTab(tab: TabId) {
    if (tab === "mods" && !modsPage) {
      modsPage = import("./pages/ModHub.svelte");
    } else if (tab === "modding" && !moddingPage) {
      moddingPage = import("./pages/Modding.svelte");
    }
    activeTab = tab;
  }

  function handleTabKeyDown(event: KeyboardEvent, tab: TabId) {
    const index = tabs.findIndex((item) => item.id === tab);
    let nextIndex: number;
    switch (event.key) {
      case "ArrowLeft":
        nextIndex = (index + tabs.length - 1) % tabs.length;
        break;
      case "ArrowRight":
        nextIndex = (index + 1) % tabs.length;
        break;
      case "Home":
        nextIndex = 0;
        break;
      case "End":
        nextIndex = tabs.length - 1;
        break;
      default:
        return;
    }
    event.preventDefault();
    const nextTab = tabs[nextIndex].id;
    selectTab(nextTab);
    tabButtons[nextTab]?.focus();
  }

  function pageLoadError(error: unknown) {
    return error instanceof Error ? error.message : String(error);
  }

  async function minimizeWindow() {
    try {
      await getCurrentWindow().minimize();
    } catch (error) {
      console.log("Failed to minimize window", error);
    }
  }

  async function toggleMaximizeWindow() {
    try {
      await getCurrentWindow().toggleMaximize();
      window.setTimeout(() => {
        void refreshMaximizedState();
      }, 80);
    } catch (error) {
      console.log("Failed to toggle maximize", error);
    }
  }

  async function closeWindow() {
    try {
      await getCurrentWindow().close();
    } catch (error) {
      console.log("Failed to close window", error);
    }
  }

  async function refreshMaximizedState() {
    try {
      isWindowMaximized = await getCurrentWindow().isMaximized();
    } catch {
      isWindowMaximized = false;
    }
  }

  async function startWindowDrag(event: MouseEvent) {
    if (event.button !== 0) {
      return;
    }

    if ((event.target as HTMLElement | null)?.closest("button")) {
      return;
    }

    try {
      await getCurrentWindow().startDragging();
    } catch (error) {
      console.log("Failed to start window drag", error);
    }
  }

  function clampNumber(value: number, min: number, max: number) {
    return Math.min(max, Math.max(min, value));
  }

  function logicalWorkAreaSize(monitor: Awaited<ReturnType<typeof currentMonitor>>) {
    if (!monitor) {
      return null;
    }

    const scale = monitor.scaleFactor || 1;
    const area = monitor.workArea?.size ?? monitor.size;

    return {
      width: area.width / scale,
      height: area.height / scale,
    };
  }

  async function fitWindowToMonitor() {
    try {
      const monitor = await currentMonitor();
      const workArea = logicalWorkAreaSize(monitor);
      if (!workArea) {
        return;
      }

      const appWindow = getCurrentWindow();
      const minWidth = Math.min(MIN_WINDOW_WIDTH, Math.max(760, workArea.width - 24));
      const minHeight = Math.min(MIN_WINDOW_HEIGHT, Math.max(600, workArea.height - 24));
      const maxWidth = Math.max(minWidth, Math.min(MAX_WINDOW_WIDTH, workArea.width - 16));
      const maxHeight = Math.max(minHeight, Math.min(MAX_WINDOW_HEIGHT, workArea.height - 16));

      const targetWidth = Math.round(clampNumber(workArea.width * 0.9, minWidth, maxWidth));
      const targetHeight = Math.round(clampNumber(workArea.height * 0.94, minHeight, maxHeight));

      await appWindow.setSizeConstraints({
        minWidth: Math.round(minWidth),
        minHeight: Math.round(minHeight),
      });
      await appWindow.setSize(new LogicalSize(targetWidth, targetHeight));
      await appWindow.center();
    } catch (error) {
      console.log("Failed to fit window to monitor", error);
    }
  }

  onMount(() => {
    const onResize = () => {
      void refreshMaximizedState();
    };
    const onContextMenu = (event: MouseEvent) => event.preventDefault();
    window.addEventListener("resize", onResize);
    document.addEventListener("contextmenu", onContextMenu);

    async function initialize() {
      await fitWindowToMonitor();
      await refreshMaximizedState();

      processing.set(true);
      processName.set("Initializing...");

      try {
        let steamPath = await invoke("get_steam_path");
        console.log(steamPath);
        gameExePath.set(steamPath as string);
        isPathValid.set(true);
      } catch (err) {
        console.log(err);
      }

      try {
        let hasDotnet = await invoke("is_dotnet11_installed");
        console.log(hasDotnet);
        isDotnetInstalled.set(hasDotnet as boolean);
      } catch (err) {
        console.log(err);
      } finally {
        processing.set(false);
        processName.set("");
      }
    }

    void initialize();
    return () => {
      window.removeEventListener("resize", onResize);
      document.removeEventListener("contextmenu", onContextMenu);
    };
  });
</script>

<main class:maximized={isWindowMaximized}>
  <div class="app-frame">
    <div class="titlebar" data-tauri-drag-region role="presentation" on:mousedown={startWindowDrag}>
      <div class="window-brand" data-tauri-drag-region>
        <img class="window-mark" src="/modstoke-mark.svg" alt="" draggable="false" />
        <span class="window-wordmark" data-tauri-drag-region>
          <span class="window-title" data-tauri-drag-region>Modstoke Manager</span>
          <span class="window-subtitle" data-tauri-drag-region>For Sons of the Forest <span class="window-credit">by OpenACAI</span></span>
        </span>
      </div>
      <div class="window-controls">
        <button class="window-button" aria-label="Minimize" title="Minimize" on:click={(event) => { event.stopPropagation(); minimizeWindow(); }}>-</button>
        <button class="window-button" aria-label="Maximize" title="Maximize" on:click={(event) => { event.stopPropagation(); toggleMaximizeWindow(); }}>□</button>
        <button class="window-button close" aria-label="Close" title="Close" on:click={(event) => { event.stopPropagation(); closeWindow(); }}>x</button>
      </div>
    </div>

  <div class="tabs" role="tablist" aria-label="Main navigation">
    {#each tabs as tab}
      <button
        type="button"
        id="tab-{tab.id}"
        class="tab {tab.id === activeTab ? 'activetab' : ''}"
        tabindex={tab.id === activeTab ? 0 : -1}
        role="tab"
        aria-selected={tab.id === activeTab}
        aria-controls="panel-{tab.id}"
        bind:this={tabButtons[tab.id]}
        on:click={() => selectTab(tab.id)}
        on:keydown={(event) => handleTabKeyDown(event, tab.id)}
      >
        {tab.label}
      </button>
    {/each}
  </div>

  <div class="container" id="panel-main" role="tabpanel" aria-labelledby="tab-main" hidden={activeTab !== "main"} tabindex="0">
    {#if activeTab === "main"}
      <Page1 />
    {/if}
  </div>
  <div class="container" id="panel-mods" role="tabpanel" aria-labelledby="tab-mods" hidden={activeTab !== "mods"} tabindex="0">
    {#if modsPage}
      {#await modsPage}
        <div class="page-status" role="status">Loading Mods...</div>
      {:then module}
        <module.default />
      {:catch error}
        <div class="page-status" role="alert">
          <strong>Mods could not be loaded.</strong>
          <p>{pageLoadError(error)}</p>
          <button type="button" on:click={() => { modsPage = import("./pages/ModHub.svelte"); }}>Retry loading Mods</button>
        </div>
      {/await}
    {/if}
  </div>
  <div class="container" id="panel-modding" role="tabpanel" aria-labelledby="tab-modding" hidden={activeTab !== "modding"} tabindex="0">
    {#if moddingPage}
      {#await moddingPage}
        <div class="page-status" role="status">Loading Mod Creation...</div>
      {:then module}
        <module.default />
      {:catch error}
        <div class="page-status" role="alert">
          <strong>Mod Creation could not be loaded.</strong>
          <p>{pageLoadError(error)}</p>
          <button type="button" on:click={() => { moddingPage = import("./pages/Modding.svelte"); }}>Retry loading Mod Creation</button>
        </div>
      {/await}
    {/if}
  </div>
  </div>

  {#if $processing}
    <div class="loading-overlay" transition:fade={{ delay: 0, duration: 150 }}>
      {$processName}
      <div class="progress-bar">
        <div class="progress" style="width: {$processProgress}%"></div>
      </div>
    </div>
  {/if}
</main>

<style>
  .container[hidden] {
    display: none;
  }

  .tab {
    border: 0;
    box-shadow: none;
    box-sizing: content-box;
    line-height: inherit;
    margin: 0;
    -webkit-mask-image: none;
    mask-image: none;
  }

  .tab:not(.activetab):not(:hover) {
    background: transparent;
  }

  .tab.activetab {
    border-bottom: 2px solid rgba(255, 255, 255, 0.75);
  }

  .tab:focus-visible,
  .page-status button:focus-visible {
    outline: 2px solid #e5e5e5;
    outline-offset: -3px;
  }

  .page-status {
    margin: auto;
    padding: 24px;
    text-align: center;
  }

  .page-status p {
    color: #c4c4c4;
    overflow-wrap: anywhere;
  }

  .app-frame {
    --frame-pad-x: clamp(18px, calc(1.65vw + 0.35vh), 52px);
    --frame-pad-top: clamp(22px, calc(0.7vw + 1.7vh), 50px);
    --frame-pad-bottom: clamp(12px, calc(0.55vw + 1.35vh), 32px);
    --titlebar-height: clamp(58px, 8vh, 82px);
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    height: 100vh;
    min-height: 0;
    padding:
      var(--frame-pad-top)
      var(--frame-pad-x)
      var(--frame-pad-bottom);
  }

  .titlebar {
    align-items: center;
    display: flex;
    flex: 0 0 auto;
    justify-content: space-between;
    min-height: var(--titlebar-height);
    overflow: visible;
    padding: 0 clamp(8px, 0.72vw, 18px) 0 clamp(8px, 0.82vw, 20px);
    user-select: none;
  }

  .window-brand {
    align-items: center;
    display: flex;
    gap: 0.7rem;
    max-width: min(70vw, 760px);
    min-width: 0;
  }

  .window-mark {
    height: clamp(34px, 5.8vh, 48px);
    width: clamp(34px, 5.8vh, 48px);
    pointer-events: none;
  }

  .window-wordmark {
    display: flex;
    flex-direction: column;
    gap: 0.28rem;
  }

  .window-title {
    color: #f2eee7;
    font-size: clamp(1rem, 1.5vw, 1.3rem);
    font-weight: 700;
    letter-spacing: 0.025em;
    line-height: 1.1;
  }

  .window-subtitle {
    color: #bcbcb8;
    font-size: 0.72rem;
    line-height: 1.3;
  }

  .window-credit {
    color: #f2a64b;
    margin-left: 0.7rem;
    white-space: nowrap;
  }

  .window-controls {
    align-items: center;
    display: flex;
    gap: 4px;
    flex: 0 0 auto;
  }

  .window-button {
    align-items: center;
    border: 0;
    display: inline-flex;
    height: clamp(28px, calc(1.1vw + 1.1vh), 38px);
    justify-content: center;
    margin: 0;
    padding: 0;
    width: clamp(34px, calc(1.35vw + 1.2vh), 46px);
  }

  .window-button.close:hover {
    background: rgba(132, 30, 30, 0.92);
  }

  .loading-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: rgba(0, 0, 0, 0.7);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    color: white;
    font-size: 24px;
    gap: 20px;
    z-index: 20;
  }

  .progress-bar {
    background: rgba(255, 255, 255, 0.2);
    width: 70%;
    height: 20px;
    border-radius: 2px;
    overflow: hidden;
  }

  .progress {
    height: 100%;
    background-color: #e5e5e5;
    transition: width 0.3s;
  }

  @media (max-width: 900px) {
    .app-frame {
      --frame-pad-x: 22px;
      --frame-pad-top: 20px;
      --frame-pad-bottom: 24px;
      --titlebar-height: clamp(50px, 7.2vh, 66px);
    }

    .titlebar {
      min-height: clamp(50px, 7.2vh, 66px);
    }

    .window-subtitle {
      font-size: 0.62rem;
    }
  }

  @media (max-height: 780px) {
    .titlebar {
      min-height: 46px;
    }
  }

  @media (max-height: 720px) {
    .app-frame {
      --frame-pad-x: 18px;
      --frame-pad-top: 16px;
      --frame-pad-bottom: 10px;
      --titlebar-height: 48px;
    }

    .window-subtitle {
      font-size: 0.68rem;
    }
  }
</style>
