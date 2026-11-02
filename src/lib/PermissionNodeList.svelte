<script lang="ts">
    import { createEventDispatcher } from "svelte";
    import LucideX from "~icons/lucide/x";
    import type { PermissionNode } from "./adminPermissions";

    export let nodes: string[];
    export let catalog: PermissionNode[];
    export let label: string;
    /** The page's datalist of suggestions. */
    export let suggestions: string;
    export let exact = false;
    export let disabled = false;

    const dispatch = createEventDispatcher<{ change: string[] }>();
    let pending = "";

    $: known = new Map(catalog.map(node => [node.node, node]));

    function add() {
        const value = pending.trim();
        pending = "";
        if (value && !nodes.includes(value)) dispatch("change", [...nodes, value]);
    }

    function describe(node: string): string {
        if (node === "*") return "Every node except sensitive and host-only ones, including nodes a later Admin version adds.";
        if (node.endsWith(".*")) return `Every ${node.slice(0, -2)} node except sensitive and host-only ones, including nodes a later Admin version adds.`;
        const entry = known.get(node);
        if (!entry) return catalog.length ? "Admin does not list this node." : "";
        return entry.description
            + (entry.sensitive ? " Sensitive: only granted when named exactly." : "")
            + (entry.hostOnly ? " Host-only: it can't be granted." : "");
    }
</script>

<div class="node-list">
    {#each nodes as node, index}
        <span class="node" class:wildcard={node.endsWith("*")} class:unknown={!!catalog.length && !node.endsWith("*") && !known.has(node)}
            class:sensitive={known.get(node)?.sensitive} title={describe(node)}>
            <code>{node}</code>
            <button type="button" aria-label={`Remove ${node} from ${label}`} {disabled}
                on:click={() => dispatch("change", nodes.filter((_, other) => other !== index))}><LucideX aria-hidden="true" /></button>
        </span>
    {:else}
        <span class="empty">None</span>
    {/each}
    <span class="adder">
        <input type="text" list={suggestions} spellcheck="false" placeholder={exact ? "Add a node" : "Add a node or pattern"}
            aria-label={`Add to ${label}`} bind:value={pending} {disabled}
            on:keydown={event => { if (event.key === "Enter") { event.preventDefault(); add(); } }} />
        <button type="button" disabled={disabled || !pending.trim()} on:click={add}>Add</button>
    </span>
</div>

<style>
    .node-list { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; min-width: 0; }
    .node { display: inline-flex; align-items: center; gap: 2px; max-width: 100%; padding: 2px 2px 2px 8px; border: 1px solid var(--border-subtle); border-radius: 6px; background: var(--surface-raised); }
    .node code { font-size: 12px; overflow-wrap: anywhere; }
    .node.wildcard { border-style: dashed; }
    .node.sensitive { border-color: rgba(238, 182, 107, .55); }
    .node.unknown { border-color: rgba(255, 180, 172, .55); }
    .node button { display: grid; place-items: center; padding: 3px; border: 0; background: transparent; box-shadow: none; color: var(--text-secondary); }
    .node button :global(svg) { width: 12px; height: 12px; }
    .node button:enabled:hover { color: var(--text-primary); }
    .empty { color: var(--text-secondary); font-size: 12px; }
    .adder { display: inline-flex; gap: 6px; min-width: 0; }
    .adder input { width: 190px; min-width: 0; font-size: 12px; }
    .adder button { padding: 6px 10px; font-size: 12px; }
</style>
