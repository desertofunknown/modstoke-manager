<script lang="ts">
    import { onDestroy, onMount } from "svelte";
    import { get } from "svelte/store";
    import LucideRefreshCw from "~icons/lucide/refresh-cw";
    import { processing, processName } from "./store";
    import { selectedInstallation, type InspectedInstallation } from "./installationProfiles";
    import { readServerLaunch, type ServerLaunchState } from "./serverLaunch";
    import {
        readServerGameSettings, writeServerGameSettings,
        type GameSetting, type GameSettingsChange, type GameSettingsState,
    } from "./serverGameSettings";

    export let installation: InspectedInstallation;

    // A value Manager does not recognize. It stays as it is unless another value is chosen.
    const KEEP = "keep";
    const GAME_MODES: [string, string][] = [
        ["Normal", "Normal"], ["Hard", "Hard"], ["HardSurvival", "Hard survival"],
        ["Peaceful", "Peaceful"], ["Creative", "Creative"], ["Custom", "Custom"],
    ];
    const LABELS: Record<string, [group: string, label: string]> = {
        "Gameplay.TreeRegrowth": ["", "Tree regrowth"],
        "Structure.Damage": ["", "Structure damage"],
        "GameSetting.Vail.EnemySpawn": ["Enemies", "Enemy spawns"],
        "GameSetting.Vail.EnemyHealth": ["Enemies", "Enemy health"],
        "GameSetting.Vail.EnemyDamage": ["Enemies", "Enemy damage"],
        "GameSetting.Vail.EnemyArmour": ["Enemies", "Enemy armour"],
        "GameSetting.Vail.EnemyAggression": ["Enemies", "Enemy aggression"],
        "GameSetting.Vail.EnemySearchParties": ["Enemies", "Search parties"],
        "GameSetting.Vail.AnimalSpawnRate": ["World", "Animal amounts"],
        "GameSetting.Environment.StartingSeason": ["World", "Starting season"],
        "GameSetting.Environment.SeasonLength": ["World", "Season length"],
        "GameSetting.Environment.DayLength": ["World", "Day length"],
        "GameSetting.Environment.PrecipitationFrequency": ["World", "Rain and snow"],
        "GameSetting.Environment.InstantSleepSeasonSwitch": ["World", "Instant season switch on sleep"],
        "GameSetting.Survival.ConsumableEffects": ["Survival", "Consumable effects"],
        "GameSetting.Survival.PlayerStatsDamage": ["Survival", "Survival damage"],
        "GameSetting.Survival.ColdPenalties": ["Survival", "Cold penalties"],
        "GameSetting.Survival.StatRegenerationPenalty": ["Survival", "Stat regeneration penalty"],
        "GameSetting.Survival.ReducedFoodInContainers": ["Survival", "Reduced food in containers"],
        "GameSetting.Survival.ReducedAmmoInContainers": ["Survival", "Reduced ammo in containers"],
        "GameSetting.Survival.SingleUseContainers": ["Survival", "Containers don't refill"],
        "GameSetting.Survival.BuildingResistance": ["Building", "Building resistance"],
        "GameSetting.Construction.NoCuttingsSpawn": ["Building", "No log off-cuts"],
        "GameSetting.FreeForm.ForcePlaceFullLoad": ["Building", "Place a full load at once"],
        "GameSetting.Survival.OneHitToCutTrees": ["Building", "One hit to cut trees"],
        "GameSetting.Survival.CreativeMode": ["Players", "Creative mode"],
        "GameSetting.Survival.PlayersImmortalMode": ["Players", "Immortal players"],
        "GameSetting.Multiplayer.Cheats": ["Players", "Client console (cheats)"],
        "GameSetting.Multiplayer.PvpDamage": ["Players", "PvP damage"],
    };
    const CUSTOM_GROUPS = ["Enemies", "World", "Survival", "Building", "Players"];

    type Draft = {
        gameMode: string; saveMinutes: number; idleSpeed: number; idleRate: number; activeRate: number;
        lanOnly: string; skipTest: string; game: Record<string, string>; custom: Record<string, string>;
    };
    type Row = { label: string; from: string; to: string };

    let busy = false;
    let ownsOperation = false;
    let destroyed = false;
    let generation = 0;
    let launch: ServerLaunchState | null = null;
    let settings: GameSettingsState | null = null;
    let original: Draft | null = null;
    let draft: Draft | null = null;
    let reviewing = false;
    let error = "";
    let notice = "";

    function encodeSetting(setting: GameSetting): string {
        const value = setting.value;
        if (value === null || value === undefined) return "";
        if (!setting.choices) return typeof value === "boolean" ? String(value) : KEEP;
        return typeof value === "string"
            ? setting.choices.find(choice => choice.toLowerCase() === value.toLowerCase()) ?? KEEP : KEEP;
    }

    // Absent values show the server's built-in defaults; unreadable numbers stay blank.
    function encodeNumber(value: unknown, fallback: number): number {
        return value === null || value === undefined ? fallback : typeof value === "number" && Number.isFinite(value) ? value : NaN;
    }

    function encodeFlag(value: unknown): string {
        return value === null || value === undefined ? "false" : typeof value === "boolean" ? String(value) : KEEP;
    }

    function encode(state: GameSettingsState): Draft {
        const mode = state.gameMode;
        return {
            gameMode: mode === null || mode === undefined ? "Normal" : typeof mode === "string"
                ? GAME_MODES.find(([name]) => name.toLowerCase() === mode.toLowerCase())?.[0] ?? KEEP : KEEP,
            saveMinutes: encodeNumber(state.saveInterval, 600) / 60,
            idleSpeed: encodeNumber(state.idleDayCycleSpeed, 0),
            idleRate: encodeNumber(state.idleTargetFramerate, 5),
            activeRate: encodeNumber(state.activeTargetFramerate, 60),
            lanOnly: encodeFlag(state.lanOnly),
            skipTest: encodeFlag(state.skipNetworkAccessibilityTest),
            game: Object.fromEntries(state.gameSettings.map(setting => [setting.key, encodeSetting(setting)])),
            custom: Object.fromEntries(state.customSettings.map(setting => [setting.key, encodeSetting(setting)])),
        };
    }

    function text(value: string): string {
        return value === "" ? "Game default" : value === "true" ? "On" : value === "false" ? "Off"
            : value === KEEP ? "Current value" : GAME_MODES.find(([name]) => name === value)?.[1] ?? value;
    }

    // Settings in each-blocks update the draft here rather than through bind:, which in legacy
    // mode would put the block's own variable into every other draft binding's update.
    function choose(part: "game" | "custom", key: string, value: string) {
        if (draft) draft = { ...draft, [part]: { ...draft[part], [key]: value } };
    }

    function differences(from: Draft, to: Draft, state: GameSettingsState) {
        const change: GameSettingsChange = {};
        const rows: Row[] = [];
        const problems: string[] = [];
        const number = (label: string, before: number, after: number | null, min: number, max: number, set: (value: number) => void) => {
            if (after === null || Number.isNaN(after) || after === before) return;
            if (after < min || after > max) { problems.push(`${label} must be from ${min} to ${max}.`); return; }
            set(after);
            rows.push({ label, from: Number.isNaN(before) ? "Current value" : String(before), to: String(after) });
        };
        const flag = (label: string, before: string, after: string, set: (value: boolean) => void) => {
            if (after === before || after === KEEP) return;
            set(after === "true");
            rows.push({ label, from: text(before), to: text(after) });
        };
        if (to.gameMode !== from.gameMode && to.gameMode !== KEEP) {
            change.gameMode = to.gameMode;
            rows.push({ label: "Game mode", from: text(from.gameMode), to: text(to.gameMode) });
        }
        number("Autosave interval (minutes)", from.saveMinutes, to.saveMinutes, 1, 1440, value => change.saveInterval = Math.round(value * 60));
        number("Idle day cycle speed", from.idleSpeed, to.idleSpeed, 0, 1, value => change.idleDayCycleSpeed = value);
        number("Idle frame rate", from.idleRate, to.idleRate, 1, 240, value => change.idleTargetFramerate = value);
        number("Active frame rate", from.activeRate, to.activeRate, 10, 240, value => change.activeTargetFramerate = value);
        flag("LAN only", from.lanOnly, to.lanOnly, value => change.lanOnly = value);
        flag("Skip network check", from.skipTest, to.skipTest, value => change.skipNetworkAccessibilityTest = value);
        for (const [field, part, list] of [["gameSettings", "game", state.gameSettings], ["customSettings", "custom", state.customSettings]] as const) {
            for (const setting of list) {
                const before = from[part][setting.key];
                const after = to[part][setting.key];
                if (after === before || after === KEEP) continue;
                (change[field] ??= {})[setting.key] = after === "" ? null : after === "true" ? true : after === "false" ? false : after;
                rows.push({ label: LABELS[setting.key]?.[1] ?? setting.key, from: text(before), to: text(after) });
            }
        }
        return { change, rows, problems };
    }

    function keyOf(value: InspectedInstallation | null): string {
        return value ? JSON.stringify([value.target.id, value.target.kind, value.target.executable, value.root, value.available]) : "";
    }

    function clear() {
        launch = null;
        settings = null;
        original = null;
        draft = null;
        reviewing = false;
    }

    const unsubscribe = processing.subscribe(value => {
        if (!value || ownsOperation) return;
        generation++;
        clear();
        notice = "Read settings again after the current Manager operation finishes.";
    });
    const unsubscribeSelection = selectedInstallation.subscribe(value => {
        if (keyOf(value) === keyOf(installation)) return;
        generation++;
        clear();
    });

    $: available = installation.available && !!installation.root && installation.target.kind === "server"
        && keyOf($selectedInstallation) === keyOf(installation);
    $: editProblem = !launch || !settings ? ""
        : launch.processState !== "stopped" ? "Stop the server and close the game before changing game settings."
        : !launch.canSave ? launch.saveProblem ?? "Settings cannot be changed right now."
        : launch.revision === null || launch.configRevision !== settings.configRevision ? "The configuration changed while it was read. Read settings again."
        : "";
    $: canEdit = available && !!launch && !!settings && !editProblem;
    $: diff = settings && original && draft ? differences(original, draft, settings) : null;
    $: dirty = !!draft && !!original && JSON.stringify(draft) !== JSON.stringify(original);
    $: if (diff && (!diff.rows.length || diff.problems.length)) reviewing = false;

    function canStart(): boolean {
        return !destroyed && !busy && !get(processing) && available;
    }

    function current(request: number, key: string): boolean {
        return !destroyed && request === generation && keyOf(installation) === key
            && keyOf(get(selectedInstallation)) === key && (ownsOperation || !get(processing));
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
            const [launchState, gameState] = await Promise.all([
                readServerLaunch(target.executable, target.id), readServerGameSettings(target.executable, target.id),
            ]);
            if (!current(request, key)) return;
            launch = launchState;
            settings = gameState;
            original = encode(gameState);
            draft = encode(gameState);
        } catch (failure) {
            if (current(request, key)) error = `Could not read game settings: ${failure}`;
        } finally { busy = false; }
    }

    async function save() {
        if (!canStart() || !canEdit || !reviewing || !diff || diff.problems.length || !diff.rows.length
            || !launch?.revision || !settings) return;
        const target = installation.target;
        const key = keyOf(installation);
        const request = ++generation;
        const change = diff.change;
        const revision = launch.revision;
        const configRevision = settings.configRevision;
        busy = true;
        ownsOperation = true;
        error = "";
        notice = "";
        processName.set("Saving dedicated-server game settings...");
        processing.set(true);
        clear();
        let saved = "";
        try {
            const result = await writeServerGameSettings(target.executable, target.id, revision, configRevision, change);
            if (current(request, key)) {
                saved = `${result.changed ? "Saved game settings." : "The game settings already had these values."}`
                    + (result.backupPath ? ` Original configuration backup: ${result.backupPath}.` : "")
                    + " They apply the next time the server starts.";
            }
        } catch (failure) {
            if (current(request, key)) {
                error = `Game settings save could not be confirmed: ${failure}. Read settings again before continuing; the configuration may already have changed.`;
            }
        } finally {
            processName.set("");
            processing.set(false);
            ownsOperation = false;
            busy = false;
        }
        if (saved) {
            await read();
            notice = saved;
        }
    }

    onMount(() => { void read(); });
    onDestroy(() => { destroyed = true; generation++; unsubscribe(); unsubscribeSelection(); });
