<script lang="ts">
    import { onMount, tick } from "svelte";
    import type { InspectedInstallation } from "../installationProfiles";
    import { readLoaderConsole, submitLoaderConsole, type LoaderConsoleEntry, type LoaderConsoleSnapshot } from "./loaderConsole";

    export let installation: InspectedInstallation;
    /** Who receives the commands, shown under the console. */
    export let note: string;

    const MAX_LINES = 2000;
    const LEVELS = new Set(["fatal", "error", "warning", "message", "info", "debug"]);
    type Line = LoaderConsoleEntry | { gap: true };

    let lines: Line[] = [];
    let lastId: number | null = null;
    let processId: number | null = null;
    let sessionId = "";
    let snapshot: LoaderConsoleSnapshot | null = null;
    let problem = "";
    let error = "";
    let generation = 0;
    let reading = false;
    let following = true;
    let output: HTMLElement;
    let command = "";
    let args = "";
    let confirming: { name: string; args: string } | null = null;
    let sending = false;
    let reply = "";
    let refused = false;
    let resultsAfter: number | null = null;
    // Set when the game stopped; its last output stays until another session connects.
    let stopped = false;

    $: commands = snapshot?.commands ?? [];
    $: selected = commands.find(entry => entry.name === command.trim()) ?? null;
    // Results carry no request id, and other consoles can send commands too.
    $: results = resultsAfter === null ? [] : lines.filter((line): line is LoaderConsoleEntry =>
        !("gap" in line) && line.id > (resultsAfter ?? 0) && line.source.toLowerCase() === "commands").slice(-5);

    function level(entry: LoaderConsoleEntry): string {
        const value = entry.level.toLowerCase();
        return LEVELS.has(value) ? value : "info";
    }

    function time(timestamp: string): string {
        const date = new Date(timestamp);
        return Number.isNaN(date.getTime()) ? "" : date.toLocaleTimeString();
    }

    function reset(keepLines = false) {
        generation++;
        if (!keepLines) lines = [];
        stopped = keepLines && lines.length > 0;
        lastId = null;
        processId = null;
        sessionId = "";
        snapshot = null;
        confirming = null;
        resultsAfter = null;
        reply = "";
    }

    async function poll() {
        if (reading || sending || document.visibilityState !== "visible") return;
        reading = true;
        let request = generation;
        try {
            // Catch up a few pages at a time when the game has written a lot since the last read.
            for (let page = 0; page < 8; page++) {
                const result = await readLoaderConsole(installation.target.executable, installation.target.id, lastId);
                if (request !== generation) return;
                error = "";
                if (!result.snapshot || result.processId === null) {
                    if (processId !== null) reset(true);
                    problem = result.problem ?? "";
                    return;
                }
                const fresh = result.snapshot;
                // A new process or session numbers its entries from scratch.
                if (lastId !== null && (result.processId !== processId || fresh.sessionId !== sessionId)) {
                    reset();
                    request = generation;
                    continue;
                }
                if (stopped) {
                    lines = [];
                    stopped = false;
                }
                processId = result.processId;
                sessionId = fresh.sessionId;
                snapshot = fresh;
                problem = "";
                const added = fresh.entries.filter(entry => lastId === null || entry.id > lastId);
                if (lastId !== null && fresh.firstId > lastId + 1) lines = [...lines, { gap: true }];
                if (added.length) {
                    lines = [...lines, ...added].slice(-MAX_LINES);
                    lastId = added[added.length - 1].id;
                }
                if (lastId === null) lastId = fresh.lastId;
                if (added.length && following) {
                    await tick();
                    output?.scrollTo({ top: output.scrollHeight });
                }
                if (!added.length || lastId >= fresh.lastId) return;
            }
        } catch (failure) {
            if (request === generation) error = `Could not read the Loader console: ${failure}`;
        } finally { reading = false; }
    }

    async function send(confirmed: boolean) {
        const name = confirmed ? confirming?.name ?? "" : command.trim();
        // A command that takes no arguments never gets the text left in the disabled field.
        const text = confirmed ? confirming?.args ?? ""
            : commands.find(entry => entry.name === name)?.acceptsArguments === false ? "" : args.trim();
        if (!name || processId === null || sending) return;
        if (!confirmed && commands.find(entry => entry.name === name)?.requiresConfirmation) {
            confirming = { name, args: text };
            return;
        }
        const request = generation;
        const cursor = lastId ?? 0;
        sending = true;
        reply = "";
        try {
            const result = await submitLoaderConsole(installation.target.executable, installation.target.id, processId, name, text, confirmed, cursor);
            if (request !== generation) return;
            confirming = null;
            refused = !result.accepted;
            if (result.accepted) {
                reply = `Sent ${name}. Its result appears under Recent results once the ${installation.target.kind === "server" ? "server" : "game"} runs it.`;
                resultsAfter = cursor;
                command = "";
                args = "";
            } else {
                reply = `${name} was not accepted: ${result.detail || "the console refused it."}`;
            }
        } catch (failure) {
            if (request === generation) {
                // Kept apart from read errors, which the next poll clears. A late result still shows.
                confirming = null;
                refused = true;
                reply = `Could not send ${name}: ${failure}`;
                resultsAfter = cursor;
            }
        } finally { sending = false; }
        void poll();
    }

    function updateFollowing() {
        following = output.scrollHeight - output.scrollTop - output.clientHeight < 24;
    }

    onMount(() => {
        void poll();
        const interval = window.setInterval(poll, 1000);
        const visible = () => { if (document.visibilityState === "visible") void poll(); };
        document.addEventListener("visibilitychange", visible);
        return () => {
            generation++;
            window.clearInterval(interval);
            document.removeEventListener("visibilitychange", visible);
        };
    });
