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
    <div class="horizontal status-actions">
        <button type="button" {disabled} aria-label={subject ? `Update ${subject}` : "Update"} class="update button-wrapper" on:click={update}>Update</button>
        <button type="button" {disabled} aria-label={subject ? `Uninstall ${subject}` : "Uninstall"} class="uninstall button-wrapper" on:click={uninstall}>Uninstall</button>
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
        align-items: center;
        display: flex;
        flex-wrap: wrap;
        gap: 8px;
        min-width: 0;
        width: auto;
    }

    .button-wrapper {
        background: var(--surface-raised, #20242c);
        border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.08));
        border-radius: 7px;
        box-sizing: border-box;
        color: var(--text-primary, #eef0f4);
        font-size: 12px;
        font-weight: 600;
        line-height: 1.25;
        margin: 0;
        min-height: 32px;
        min-width: 0;
        overflow-wrap: anywhere;
        padding: 7px 12px;
        white-space: normal;
        width: auto;
    }

    .install,
    .update {
        background: var(--accent, #eeb66b);
        border-color: var(--accent, #eeb66b);
        color: #201b15;
    }

    .install:enabled:hover,
    .update:enabled:hover {
        background: #f3c58b;
        border-color: #f3c58b;
        color: #201b15;
    }

    .uninstall:enabled:hover {
        background: rgba(214, 133, 133, 0.1);
        border-color: rgba(214, 133, 133, 0.3);
        color: #e6a4a4;
    }
</style>
