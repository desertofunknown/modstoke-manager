<script lang="ts">
    import InstallFeature from "../lib/InstallationComponent.svelte";
    import { isPathValid } from "../lib/store";
    import type { FeatureInstaller } from "./featureInstaller";

    export let features: FeatureInstaller[] = [];
</script>

<div class="column">
    {#if $isPathValid}
        {#each features as feature}
            <InstallFeature feature={feature} />
        {/each}
    {:else}
        <p class="unavailable" role="status">Select a valid game installation to manage loaders and tools.</p>
    {/if}
</div>

<style>
    .unavailable {
        margin: 0;
        color: #a2a2a2;
        font-size: 0.9em;
        text-align: center;
    }
</style>