</script>

<div class="loader-console">
    <div class="console-meta">
        <span>{snapshot ? `${snapshot.phase || "Starting"}${snapshot.ready ? "" : " · commands not available yet"}` : "Not connected"}</span>
        <span>{following ? "Following new lines" : "Paused while scrolled up"}</span>
        <button type="button" class="small" disabled={!lines.length} on:click={() => lines = []}>Clear view</button>
    </div>
    {#if snapshot?.summary}<p class="note">{snapshot.summary}</p>{/if}
    {#if stopped && problem}<p class="note">{problem} The last output stays below.</p>{/if}
    {#if error}<p class="message error" role="alert">{error}</p>{/if}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex (Keyboard users need to focus and scroll the log.) -->
    <div class="console-output" role="log" bind:this={output} on:scroll={updateFollowing} tabindex="0">
        {#each lines as line}
            {#if "gap" in line}
                <div class="gap">Some earlier entries were dropped by the Loader.</div>
            {:else}
                <div class="entry level-{level(line)}"><span class="time">{time(line.timestamp)}</span> <span class="source">[{line.source}]</span> {line.message}</div>
            {/if}
        {:else}
            <span class="empty">{problem || "Waiting for output…"}</span>
        {/each}
    </div>

    <form class="command-bar" on:submit|preventDefault={() => send(false)}>
        <input type="text" list="loader-console-commands" placeholder="Command" aria-label="Command" spellcheck="false"
            bind:value={command} disabled={!snapshot || sending || !!confirming} />
        <input type="text" placeholder={selected && !selected.acceptsArguments ? "No arguments" : "Arguments"} aria-label="Arguments" maxlength="1024" spellcheck="false"
            bind:value={args} disabled={!snapshot || sending || !!confirming || (!!selected && !selected.acceptsArguments)} />
        <button type="submit" class="primary-button" disabled={!snapshot?.ready || sending || !!confirming || !command.trim() || (!!selected && !selected.enabled)}>
            {sending ? "Sending…" : "Run"}</button>
    </form>
    <datalist id="loader-console-commands">
        {#each commands.filter(entry => entry.enabled) as entry}<option value={entry.name}>{entry.description}</option>{/each}
    </datalist>
    {#if selected}
        <p class="note">{selected.description}{selected.requiresConfirmation ? " Asks you to confirm before it is sent." : ""}{selected.enabled ? "" : " Not available right now."}</p>
    {/if}
    {#if confirming}
        <div class="confirm" role="group" aria-label="Confirm command">
            <p>Send <code>{confirming.name}{confirming.args ? ` ${confirming.args}` : ""}</code>? This command asks for confirmation.</p>
            <div class="actions">
                <button type="button" class="primary-button" disabled={sending} on:click={() => send(true)}>Send it</button>
                <button type="button" disabled={sending} on:click={() => confirming = null}>Cancel</button>
            </div>
        </div>
    {/if}
    {#if reply}<p class="message {refused ? 'error' : 'notice'}" role="status">{reply}</p>{/if}
    {#if results.length}
        <div class="results">
            <h3>Recent results</h3>
            {#each results as entry}<p class="entry level-{level(entry)}"><span class="time">{time(entry.timestamp)}</span> {entry.message}</p>{/each}
            <p class="note">These can include results of commands sent from another console.</p>
        </div>
    {/if}
    {#if commands.length}
        <details class="mods">
            <summary>Commands ({commands.length})</summary>
            <ul>{#each commands as entry}<li><code>{entry.name}</code> {entry.description}{entry.requiresConfirmation ? " (asks to confirm)" : ""}{entry.enabled ? "" : " (not available now)"}</li>{/each}</ul>
        </details>
    {/if}
    {#if snapshot?.mods.length}
        <details class="mods">
            <summary>Loaded mods ({snapshot.mods.length})</summary>
            <ul>{#each snapshot.mods as mod}<li>{mod}</li>{/each}</ul>
        </details>
    {/if}
    <p class="note">{note}</p>
</div>

<style>
    .loader-console { display: flex; flex-direction: column; gap: 12px; min-width: 0; }
    .console-meta { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 12px; color: var(--text-secondary); font-size: 11px; }
    .small { font-size: 11px; min-height: 30px; padding: 5px 9px; }
    .console-output { height: 46vh; min-height: 240px; padding: 14px; overflow: auto; border: 1px solid var(--border-subtle); border-radius: 8px; background: var(--surface-base); color: var(--text-primary); font: 12px/1.55 Consolas, "Cascadia Mono", monospace; overflow-wrap: anywhere; white-space: pre-wrap; user-select: text; }
    .entry { margin: 0; }
    .time, .source { color: var(--text-secondary); }
    .entry.level-warning { color: #e6c89e; }
    .entry.level-error, .entry.level-fatal { color: #ffb4ac; }
    .entry.level-debug { color: var(--text-secondary); }
    .gap { margin: 4px 0; color: var(--text-secondary); font-style: italic; }
    .empty { color: var(--text-secondary); font-family: inherit; }
    .command-bar { display: grid; grid-template-columns: minmax(140px, 1fr) minmax(0, 2fr) auto; gap: 8px; }
    .command-bar input { min-width: 0; font: 12px Consolas, "Cascadia Mono", monospace; }
    .primary-button { color: #251c10; background: var(--accent); border-color: var(--accent); }
    .primary-button:enabled:hover { color: #251c10; background: #ffd296; border-color: #ffd296; }
    .confirm { padding: 12px 14px; background: var(--accent-soft); border-left: 2px solid var(--accent); border-radius: 3px; }
    .confirm p { margin: 0 0 10px; font-size: 12px; overflow-wrap: anywhere; }
    .actions { display: flex; flex-wrap: wrap; gap: 8px; }
    .results { padding: 12px 14px; border: 1px solid var(--border-subtle); border-radius: 8px; }
    .results h3 { margin: 0 0 8px; font-size: 13px; }
    .results .entry { font: 12px/1.55 Consolas, "Cascadia Mono", monospace; overflow-wrap: anywhere; white-space: pre-wrap; }
    .mods summary { cursor: pointer; color: var(--text-secondary); font-size: 12px; }
    .mods ul { margin: 8px 0 0; padding-left: 20px; font-size: 12px; }
    .note { margin: 0; color: var(--text-secondary); font-size: 12px; line-height: 1.6; overflow-wrap: anywhere; }
    .message { margin: 0; padding: 12px 14px; font-size: 13px; line-height: 1.6; border-radius: 8px; overflow-wrap: anywhere; }
    .message.error { color: #ffb4ac; background: rgba(168, 61, 51, .1); border: 1px solid #693b44; }
    .message.notice { color: var(--text-primary); background: var(--surface-raised); border: 1px solid var(--border-subtle); }
    @media (max-width: 760px) {
        .console-output { height: 40vh; }
        .command-bar { grid-template-columns: minmax(0, 1fr); }
    }
</style>
