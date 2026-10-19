<script lang="ts">
    import * as dialog from "@tauri-apps/plugin-dialog";
    import LucidePlus from "~icons/lucide/plus";
    import LucideFolder from "~icons/lucide/folder";
    import { processing } from "./store";
    import { installationProfiles, selectedInstallation, installationProfileError, profilesReady,
        addInstallationProfile, selectInstallationProfile, renameInstallationProfile,
        removeInstallationProfile, refreshInstallationProfiles } from "./installationProfiles";

    export let kind: "player" | "server" | null = null;
    let busy = false;
    let error = "";
    let renameId: string | null = null;
    let name = "";
    $: disabled = busy || $processing || !$profilesReady;
    $: targets = ($installationProfiles?.registry.targets ?? []).filter(target => !kind || target.kind === kind);
    $: currentInstallation = $selectedInstallation && (!kind || $selectedInstallation.target.kind === kind) ? $selectedInstallation : null;
    $: selectedId = currentInstallation?.target.id ?? "";
    $: if (renameId && renameId !== selectedId) renameId = null;

    async function perform(action: () => Promise<unknown>, retry = false) {
        if (busy || $processing || (!retry && !$profilesReady)) return;
        busy = true;
        error = "";
        try { await action(); }
        catch (cause) { error = cause instanceof Error ? cause.message : String(cause); }
        finally { busy = false; }
    }

    function add() {
        return perform(async () => {
            const result = await dialog.open({ multiple: false, title: kind === "server" ? "Select SonsOfTheForestDS.exe" : kind === "player" ? "Select SonsOfTheForest.exe" : "Select player or dedicated server executable",
                filters: [{ name: "Sons Of The Forest executable", extensions: ["exe"] }] });
            const executable = Array.isArray(result) ? result[0] : result;
            if (executable) await addInstallationProfile(executable, kind ?? undefined);
        });
    }

    function beginRename() {
        if (disabled || !currentInstallation) return;
        renameId = currentInstallation.target.id;
        name = currentInstallation.target.name;
    }

    function saveName() {
        const id = renameId;
        if (!id || !name.trim()) return;
        return perform(async () => { await renameInstallationProfile(id, name.trim()); renameId = null; });
    }

    function forget() {
        const target = currentInstallation?.target;
        if (!target) return;
        return perform(async () => {
            const confirmed = await dialog.confirm(`Forget the saved profile “${target.name}”? Only this profile will be forgotten. All installation files remain untouched.`,
                { title: "Forget installation profile", kind: "warning", okLabel: "Forget profile", cancelLabel: "Cancel" });
            if (confirmed) await removeInstallationProfile(target.id);
        });
    }
