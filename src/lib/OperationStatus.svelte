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
    <div class="operation-heading">
      <div class="operation-symbol" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 3v3m0 12v3M3 12h3m12 0h3M5.64 5.64l2.12 2.12m8.48 8.48 2.12 2.12M5.64 18.36l2.12-2.12m8.48-8.48 2.12-2.12" />
        </svg>
      </div>
      <div class="operation-eyebrow">OPERATION IN PROGRESS</div>
      <span class="activity-dot" aria-hidden="true"></span>
    </div>
    <div class="operation-body">
      <h2 aria-live="polite" aria-atomic="true">{operationName}</h2>
      <div class="progress-caption" aria-hidden="true">
        <span>{determinate ? "Progress" : "Working…"}</span>
        {#if determinate}<span class="percentage">{Math.floor(percent)}<span>%</span></span>{/if}
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
      <p class="elapsed" aria-live="off">{elapsedSeconds > 15 ? elapsedLabel : "\u00a0"}</p>
    </div>
    <div class="operation-footer">
      <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="12" r="9" />
        <path d="M12 11v5m0-9v.01" />
      </svg>
      <p class="keep-open">Keep Modstoke open while this finishes.</p>
    </div>
  </section>
</div>

<style>
  .operation-backdrop {
    position: fixed;
    inset: var(--operation-top-offset, 38px) 0 0;
    z-index: 100;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgba(7, 9, 13, 0.76);
    backdrop-filter: blur(9px);
    overflow: auto;
  }

  .operation-panel {
    box-sizing: border-box;
    width: min(100%, 440px);
    overflow: hidden;
    border: 1px solid rgba(238, 182, 107, 0.18);
    border-radius: 16px;
    background: var(--surface-panel, #171a21);
    color: var(--text-primary, #eef0f4);
    box-shadow: 0 32px 110px rgba(0, 0, 0, 0.5), inset 0 1px 0 rgba(255, 255, 255, 0.035);
  }

  .operation-heading {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 22px 26px;
    border-bottom: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.08));
    background: linear-gradient(100deg, rgba(238, 182, 107, 0.035), transparent);
  }

  .operation-symbol {
    display: grid;
    flex: 0 0 32px;
    width: 32px;
    height: 32px;
    place-items: center;
    border-radius: 8px;
    color: var(--accent, #eeb66b);
    background: var(--accent-soft, rgba(238, 182, 107, 0.12));
  }

  .operation-symbol svg {
    width: 18px;
    height: 18px;
  }

  .operation-eyebrow {
    flex: 1;
    color: var(--text-secondary, #9da6b6);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.12em;
    line-height: 1.6;
  }

  .activity-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent, #eeb66b);
    box-shadow: 0 0 0 4px var(--accent-soft, rgba(238, 182, 107, 0.12));
  }

  .operation-body { padding: 26px 28px 17px; }

  h2 {
    margin: 0 0 29px;
    color: var(--text-primary, #eef0f4);
    font-size: 23px;
    font-weight: 600;
    line-height: 1.35;
    letter-spacing: -0.025em;
    overflow-wrap: anywhere;
    text-align: left;
  }

  .progress-caption {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    align-items: baseline;
    min-height: 22px;
    margin-bottom: 12px;
    color: var(--text-secondary, #9da6b6);
    font-size: 11px;
  }

  .percentage { color: var(--accent, #eeb66b); font-size: 19px; font-weight: 600; line-height: 1.2; font-variant-numeric: tabular-nums; }
  .percentage span { margin-left: 2px; font-size: 11px; font-weight: 500; }
  .progress-track { height: 5px; overflow: hidden; border-radius: 6px; background: #2c313b; }
  .progress-fill { height: 100%; border-radius: inherit; background: linear-gradient(90deg, #c98f47, var(--accent, #eeb66b)); }
  .indeterminate .progress-fill { animation: progress-travel 1.8s ease-in-out infinite alternate; }
  .elapsed { margin: 12px 0 0; color: #818b9b; font-size: 11px; line-height: 1.5; }
  .operation-footer { display: flex; align-items: flex-start; gap: 9px; padding: 17px 28px; border-top: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.08)); background: rgba(0, 0, 0, 0.1); }
  .operation-footer svg { flex: 0 0 14px; width: 14px; height: 14px; margin-top: 2px; color: #818b9b; }
  .keep-open { margin: 0; color: var(--text-secondary, #9da6b6); font-size: 11px; line-height: 1.6; }

  @keyframes progress-travel {
    from { transform: translateX(0); }
    to { transform: translateX(213%); }
  }

  @media (prefers-reduced-motion: reduce) {
    .indeterminate .progress-fill { animation: none; transform: translateX(106%); opacity: 0.75; }
  }

  @media (max-width: 420px) {
    .operation-backdrop { padding: 16px; }
    .operation-heading { padding: 19px 20px; }
    .operation-body { padding: 24px 20px 17px; }
    .operation-footer { padding: 16px 20px; }
  }
</style>
