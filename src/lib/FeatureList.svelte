<script lang="ts">
    import InstallFeature from "../lib/InstallationComponent.svelte";
    import { isPathValid } from "../lib/store";
    import type { FeatureInstaller } from "./featureInstaller";

    export let features: FeatureInstaller[] = [];
</script>

<div class="column feature-list">
    {#if $isPathValid}
        {#each features as feature}
            <InstallFeature feature={feature} />
        {/each}
    {:else}
        <div class="unavailable" role="status">
            <strong>Loaders and tools</strong>
            <p>Select a valid game installation to manage loaders and tools.</p>
        </div>
    {/if}
</div>

<style>
    .feature-list {
        align-items: stretch;
        gap: 12px;
        min-width: 0;
        width: 100%;
    }

    .unavailable {
        background: var(--surface-panel, #171a21);
        border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.08));
        border-radius: 10px;
        color: var(--text-secondary, #9da6b6);
        font-size: 13px;
        line-height: 1.5;
        margin: 0;
        padding: 18px;
        text-align: left;
    }

    .unavailable strong {
        color: var(--text-primary, #eef0f4);
        font-size: 14px;
        font-weight: 600;
    }

    .unavailable p {
        margin: 5px 0 0;
    }
</style>
