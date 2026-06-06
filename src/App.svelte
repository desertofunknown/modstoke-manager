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
  import Page2 from "./pages/Mods.svelte";
  import Page3 from "./pages/NexusVortex.svelte";
  import Page4 from "./pages/Modding.svelte";
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { invoke } from "@tauri-apps/api/core";
  import { currentMonitor, getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";

  let showOverlay: boolean = false;

  processing.subscribe((value) => {
    showOverlay = value;
  });

  type TabId = "main" | "mods" | "vortex" | "modding";

  type Tab = {
    id: TabId;
    label: string;
  };

  let tabs: Tab[] = [
    { id: "main", label: "Main" },
    { id: "mods", label: "Mods" },
    { id: "vortex", label: "Vortex" },
    { id: "modding", label: "Mod Creation" },
  ];

  let activeTab: TabId = "main";
  let isWindowMaximized = false;

  const MIN_WINDOW_WIDTH = 980;
  const MIN_WINDOW_HEIGHT = 680;
  const MAX_WINDOW_WIDTH = 1360;
  const MAX_WINDOW_HEIGHT = 900;

  function selectTab(tab: TabId) {
    activeTab = tab;
  }

  function handleKeyPress(event: KeyboardEvent, tab: TabId) {
    if (event.key === "Enter" || event.key === " ") {
      selectTab(tab);
    }
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

      const targetWidth = Math.round(clampNumber(workArea.width * 0.86, minWidth, maxWidth));
      const targetHeight = Math.round(clampNumber(workArea.height * 0.88, minHeight, maxHeight));

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

  onMount(async () => {
    await fitWindowToMonitor();
    await refreshMaximizedState();

    const onResize = () => {
      void refreshMaximizedState();
    };
    window.addEventListener("resize", onResize);

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

    document.addEventListener("contextmenu", (event) => event.preventDefault());
  });
</script>

<main class:maximized={isWindowMaximized}>
  <div class="app-frame">
    <div class="titlebar" data-tauri-drag-region role="presentation" on:mousedown={startWindowDrag}>
      <div class="window-brand" data-tauri-drag-region>
        <span class="studio-logo" aria-label="Endnight" data-tauri-drag-region>
          <span class="studio-logo-image" aria-hidden="true" data-tauri-drag-region></span>
        </span>
        <span class="window-subtitle" data-tauri-drag-region>OpenACAI Mod Manager - an Endnight Games mod manager</span>
        <span class="window-credit" data-tauri-drag-region>OpenACAI Inc / Alex Cooper</span>
      </div>
      <div class="window-controls">
        <button class="window-button" aria-label="Minimize" title="Minimize" on:click={(event) => { event.stopPropagation(); minimizeWindow(); }}>-</button>
        <button class="window-button" aria-label="Maximize" title="Maximize" on:click={(event) => { event.stopPropagation(); toggleMaximizeWindow(); }}>□</button>
        <button class="window-button close" aria-label="Close" title="Close" on:click={(event) => { event.stopPropagation(); closeWindow(); }}>x</button>
      </div>
    </div>

  <div class="tabs">
    {#each tabs as tab}
      <div
        class="tab {tab.id === activeTab ? 'activetab' : ''}"
        tabindex="0"
        role="button"
        on:click={() => selectTab(tab.id)}
        on:keydown={(e) => handleKeyPress(e, tab.id)}
      >
        {tab.label}
      </div>
    {/each}
  </div>

  <div class="container">
    {#if activeTab === "main"}
      <Page1 />
    {:else if activeTab === "mods"}
      <Page2 />
    {:else if activeTab === "vortex"}
      <Page3 />
    {:else}
      <Page4 />
    {/if}
  </div>
  </div>

  {#if showOverlay}
    <div class="loading-overlay" transition:fade={{ delay: 0, duration: 150 }}>
      {$processName}
      <div class="progress-bar">
        <div class="progress" style="width: {$processProgress}%"></div>
      </div>
    </div>
  {/if}
</main>

<style>
  .app-frame {
    --frame-pad-x: clamp(18px, calc(1.65vw + 0.35vh), 52px);
    --frame-pad-top: clamp(22px, calc(0.7vw + 1.7vh), 50px);
    --frame-pad-bottom: clamp(12px, calc(0.55vw + 1.35vh), 32px);
    --logo-aspect: 2;
    --logo-height: clamp(24px, min(3.35vw, 5.4vh), 48px);
    --logo-width: calc(var(--logo-height) * var(--logo-aspect));
    --logo-bleed: calc(var(--logo-height) * 0.22);
    --titlebar-height: clamp(58px, calc(var(--logo-height) + 34px), 92px);
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
    align-items: flex-start;
    display: flex;
    flex-direction: column;
    gap: 0.12em;
    max-width: min(60vw, 760px);
    min-width: 0;
  }

  .studio-logo {
    align-items: center;
    display: inline-flex;
    isolation: isolate;
    line-height: 1;
    margin: var(--logo-bleed) 0 calc(var(--logo-bleed) * 0.34);
    max-width: min(var(--logo-width), 42vw);
    min-height: var(--logo-height);
    overflow: visible;
    position: relative;
    text-transform: uppercase;
    width: var(--logo-width);
  }

  .studio-logo::before {
    animation: endnight-logo-bloom 2.6s ease-in-out infinite;
    background-image: var(--sons-endnight-logo-blur);
    background-position: left center;
    background-repeat: no-repeat;
    background-size: 100% 100%;
    content: "";
    filter: brightness(1.32) contrast(1.1) saturate(1.18) blur(1.5px);
    height: calc(var(--logo-height) + (var(--logo-bleed) * 2));
    left: calc(var(--logo-bleed) * -0.65);
    mix-blend-mode: screen;
    opacity: 0.46;
    pointer-events: none;
    position: absolute;
    top: calc(var(--logo-bleed) * -0.95);
    width: calc(var(--logo-width) + (var(--logo-bleed) * 1.3));
    z-index: 0;
  }

  .studio-logo::after {
    animation: endnight-logo-scan 4.8s cubic-bezier(0.2, 0.9, 0.32, 1) infinite;
    background:
      linear-gradient(90deg, transparent 0%, rgba(255, 255, 255, 0.0) 27%, rgba(255, 255, 255, 0.54) 48%, rgba(84, 229, 255, 0.28) 54%, transparent 76%),
      linear-gradient(0deg, transparent 0%, rgba(255, 255, 255, 0.24) 46%, transparent 52%);
    content: "";
    height: calc(var(--logo-height) * 1.05);
    inset: calc(var(--logo-bleed) * -0.12) calc(var(--logo-bleed) * -0.32);
    mix-blend-mode: screen;
    opacity: 0;
    pointer-events: none;
    position: absolute;
    transform: translateX(calc(var(--logo-width) * -0.72)) skewX(-10deg);
    z-index: 4;
  }

  .studio-logo-image {
    background-image: var(--sons-endnight-logo);
    background-position: left center;
    background-repeat: no-repeat;
    background-size: 100% 100%;
    display: inline-block;
    filter:
      brightness(1.14)
      contrast(1.15)
      drop-shadow(calc(var(--logo-height) * -0.045) 0 rgba(255, 40, 67, 0.95))
      drop-shadow(calc(var(--logo-height) * 0.045) 0 rgba(62, 242, 255, 0.92))
      drop-shadow(0 0 9px rgba(255, 255, 255, 0.28));
    height: var(--logo-height);
    isolation: isolate;
    position: relative;
    transform: translateZ(0);
    width: var(--logo-width);
    z-index: 1;
  }

  .studio-logo-image::before,
  .studio-logo-image::after {
    background-image: var(--sons-endnight-logo);
    background-position: left center;
    background-repeat: no-repeat;
    background-size: 100% 100%;
    content: "";
    inset: 0;
    mix-blend-mode: screen;
    opacity: 0.58;
    pointer-events: none;
    position: absolute;
  }

  .studio-logo-image::before {
    animation: endnight-logo-red-shift 1.22s steps(2, end) infinite;
    filter: brightness(1.45) sepia(1) saturate(4) hue-rotate(304deg);
    transform: translateX(calc(var(--logo-height) * -0.045));
  }

  .studio-logo-image::after {
    animation: endnight-logo-cyan-shift 1.04s steps(2, end) infinite;
    filter: brightness(1.5) saturate(3) hue-rotate(128deg);
    transform: translateX(calc(var(--logo-height) * 0.05));
  }

  @keyframes endnight-logo-red-shift {
    0%,
    100% {
      opacity: 0.38;
      transform: translateX(calc(var(--logo-height) * -0.035));
    }

    28% {
      opacity: 0.72;
      transform: translateX(calc(var(--logo-height) * -0.07)) translateY(-1px);
    }

    63% {
      opacity: 0.28;
      transform: translateX(calc(var(--logo-height) * -0.018));
    }
  }

  @keyframes endnight-logo-bloom {
    0%,
    100% {
      opacity: 0.34;
      transform: scale(0.985);
    }

    24% {
      opacity: 0.66;
      transform: scale(1.025);
    }

    52% {
      opacity: 0.44;
      transform: scale(1.01) translateX(calc(var(--logo-height) * 0.015));
    }

    78% {
      opacity: 0.58;
      transform: scale(1.035) translateX(calc(var(--logo-height) * -0.012));
    }
  }

  @keyframes endnight-logo-scan {
    0%,
    58%,
    100% {
      opacity: 0;
      transform: translateX(calc(var(--logo-width) * -0.72)) skewX(-10deg);
    }

    64% {
      opacity: 0.42;
    }

    70% {
      opacity: 0.9;
      transform: translateX(calc(var(--logo-width) * 0.28)) skewX(-10deg);
    }

    76% {
      opacity: 0.08;
      transform: translateX(calc(var(--logo-width) * 0.78)) skewX(-10deg);
    }
  }

  @keyframes endnight-logo-cyan-shift {
    0%,
    100% {
      opacity: 0.42;
      transform: translateX(calc(var(--logo-height) * 0.036));
    }

    34% {
      opacity: 0.76;
      transform: translateX(calc(var(--logo-height) * 0.078)) translateY(1px);
    }

    71% {
      opacity: 0.3;
      transform: translateX(calc(var(--logo-height) * 0.018));
    }
  }

  .window-subtitle {
    color: #c4c4c4;
    font-size: clamp(0.62rem, calc(0.31vw + 0.22vh), 0.86rem);
    font-weight: 700;
    letter-spacing: 0.075em;
    line-height: 1.1;
    text-transform: uppercase;
  }

  .window-credit {
    color: #777;
    font-size: clamp(0.52rem, calc(0.25vw + 0.18vh), 0.72rem);
    font-weight: 800;
    letter-spacing: 0.14em;
    line-height: 1.1;
    text-transform: uppercase;
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
      --logo-height: 24px;
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

    .window-credit {
      display: none;
    }
  }

  @media (max-height: 720px) {
    .app-frame {
      --frame-pad-x: 18px;
      --frame-pad-top: 16px;
      --frame-pad-bottom: 10px;
      --logo-height: 22px;
      --titlebar-height: 48px;
    }

    .window-subtitle {
      font-size: 0.58rem;
    }
  }
</style>
