<script lang="ts">
    import { onMount, tick } from "svelte";
    import type { InspectedInstallation } from "../installationProfiles";
    import { readServerConsole, type ConsoleSource } from "./serverConsole";
    import LoaderConsole from "./LoaderConsole.svelte";

    export let installation: InspectedInstallation;

    const MAX_LINES = 2000;

    let source: ConsoleSource | "commands" = "game";
    let lines: string[] = [];
    let file: string | null = null;
    let offset: number | null = null;
    let problem = "";
    let error = "";
    let generation = 0;
    let reading = false;
    let following = true;
    let output: HTMLElement;

    function selectSource(next: ConsoleSource | "commands") {
        if (next === source) return;
        source = next;
        generation++;
        lines = [];
        file = null;
        offset = null;
        problem = "";
        error = "";
        following = true;
        void poll();
    }

    async function poll() {
        if (source === "commands" || reading || document.visibilityState !== "visible") return;
        const request = generation;
        reading = true;
        try {
            const chunk = await readServerConsole(installation.target.executable, installation.target.id, source, file, offset);
            if (request !== generation) return;
            error = "";
            problem = chunk.problem ?? "";
            file = chunk.file;
            offset = chunk.file ? chunk.offset : null;
            if (chunk.reset) lines = [];
            if (chunk.lines.length) {
                lines = [...lines, ...chunk.lines].slice(-MAX_LINES);
                if (following) {
                    await tick();
                    output?.scrollTo({ top: output.scrollHeight });
                }
            }
        } catch (failure) {
            if (request === generation) error = `Could not read the server log: ${failure}`;
        } finally { reading = false; }
    }

    function updateFollowing() {
        following = output.scrollHeight - output.scrollTop - output.clientHeight < 24;
    }

    onMount(() => {
        void poll();
        const interval = window.setInterval(poll, 1500);
        const visible = () => { if (document.visibilityState === "visible") void poll(); };
        document.addEventListener("visibilitychange", visible);
        return () => {
            generation++;
            window.clearInterval(interval);
            document.removeEventListener("visibilitychange", visible);
        };
    });
</script>

<section class="server-console" aria-labelledby="server-console-heading">
    <div class="console-heading">
        <div>
            <p class="eyebrow">Live output</p>
            <h2 id="server-console-heading">Console</h2>
        </div>
        <div class="source-switch" role="group" aria-label="Log source">
            <button type="button" class:active={source === "game"} aria-pressed={source === "game"} on:click={() => selectSource("game")}>Game log</button>
            <button type="button" class:active={source === "loader"} aria-pressed={source === "loader"} on:click={() => selectSource("loader")}>Loader log</button>
            <button type="button" class:active={source === "commands"} aria-pressed={source === "commands"} on:click={() => selectSource("commands")}>Loader console</button>
        </div>
    </div>
    {#if source === "commands"}
        <LoaderConsole {installation} note="Commands go to the server's Modstoke Loader, and Admin treats them as the host. Their results appear here, not in the server's own window or the Loader log." />
    {:else}
    <div class="console-meta">
        <span class="path">{file ?? "No log file"}</span>
        <span>{following ? "Following new lines" : "Paused while scrolled up"}</span>
        <button type="button" class="small" disabled={!lines.length} on:click={() => lines = []}>Clear view</button>
    </div>
    {#if error}<p class="message error" role="alert">{error}</p>{/if}
    {#if problem && lines.length}<p class="note">{problem}</p>{/if}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex (Keyboard users need to focus and scroll the log.) -->
    <pre class="console-output" bind:this={output} on:scroll={updateFollowing} tabindex="0">{#if lines.length}{lines.join("\n")}{:else}<span class="empty">{problem || "Waiting for output…"}</span>{/if}</pre>
    <p class="note">Shows the server's log files, which can trail its own console window by a few seconds. The game log needs Log files turned on in Server settings. The Loader log and console appear when Modstoke Loader is installed on this server.</p>
    {/if}
</section>

<style>
    .server-console { display: flex; flex-direction: column; gap: 14px; min-width: 0; padding: 22px; background: var(--surface-panel); border: 1px solid var(--border-subtle); border-radius: 10px; }
    .console-heading, .console-meta { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 12px; }
    .eyebrow { margin: 0 0 6px; color: var(--text-secondary); font-size: 11px; font-weight: 600; letter-spacing: 1px; text-transform: uppercase; }
    h2 { margin: 0; font-size: 19px; }
    .source-switch { display: flex; gap: 4px; }
    .source-switch button { background: transparent; border-color: var(--border-subtle); box-shadow: none; color: var(--text-secondary); }
    .source-switch button.active { color: var(--accent); background: var(--accent-soft); border-color: rgba(238, 182, 107, .25); }
    .console-meta { color: var(--text-secondary); font-size: 11px; }
    .console-meta .path { flex: 1 1 200px; min-width: 0; overflow-wrap: anywhere; user-select: text; }
    .small { font-size: 11px; min-height: 30px; padding: 5px 9px; }
    .console-output { height: 52vh; min-height: 260px; margin: 0; padding: 14px; overflow: auto; border: 1px solid var(--border-subtle); border-radius: 8px; background: var(--surface-base); color: var(--text-primary); font: 12px/1.55 Consolas, "Cascadia Mono", monospace; white-space: pre-wrap; overflow-wrap: anywhere; user-select: text; }
    .empty { color: var(--text-secondary); font-family: inherit; }
    .note { margin: 0; color: var(--text-secondary); font-size: 12px; line-height: 1.6; }
    .message { margin: 0; padding: 12px 14px; font-size: 13px; line-height: 1.6; border-radius: 8px; overflow-wrap: anywhere; }
    .message.error { color: #ffb4ac; background: rgba(168, 61, 51, .1); border: 1px solid #693b44; }
    @media (max-width: 760px) {
        .server-console { padding: 18px; }
        .console-output { height: 45vh; }
    }
</style>
