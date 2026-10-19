<script lang="ts">
  import {
    isDotnetInstalled,
    processName,
    processProgress,
    processing,
  } from "./lib/store";
  import Page1 from "./pages/MainPage.svelte";
  import OperationStatus from "./lib/OperationStatus.svelte";
  import LucideMinus from "~icons/lucide/minus";
  import LucideSquare from "~icons/lucide/square";
  import LucideCopy from "~icons/lucide/copy";
  import LucideX from "~icons/lucide/x";
  import LucideLayoutDashboard from "~icons/lucide/layout-dashboard";
  import LucideLibrary from "~icons/lucide/library";
  import LucideServer from "~icons/lucide/server";
  import LucidePackage from "~icons/lucide/package";
  import LucideLayers from "~icons/lucide/layers";
  import LucideUsers from "~icons/lucide/users";
  import LucideCode from "~icons/lucide/code-2";
  import LucideTrees from "~icons/lucide/trees";
  import { initializeInstallationProfiles, selectedInstallation } from "./lib/installationProfiles";
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { currentMonitor, getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";

  type TabId = "main" | "installed" | "mods" | "loaders" | "servers" | "multiplayer" | "modding";
  type Tab = { id: TabId; label: string; group: string };
  const tabs: Tab[] = [
    { id: "main", label: "Overview", group: "Game" },
    { id: "installed", label: "Installed mods", group: "Game" },
    { id: "mods", label: "Browse mods", group: "Game" },
    { id: "loaders", label: "Loaders", group: "Toolkit" },
    { id: "servers", label: "Dedicated servers", group: "Toolkit" },
    { id: "multiplayer", label: "Multiplayer", group: "Toolkit" },
    { id: "modding", label: "Creator tools", group: "Toolkit" },
  ];
  $: serverSelected = $selectedInstallation?.target.kind === "server";
  let activeTab: TabId = "main";
  let catalogRevision = 0;
  let displayedCatalogRevision = 0;
  let tabButtons: Partial<Record<TabId, HTMLButtonElement>> = {};
  let modsPage: Promise<typeof import("./pages/ModHub.svelte")> | null = null;
  let moddingPage: Promise<typeof import("./pages/Modding.svelte")> | null = null;
  let installedPage: Promise<typeof import("./pages/InstalledMods.svelte")> | null = null;
  let serversPage: Promise<typeof import("./pages/Servers.svelte")> | null = null;
  let isWindowMaximized = false;

  const MIN_WINDOW_WIDTH = 980;
  const MIN_WINDOW_HEIGHT = 680;
  const MAX_WINDOW_WIDTH = 1920;
  const MAX_WINDOW_HEIGHT = 1240;

  function selectTab(tab: TabId) {
    if (!tabs.some(item => item.id === tab)) return;
    if (tab === "mods" && !modsPage) {
      modsPage = import("./pages/ModHub.svelte");
    } else if (tab === "modding" && !moddingPage) {
      moddingPage = import("./pages/Modding.svelte");
    }
    if (tab === "installed" && !installedPage) installedPage = import("./pages/InstalledMods.svelte");
    if (tab === "servers" && !serversPage) serversPage = import("./pages/Servers.svelte");
    if (tab === "mods") displayedCatalogRevision = catalogRevision;
    activeTab = tab;
  }

  function handleTabKeyDown(event: KeyboardEvent, tab: TabId) {
    const index = tabs.findIndex((item) => item.id === tab);
    let nextIndex: number;
    switch (event.key) {
      case "ArrowLeft":
      case "ArrowUp":
        nextIndex = (index + tabs.length - 1) % tabs.length;
        break;
      case "ArrowRight":
      case "ArrowDown":
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
      await refreshMaximizedState();
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
    const onContextMenu = (event: MouseEvent) => {
      const target = event.target;
      if (!(target instanceof HTMLElement) || !target.closest('input, textarea, [contenteditable="true"]')) {
        event.preventDefault();
      }
    };
    window.addEventListener("resize", onResize);
    document.addEventListener("contextmenu", onContextMenu);

    async function initialize() {
      await fitWindowToMonitor();
      await refreshMaximizedState();

      processing.set(true);
      processName.set("Initializing...");
      processProgress.set(0);

      await initializeInstallationProfiles();

      try {
        isDotnetInstalled.set(await invoke<boolean>("is_dotnet11_installed"));
      } catch (err) {
        console.log(err);
      } finally {
        processing.set(false);
        processName.set("");
        processProgress.set(0);
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
    <div class="titlebar" class:operation-active={$processing} data-tauri-drag-region role="presentation" on:mousedown={startWindowDrag}>
      <div class="window-brand" data-tauri-drag-region>
        <img class="window-mark" src="/modstoke-mark.svg" alt="" draggable="false" />
        <span class="window-wordmark" data-tauri-drag-region>
          <span class="window-title" data-tauri-drag-region>Modstoke Manager</span>
          <span class="window-credit" data-tauri-drag-region>by OpenACAI</span>
        </span>
      </div>
      <div class="window-controls">
        <button class="window-button" aria-label="Minimize" title="Minimize" on:click={(event) => { event.stopPropagation(); minimizeWindow(); }}><LucideMinus aria-hidden="true" /></button>
        <button class="window-button" aria-label={isWindowMaximized ? "Restore window" : "Maximize"} title={isWindowMaximized ? "Restore window" : "Maximize"} on:click={(event) => { event.stopPropagation(); toggleMaximizeWindow(); }}>
          {#if isWindowMaximized}<LucideCopy aria-hidden="true" />{:else}<LucideSquare aria-hidden="true" />{/if}
        </button>
        <button class="window-button close" aria-label="Close" title="Close" on:click={(event) => { event.stopPropagation(); closeWindow(); }}><LucideX aria-hidden="true" /></button>
      </div>
    </div>

  <div class="workspace" inert={$processing} aria-busy={$processing}>
  <button class="skip-content" on:click={() => document.getElementById(`panel-${activeTab}`)?.focus()}>Skip to content</button>
  <aside class="sidebar" aria-label="Workspace">
    <div class="sidebar-brand">
      <img src="/modstoke-mark.svg" alt="" draggable="false" />
      <span>Modstoke<small>Manager</small></span>
    </div>
    <div class="game-context">
      <span class="game-icon"><LucideTrees aria-hidden="true" /></span>
      <span class="game-label">Sons of the Forest<small>{serverSelected ? "Dedicated server" : "Game workspace"}</small></span>
    </div>
  <div class="tabs" role="tablist" aria-label="Main navigation" aria-orientation="vertical">
    {#each tabs as tab, index}
      {#if index === 0 || tabs[index - 1].group !== tab.group}<span class="navigation-label" role="presentation">{tab.group}</span>{/if}
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
        {#if tab.id === "main"}<LucideLayoutDashboard aria-hidden="true" />{:else if tab.id === "mods"}<LucideLibrary aria-hidden="true" />{:else if tab.id === "installed"}<LucidePackage aria-hidden="true" />{:else if tab.id === "servers"}<LucideServer aria-hidden="true" />{:else if tab.id === "loaders"}<LucideLayers aria-hidden="true" />{:else if tab.id === "multiplayer"}<LucideUsers aria-hidden="true" />{:else}<LucideCode aria-hidden="true" />{/if}
        <span>{tab.label}</span>
      </button>
    {/each}
  </div>
    <div class="sidebar-footer">
      <span>by OpenACAI</span>
      <small>Game & server toolkit</small>
      <small>modstoke.com <span class="planned-site">planned</span></small>
    </div>
  </aside>

  <div class="page-workspace">
  <div class="container" id="panel-main" role="tabpanel" aria-labelledby="tab-main" hidden={activeTab !== "main"} tabindex="0">
    {#if activeTab === "main"}
      <Page1 on:navigate={(event) => selectTab(event.detail.id)} />
    {/if}
  </div>
  {#each ["loaders", "multiplayer"] as section}
    <div class="container" id="panel-{section}" role="tabpanel" aria-labelledby="tab-{section}" hidden={activeTab !== section} tabindex="0">
      {#if activeTab === section}<Page1 view={section === "loaders" ? "loaders" : "multiplayer"} on:navigate={(event) => selectTab(event.detail.id)} />{/if}
    </div>
  {/each}
  <div class="container" id="panel-installed" role="tabpanel" aria-labelledby="tab-installed" hidden={activeTab !== "installed"} tabindex="0">
    {#if activeTab === "installed" && installedPage}
      {#await installedPage}<div class="page-status" role="status">Loading installed mods…</div>
      {:then module}<module.default on:navigate={(event) => selectTab(event.detail.id)} on:changed={() => catalogRevision++} />
      {:catch error}<div class="page-status" role="alert"><p>{pageLoadError(error)}</p><button on:click={() => installedPage = import("./pages/InstalledMods.svelte")}>Retry</button></div>{/await}
    {/if}
  </div>
  <div class="container" id="panel-servers" role="tabpanel" aria-labelledby="tab-servers" hidden={activeTab !== "servers"} tabindex="0">
    {#if activeTab === "servers" && serversPage}
      {#await serversPage}<div class="page-status" role="status">Loading dedicated servers…</div>
      {:then module}<module.default />
      {:catch error}<div class="page-status" role="alert"><p>{pageLoadError(error)}</p><button on:click={() => serversPage = import("./pages/Servers.svelte")}>Retry</button></div>{/await}
    {/if}
  </div>
  <div class="container" id="panel-mods" role="tabpanel" aria-labelledby="tab-mods" hidden={activeTab !== "mods"} tabindex="0">
    {#if modsPage}
      {#await modsPage}
        <div class="page-status" role="status">Loading mod library…</div>
      {:then module}
        {#key displayedCatalogRevision}<module.default />{/key}
      {:catch error}
        <div class="page-status" role="alert">
          <strong>The mod library could not be loaded.</strong>
          <p>{pageLoadError(error)}</p>
          <button type="button" on:click={() => { modsPage = import("./pages/ModHub.svelte"); }}>Retry loading library</button>
        </div>
      {/await}
    {/if}
  </div>
  <div class="container" id="panel-modding" role="tabpanel" aria-labelledby="tab-modding" hidden={activeTab !== "modding"} tabindex="0">
    {#if moddingPage && activeTab === "modding"}
      {#await moddingPage}
        <div class="page-status" role="status">Loading creator tools…</div>
      {:then module}
        <module.default />
      {:catch error}
        <div class="page-status" role="alert">
          <strong>Creator tools could not be loaded.</strong>
          <p>{pageLoadError(error)}</p>
          <button type="button" on:click={() => { moddingPage = import("./pages/Modding.svelte"); }}>Retry loading creator tools</button>
        </div>
      {/await}
    {/if}
  </div>
  </div>
  </div>
  {#if $processing}
    <OperationStatus name={$processName} progress={$processProgress} />
  {/if}
  </div>
</main>

<style>
  .container[hidden] { display: none; }

  .app-frame {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    overflow: hidden;
  }

  .titlebar {
    align-items: center;
    background: #0d0f13;
    border-bottom: 1px solid var(--border-subtle);
    display: flex;
    flex: 0 0 38px;
    justify-content: space-between;
    min-width: 0;
    padding-left: 14px;
    position: relative;
    user-select: none;
    z-index: 0;
  }

  .titlebar.operation-active { z-index: 101; }
  .window-brand, .window-wordmark, .window-controls { display: flex; align-items: center; }
  .window-brand { gap: 8px; pointer-events: none; }
  .window-mark { width: 20px; height: 20px; }
  .window-wordmark { gap: 12px; }
  .window-title { color: #b6bdc9; font-size: 12px; font-weight: 600; }
  .window-credit { color: #737d8e; font-size: 11px; }
  .window-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 46px;
    height: 38px;
    min-height: 0;
    border: 0;
    border-radius: 0;
    box-shadow: none;
    background: transparent;
    padding: 0;
    margin: 0;
    color: #a9b1bf;
    font-size: 14px;
  }
  .window-button.close:enabled:hover { background: #ae353f; color: white; }

  .workspace {
    display: flex;
    flex: 1 1 auto;
    min-height: 0;
    min-width: 0;
    position: relative;
    isolation: isolate;
  }

  .sidebar {
    background: #12151b;
    border-right: 1px solid var(--border-subtle);
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    flex: 0 0 202px;
    padding: 24px 12px 18px;
    min-height: 0;
    overflow-y: auto;
  }

  .sidebar-brand { display: flex; align-items: center; gap: 10px; padding: 0 8px 26px; }
  .sidebar-brand img { width: 36px; height: 36px; }
  .sidebar-brand > span { font-size: 20px; line-height: 1.2; font-weight: 700; letter-spacing: -0.6px; }
  .sidebar-brand small { display: block; margin-top: 4px; color: var(--text-secondary); font-size: 11px; font-weight: 500; letter-spacing: 0.7px; }
  .game-context { display: flex; align-items: center; gap: 9px; padding: 12px 8px; margin-bottom: 6px; border-top: 1px solid var(--border-subtle); border-bottom: 1px solid var(--border-subtle); }
  .game-icon { display: flex; color: #9aab9c; font-size: 23px; }
  .game-label { color: #d7dce5; font-size: 12px; line-height: 1.4; font-weight: 600; }
  .game-label small { display: block; color: var(--text-secondary); font-size: 11px; margin-top: 3px; font-weight: 400; }
  .navigation-label { padding: 16px 12px 5px; font-size: 10px; font-weight: 600; letter-spacing: 1.3px; text-transform: uppercase; color: #7d8797; }
  .tabs { display: flex; flex-direction: column; gap: 5px; }
  .tab {
    align-items: center;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 8px;
    box-shadow: none;
    color: var(--text-secondary);
    display: flex;
    font-size: 12px;
    font-weight: 500;
    gap: 9px;
    line-height: 1.4;
    margin: 0;
    min-height: 42px;
    padding: 10px 12px;
    text-align: left;
    width: 100%;
  }
  .tab :global(svg) { flex-shrink: 0; width: 18px; height: 18px; }
  .tab:enabled:hover { color: var(--text-primary); background: var(--surface-raised); border-color: transparent; }
  .tab.activetab { color: var(--accent); background: var(--accent-soft); border-color: rgba(238, 182, 107, 0.13); }
  .sidebar-footer { display: flex; flex-direction: column; gap: 5px; margin-top: auto; padding: 32px 10px 0; color: #9ba5b4; font-size: 11px; }
  .sidebar-footer small { font-size: 10px; color: #7d8797; }
  .planned-site { color: #77808d; margin-left: 4px; }

  .page-workspace { display: flex; flex: 1 1 0; min-width: 0; min-height: 0; }
  .skip-content { position: absolute; top: 6px; left: 214px; z-index: 2; transform: translateY(-200vh); }
  .skip-content:focus-visible { transform: none; }
  .page-status { margin: auto; padding: 32px; max-width: 480px; text-align: center; background: var(--surface-panel); border: 1px solid var(--border-subtle); border-radius: 12px; }
  .page-status p { color: var(--text-secondary); overflow-wrap: anywhere; }

  @media (max-width: 900px) {
    .sidebar { flex-basis: 160px; padding-left: 8px; padding-right: 8px; }
    .sidebar-brand { gap: 8px; padding-left: 5px; }
    .sidebar-brand > span { font-size: 18px; }
    .sidebar-brand img { width: 30px; height: 30px; }
    .game-context { padding-left: 5px; gap: 7px; }
    .game-label { font-size: 11px; }
    .tab { gap: 9px; font-size: 12px; padding-left: 9px; }
    .skip-content { left: 170px; }
  }
</style>