</script>

<div class="game-settings">
    <section class="panel intro" aria-labelledby="game-settings-heading">
        <div class="heading-row">
            <div>
                <p class="eyebrow">Dedicated server</p>
                <h2 id="game-settings-heading">Game settings</h2>
            </div>
            <button type="button" disabled={busy || $processing || !available} on:click={read}>
                <LucideRefreshCw aria-hidden="true" /> {busy ? "Working…" : settings ? "Read again" : "Read settings"}
            </button>
        </div>
        <p class="note">Saved to dedicatedserver.cfg after a backup of the original. Changes apply the next time the server starts.</p>
        {#if editProblem}<div class="status-notes" role="status"><p>{editProblem}</p></div>{/if}
    </section>

    {#if error}<p class="message error" role="alert">{error}</p>{/if}
    {#if notice}<p class="message notice" role="status">{notice}</p>{/if}

    {#if draft && settings}
        <section class="panel" aria-labelledby="world-heading">
            <h3 id="world-heading">World</h3>
            <div class="setting-row">
                <label class="setting-label" for="game-mode">Game mode</label>
                <div class="setting-content">
                    <select id="game-mode" bind:value={draft.gameMode} disabled={busy || $processing || !canEdit}>
                        {#if original?.gameMode === KEEP}<option value={KEEP}>Current value (not recognized)</option>{/if}
                        {#each GAME_MODES as [name, label]}<option value={name}>{label}</option>{/each}
                    </select>
                    <p class="note">Shapes a new world only. While save slot {launch?.config?.saveSlot ?? "?"} already holds a world and SaveMode is {launch?.config?.saveMode ?? "Continue"}, the server keeps that world's own settings.</p>
                </div>
            </div>
            <div class="setting-row">
                <label class="setting-label" for="save-interval">Autosave interval</label>
                <div class="setting-content">
                    <div class="field-controls"><input id="save-interval" type="number" min="1" max="1440" step="1" bind:value={draft.saveMinutes} disabled={busy || $processing || !canEdit} /><span class="unit">minutes</span></div>
                    <p class="note">How often the server saves the world, from 1 minute to 24 hours.</p>
                </div>
            </div>
            {#each settings.gameSettings as setting (setting.key)}
                <div class="setting-row">
                    <label class="setting-label" for={setting.key}>{LABELS[setting.key]?.[1] ?? setting.key}</label>
                    <div class="setting-content">
                        <select id={setting.key} value={draft.game[setting.key]} disabled={busy || $processing || !canEdit}
                            on:change={event => choose("game", setting.key, event.currentTarget.value)}>
                            {#if original?.game[setting.key] === KEEP}<option value={KEEP}>Current value (not recognized)</option>{/if}
                            <option value="">Game default</option><option value="true">On</option><option value="false">Off</option>
                        </select>
                        <p class="note">Applies every time the server starts, including to an existing world.</p>
                    </div>
                </div>
            {/each}
        </section>

        <section class="panel" aria-labelledby="custom-heading">
            <h3 id="custom-heading">Custom game</h3>
            <p class="note section-note">{draft.gameMode === "Custom"
                ? "Used when the server creates a Custom world. Settings left at Game default use the game's own value."
                : "Used only when Game mode is Custom and the server creates a new world."}</p>
            {#each CUSTOM_GROUPS as group}
                <h4>{group}</h4>
                <div class="custom-grid">
                    {#each settings.customSettings.filter(setting => LABELS[setting.key]?.[0] === group) as setting (setting.key)}
                        <label class="custom-field">
                            <span>{LABELS[setting.key]?.[1] ?? setting.key}</span>
                            <select value={draft.custom[setting.key]} disabled={busy || $processing || !canEdit}
                                on:change={event => choose("custom", setting.key, event.currentTarget.value)}>
                                {#if original?.custom[setting.key] === KEEP}<option value={KEEP}>Current value (not recognized)</option>{/if}
                                <option value="">Game default</option>
                                {#if setting.choices}
                                    {#each setting.choices as choice}<option value={choice}>{choice}</option>{/each}
                                {:else}
                                    <option value="true">On</option><option value="false">Off</option>
                                {/if}
                            </select>
                        </label>
                    {/each}
                </div>
            {/each}
        </section>

        <section class="panel" aria-labelledby="performance-heading">
            <h3 id="performance-heading">Performance and network</h3>
            <div class="setting-row">
                <label class="setting-label" for="idle-speed">Idle day cycle speed</label>
                <div class="setting-content">
                    <input id="idle-speed" type="number" min="0" max="1" step="0.05" bind:value={draft.idleSpeed} disabled={busy || $processing || !canEdit} />
                    <p class="note">How fast time passes while nobody is connected, from 0 (stopped) to 1 (normal).</p>
                </div>
            </div>
            <div class="setting-row">
                <label class="setting-label" for="idle-rate">Idle frame rate</label>
                <div class="setting-content">
                    <input id="idle-rate" type="number" min="1" max="240" step="1" bind:value={draft.idleRate} disabled={busy || $processing || !canEdit} />
                    <p class="note">Server frame rate while empty, from 1 to 240.</p>
                </div>
            </div>
            <div class="setting-row">
                <label class="setting-label" for="active-rate">Active frame rate</label>
                <div class="setting-content">
                    <input id="active-rate" type="number" min="10" max="240" step="1" bind:value={draft.activeRate} disabled={busy || $processing || !canEdit} />
                    <p class="note">Server frame rate while players are connected, from 10 to 240.</p>
                </div>
            </div>
            <div class="setting-row">
                <label class="setting-label" for="lan-only">LAN only</label>
                <div class="setting-content">
                    <select id="lan-only" bind:value={draft.lanOnly} disabled={busy || $processing || !canEdit}>
                        {#if original?.lanOnly === KEEP}<option value={KEEP}>Current value (not recognized)</option>{/if}
                        <option value="false">Off</option><option value="true">On</option>
                    </select>
                    <p class="note">Shows the server on the local network only.</p>
                </div>
            </div>
            <div class="setting-row">
                <label class="setting-label" for="skip-test">Skip network check</label>
                <div class="setting-content">
                    <select id="skip-test" bind:value={draft.skipTest} disabled={busy || $processing || !canEdit}>
                        {#if original?.skipTest === KEEP}<option value={KEEP}>Current value (not recognized)</option>{/if}
                        <option value="false">Off</option><option value="true">On</option>
                    </select>
                    <p class="note">Skips the startup port and listing check. Use it only when that check fails on a network you know works.</p>
                </div>
            </div>
        </section>

        {#if settings.ignoredKeys.length}
            <p class="note ignored">The configuration also has entries the server does not read. They are kept as they are: {settings.ignoredKeys.join(", ")}.</p>
        {/if}

        <section class="panel review-bar" aria-label="Review game settings">
            {#each diff?.problems ?? [] as problem}<p class="field-error">{problem}</p>{/each}
            {#if reviewing && diff}
                <h3>Review {diff.rows.length} {diff.rows.length === 1 ? "change" : "changes"}</h3>
                <table>
                    <thead><tr><th>Setting</th><th>Current</th><th>New</th></tr></thead>
                    <tbody>{#each diff.rows as row}<tr><td>{row.label}</td><td>{row.from}</td><td>{row.to}</td></tr>{/each}</tbody>
                </table>
                <p class="note">The original configuration is backed up before saving. The server stays stopped.</p>
                <div class="actions">
                    <button class="primary-button" type="button" disabled={busy || $processing || !canEdit} on:click={save}>Save game settings</button>
                    <button type="button" disabled={busy || $processing} on:click={() => reviewing = false}>Cancel</button>
                </div>
            {:else}
                <div class="actions">
                    <button class="primary-button" type="button" disabled={busy || $processing || !canEdit || !diff?.rows.length || !!diff?.problems.length}
                        on:click={() => reviewing = true}>Review changes{diff?.rows.length ? ` (${diff.rows.length})` : ""}</button>
                    <button type="button" disabled={busy || $processing || !dirty} on:click={() => { if (original) draft = structuredClone(original); }}>Discard changes</button>
                </div>
            {/if}
        </section>
    {/if}
</div>

<style>
    .game-settings { display: flex; flex-direction: column; gap: 16px; min-width: 0; }
    .panel { min-width: 0; padding: 20px 22px; background: var(--surface-panel); border: 1px solid var(--border-subtle); border-radius: 10px; }
    .heading-row { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 16px; }
    .eyebrow { margin: 0 0 6px; color: var(--text-secondary); font-size: 11px; font-weight: 600; letter-spacing: 1px; text-transform: uppercase; }
    h2 { margin: 0; font-size: 19px; }
    h3 { margin: 0 0 6px; font-size: 15px; }
    h4 { margin: 18px 0 10px; color: var(--text-secondary); font-size: 11px; font-weight: 600; letter-spacing: 1px; text-transform: uppercase; }
    button { display: inline-flex; justify-content: center; align-items: center; gap: 7px; flex-shrink: 0; }
    button :global(svg) { width: 15px; height: 15px; }
    .primary-button { color: #251c10; background: var(--accent); border-color: var(--accent); }
    .primary-button:enabled:hover { color: #251c10; background: #ffd296; border-color: #ffd296; }
    .note { margin: 8px 0 0; color: var(--text-secondary); font-size: 12px; line-height: 1.6; overflow-wrap: anywhere; }
    .section-note { margin-top: 0; }
    .status-notes { margin-top: 16px; padding: 12px 14px; background: var(--accent-soft); border-left: 2px solid var(--accent); border-radius: 3px; }
    .status-notes p { margin: 0; color: #e6c89e; font-size: 12px; line-height: 1.6; }
    .message { margin: 0; padding: 14px 16px; font-size: 13px; line-height: 1.6; border: 1px solid var(--border-subtle); border-radius: 8px; overflow-wrap: anywhere; }
    .message.error { color: #ffb4ac; background: rgba(168, 61, 51, .1); }
    .message.notice { color: var(--text-primary); background: var(--surface-raised); }
    .setting-row { display: grid; grid-template-columns: 170px minmax(0, 1fr); gap: 20px; padding: 16px 0; border-top: 1px solid var(--border-subtle); }
    h3 + .setting-row { margin-top: 12px; }
    .setting-label { padding-top: 9px; font-size: 13px; font-weight: 600; }
    .setting-content { min-width: 0; }
    .setting-content select, .setting-content input { max-width: 260px; width: 100%; box-sizing: border-box; }
    .field-controls { display: flex; align-items: center; gap: 8px; }
    .unit { color: var(--text-secondary); font-size: 12px; }
    .custom-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(210px, 1fr)); gap: 12px 16px; }
    .custom-field { display: grid; gap: 6px; min-width: 0; font-size: 12px; color: var(--text-secondary); }
    .custom-field select { width: 100%; min-width: 0; }
    .ignored { margin: 0 2px; }
    .field-error { margin: 0 0 10px; color: #ffb4ac; font-size: 12px; }
    .actions { display: flex; flex-wrap: wrap; gap: 8px; }
    .review-bar h3 { margin-bottom: 12px; }
    .review-bar .note { margin-bottom: 14px; }
    table { width: 100%; border-collapse: collapse; font-size: 12px; text-align: left; }
    th, td { padding: 8px 6px; border-bottom: 1px solid var(--border-subtle); overflow-wrap: anywhere; }
    th { color: var(--text-secondary); font-weight: 600; }
    @media (max-width: 760px) {
        .panel { padding: 18px; }
        .setting-row { grid-template-columns: minmax(0, 1fr); gap: 8px; }
        .setting-label { padding-top: 0; }
    }
</style>
