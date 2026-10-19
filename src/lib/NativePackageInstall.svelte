<script lang="ts">
    import { createEventDispatcher, onDestroy } from "svelte";
    import { get } from "svelte/store";
    import { open } from "@tauri-apps/plugin-dialog";
    import { selectedInstallation } from "./installationProfiles";
    import { processing, processName } from "./store";
    import type { ProfileState } from "./modProfile";
    import {
        nativeInstallationKey, assertNativeInstallation, inspectNativePackage,
        installNativePackage, readNativePackageProfile, type NativePackagePreview
    } from "./nativePackage";

    export let gameRoot: string;
    export let refreshToken = 0;
    const dispatch = createEventDispatcher<{ changed: void }>();
    let preview: NativePackagePreview | null = null;
    let source = "";
    let state: ProfileState | null = null;
    let error = "";
    let notice = "";
    let busy = false;
    let generation = 0;
    let destroyed = false;
    let activeKey = "";
    let activeRoot = "";
    let loaded = false;
    let seenRefresh = refreshToken;

    function reset(identity: string, root: string) {
        if (identity === activeKey && root === activeRoot) return;
        activeKey = identity;
        activeRoot = root;
        generation++;
        preview = null;
        source = "";
        state = null;
        error = "";
        notice = "";
        loaded = false;
    }

    // Invalidate synchronously even if selection changes away and back before a render.
    const unsubscribe = selectedInstallation.subscribe(value => reset(nativeInstallationKey(value), gameRoot));
    $: reset(nativeInstallationKey($selectedInstallation), gameRoot);
    $: if (refreshToken !== seenRefresh) {
        seenRefresh = refreshToken;
        generation++;
        loaded = false;
        preview = null;
        source = "";
    }
    $: available = !!gameRoot && !!$selectedInstallation?.available && $selectedInstallation.root === gameRoot;
    $: if (available && !loaded && !busy && !$processing) void refresh();
    $: pending = state?.profile?.pendingPlan;
    $: nativePending = pending?.steps.some(step => String(step.package.source.provider) === "modstoke-native-local") ?? false;
    $: actionLabel = preview?.action === "resume" ? "Resume" : preview?.action === "update" ? "Update" : "Install";

    function canStart() {
        return !destroyed && !busy && !get(processing) && !!gameRoot
            && !!get(selectedInstallation)?.available && get(selectedInstallation)?.root === gameRoot;
    }

    function begin(label: string) {
        const root = gameRoot;
        const identity = nativeInstallationKey(get(selectedInstallation));
        reset(identity, root);
        const request = ++generation;
        busy = true;
        processing.set(true);
        processName.set(label);
        error = "";
        notice = "";
        return { root, identity, request };
    }

    type Request = ReturnType<typeof begin>;
    function current(context: Request) {
        return !destroyed && context.request === generation && context.root === gameRoot
            && context.identity === nativeInstallationKey(get(selectedInstallation));
    }

    function finish() {
        busy = false;
        processName.set("");
        processing.set(false);
    }

    async function reload(context: Request) {
        if (!current(context)) return;
        loaded = true;
        state = null;
        try {
            const result = await readNativePackageProfile(context.root, context.identity);
            if (current(context)) state = result;
        } catch (failure) {
            if (current(context)) error += `${error ? " " : ""}Package recovery status could not be read: ${failure}`;
        }
    }

    async function refresh() {
        if (!canStart()) return;
        const context = begin("Reading native package recovery status...");
        try { await reload(context); }
        finally { finish(); }
    }

    async function choose() {
        if (!canStart()) return;
        const context = begin("Inspecting native package ZIP...");
        preview = null;
        source = "";
        try {
            const selected = await open({ title: "Select native mod ZIP", multiple: false, directory: false,
                filters: [{ name: "Native mod ZIP", extensions: ["zip"] }] });
            if (!selected || Array.isArray(selected) || !current(context)) return;
            assertNativeInstallation(context.root, context.identity);
            const result = await inspectNativePackage(context.root, selected, context.identity);
            if (current(context)) { source = selected; preview = result; }
        } catch (failure) {
            if (current(context)) { error = String(failure); await reload(context); }
        } finally { finish(); }
    }

    async function install() {
        if (!canStart() || !preview || !source) return;
        const capturedPreview = preview;
        const capturedSource = source;
        // A review belongs to the exact selected installation, including availability.
        if (activeRoot !== gameRoot || activeKey !== nativeInstallationKey(get(selectedInstallation))) return;
        const context = begin(`${actionLabel === "Resume" ? "Resuming" : actionLabel === "Update" ? "Updating" : "Installing"} native package...`);
        preview = null;
        source = "";
        try {
            await installNativePackage(context.root, capturedSource, capturedPreview, context.identity);
            if (current(context)) notice = "Package files installed. Start the game or server to load the package.";
        } catch (failure) {
            if (current(context)) error = String(failure);
        } finally {
            try {
                await reload(context);
                if (current(context)) dispatch("changed");
            } finally { finish(); }
        }
    }

    onDestroy(() => { destroyed = true; generation++; unsubscribe(); });
</script>

