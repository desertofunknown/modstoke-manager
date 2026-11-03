<script lang="ts">
    import LucideSettings from "~icons/lucide/settings";
    import { gameSkin, SKIN_CHOICES, skinChoice } from "../lib/skins";
</script>

<div class="settings-page">
    <header class="page-heading">
        <div>
            <p class="eyebrow">Manager</p>
            <h1>Settings</h1>
            <p class="page-description">Change how Modstoke Manager looks.</p>
        </div>
        <span class="workspace-mark" aria-hidden="true"><LucideSettings /></span>
    </header>

    <fieldset class="settings-group">
        <legend>Skin</legend>
        {#each SKIN_CHOICES as choice}
            <label class="skin-option" class:selected={$skinChoice === choice.id}>
                <input type="radio" name="skin" value={choice.id} bind:group={$skinChoice} />
                <span><strong>{choice.label}</strong><small>{choice.description}</small></span>
            </label>
        {/each}
        {#if $skinChoice !== "modstoke" && $gameSkin.status === "reading"}
            <p class="skin-note">Reading the Sons of the Forest skin from your game files.</p>
        {:else if $skinChoice !== "modstoke" && $gameSkin.status === "unavailable"}
            <p class="skin-note">{$gameSkin.problem} Until the Manager can read your game files, it uses the Modstoke skin.</p>
        {/if}
    </fieldset>
</div>

<style>
    .settings-page { display: flex; flex-direction: column; gap: 22px; width: 100%; max-width: 760px; min-width: 0; text-align: left; }
    .page-heading { display: flex; align-items: center; justify-content: space-between; gap: 20px; padding: 3px 0 1px; }
    .eyebrow { margin: 0 0 7px; color: var(--text-secondary); font-size: 11px; font-weight: 600; letter-spacing: 1.1px; text-transform: uppercase; }
    h1 { margin: 0; }
    .page-description { margin: 9px 0 0; color: var(--text-secondary); font-size: 13px; }
    .workspace-mark { display: grid; place-items: center; width: 46px; height: 46px; border: 1px solid var(--border-subtle); border-radius: 10px; background: var(--surface-panel); color: var(--accent); flex-shrink: 0; }
    .workspace-mark :global(svg) { width: 22px; height: 22px; }
    .settings-group { min-width: 0; margin: 0; padding: 20px 22px 16px; background: var(--surface-panel); border: 1px solid var(--border-subtle); border-radius: 10px; }
    legend { float: left; width: 100%; margin-bottom: 10px; padding: 0; font-size: 15px; font-weight: 600; }
    .skin-option { display: flex; align-items: flex-start; gap: 12px; padding: 11px 13px; margin-bottom: 4px; border: 1px solid transparent; border-radius: 8px; cursor: pointer; }
    .skin-option:hover { background: var(--surface-raised); }
    .skin-option.selected { background: var(--accent-soft); border-color: rgba(238, 182, 107, 0.2); }
    .skin-option input { flex-shrink: 0; width: 16px; height: 16px; margin: 2px 0 0; padding: 0; accent-color: var(--accent); cursor: pointer; }
    .skin-option strong { display: block; font-size: 13px; font-weight: 600; }
    .skin-option small { display: block; margin-top: 3px; color: var(--text-secondary); font-size: 12px; }
    .skin-note { margin: 8px 13px 2px; color: var(--text-secondary); font-size: 12px; }
    @media (max-width: 760px) {
        .workspace-mark { display: none; }
        .settings-group { padding: 16px 16px 12px; }
    }
</style>
