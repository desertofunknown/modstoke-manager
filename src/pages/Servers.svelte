<script lang="ts">
    import LucideServer from "~icons/lucide/server";
    import LucidePackage from "~icons/lucide/package";
    import LucideArchive from "~icons/lucide/archive";
    import PathSelector from "../lib/PathSelector.svelte";
    import ServerLaunch from "../lib/ServerLaunch.svelte";
    import ServerInstallation from "../lib/ServerInstallation.svelte";
    import ServerWorldRecovery from "../lib/ServerWorldRecovery.svelte";
    import { installationProfiles, selectedInstallation, profilesReady } from "../lib/installationProfiles";
    import { processing } from "../lib/store";

    type Section = "server" | "packages" | "backups";
    let section: Section = "server";
    let activeInstallationKey = "";

    $: installation = $selectedInstallation?.target.kind === "server" ? $selectedInstallation : null;
    $: serverCount = $installationProfiles?.registry.targets.filter(target => target.kind === "server").length ?? 0;
    $: installationKey = installation
        ? JSON.stringify([installation.target.id, installation.target.executable, installation.root, installation.available]) : "";
    $: if (installationKey !== activeInstallationKey) {
        activeInstallationKey = installationKey;
        section = "server";
    }

    function selectSection(next: Section) {
        if (!$processing) section = next;
    }
</script>

