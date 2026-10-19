<script lang="ts">
    import { createEventDispatcher, onDestroy } from "svelte";
    import { get } from "svelte/store";
    import { open, save } from "@tauri-apps/plugin-dialog";
    import { writeTextFile } from "@tauri-apps/plugin-fs";
    import { processing, processName } from "./store";
    import { selectedInstallation } from "./installationProfiles";
    import { nativeInstallationKey, readNativePackageProfile } from "./nativePackage";
    import type { ProfileState } from "./modProfile";
    import {
        parseNativePack, prepareNativePack, inspectNativePackInstall, installNativePack,
        type NativePackDocument, type NativePackPreparation, type NativePackInstallPreview
    } from "./nativePack";

    export let gameRoot: string;
    export let refreshToken = 0;
    const dispatch = createEventDispatcher<{ changed: void }>();

    let name = "Community pack";
    let document: NativePackDocument | null = null;
    let expected: string | null = null;
    let preparation: NativePackPreparation | null = null;
    let error = "";
    let notice = "";
    let busy = false;
    let generation = 0;
    let destroyed = false;
    let sources: string[] = [];
    let review: NativePackInstallPreview | null = null;
    let state: ProfileState | null = null;
    let activeKey = "";
    let activeRoot = "";
    let activeRefresh = refreshToken;
    let loaded = false;

    function syncSelection(identity: string, root: string) {
        if (identity === activeKey && root === activeRoot) return;
        activeKey = identity;
        activeRoot = root;
        generation++;
        review = null;
        state = null;
        loaded = false;
        error = "";
        notice = "";
    }
    function syncRefresh(token: number) {
        if (token === activeRefresh) return;
        activeRefresh = token;
        generation++;
        review = null;
        state = null;
        loaded = false;
    }
    const unsubscribe = selectedInstallation.subscribe(value => syncSelection(nativeInstallationKey(value), gameRoot));
    $: syncSelection(nativeInstallationKey($selectedInstallation), gameRoot);
    $: syncRefresh(refreshToken);
    $: available = !!gameRoot && !!$selectedInstallation?.available && $selectedInstallation.root === gameRoot;
    $: pending = state?.profile?.pendingPlan;
    $: if (available && !loaded && !busy && !$processing) void refresh();

    function canStart() { return !destroyed && !busy && !get(processing); }
    function canUseInstallation() {
        syncSelection(nativeInstallationKey(get(selectedInstallation)), gameRoot);
        syncRefresh(refreshToken);
        return canStart() && !!gameRoot && !!get(selectedInstallation)?.available
            && get(selectedInstallation)?.root === gameRoot;
    }
    function begin(label: string) {
        syncSelection(nativeInstallationKey(get(selectedInstallation)), gameRoot);
        syncRefresh(refreshToken);
        busy = true;
        processing.set(true);
        processName.set(label);
        error = "";
        notice = "";
        return { request: ++generation, root: gameRoot, identity: activeKey, refresh: refreshToken };
    }
    type Request = ReturnType<typeof begin>;
    function current(context: Request) {
        return !destroyed && context.request === generation && context.root === gameRoot
            && context.identity === nativeInstallationKey(get(selectedInstallation)) && context.refresh === refreshToken;
    }
    function finish() {
        busy = false;
        processName.set("");
        processing.set(false);
    }
    function reset() {
        if (!canStart()) return;
        generation++;
        name = "Community pack";
        document = null;
        expected = null;
        preparation = null;
        sources = [];
        review = null;
        error = "";
        notice = "";
    }

    async function reload(context: Request) {
        if (!current(context)) return;
        loaded = true;
        state = null;
        try {
            const result = await readNativePackageProfile(context.root, context.identity);
            if (current(context)) state = result;
        } catch (failure) {
            if (current(context)) error += `${error ? " " : ""}Pack recovery status could not be read: ${failure}`;
        }
    }

    async function refresh() {
        if (!canUseInstallation()) return;
        const context = begin("Reading native pack recovery status...");
        review = null;
        try { await reload(context); }
        finally { finish(); }
    }

    function loadPending() {
        if (!canUseInstallation() || !state?.profile?.pendingPlan?.nativePack) return;
        generation++;
        document = state.profile.pendingPlan.nativePack;
        expected = JSON.stringify(document);
        name = document.name;
        sources = [];
        preparation = null;
        review = null;
        error = "";
        notice = "Select every ZIP in the pending pack, including packages already applied, then review to resume.";
    }

    async function inspectInstall() {
        if (!canUseInstallation() || !preparation || !sources.length) return;
        if (state?.profile?.pendingPlan && !state.profile.pendingPlan.nativePack) return;
        const capturedSources = [...sources];
        const capturedDocument = preparation.document;
        const context = begin("Reviewing native pack installation...");
        review = null;
        try {
            const result = await inspectNativePackInstall(context.root, capturedSources, capturedDocument, context.identity);
            if (current(context)) review = result;
        } catch (failure) {
            if (current(context)) { error = String(failure); await reload(context); }
        } finally { finish(); }
    }

    async function applyPack() {
        if (!canUseInstallation() || !review || !preparation || !sources.length) return;
        if (activeRoot !== gameRoot || activeKey !== nativeInstallationKey(get(selectedInstallation))) return;
        const capturedReview = review;
        const capturedSources = [...sources];
        const context = begin(capturedReview.planId ? "Resuming native pack installation..." : "Installing native pack...");
        review = null;
        try {
            await installNativePack(context.root, capturedSources, capturedReview, context.identity);
            if (current(context)) notice = "Pack files installed. Start the game or server to load the packages.";
        } catch (failure) {
            if (current(context)) error = String(failure);
        } finally {
            try {
                await reload(context);
                if (current(context)) dispatch("changed");
            } finally { finish(); }
        }
    }

    async function importFile(event: Event) {
        const input = event.currentTarget as HTMLInputElement;
        const file = input.files?.[0];
        input.value = "";
        if (!file || !canStart()) return;
        const request = begin("Reading portable pack descriptor...");
        preparation = null;
        sources = [];
        review = null;
        try {
            if (file.size > 256 * 1024) throw new Error("Pack descriptors must be 256 KiB or smaller.");
            const text = await file.text();
            if (!current(request)) return;
            const result = await parseNativePack(text);
            if (current(request)) {
                document = result;
                expected = text;
                name = result.name;
            }
        } catch (failure) {
            if (current(request)) error = String(failure);
        } finally { finish(); }
    }

    async function choose() {
        if (!canStart() || !name.trim()) return;
        const capturedName = name;
        const capturedExpected = expected;
        const request = begin(capturedExpected === null ? "Preparing portable native pack..." : "Verifying portable pack ZIPs...");
        preparation = null;
        sources = [];
        review = null;
        try {
            const selected = await open({ title: "Select all native pack ZIPs", multiple: true, directory: false,
                filters: [{ name: "Native mod ZIPs", extensions: ["zip"] }] });
            if (!selected || !current(request)) return;
            const selectedSources = Array.isArray(selected) ? selected : [selected];
            if (!selectedSources.length) return;
            const result = await prepareNativePack(capturedName, selectedSources, capturedExpected);
            if (current(request)) {
                sources = selectedSources;
                preparation = result;
                document = result.document;
                name = result.document.name;
                expected = JSON.stringify(result.document);
            }
        } catch (failure) {
            if (current(request)) error = String(failure);
        } finally { finish(); }
    }

    async function exportPack() {
        if (!canStart() || !preparation) return;
        const text = JSON.stringify(preparation.document, null, 2) + "\n";
        const request = begin("Saving portable pack descriptor...");
        try {
            const path = await save({ title: "Save portable native pack", defaultPath: "native-pack.json",
                filters: [{ name: "Native pack descriptor", extensions: ["json"] }] });
            if (!path || !current(request)) return;
            await writeTextFile(path, text);
            if (current(request)) notice = "Pack descriptor saved. Share it together with the matching ZIPs.";
        } catch (failure) {
            if (current(request)) error = String(failure);
        } finally { finish(); }
    }

    onDestroy(() => { destroyed = true; generation++; sources = []; unsubscribe(); });
