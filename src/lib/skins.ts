import { invoke } from "@tauri-apps/api/core";
import { derived, get, writable } from "svelte/store";
import { installationProfiles, profilesReady, type InstallationRegistryState } from "./installationProfiles";

export type Skin = "modstoke" | "sons";
export type SkinChoice = "game" | Skin;

export const SKIN_CHOICES: { id: SkinChoice; label: string; description: string }[] = [
    { id: "game", label: "Match the game", description: "Use the skin made for the game you're managing." },
    { id: "modstoke", label: "Modstoke", description: "Plain dark panels." },
    { id: "sons", label: "Sons of the Forest", description: "The ragged frame and title screen backdrop from the game's menus, read from your game files." },
];

const SKIN_KEY = "openacai-manager-skin";
// Sons of the Forest is the only game the Manager handles so far.
const GAME_SKIN: Skin = "sons";

function savedChoice(): SkinChoice {
    const saved = localStorage.getItem(SKIN_KEY);
    return SKIN_CHOICES.find(choice => choice.id === saved)?.id ?? "game";
}

export const skinChoice = writable<SkinChoice>(savedChoice());
skinChoice.subscribe(choice => localStorage.setItem(SKIN_KEY, choice));

type GameSkinFiles = { frameMask: string; backdrop: string; font: string | null };
export type GameSkinState = { status: "waiting" | "reading" | "ready" | "unavailable"; problem: string };

// The Manager ships none of Endnight's art: the Sons skin's frame and backdrop, and the Roboto font
// the game ships with, come from the player's own installation.
export const gameSkin = writable<GameSkinState>({ status: "waiting", problem: "" });
const SOURCE_KEY = "openacai-manager-game-skin-source";
// The installation read last time, so the skin can load before the installation profiles do.
const remembered = localStorage.getItem(SOURCE_KEY);
let source = "";

export function resolveSkin(choice: SkinChoice, status: GameSkinState["status"]): Skin {
    const skin = choice === "game" ? GAME_SKIN : choice;
    if (skin !== "sons" || status === "ready") return skin;
    // Without the game's files there is no frame to draw. A launch that read them last time keeps
    // the Sons layout while they load, rather than switching skins once they arrive.
    return remembered && status !== "unavailable" ? "sons" : "modstoke";
}

async function readGameSkin(executable: string): Promise<void> {
    source = executable;
    gameSkin.set({ status: "reading", problem: "" });
    try {
        const files = await invoke<GameSkinFiles>("read_game_skin", { executable });
        if (source !== executable) return;
        const sheet = new CSSStyleSheet();
        sheet.replaceSync(`:root { --sons-frame-mask: url("${files.frameMask}"); --sons-backdrop: url("${files.backdrop}"); }`);
        document.adoptedStyleSheets = [...document.adoptedStyleSheets, sheet];
        if (files.font) {
            const font = Uint8Array.from(atob(files.font), character => character.charCodeAt(0));
            document.fonts.add(new FontFace("SonsRoboto", font, { weight: "400 900" }));
        }
        localStorage.setItem(SOURCE_KEY, executable);
        gameSkin.set({ status: "ready", problem: "" });
    } catch (error) {
        if (source !== executable) return;
        // Forgotten, so the next change to the installation profiles tries again.
        source = "";
        gameSkin.set({ status: "unavailable", problem: String(error) });
    }
}

function playerExecutable(state: InstallationRegistryState | null): string | undefined {
    const selected = state?.selected?.target;
    return selected?.kind === "player"
        ? selected.executable
        : state?.registry.targets.find(target => target.kind === "player")?.executable;
}

if (remembered) void readGameSkin(remembered);

derived([installationProfiles, profilesReady], values => values).subscribe(([state, ready]) => {
    if (!ready || get(gameSkin).status === "ready") return;
    const executable = playerExecutable(state);
    if (executable && executable !== source) {
        void readGameSkin(executable);
    } else if (!executable && !source) {
        gameSkin.set({
            status: "unavailable",
            problem: state ? "Add your Sons of the Forest installation to use this skin." : "The Manager could not read your saved installations.",
        });
    }
});
