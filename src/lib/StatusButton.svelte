<script lang="ts">
    export let isUpdateAvailable: boolean;
    export let isModInstalled: boolean;
    export let disabled = false;

    export let update: () => Promise<void>;
    export let uninstall: () => Promise<void>;
    export let install: () => Promise<void>;
</script>

{#if isUpdateAvailable}
    <div class="horizontal">
        <button {disabled} class="update btn-left button-wrapper" on:click={update}>Update</button>
        <button {disabled} class="uninstall btn-right button-wrapper" on:click={uninstall}>Uninstall</button>
    </div>
{:else}
    {#if isModInstalled}
        <button {disabled} class="uninstall button-wrapper" on:click={uninstall}>Uninstall</button>
    {:else}
        <button {disabled} class="install button-wrapper" on:click={install}>Install</button>
    {/if}
{/if}

  
<style>
    .horizontal {
        display: flex;
        flex-direction: row;
        align-items: center;
        gap: 0.35em;
        width: 100%;
    }

    .horizontal > * {
        /* margin-right: 1em; */
        flex: 1;
    }

    .button-wrapper {
        width: 100%;
        flex: 0 0 auto;
        margin-bottom: 0;
    }
</style>
