<script lang="ts">
  import { onDestroy, onMount } from "svelte";

  export let name: string;
  export let progress: number;

  let elapsedSeconds = 0;
  let timer: ReturnType<typeof setInterval> | undefined;

  $: operationName = name.trim() || "Working…";
  $: percent = Number.isFinite(progress) ? Math.min(100, Math.max(0, progress)) : 0;
  $: determinate = percent > 0;
  $: elapsedLabel = elapsedSeconds < 60
    ? `Working for ${elapsedSeconds} seconds`
    : `Working for ${Math.floor(elapsedSeconds / 60)} ${Math.floor(elapsedSeconds / 60) === 1 ? "minute" : "minutes"}`;

  onMount(() => {
    const startedAt = Date.now();
    timer = setInterval(() => {
      elapsedSeconds = Math.floor((Date.now() - startedAt) / 1000);
    }, 1000);
  });

  onDestroy(() => {
    if (timer !== undefined) clearInterval(timer);
  });
</script>

<div class="operation-backdrop">
  <section class="operation-panel" aria-label="Operation in progress">
    <div class="operation-eyebrow"><span class="activity-dot" aria-hidden="true"></span>IN PROGRESS</div>
    <h2 aria-live="polite" aria-atomic="true">{operationName}</h2>
    <div class="progress-caption" aria-hidden="true">
      <span>{determinate ? "Progress" : "Working…"}</span>
      {#if determinate}<span class="percentage">{Math.floor(percent)}%</span>{/if}
    </div>
    <div
      class="progress-track"
      class:indeterminate={!determinate}
      role="progressbar"
      aria-label={operationName}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={determinate ? percent : undefined}
    >
      <div class="progress-fill" style:width={determinate ? `${percent}%` : "32%"}></div>
    </div>
    <p class="keep-open">Keep Modstoke open while this finishes.</p>
    <p class="elapsed" aria-live="off">{elapsedSeconds > 15 ? elapsedLabel : "\u00a0"}</p>
  </section>
</div>

<style>
  .operation-backdrop {
    position: fixed;
    inset: var(--operation-top-offset, 0px) 0 0;
    z-index: 100;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgba(7, 9, 13, 0.78);
    backdrop-filter: blur(7px);
    overflow: auto;
  }

  .operation-panel {
    box-sizing: border-box;
    width: min(100%, 430px);
    padding: 30px;
    border: 1px solid #3e3930;
    border-radius: 18px;
    background: #171a20;
    color: #f5f2eb;
    box-shadow: 0 24px 80px rgba(0, 0, 0, 0.4);
  }

  .operation-eyebrow {
    display: flex;
    align-items: center;
    gap: 9px;
    color: #f2bf70;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.12em;
  }

  .activity-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #f2bf70;
  }

  h2 {
    margin: 16px 0 25px;
    font-size: 21px;
    font-weight: 600;
    line-height: 1.4;
    overflow-wrap: anywhere;
  }

  .progress-caption {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 10px;
    color: #b9bcc5;
    font-size: 12px;
  }

  .percentage { color: #f2bf70; font-variant-numeric: tabular-nums; }
  .progress-track { height: 5px; overflow: hidden; border-radius: 6px; background: #33353d; }
  .progress-fill { height: 100%; border-radius: inherit; background: #efb966; }
  .indeterminate .progress-fill { animation: progress-travel 1.8s ease-in-out infinite alternate; }
  .keep-open { margin: 20px 0 0; color: #bec0c8; font-size: 13px; line-height: 1.5; }
  .elapsed { margin: 7px 0 0; color: #969ca9; font-size: 12px; line-height: 1.5; }

  @keyframes progress-travel {
    from { transform: translateX(0); }
    to { transform: translateX(213%); }
  }

  @media (prefers-reduced-motion: reduce) {
    .indeterminate .progress-fill { animation: none; transform: translateX(106%); opacity: 0.75; }
  }

  @media (max-width: 420px) {
    .operation-backdrop { padding: 16px; }
    .operation-panel { padding: 24px; }
  }
</style>