</script>
<section id="path-selector" aria-labelledby="installation-label">
    <div class="selector-heading">
        <label id="installation-label" for="installation-profile">Installation</label>
        {#if currentInstallation}
            <span class="profile-kind">{currentInstallation.target.kind === "server" ? "Dedicated server" : "Player installation"}</span>
        {:else if !$profilesReady}
            <span class="profile-kind">Loading profiles…</span>
        {/if}
    </div>
    <div class="profile-row">
        <select id="installation-profile" value={selectedId} disabled={disabled || targets.length === 0}
            on:change={(event) => { const select = event.currentTarget; const id = select.value; select.value = selectedId; void perform(() => selectInstallationProfile(id)); }}>
            {#if !selectedId}<option value="">{targets.length ? "Choose an installation" : "No saved installations"}</option>{/if}
            {#each targets as target (target.id)}
                <option value={target.id}>{target.name} · {target.kind === "server" ? "Server" : "Player"}</option>
            {/each}
        </select>
        <div class="profile-actions">
            <button class="add-button" type="button" on:click={add} disabled={disabled}><LucidePlus aria-hidden="true" /> Add</button>
            <button type="button" on:click={beginRename} disabled={disabled || !selectedId}>Rename</button>
            <button class="forget-button" type="button" on:click={forget} disabled={disabled || !selectedId}>Forget</button>
        </div>
    </div>
    {#if renameId}
        <form class="rename-row" on:submit|preventDefault={saveName}>
            <label for="installation-name">Profile name</label>
            <div class="rename-controls">
                <input id="installation-name" bind:value={name} disabled={disabled} required />
                <button class="save-button" type="submit" disabled={disabled || !name.trim()}>Save</button>
                <button type="button" on:click={() => renameId = null} disabled={disabled}>Cancel</button>
            </div>
        </form>
    {/if}
    {#if currentInstallation}
        <div class="path-row">
            <LucideFolder aria-hidden="true" />
            <input class="path-input" aria-label="Selected installation executable" title={currentInstallation.target.executable} value={currentInstallation.target.executable} readonly disabled={disabled} />
        </div>
        {#if !currentInstallation.available}
            <p class="error installation-problem" role="status">{currentInstallation.problem ?? "This installation is unavailable."}</p>
        {/if}
    {:else if $profilesReady}
        <p class="hint">{kind === "server" ? "Add SonsOfTheForestDS.exe from an existing dedicated-server installation." : kind === "player" ? "Choose a saved player installation or add SonsOfTheForest.exe." : "Add SonsOfTheForest.exe or SonsOfTheForestDS.exe to save an installation."}</p>
    {/if}
    {#if error || $installationProfileError}
        <div class="error-row">
            <div class="errors" role="alert">
                {#if error}<p class="error">{error}</p>{/if}
                {#if $installationProfileError && $installationProfileError !== error}<p class="error">{$installationProfileError}</p>{/if}
            </div>
            <button type="button" disabled={busy || $processing} on:click={() => perform(refreshInstallationProfiles, true)}>Retry</button>
        </div>
    {/if}
</section>

<style>
    #path-selector {
        display: flex;
        flex-direction: column;
        gap: 12px;
        width: 100%;
        min-width: 0;
        margin: 0;
        padding: 18px 20px 15px;
        border: 1px solid var(--border-subtle, #30353f);
        border-radius: 10px;
        background: var(--surface-panel, #171a21);
        box-sizing: border-box;
        text-align: left;
    }

    .selector-heading { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
    label { color: var(--text-primary, #eef0f4); font-size: 12px; font-weight: 600; line-height: 1.4; }
    .profile-kind { color: var(--text-secondary, #9da6b6); font-size: 11px; line-height: 1.4; }
    .profile-row { display: flex; align-items: center; gap: 12px; min-width: 0; }
    select, input {
        min-width: 0;
        box-sizing: border-box;
        margin: 0;
        border: 1px solid #3a424f;
        border-radius: 7px;
        padding: 10px 12px;
        color: var(--text-primary, #eef0f4);
        background: var(--surface-base, #101217);
        font: inherit;
        font-size: 13px;
        line-height: 1.35;
    }
    select { flex: 1 1 0; width: 0; height: 40px; cursor: pointer; }
    option { color: var(--text-primary, #eef0f4); background: var(--surface-panel, #171a21); }
    .profile-actions { display: flex; align-items: center; gap: 6px; }
    button { display: inline-flex; align-items: center; justify-content: center; gap: 5px; flex-shrink: 0; min-height: 36px; margin: 0; padding: 8px 11px; border: 1px solid var(--border-subtle, #30353f); border-radius: 7px; color: var(--text-secondary, #9da6b6); background: transparent; font-size: 12px; font-weight: 500; line-height: 1.35; }
    button :global(svg) { width: 14px; height: 14px; }
    .add-button { color: var(--text-primary, #eef0f4); background: var(--surface-raised, #20242c); }
    button:hover:not(:disabled) { border-color: #515967; background: var(--surface-raised, #20242c); color: var(--text-primary, #eef0f4); }
    .forget-button:hover:not(:disabled) { color: #efb1ab; }
    .path-row { display: flex; align-items: center; gap: 8px; min-width: 0; color: #8590a2; }
    .path-row :global(svg) { width: 13px; height: 13px; flex-shrink: 0; }
    .path-input { flex: 1; width: 100%; padding: 2px 0; border: 0; border-radius: 0; color: var(--text-secondary, #9da6b6); background: transparent; font-family: "Cascadia Code", Consolas, monospace; font-size: 10px; line-height: 1.6; text-overflow: ellipsis; }
    .path-input:disabled { opacity: .65; }
    .rename-row { display: flex; flex-direction: column; gap: 8px; padding: 13px 0; border-top: 1px solid var(--border-subtle, #30353f); border-bottom: 1px solid var(--border-subtle, #30353f); }
    .rename-controls { display: flex; align-items: center; gap: 8px; }
    .rename-controls input { flex: 1; }
    .save-button { color: var(--accent, #eeb66b); background: var(--accent-soft, #eeb66b12); }
    .error-row { display: flex; align-items: center; gap: 12px; padding-top: 12px; border-top: 1px solid var(--border-subtle, #30353f); }
    .errors { flex: 1; min-width: 0; }
    .error, .hint { margin: 0; font-size: 12px; line-height: 1.55; overflow-wrap: anywhere; }
    .error { color: #efb1ab; }
    .errors p + p { margin-top: 7px; }
    .hint { color: var(--text-secondary, #9da6b6); }
    .installation-problem { padding-top: 1px; }
    :disabled { opacity: .45; cursor: default; }
    button:focus-visible, select:focus-visible, input:focus-visible { outline: 2px solid var(--accent, #eeb66b); outline-offset: 3px; }
    .path-input:focus-visible { outline-offset: 2px; border-radius: 2px; }

    @media (max-width: 850px) {
        #path-selector { padding: 16px; }
        .profile-row { align-items: stretch; flex-direction: column; gap: 10px; }
        select { width: 100%; flex: auto; }
        .profile-actions { justify-content: flex-end; }
    }
</style>
