<script lang="ts">
    import { onDestroy, onMount } from "svelte";
    import { get } from "svelte/store";
    import LucideRefreshCw from "~icons/lucide/refresh-cw";
    import LucideFolderOpen from "~icons/lucide/folder-open";
    import LucidePlus from "~icons/lucide/plus";
    import { processing, processName } from "../store";
    import { openFolder as showFolder } from "../utils";
    import { selectedInstallation, type InspectedInstallation } from "../installationProfiles";
    import PermissionNodeList from "./PermissionNodeList.svelte";
    import {
        checkAdminPermissions, readAdminPermissions, writeAdminPermissions,
        type AdminAssignment, type AdminPermissionsState, type AdminPolicy, type AdminPolicyCheck, type AdminRole,
    } from "./adminPermissions";

    export let installation: InspectedInstallation;

    type Row = { label: string; from: string; to: string };
    const GAME_ROLES = [["owner", "Owner"], ["admin", "Admin"], ["trusted", "Trusted"], ["guest", "Guest"]] as const;

    let busy = false;
    let ownsOperation = false;
    let destroyed = false;
    let generation = 0;
    let state: AdminPermissionsState | null = null;
    let original: AdminPolicy | null = null;
    let draft: AdminPolicy | null = null;
    let check: AdminPolicyCheck | null = null;
    let checkedKey = "";
    let checkTimer: ReturnType<typeof setTimeout> | undefined;
    let reviewing = false;
    let filter = "";
    let error = "";
    let notice = "";

    function keyOf(value: InspectedInstallation | null): string {
        return value ? JSON.stringify([value.target.id, value.target.kind, value.target.executable, value.root, value.available]) : "";
    }

    function clear() {
        state = null;
        original = null;
        draft = null;
        check = null;
        checkedKey = "";
        reviewing = false;
    }

    // Another Manager operation keeps the draft: saving still refuses a file that changed meanwhile.
    const unsubscribe = processing.subscribe(value => {
        if (!value || ownsOperation) return;
        generation++;
        checkedKey = "";
    });
    const unsubscribeSelection = selectedInstallation.subscribe(value => {
        if (keyOf(value) === keyOf(installation)) return;
        generation++;
        clear();
    });

    $: available = installation.available && !!installation.root && keyOf($selectedInstallation) === keyOf(installation);
    $: editable = available && !!state?.revision && (state.state === "valid" || state.state === "invalid");
    $: roleIds = draft?.roles.map(role => role.id) ?? [];
    $: draftKey = draft ? JSON.stringify(draft) : "";
    $: dirty = !!draft && !!original && draftKey !== JSON.stringify(original);
    $: localProblems = draft ? numberProblems(draft) : [];
    $: if (editable && draftKey && !$processing) scheduleCheck(draftKey);
    $: checking = !!draft && !localProblems.length && checkedKey !== draftKey;
    $: problems = localProblems.length ? localProblems : check?.problems ?? [];
    $: rows = original && draft ? differences(original, draft) : [];
    $: if (!rows.length || problems.length || checking) reviewing = false;
    $: nodeNames = (state?.nodes ?? []).filter(node => !node.hostOnly).map(node => node.node).sort();
    $: patterns = ["*", ...new Set(nodeNames.flatMap(node => node.split(".").slice(0, -1)
        .map((_, index, parts) => parts.slice(0, index + 1).join(".") + ".*")))].sort();
    $: sensitive = (state?.nodes ?? []).filter(node => node.sensitive).map(node => node.node);
    $: shown = (draft?.assignments ?? []).map((assignment, index) => ({ assignment, index }))
        .filter(({ assignment }) => matches(assignment, filter));

    function numberProblems(policy: AdminPolicy): string[] {
        const result: string[] = [];
        const rank = (value: number) => Number.isInteger(value) && value >= 0 && value <= 999;
        if (!rank(policy.defaults.remoteMinimumRank)) result.push("The lowest rank for remote F7 must be a whole number from 0 to 999.");
        for (const role of policy.roles) {
            if (!rank(role.rank)) result.push(`Role ${role.id || "(no id)"}: the rank must be a whole number from 0 to 999.`);
        }
        return result;
    }

    // Admin's own rules decide validity, so the backend checks every draft.
    function scheduleCheck(key: string) {
        clearTimeout(checkTimer);
        if (localProblems.length || checkedKey === key) return;
        checkTimer = setTimeout(async () => {
            if (!draft || JSON.stringify(draft) !== key) return;
            const target = installation.target;
            const request = generation;
            try {
                const result = await checkAdminPermissions(target.executable, target.id, outgoing(draft));
                if (request === generation && !destroyed && draftKey === key) { check = result; checkedKey = key; }
            } catch (failure) {
                if (request === generation && !destroyed && draftKey === key) {
                    check = { problems: [`Could not check the permissions: ${failure}`], warnings: [] };
                    checkedKey = key;
                }
            }
        }, 250);
    }

    // Drops what Admin leaves out of an assignment when it is empty.
    function outgoing(policy: AdminPolicy): AdminPolicy {
        return {
            ...policy,
            assignments: policy.assignments.map(assignment => {
                const result: AdminAssignment = { steamId: assignment.steamId };
                if (assignment.role) result.role = assignment.role;
                if (assignment.allow?.length) result.allow = assignment.allow;
                if (assignment.deny?.length) result.deny = assignment.deny;
                if (assignment.note) result.note = assignment.note;
                if (assignment.expiresUtc) result.expiresUtc = assignment.expiresUtc;
                if (assignment.addedBy) result.addedBy = assignment.addedBy;
                if (assignment.addedUtc) result.addedUtc = assignment.addedUtc;
                return result;
            }),
        };
    }

    function roleName(id: string | null | undefined, policy: AdminPolicy | null = draft): string {
        if (!id) return "None";
        const role = policy?.roles.find(role => role.id === id);
        return role ? `${role.name} (${id})` : id;
    }

    function list(values: string[] | undefined): string {
        return values?.length ? values.join(", ") : "None";
    }

    function changes(before: string[] | undefined, after: string[] | undefined): string {
        const added = (after ?? []).filter(value => !(before ?? []).includes(value)).map(value => `+${value}`);
        const removed = (before ?? []).filter(value => !(after ?? []).includes(value)).map(value => `−${value}`);
        return [...added, ...removed].join(", ");
    }

    function when(utc: string | null | undefined): string {
        if (!utc) return "Never";
        const date = new Date(utc);
        return Number.isNaN(date.getTime()) ? utc : date.toLocaleString();
    }

    function differences(from: AdminPolicy, to: AdminPolicy): Row[] {
        const result: Row[] = [];
        const row = (label: string, before: string, after: string) => { if (before !== after) result.push({ label, from: before, to: after }); };
        row("Remote F7 staff access", from.defaults.remoteOperators ? "On" : "Off", to.defaults.remoteOperators ? "On" : "Off");
        row("Lowest rank for remote F7", String(from.defaults.remoteMinimumRank), String(to.defaults.remoteMinimumRank));
        row("Role for every player", roleName(from.defaults.unassignedRole, from), roleName(to.defaults.unassignedRole, to));
        for (const [key, label] of GAME_ROLES) {
            row(`Game ${label} role`, roleName(from.gameRoleMap[key], from), roleName(to.gameRoleMap[key], to));
        }
        const summary = (role: AdminRole) => `Rank ${role.rank}, ${role.allow.length} allowed, ${role.deny.length} denied`;
        for (const role of from.roles) {
            if (!to.roles.some(other => other.id === role.id)) result.push({ label: `Role ${role.name} (${role.id})`, from: summary(role), to: "Removed" });
        }
        for (const role of to.roles) {
            const before = from.roles.find(other => other.id === role.id);
            const label = `Role ${role.name || role.id} (${role.id})`;
            if (!before) { result.push({ label, from: "—", to: summary(role) }); continue; }
            row(`${label}: name`, before.name, role.name);
            row(`${label}: rank`, String(before.rank), String(role.rank));
            row(`${label}: inherits`, list(before.inherits), list(role.inherits));
            const allow = changes(before.allow, role.allow);
            if (allow) result.push({ label: `${label}: allow`, from: `${before.allow.length} entries`, to: allow });
            const deny = changes(before.deny, role.deny);
            if (deny) result.push({ label: `${label}: deny`, from: `${before.deny.length} entries`, to: deny });
        }
        const describe = (assignment: AdminAssignment) => [roleName(assignment.role, to),
            assignment.allow?.length ? `+${assignment.allow.length} allowed` : "", assignment.deny?.length ? `${assignment.deny.length} denied` : "",
            assignment.expiresUtc ? `until ${when(assignment.expiresUtc)}` : ""].filter(Boolean).join(", ");
        for (const assignment of from.assignments) {
            if (!to.assignments.some(other => other.steamId === assignment.steamId)) {
                result.push({ label: `Player ${assignment.steamId}`, from: describe(assignment), to: "Removed" });
            }
        }
        for (const assignment of to.assignments) {
            const before = from.assignments.find(other => other.steamId === assignment.steamId);
            const label = `Player ${assignment.steamId || "(no SteamID64)"}`;
            if (!before) { result.push({ label, from: "—", to: describe(assignment) }); continue; }
            row(`${label}: role`, roleName(before.role, from), roleName(assignment.role, to));
            const allow = changes(before.allow, assignment.allow);
            if (allow) result.push({ label: `${label}: extra allow`, from: list(before.allow), to: allow });
            const deny = changes(before.deny, assignment.deny);
            if (deny) result.push({ label: `${label}: extra deny`, from: list(before.deny), to: deny });
            row(`${label}: note`, before.note || "None", assignment.note || "None");
            row(`${label}: expires`, when(before.expiresUtc), when(assignment.expiresUtc));
        }
        return result;
    }

    function matches(assignment: AdminAssignment, text: string): boolean {
        const query = text.trim().toLowerCase();
        return !query || [assignment.steamId, assignment.note ?? "", assignment.role ?? "", roleName(assignment.role)]
            .some(value => value.toLowerCase().includes(query));
    }

    function expired(assignment: AdminAssignment): boolean {
        return !!assignment.expiresUtc && Date.parse(assignment.expiresUtc) <= Date.now();
    }

    function nowUtc(): string {
        return new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
    }

    // Expiry is edited in local time and stored as UTC.
    function localInput(utc: string | null | undefined): string {
        const date = utc ? new Date(utc) : null;
        if (!date || Number.isNaN(date.getTime())) return "";
        const pad = (value: number) => String(value).padStart(2, "0");
        return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
    }

    function utcFrom(local: string): string | null {
        const date = local ? new Date(local) : null;
        return date && !Number.isNaN(date.getTime()) ? date.toISOString().replace(/\.\d{3}Z$/, "Z") : null;
    }

    // Each-block fields update the draft here rather than through bind:, which in legacy mode
    // would put the block's own variables into every other draft binding's update.
    function setDefaults(patch: Partial<AdminPolicy["defaults"]>) {
        if (draft) draft = { ...draft, defaults: { ...draft.defaults, ...patch } };
    }

    function setGameRole(key: keyof AdminPolicy["gameRoleMap"], value: string) {
        if (draft) draft = { ...draft, gameRoleMap: { ...draft.gameRoleMap, [key]: value || null } };
    }

    function setRole(index: number, patch: Partial<AdminRole>) {
        if (draft) draft = { ...draft, roles: draft.roles.map((role, other) => other === index ? { ...role, ...patch } : role) };
    }

    // A changed id follows into everything that refers to the role, unless the old or new id is
    // empty or shared with another role, where moving references would mix two roles up.
    function renameRole(index: number, next: string) {
        if (!draft) return;
        const previous = draft.roles[index].id;
        const id = next.trim();
        if (id === previous) return;
        const follow = !!id && !!previous
            && !draft.roles.some((role, other) => other !== index && (role.id === id || role.id === previous));
        const swap = (value: string | null | undefined) => follow && value === previous ? id : value;
        draft = {
            ...draft,
            defaults: { ...draft.defaults, unassignedRole: swap(draft.defaults.unassignedRole) || null },
            gameRoleMap: Object.fromEntries(Object.entries(draft.gameRoleMap).map(([key, value]) => [key, swap(value) || null])) as AdminPolicy["gameRoleMap"],
            roles: draft.roles.map((role, other) => ({
                ...role, id: other === index ? id : role.id, inherits: role.inherits.map(parent => swap(parent) ?? parent),
            })),
            assignments: draft.assignments.map(assignment => ({ ...assignment, role: swap(assignment.role) })),
        };
    }

    function toggleInherit(index: number, parent: string, on: boolean) {
        if (!draft) return;
        const inherits = draft.roles[index].inherits.filter(value => value !== parent);
        setRole(index, { inherits: on ? [...inherits, parent] : inherits });
    }

    function addRole() {
        if (!draft) return;
        const ids = new Set(draft.roles.map(role => role.id));
        const ranks = new Set(draft.roles.map(role => role.rank));
        let number = 1;
        while (ids.has(number === 1 ? "new-role" : `new-role-${number}`)) number++;
        let rank = 1;
        while (ranks.has(rank)) rank++;
        const role: AdminRole = { id: number === 1 ? "new-role" : `new-role-${number}`, name: "New role", rank, inherits: [], allow: [], deny: [] };
        draft = { ...draft, roles: [...draft.roles, role] };
    }

    // Removing a role clears inheritance and game-role mappings to it. Assignments keep it and are
    // flagged, so each player is reassigned on purpose.
    function removeRole(index: number) {
        if (!draft) return;
        const id = draft.roles[index].id;
        const keep = (value: string | null) => value === id ? null : value;
        draft = {
            ...draft,
            defaults: { ...draft.defaults, unassignedRole: keep(draft.defaults.unassignedRole) },
            gameRoleMap: Object.fromEntries(Object.entries(draft.gameRoleMap).map(([key, value]) => [key, keep(value)])) as AdminPolicy["gameRoleMap"],
            roles: draft.roles.filter((_, other) => other !== index)
                .map(role => ({ ...role, inherits: role.inherits.filter(parent => parent !== id) })),
        };
    }

    function setAssignment(index: number, patch: Partial<AdminAssignment>) {
        if (draft) draft = { ...draft, assignments: draft.assignments.map((assignment, other) => other === index ? { ...assignment, ...patch } : assignment) };
    }

    function addAssignment() {
        if (!draft) return;
        const lowest = [...draft.roles].sort((a, b) => a.rank - b.rank)[0];
        filter = "";
        draft = { ...draft, assignments: [...draft.assignments, { steamId: "", role: lowest?.id ?? null, addedBy: "manager", addedUtc: nowUtc() }] };
    }

    function removeAssignment(index: number) {
        if (draft) draft = { ...draft, assignments: draft.assignments.filter((_, other) => other !== index) };
    }

    function same(a: string | null | undefined, b: string | null | undefined): boolean {
        return !!a && !!b && a.toLowerCase() === b.toLowerCase();
    }

    function verdict(current: AdminPermissionsState): { tone: string; text: string } {
        const status = current.status;
        if (current.statusProblem) return { tone: "warning", text: current.statusProblem };
        if (!status) return { tone: "", text: "Admin hasn't reported on this file yet. Admin 0.5.216 or later reports here after it reads the file." };
        const checked = when(status.checkedUtc);
        if (!same(status.sha256, current.revision)) {
            return { tone: "", text: `Admin hasn't read the current file yet. While the game or server runs it picks up changes within about 5 seconds; otherwise at its next start. Last check: ${checked}.` };
        }
        if (!status.valid) {
            return { tone: "error", text: `Admin rejected the current file${status.error ? `: ${status.error}` : ""}. ${status.activeSha256
                ? "The rules it last accepted stay active." : "It is using its built-in rules until the file is fixed."} Checked ${checked}.` };
        }
        return same(status.activeSha256, current.revision)
            ? { tone: "good", text: `Admin is using the current file. Checked ${checked}.` }
            : { tone: "", text: `Admin accepted the current file but is still using other rules. Checked ${checked}.` };
    }

    function canStart(): boolean {
        return !destroyed && !busy && !get(processing) && available;
    }

    function current(request: number, key: string): boolean {
        return !destroyed && request === generation && keyOf(installation) === key
            && keyOf(get(selectedInstallation)) === key && (ownsOperation || !get(processing));
    }

    function load(result: AdminPermissionsState) {
        state = result;
        filter = "";
        original = result.policy ? structuredClone(result.policy) : null;
        draft = result.policy ? structuredClone(result.policy) : null;
        check = { problems: result.problems, warnings: result.warnings };
        checkedKey = result.policy ? JSON.stringify(result.policy) : "";
        reviewing = false;
    }

    async function read() {
        if (!canStart()) return;
        const target = installation.target;
        const key = keyOf(installation);
        const request = ++generation;
        busy = true;
        clear();
        error = "";
        notice = "";
        try {
            const result = await readAdminPermissions(target.executable, target.id);
            if (current(request, key)) load(result);
        } catch (failure) {
            if (current(request, key)) error = `Could not read staff permissions: ${failure}`;
        } finally { busy = false; }
    }

    // Admin reports on a saved file within a few seconds while the game or server runs.
    async function followStatus(request: number, key: string, revision: string) {
        for (let attempt = 0; attempt < 6; attempt++) {
            await new Promise(resolve => setTimeout(resolve, 2500));
            if (!current(request, key) || busy || !state) return;
            try {
                const fresh = await readAdminPermissions(installation.target.executable, installation.target.id);
                if (!current(request, key) || !state || state.revision !== revision) return;
                if (fresh.revision !== revision) {
                    notice = "permissions.json changed again after the save. Read it again to see the current file.";
                    return;
                }
                state = { ...state, status: fresh.status, statusProblem: fresh.statusProblem };
                if (same(fresh.status?.sha256, revision)) return;
            } catch { return; }
        }
    }

    async function openFolder() {
        if (!state) return;
        try {
            await showFolder(state.folder);
        } catch (failure) {
            error = `Could not open the folder: ${failure}`;
        }
    }

    async function save() {
        if (!canStart() || !editable || !reviewing || !draft || !state?.revision || problems.length || checking || !rows.length) return;
        const target = installation.target;
        const key = keyOf(installation);
        const request = ++generation;
        const policy = outgoing(draft);
        const revision = state.revision;
        busy = true;
        ownsOperation = true;
        error = "";
        notice = "";
        processName.set("Saving staff permissions...");
        processing.set(true);
        let saved: string | null = null;
        try {
            const result = await writeAdminPermissions(target.executable, target.id, revision, policy);
            if (current(request, key)) {
                load(result.state);
                notice = result.changed
                    ? `Saved staff permissions.${result.backupPath ? ` The previous file is backed up at ${result.backupPath}.` : ""}`
                    : "permissions.json already had these rules.";
                saved = result.changed ? result.revision : null;
            }
        } catch (failure) {
            if (current(request, key)) error = `Staff permissions were not saved: ${failure}`;
        } finally {
            processName.set("");
            processing.set(false);
            ownsOperation = false;
            busy = false;
        }
        if (saved) void followStatus(generation, key, saved);
    }

    onMount(() => { void read(); });
    onDestroy(() => { destroyed = true; generation++; clearTimeout(checkTimer); unsubscribe(); unsubscribeSelection(); });