<section class="native-install" aria-label="Install native mod ZIP">
    <div class="heading">
        <div><h2>Install native mod ZIP</h2><p>Review a local Modstoke package for this installation.</p></div>
        <button type="button" disabled={!available || busy || $processing} on:click={choose}>Select ZIP…</button>
    </div>
    {#if !available}<p>Select an available player or dedicated-server installation.</p>{/if}
    {#if busy}<p role="status">{$processName}</p>{/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    {#if notice}<p role="status">{notice}</p>{/if}
    {#if pending}
        <div class="pending" role="status">
            {#if pending.nativePack}
                <strong>Native pack installation is incomplete.</strong>
                <p>Load the pending pack under Portable native packs and select all matching ZIPs, or use Pack recovery.</p>
            {:else if nativePending}
                <strong>Native package install is incomplete.</strong>
                <p>Select the same ZIP to review and resume it, or use Pack recovery.</p>
            {:else}
                <strong>A catalog install is incomplete.</strong>
                <p>Finish it through the catalog or use Pack recovery before installing a native ZIP.</p>
            {/if}
        </div>
    {/if}
    {#if preview}
        <div class="review">
            <h3>{preview.archive.name} · {preview.archive.version}</h3>
            <dl>
                <dt>Mod ID</dt><dd>{preview.archive.id}</dd>
                <dt>Installation</dt><dd>{$selectedInstallation?.target.name} ({$selectedInstallation?.target.kind})</dd>
                <dt>Root</dt><dd>{gameRoot}</dd>
                <dt>Destination</dt><dd>{preview.archive.packagePath}</dd>
            </dl>
            <p><strong>Required dependencies ({preview.archive.required.length})</strong></p>
            {#if preview.archive.required.length}
                <ul class="dependencies">{#each preview.archive.required as dependency}<li>{dependency.id} · exactly {dependency.version}</li>{/each}</ul>
            {:else}<p>No required dependencies declared.</p>{/if}
            <p>Required versions must already be installed. Inspection does not change live package files or profile records.</p>
            {#if preview.notices.length}<ul class="notices">{#each preview.notices as item}<li>{item}</li>{/each}</ul>{/if}
            <details>
                <summary>Archive details · {preview.archive.files.length} files</summary>
                <dl>
                    <dt>ZIP</dt><dd>{source}</dd>
                    <dt>Size</dt><dd>{preview.archive.size.toLocaleString()} bytes</dd>
                    <dt>SHA-256</dt><dd>{preview.archive.sha256}</dd>
                    <dt>Manifest</dt><dd>{preview.archive.manifestPath}</dd>
                    <dt>Assembly</dt><dd>{preview.archive.assemblyPath}</dd>
                </dl>
            </details>
            <button type="button" disabled={!available || busy || $processing} on:click={install}>{actionLabel} package</button>
        </div>
    {/if}
    <div class="footer"><p>ZIPs contain native code. Install packages from authors you trust. File checks do not establish game or server compatibility.</p>
        <button type="button" disabled={!available || busy || $processing} on:click={refresh}>Refresh status</button></div>
</section>

<style>
    .native-install { box-sizing: border-box; flex-shrink: 0; min-width: 0; padding: 16px; color: var(--text-primary); background: var(--surface-panel); border: 1px solid var(--border-subtle); border-radius: 10px; }
    .heading, .footer { display: flex; justify-content: space-between; align-items: flex-start; gap: 10px; flex-wrap: wrap; }
    h2 { margin: 0; font-size: 16px; }
    h3 { margin: 0; font-size: 14px; overflow-wrap: anywhere; }
    p { margin: .45em 0; font-size: 13px; line-height: 1.55; overflow-wrap: anywhere; }
    .heading p, .footer p { color: var(--text-secondary); }
    button { margin: 0; padding: 8px 12px; flex-shrink: 0; max-width: 100%; white-space: normal; overflow-wrap: anywhere; font-size: 13px; border-radius: 8px; }
    :disabled { opacity: .55; }
    .error { color: #ffb4ac; }
    .pending { background: var(--accent-soft); border: 1px solid color-mix(in srgb, var(--accent) 35%, transparent); padding: 14px; margin: .7em 0; font-size: 13px; border-radius: 8px; }
    .review { margin-top: .7em; padding: 14px; border: 1px solid var(--border-subtle); background: var(--surface-base); min-width: 0; border-radius: 8px; }
    dl { display: grid; grid-template-columns: 6em minmax(0, 1fr); gap: .3em .7em; font-size: 13px; line-height: 1.55; }
    dt { color: var(--text-secondary); }
    dd { margin: 0; overflow-wrap: anywhere; user-select: text; }
    ul { padding-left: 1.3em; line-height: 1.5; overflow-wrap: anywhere; font-size: 13px; }
    .dependencies, .notices { max-height: 12em; overflow-y: auto; }
    details { margin: .7em 0; }
    summary { cursor: pointer; font-size: 13px; padding: 6px 0; }
    .footer { margin-top: .7em; align-items: center; }
    .heading > div, .footer p { min-width: 0; flex: 1 1 240px; }
    @media (max-width: 600px) { .heading, .footer { flex-direction: column; } .heading > div, .footer p { flex-basis: auto; } dl { grid-template-columns: 1fr; } }
</style>