</script>

<section class="native-pack" aria-label="Portable native packs">
    <h2>Portable native packs</h2>
    <p class="muted">Prepare and share a descriptor, then review its ZIPs for the selected installation. Preparation and export do not change installed files or profiles.</p>
    <div class="controls">
        <label class="name">Pack name<input type="text" bind:value={name} disabled={busy || $processing || document !== null} /></label>
        <button type="button" disabled={busy || $processing || !name.trim()} on:click={choose}>{document ? "Verify ZIPs…" : "Prepare pack…"}</button>
        <label class="import" class:disabled={busy || $processing}>Import pack…<input type="file" accept=".json,application/json" disabled={busy || $processing} on:change={importFile} /></label>
        <button type="button" disabled={busy || $processing} on:click={reset}>Create new / reset</button>
    </div>
    {#if busy}<p role="status">{$processName}</p>{/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    {#if notice}<p role="status">{notice}</p>{/if}
    {#if pending}
        <div class="pending" role="status">
            {#if pending.nativePack}
                <strong>Unfinished native pack: {pending.nativePack.name}</strong>
                <p>Load its descriptor and verify every ZIP, including packages already applied, to review and resume.</p>
                <button type="button" disabled={!available || busy || $processing} on:click={loadPending}>Load pending pack</button>
            {:else}
                <strong>Another installation plan is unfinished.</strong>
                <p>Finish it through its installer or use Pack recovery before installing a native pack.</p>
            {/if}
        </div>
    {/if}
    {#if document}
        <div class="review">
            <h3>{document.name} · {document.packages.length} packages</h3>
            {#if !preparation}
                <p class="unverified" role="status"><strong>Unverified archives.</strong> Select all matching ZIPs to check the packages listed below.</p>
            {:else}
                <p role="status">All selected ZIPs inspected and matched to this descriptor.</p>
                <p>{preparation.totalArchiveBytes.toLocaleString()} archive bytes · {preparation.totalDecodedBytes.toLocaleString()} decoded bytes · {preparation.fileCount.toLocaleString()} files</p>
            {/if}
            <details open={!preparation}>
                <summary>{preparation ? "Package order and verified archive details" : "Declared packages and required ZIP hashes"}</summary>
                <ol>
                    {#each document.packages as item}
                        <li>
                            <strong>{item.name} · {item.version}</strong>
                            <dl>
                                <dt>ID</dt><dd>{item.id}</dd>
                                <dt>Folder</dt><dd>{item.folder}</dd>
                                <dt>Archive bytes</dt><dd>{item.size.toLocaleString()}</dd>
                                <dt>SHA-256</dt><dd>{item.sha256}</dd>
                                <dt>Exact dependencies</dt>
                                <dd>{#if item.required.length}<ul>{#each item.required as dependency}<li>{dependency.id} · exactly {dependency.version}</li>{/each}</ul>{:else}None declared.{/if}</dd>
                            </dl>
                        </li>
                    {/each}
                </ol>
            </details>
            <button type="button" disabled={busy || $processing || !preparation} on:click={exportPack}>Save pack…</button>
            <button type="button" disabled={!available || busy || $processing || !preparation || !sources.length || (!!pending && !pending.nativePack)} on:click={inspectInstall}>Review installation…</button>
        </div>
    {/if}
    {#if review}
        <div class="review">
            <h3>{review.planId ? "Resume pack" : "Install pack"}: {review.document.name}</h3>
            <dl>
                <dt>Installation</dt><dd>{$selectedInstallation?.target.name} ({$selectedInstallation?.target.kind})</dd>
                <dt>Root</dt><dd>{gameRoot}</dd>
            </dl>
            <ol>
                {#each review.packages as item}
                    <li><strong>{item.name} · {item.version}</strong><p>{item.id} · {item.action} · {item.status}</p></li>
                {/each}
            </ol>
            {#if review.notices.length}<ul class="notices">{#each review.notices as item}<li>{item}</li>{/each}</ul>{/if}
            <p>Installation uses one recovery checkpoint. A failure may leave packages applied and the plan unfinished; resume it or use Pack recovery.</p>
            <button type="button" disabled={!available || busy || $processing} on:click={applyPack}>{review.planId ? "Resume pack" : "Install pack"}</button>
        </div>
    {/if}
    <div class="footer">
        {#if !available}<p class="muted">Select an available player or dedicated-server installation to review and install.</p>{/if}
        <button type="button" disabled={!available || busy || $processing} on:click={refresh}>Refresh pack status</button>
    </div>
    <p class="muted">The descriptor contains package metadata and hashes, not ZIPs or local paths. File verification is not proof of installation, runtime compatibility, or server admission.</p>
    <p class="muted">Importing or selecting a descriptor does not activate server admission requirements.</p>
</section>

<style>
    .native-pack { box-sizing: border-box; flex-shrink: 0; min-width: 0; padding: 16px; color: var(--text-primary); background: var(--surface-panel); border: 1px solid var(--border-subtle); border-radius: 10px; }
    h2 { margin: 0; font-size: 16px; }
    h3 { margin: 0; font-size: 14px; overflow-wrap: anywhere; }
    p { margin: .45em 0; font-size: 13px; line-height: 1.55; overflow-wrap: anywhere; }
    .muted { color: var(--text-secondary); }
    .controls { display: flex; flex-wrap: wrap; align-items: flex-end; gap: .6em; margin-top: .7em; }
    .name { display: flex; flex-direction: column; gap: .3em; font-size: 13px; min-width: 0; flex: 1 1 180px; font-weight: 500; }
    .name input { box-sizing: border-box; width: 100%; padding: 9px 12px; color: var(--text-primary); background: var(--surface-base); border: 1px solid var(--border-subtle); border-radius: 8px; font-size: 13px; }
    button, .import { margin: 0; padding: 8px 12px; }
    .import { position: relative; overflow: hidden; border: 1px solid var(--border-subtle); border-radius: 8px; background: var(--surface-raised); cursor: pointer; font-size: 13px; max-width: 100%; box-sizing: border-box; }
    .import input { position: absolute; inset: 0; width: 100%; opacity: 0; cursor: pointer; }
    .import:focus-within { outline: 2px solid var(--accent); outline-offset: 2px; }
    :disabled, .disabled { opacity: .55; }
    .error { color: #ffb4ac; }
    .unverified { color: var(--accent); }
    .pending { background: var(--accent-soft); border: 1px solid color-mix(in srgb, var(--accent) 35%, transparent); padding: 14px; margin: .7em 0; font-size: 13px; border-radius: 8px; }
    .footer { display: flex; flex-wrap: wrap; align-items: center; gap: .6em; margin-top: .7em; }
    .notices { padding-left: 1.3em; font-size: 13px; line-height: 1.5; overflow-wrap: anywhere; }
    .review { margin-top: .7em; padding: 14px; border: 1px solid var(--border-subtle); background: var(--surface-base); min-width: 0; border-radius: 8px; }
    details { margin: .7em 0; }
    summary { cursor: pointer; font-size: 13px; padding: 6px 0; }
    ol { padding-left: 1.5em; font-size: 13px; line-height: 1.55; overflow-wrap: anywhere; }
    dl { display: grid; grid-template-columns: 8em minmax(0, 1fr); gap: .3em .7em; font-size: 13px; line-height: 1.55; }
    dt { color: var(--text-secondary); }
    dd { margin: 0; overflow-wrap: anywhere; user-select: text; }
    dd ul { margin: 0; padding-left: 1.3em; }
    button { max-width: 100%; white-space: normal; overflow-wrap: anywhere; font-size: 13px; border-radius: 8px; }
    .review > button { margin: 4px 4px 0 0; }
    @media (max-width: 600px) { dl { grid-template-columns: 1fr; } .name { width: 100%; flex-basis: auto; } }
</style>