</script>

<div class="admin-permissions">
    <section class="panel intro" aria-labelledby="permissions-heading">
        <div class="heading-row">
            <div>
                <p class="eyebrow">Modstoke Admin</p>
                <h2 id="permissions-heading">Staff permissions</h2>
            </div>
            <div class="actions">
                {#if state && state.state !== "no-folder"}
                    <button type="button" disabled={busy} on:click={openFolder}><LucideFolderOpen aria-hidden="true" /> Open folder</button>
                {/if}
                <button type="button" disabled={busy || $processing || !available} on:click={read}>
                    <LucideRefreshCw aria-hidden="true" /> {busy ? "Working…" : state ? "Read again" : "Read permissions"}
                </button>
            </div>
        </div>
        <p class="note">Roles and staff for this installation's Admin, saved to permissions.json. Admin enforces them on the host, picks up a saved file within about 5 seconds while the game or server runs, and keeps the rules it last accepted if the file has a problem.</p>
        {#if state}
            <p class="note">{state.adminVersion ? `Admin ${state.adminVersion} installed.` : "Admin's package was not found in the Loader's Mods folder."} File: <span class="path">{state.path}</span></p>
            {#if state.state === "valid" || state.state === "invalid"}
                {@const status = verdict(state)}
                <div class="status-notes {status.tone}" role="status">
                    <p>{status.text}</p>
                    {#each (state.status && same(state.status.sha256, state.revision) ? state.status.warnings : []) as warning}<p>{warning}</p>{/each}
                </div>
            {/if}
        {/if}
    </section>

    {#if error}<p class="message error" role="alert">{error}</p>{/if}
    {#if notice}<p class="message notice" role="status">{notice}</p>{/if}

    {#if state?.state === "no-folder"}
        <p class="message notice">Admin hasn't run in this installation yet. Install Modstoke Admin 0.5.216 or later and start the game or server once; Admin then creates its folder and permissions.json.</p>
    {:else if state?.state === "missing"}
        <p class="message notice">There is no permissions.json yet. Admin 0.5.216 or later creates it on its next start and moves any operators from settings.ini into it.</p>
    {:else if state && !state.policy}
        <section class="panel">
            <h3>{state.state === "unsupported" ? "Newer permissions file" : "Unreadable permissions file"}</h3>
            {#each state.problems as problem}<p class="field-error">{problem}</p>{/each}
            <p class="note">Fix the file in a text editor or in Admin's F7 Permissions tab, then read it again. Admin keeps the rules it last accepted meanwhile, or its built-in rules after a restart.</p>
        </section>
    {/if}

    {#if draft && state}
        <datalist id="admin-permission-nodes">{#each nodeNames as node}<option value={node}></option>{/each}</datalist>
        <datalist id="admin-permission-patterns">{#each [...patterns, ...nodeNames] as node}<option value={node}></option>{/each}</datalist>
        {#if state.nodesProblem}<p class="message notice">{state.nodesProblem}</p>{/if}
        {#if !state.nodes.length && !state.nodesProblem}
            <p class="message notice">Admin hasn't written its list of permission nodes here yet, so node names can't be suggested or checked. Start Admin 0.5.216 or later once to create it.</p>
        {/if}

        <section class="panel" aria-labelledby="access-heading">
            <h3 id="access-heading">Access</h3>
            <div class="setting-row">
                <label class="setting-label" for="remote-operators">Remote F7 staff access</label>
                <div class="setting-content">
                    <select id="remote-operators" value={String(draft.defaults.remoteOperators)} disabled={busy || $processing || !editable}
                        on:change={event => setDefaults({ remoteOperators: event.currentTarget.value === "true" })}>
                        <option value="true">On</option><option value="false">Off</option>
                    </select>
                    <p class="note">When off, only the host machine, its console and the Command Bridge can use staff tools.</p>
                </div>
            </div>
            <div class="setting-row">
                <label class="setting-label" for="remote-rank">Lowest rank for remote F7</label>
                <div class="setting-content">
                    <input id="remote-rank" type="number" min="0" max="999" step="1" value={draft.defaults.remoteMinimumRank} disabled={busy || $processing || !editable}
                        on:input={event => setDefaults({ remoteMinimumRank: event.currentTarget.valueAsNumber })} />
                    <p class="note">Staff below this rank can't open remote F7 at all. Their role also needs <code>remote.use</code>.</p>
                </div>
            </div>
            <div class="setting-row">
                <label class="setting-label" for="unassigned-role">Role for every player</label>
                <div class="setting-content">
                    <select id="unassigned-role" value={draft.defaults.unassignedRole ?? ""} disabled={busy || $processing || !editable}
                        on:change={event => setDefaults({ unassignedRole: event.currentTarget.value || null })}>
                        <option value="">None</option>
                        {#each roleIds as id}<option value={id}>{roleName(id)}</option>{/each}
                    </select>
                    <p class="note">A floor every player gets, including the game's Guest and Trusted players. Banned players always get nothing.</p>
                </div>
            </div>
            <h4>The game's own roles</h4>
            <p class="note section-note">A player's role is the highest-ranked of their assignment below, their game role mapped here, and the role for every player. Map from the top down: a game role can have a role only when the game roles above it do, and never one that ranks above theirs.</p>
            <div class="role-map">
                {#each GAME_ROLES as [key, label]}
                    <label class="map-field">
                        <span>Game {label}</span>
                        <select value={draft.gameRoleMap[key] ?? ""} disabled={busy || $processing || !editable}
                            on:change={event => setGameRole(key, event.currentTarget.value)}>
                            <option value="">No role</option>
                            {#each roleIds as id}<option value={id}>{roleName(id)}</option>{/each}
                        </select>
                    </label>
                {/each}
            </div>
        </section>

        <section class="panel" aria-labelledby="roles-heading">
            <div class="heading-row">
                <h3 id="roles-heading">Roles</h3>
                <button type="button" disabled={busy || $processing || !editable || draft.roles.length >= 32} on:click={addRole}><LucidePlus aria-hidden="true" /> Add role</button>
            </div>
            <p class="note section-note">Staff can only act on players ranked below them. Deny always beats allow. A pattern like <code>moderate.*</code> or <code>*</code> also covers nodes a later Admin version adds{sensitive.length ? `, but never the sensitive nodes ${sensitive.join(", ")}, which have to be named exactly` : ", but never sensitive nodes, which have to be named exactly"}.</p>
            <p class="note section-note">From Admin 0.5.217, staff with <code>permissions.edit</code> can also edit roles in F7: only roles ranked below their own that no game role or the role for every player uses, and only with nodes their own lasting role holds. Admin refuses to delete a role anything still uses; here, removing a role also clears its mappings and inheritance.</p>
            {#each draft.roles as role, index}
                <details class="role">
                    <summary><strong>{role.name || "Unnamed role"}</strong> <code>{role.id}</code><span>Rank {role.rank} · {role.allow.length} allowed · {role.deny.length} denied{role.inherits.length ? ` · inherits ${role.inherits.join(", ")}` : ""}</span></summary>
                    <div class="role-fields">
                        <label class="field"><span>Name</span>
                            <input type="text" maxlength="48" value={role.name} disabled={busy || $processing || !editable}
                                on:input={event => setRole(index, { name: event.currentTarget.value })} /></label>
                        <label class="field"><span>Id</span>
                            <input type="text" maxlength="32" spellcheck="false" value={role.id} disabled={busy || $processing || !editable}
                                on:change={event => renameRole(index, event.currentTarget.value)} /></label>
                        <label class="field"><span>Rank</span>
                            <input type="number" min="0" max="999" step="1" value={role.rank} disabled={busy || $processing || !editable}
                                on:input={event => setRole(index, { rank: event.currentTarget.valueAsNumber })} /></label>
                    </div>
                    <div class="role-part">
                        <span class="part-label">Inherits</span>
                        <div class="inherits">
                            {#each draft.roles.filter(other => other.id !== role.id && (other.rank < role.rank || role.inherits.includes(other.id))) as parent}
                                <label class="inline-check"><input type="checkbox" checked={role.inherits.includes(parent.id)} disabled={busy || $processing || !editable}
                                    on:change={event => toggleInherit(index, parent.id, event.currentTarget.checked)} /> {parent.name} ({parent.id})</label>
                            {:else}
                                {#if !role.inherits.some(parent => parent === role.id || !roleIds.includes(parent))}<span class="note">No lower-ranked roles.</span>{/if}
                            {/each}
                            {#each role.inherits.filter(parent => parent === role.id || !roleIds.includes(parent)) as parent}
                                <label class="inline-check"><input type="checkbox" checked disabled={busy || $processing || !editable}
                                    on:change={() => toggleInherit(index, parent, false)} /> {parent || "(empty)"} ({parent === role.id ? "itself" : "missing"})</label>
                            {/each}
                        </div>
                    </div>
                    <div class="role-part">
                        <span class="part-label">Allow</span>
                        <PermissionNodeList nodes={role.allow} catalog={state.nodes} label={`${role.id} allow`} suggestions="admin-permission-patterns"
                            disabled={busy || $processing || !editable} on:change={event => setRole(index, { allow: event.detail })} />
                    </div>
                    <div class="role-part">
                        <span class="part-label">Deny</span>
                        <PermissionNodeList nodes={role.deny} catalog={state.nodes} label={`${role.id} deny`} suggestions="admin-permission-patterns"
                            disabled={busy || $processing || !editable} on:change={event => setRole(index, { deny: event.detail })} />
                    </div>
                    <div class="actions">
                        <button type="button" class="danger-button" disabled={busy || $processing || !editable || draft.roles.length === 1}
                            title={draft.roles.length === 1 ? "Admin needs at least one role." : undefined} on:click={() => removeRole(index)}>Remove role</button>
                    </div>
                </details>
            {/each}
        </section>

        <section class="panel" aria-labelledby="staff-heading">
            <div class="heading-row">
                <h3 id="staff-heading">Staff</h3>
                <div class="actions">
                    {#if draft.assignments.length > 6 || filter}<input type="search" placeholder="Find a player" aria-label="Find a player" bind:value={filter} />{/if}
                    <button type="button" disabled={busy || $processing || !editable || draft.assignments.length >= 512} on:click={addAssignment}><LucidePlus aria-hidden="true" /> Add player</button>
                </div>
            </div>
            <p class="note section-note">Give a player a role by their SteamID64, the 17-digit number in their Steam profile link. Extra allow entries name exact nodes; extra deny entries may use patterns. An expired entry counts for nothing. While a player's game role is Guest, for example after a quarantine, their role and extra allow entries here are set aside, but the extra denies still apply.</p>
            <p class="note section-note">Staff with <code>permissions.assign</code> can also give roles in F7: only roles ranked below their own whose nodes they hold, only to players ranked below them, and never to themselves.</p>
            {#each shown as { assignment, index } (index)}
                <div class="assignment" class:expired={expired(assignment)}>
                    <div class="assignment-fields">
                        <label class="field"><span>SteamID64</span>
                            <input type="text" inputmode="numeric" maxlength="17" spellcheck="false" placeholder="7656119…" value={assignment.steamId}
                                disabled={busy || $processing || !editable} on:input={event => setAssignment(index, { steamId: event.currentTarget.value })} /></label>
                        <label class="field"><span>Role</span>
                            <select value={assignment.role ?? ""} disabled={busy || $processing || !editable}
                                on:change={event => setAssignment(index, { role: event.currentTarget.value || null })}>
                                <option value="">No role</option>
                                {#if assignment.role && !roleIds.includes(assignment.role)}<option value={assignment.role}>{assignment.role} (missing)</option>{/if}
                                {#each roleIds as id}<option value={id}>{roleName(id)}</option>{/each}
                            </select></label>
                        <label class="field"><span>Note</span>
                            <input type="text" maxlength="120" value={assignment.note ?? ""} disabled={busy || $processing || !editable}
                                on:input={event => setAssignment(index, { note: event.currentTarget.value })} /></label>
                        <label class="field"><span>Expires {expired(assignment) ? "(expired)" : ""}</span>
                            <input type="datetime-local" value={localInput(assignment.expiresUtc)} disabled={busy || $processing || !editable}
                                on:change={event => setAssignment(index, { expiresUtc: utcFrom(event.currentTarget.value) })} /></label>
                    </div>
                    <details class="extra">
                        <summary>Extra nodes{assignment.allow?.length || assignment.deny?.length ? ` (${(assignment.allow?.length ?? 0) + (assignment.deny?.length ?? 0)})` : ""}</summary>
                        <div class="role-part">
                            <span class="part-label">Allow</span>
                            <PermissionNodeList nodes={assignment.allow ?? []} catalog={state.nodes} label={`${assignment.steamId} allow`} suggestions="admin-permission-nodes" exact
                                disabled={busy || $processing || !editable} on:change={event => setAssignment(index, { allow: event.detail })} />
                        </div>
                        <div class="role-part">
                            <span class="part-label">Deny</span>
                            <PermissionNodeList nodes={assignment.deny ?? []} catalog={state.nodes} label={`${assignment.steamId} deny`} suggestions="admin-permission-patterns"
                                disabled={busy || $processing || !editable} on:change={event => setAssignment(index, { deny: event.detail })} />
                        </div>
                    </details>
                    <div class="assignment-footer">
                        <span class="note">{assignment.addedBy ? `Added by ${assignment.addedBy}` : "Added"}{assignment.addedUtc ? ` on ${when(assignment.addedUtc)}` : ""}. Expiry is shown in your local time.</span>
                        <button type="button" class="danger-button" disabled={busy || $processing || !editable} on:click={() => removeAssignment(index)}>Remove</button>
                    </div>
                </div>
            {:else}
                <p class="note">{draft.assignments.length ? "No player matches." : "No players have a role yet."}</p>
            {/each}
        </section>

        <section class="panel review-bar" aria-label="Review staff permissions">
            {#each problems as problem}<p class="field-error">{problem}</p>{/each}
            {#each (check && checkedKey === draftKey ? check.warnings : []) as warning}<p class="field-warning">{warning}</p>{/each}
            {#if reviewing}
                <h3>Review {rows.length} {rows.length === 1 ? "change" : "changes"}</h3>
                <table>
                    <thead><tr><th>Setting</th><th>Current</th><th>New</th></tr></thead>
                    <tbody>{#each rows as row}<tr><td>{row.label}</td><td>{row.from}</td><td>{row.to}</td></tr>{/each}</tbody>
                </table>
                <p class="note">The current file is backed up first. If the file changed since it was read, in F7 or elsewhere, nothing is saved.</p>
                <div class="actions">
                    <button class="primary-button" type="button" disabled={busy || $processing || !editable} on:click={save}>Save permissions</button>
                    <button type="button" disabled={busy || $processing} on:click={() => reviewing = false}>Cancel</button>
                </div>
            {:else}
                <div class="actions">
                    <button class="primary-button" type="button" disabled={busy || $processing || !editable || !rows.length || !!problems.length || checking}
                        on:click={() => reviewing = true}>{checking ? "Checking…" : `Review changes${rows.length ? ` (${rows.length})` : ""}`}</button>
                    <button type="button" disabled={busy || $processing || !dirty} on:click={() => { if (original) draft = structuredClone(original); }}>Discard changes</button>
                </div>
            {/if}
        </section>
    {/if}
</div>

<style>
    .admin-permissions { display: flex; flex-direction: column; gap: 16px; min-width: 0; text-align: left; }
    .panel { min-width: 0; padding: 20px 22px; background: var(--surface-panel); border: 1px solid var(--border-subtle); border-radius: 10px; }
    .heading-row { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 12px 16px; }
    .eyebrow { margin: 0 0 6px; color: var(--text-secondary); font-size: 11px; font-weight: 600; letter-spacing: 1px; text-transform: uppercase; }
    h2 { margin: 0; font-size: 19px; }
    h3 { margin: 0 0 6px; font-size: 15px; }
    h4 { margin: 18px 0 6px; color: var(--text-secondary); font-size: 11px; font-weight: 600; letter-spacing: 1px; text-transform: uppercase; }
    button { display: inline-flex; justify-content: center; align-items: center; gap: 7px; flex-shrink: 0; }
    button :global(svg) { width: 15px; height: 15px; }
    .primary-button { color: #251c10; background: var(--accent); border-color: var(--accent); }
    .primary-button:enabled:hover { color: #251c10; background: #ffd296; border-color: #ffd296; }
    .danger-button { color: #ffb4ac; }
    .note { margin: 8px 0 0; color: var(--text-secondary); font-size: 12px; line-height: 1.6; overflow-wrap: anywhere; }
    .section-note { margin: 0 0 12px; }
    .path { user-select: text; }
    .status-notes { margin-top: 14px; padding: 12px 14px; background: var(--surface-raised); border-left: 2px solid var(--border-subtle); border-radius: 3px; }
    .status-notes p { margin: 0; font-size: 12px; line-height: 1.6; overflow-wrap: anywhere; }
    .status-notes p + p { margin-top: 6px; color: var(--text-secondary); }
    .status-notes.good { border-left-color: #7fc79a; }
    .status-notes.warning { border-left-color: var(--accent); }
    .status-notes.error { border-left-color: #ffb4ac; background: rgba(168, 61, 51, .1); }
    .message { margin: 0; padding: 14px 16px; font-size: 13px; line-height: 1.6; border: 1px solid var(--border-subtle); border-radius: 8px; overflow-wrap: anywhere; }
    .message.error { color: #ffb4ac; background: rgba(168, 61, 51, .1); }
    .message.notice { color: var(--text-primary); background: var(--surface-raised); }
    .setting-row { display: grid; grid-template-columns: 170px minmax(0, 1fr); gap: 20px; padding: 16px 0; border-top: 1px solid var(--border-subtle); }
    h3 + .setting-row { margin-top: 12px; }
    .setting-label { padding-top: 9px; font-size: 13px; font-weight: 600; }
    .setting-content { min-width: 0; }
    .setting-content select, .setting-content input { max-width: 260px; width: 100%; box-sizing: border-box; }
    .role-map { display: grid; grid-template-columns: repeat(auto-fill, minmax(190px, 1fr)); gap: 12px 16px; }
    .map-field, .field { display: grid; gap: 6px; min-width: 0; color: var(--text-secondary); font-size: 12px; }
    .map-field select, .field input, .field select { width: 100%; min-width: 0; box-sizing: border-box; }
    .role { margin-top: 10px; border: 1px solid var(--border-subtle); border-radius: 8px; background: var(--surface-base); }
    .role > summary { padding: 12px 14px; cursor: pointer; font-size: 13px; overflow-wrap: anywhere; }
    .role > summary code { margin: 0 6px; color: var(--text-secondary); font-size: 12px; }
    .role > summary span { color: var(--text-secondary); font-size: 12px; }
    .role[open] > summary { border-bottom: 1px solid var(--border-subtle); }
    .role > :not(summary) { margin: 12px 14px; }
    .role-fields, .assignment-fields { display: grid; grid-template-columns: repeat(auto-fill, minmax(170px, 1fr)); gap: 12px 16px; }
    .role-part { display: grid; grid-template-columns: 70px minmax(0, 1fr); gap: 12px; align-items: start; margin-top: 12px; }
    .part-label { padding-top: 5px; color: var(--text-secondary); font-size: 12px; font-weight: 600; }
    .inherits { display: flex; flex-wrap: wrap; gap: 6px 16px; padding-top: 4px; }
    .inline-check { display: inline-flex; align-items: center; gap: 6px; font-size: 12px; }
    .inline-check input { float: none; flex-shrink: 0; margin: 0; }
    .assignment { padding: 14px 0; border-top: 1px solid var(--border-subtle); }
    .assignment.expired .assignment-fields { opacity: .7; }
    .extra { margin-top: 10px; }
    .extra > summary { cursor: pointer; color: var(--text-secondary); font-size: 12px; }
    .assignment-footer { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 8px; margin-top: 10px; }
    .assignment-footer .note { margin: 0; }
    .actions { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
    .actions input[type="search"] { width: 200px; }
    .field-error { margin: 0 0 10px; color: #ffb4ac; font-size: 12px; overflow-wrap: anywhere; }
    .field-warning { margin: 0 0 10px; color: #e6c89e; font-size: 12px; overflow-wrap: anywhere; }
    .review-bar h3 { margin-bottom: 12px; }
    .review-bar .note { margin-bottom: 14px; }
    table { width: 100%; border-collapse: collapse; font-size: 12px; text-align: left; }
    th, td { padding: 8px 6px; border-bottom: 1px solid var(--border-subtle); overflow-wrap: anywhere; }
    th { color: var(--text-secondary); font-weight: 600; }
    code { font-size: 12px; }
    @media (max-width: 760px) {
        .panel { padding: 18px; }
        .setting-row { grid-template-columns: minmax(0, 1fr); gap: 8px; }
        .setting-label { padding-top: 0; }
        .role-part { grid-template-columns: minmax(0, 1fr); gap: 6px; }
    }
</style>
