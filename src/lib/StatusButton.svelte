<script lang="ts">
    export let isUpdateAvailable: boolean;
    export let isModInstalled: boolean;
    export let disabled = false;
    export let subject = "";

    export let update: () => Promise<void>;
    export let uninstall: () => Promise<void>;
    export let install: () => Promise<void>;
</script>

{#if isUpdateAvailable}
    <div class="horizontal">
        <button type="button" {disabled} aria-label={subject ? `Update ${subject}` : "Update"} class="update btn-left button-wrapper" on:click={update}>Update</button>
        <button type="button" {disabled} aria-label={subject ? `Uninstall ${subject}` : "Uninstall"} class="uninstall btn-right button-wrapper" on:click={uninstall}>Uninstall</button>
    </div>
{:else}
    {#if isModInstalled}
        <button type="button" {disabled} aria-label={subject ? `Uninstall ${subject}` : "Uninstall"} class="uninstall button-wrapper" on:click={uninstall}>Uninstall</button>
    {:else}
        <button type="button" {disabled} aria-label={subject ? `Install ${subject}` : "Install"} class="install button-wrapper" on:click={install}>Install</button>
    {/if}
{/if}

  
<style>
    .horizontal {
        display: flex;
        flex-direction: row;
        flex-wrap: wrap;
        align-items: stretch;
        gap: 0.35em;
        min-width: 0;
        width: 100%;
    }

    .horizontal > .button-wrapper {
        flex: 1 1 5.5em;
        width: auto;
    }

    .button-wrapper {
        width: 100%;
        box-sizing: border-box;
        min-width: 0;
        overflow-wrap: anywhere;
        white-space: normal;
        margin-bottom: 0;
    }
</style>