<div class="servers-page">
    <header class="page-heading">
        <div>
            <p class="eyebrow">Dedicated hosting</p>
            <h1>Servers</h1>
            <p class="page-description">Manage your server settings, packages and world backups.</p>
        </div>
        <span class="workspace-mark" aria-hidden="true"><LucideServer /></span>
    </header>

    <PathSelector kind="server" />

    {#if !$profilesReady}
        <div class="empty-state" role="status"><p>Loading server installations…</p></div>
    {:else if !installation}
        <section class="empty-state" aria-labelledby="server-empty-heading">
            <span class="empty-icon" aria-hidden="true"><LucideServer /></span>
            <h2 id="server-empty-heading">{serverCount ? "Choose a dedicated server" : "Add your dedicated server"}</h2>
            <p>{serverCount ? "Select a saved server above, or add another installation." : "Connect an installed Sons of the Forest dedicated server to get started."}</p>
            <div class="setup-instruction">
                <span class="step-number" aria-hidden="true">1</span>
                <div><strong>Install the dedicated server</strong><p>Use Steam's Sons of the Forest Dedicated Server setup.</p></div>
            </div>
            <div class="setup-instruction">
                <span class="step-number" aria-hidden="true">2</span>
                <div><strong>Add its executable above</strong><p>Choose <strong>SonsOfTheForestDS.exe</strong> from the installed server folder.</p></div>
            </div>
        </section>
    {:else if !installation.available || !installation.root}
        <section class="empty-state" aria-labelledby="server-unavailable-heading">
            <span class="empty-icon" aria-hidden="true"><LucideServer /></span>
            <h2 id="server-unavailable-heading">Server installation unavailable</h2>
            <p>{installation.problem ?? "The saved server installation could not be found."}</p>
            <p>Choose another server above, or add its current SonsOfTheForestDS.exe location.</p>
        </section>
    {:else}
        <nav class="server-navigation" aria-label="Server workspace">
            <button type="button" class:active={section === "server"} aria-current={section === "server" ? "page" : undefined}
                disabled={$processing} on:click={() => selectSection("server")}><LucideServer aria-hidden="true" /> Server</button>
            <button type="button" class:active={section === "packages"} aria-current={section === "packages" ? "page" : undefined}
                disabled={$processing} on:click={() => selectSection("packages")}><LucidePackage aria-hidden="true" /> Packages</button>
            <button type="button" class:active={section === "backups"} aria-current={section === "backups" ? "page" : undefined}
                disabled={$processing} on:click={() => selectSection("backups")}><LucideArchive aria-hidden="true" /> World backups</button>
        </nav>
        <div class="server-content">
            {#key installationKey}
                {#if section === "server"}
                    <ServerLaunch {installation} />
                {:else if section === "packages"}
                    <ServerInstallation {installation} />
                {:else}
                    <ServerWorldRecovery {installation} />
                {/if}
            {/key}
        </div>
    {/if}
</div>

<style>
    .servers-page { display: flex; flex-direction: column; gap: 22px; width: 100%; min-width: 0; text-align: left; }
    .page-heading { display: flex; align-items: center; justify-content: space-between; gap: 20px; padding: 3px 0 1px; }
    .eyebrow { margin: 0 0 7px; color: var(--text-secondary); font-size: 11px; font-weight: 600; letter-spacing: 1.1px; text-transform: uppercase; }
    h1 { margin: 0; }
    .page-description { margin: 9px 0 0; color: var(--text-secondary); font-size: 13px; }
    .workspace-mark { display: grid; place-items: center; width: 46px; height: 46px; border: 1px solid var(--border-subtle); border-radius: 10px; background: var(--surface-panel); color: var(--accent); flex-shrink: 0; }
    .workspace-mark :global(svg) { width: 22px; height: 22px; }
    .server-navigation { display: flex; flex-wrap: wrap; gap: 4px; padding-bottom: 10px; border-bottom: 1px solid var(--border-subtle); }
    .server-navigation button { display: flex; align-items: center; gap: 8px; background: transparent; border-color: transparent; box-shadow: none; color: var(--text-secondary); padding: 9px 14px; }
    .server-navigation button :global(svg) { width: 16px; height: 16px; }
    .server-navigation button:enabled:hover { color: var(--text-primary); background: var(--surface-raised); }
    .server-navigation button.active { color: var(--accent); background: var(--accent-soft); }
    .server-content { min-width: 0; }
    .empty-state { min-width: 0; padding: 32px; background: var(--surface-panel); border: 1px solid var(--border-subtle); border-radius: 10px; }
    .empty-icon { display: grid; place-items: center; width: 46px; height: 46px; margin-bottom: 18px; border-radius: 10px; background: var(--accent-soft); color: var(--accent); }
    .empty-icon :global(svg) { width: 23px; height: 23px; }
    .empty-state h2 { margin: 0 0 9px; font-size: 20px; }
    .empty-state > p { margin: 0 0 22px; max-width: 620px; color: var(--text-secondary); font-size: 13px; overflow-wrap: anywhere; }
    .empty-state > p:last-child { margin-bottom: 0; }
    .setup-instruction { display: flex; align-items: flex-start; gap: 13px; margin-top: 18px; }
    .step-number { display: grid; place-items: center; flex-shrink: 0; width: 25px; height: 25px; border: 1px solid var(--border-subtle); border-radius: 50%; color: var(--text-secondary); font-size: 11px; }
    .setup-instruction strong { font-size: 13px; font-weight: 500; }
    .setup-instruction p { margin: 4px 0 0; color: var(--text-secondary); font-size: 12px; overflow-wrap: anywhere; }
    .setup-instruction p strong { color: var(--text-primary); font-size: inherit; }
    .server-content :global(.world-recovery) { padding: 22px; background: var(--surface-panel); border-color: var(--border-subtle); border-radius: 10px; }
    .server-content :global(.world-recovery .note) { color: var(--text-secondary); }
    .server-content :global(.world-recovery .observation) { border-color: var(--border-subtle); }
    .server-content :global(.world-recovery .review) { background: var(--surface-raised); border-color: rgba(238, 182, 107, .3); border-radius: 7px; }
    @media (max-width: 760px) {
        .servers-page { gap: 18px; }
        .workspace-mark { display: none; }
        .empty-state { padding: 22px; }
        .server-navigation button { padding: 9px 11px; }
        .server-content :global(.world-recovery) { padding: 18px; }
    }
</style>
