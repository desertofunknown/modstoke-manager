import { convertFileSrc, invoke } from "@tauri-apps/api/core";
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

type GameSkinFiles = { revision: string; font: boolean };
export type GameSkinState = { status: "waiting" | "reading" | "ready" | "unavailable"; problem: string };

// The Manager ships none of Endnight's art: the Sons skin's frame and backdrop, and the Roboto font
// the game ships with, are read from the player's own installation into a cache that Rust serves at
// gameskin://, so the webview loads them like any image or font.
export const gameSkin = writable<GameSkinState>({ status: "waiting", problem: "" });
const SKIN_FILES_KEY = "openacai-manager-game-skin";
type RememberedSkin = GameSkinFiles & { executable: string };
// What was read last time, so the skin shows at once and is checked against the game afterwards.
const remembered = rememberedSkin();
const skinSheet = new CSSStyleSheet();
document.adoptedStyleSheets = [...document.adoptedStyleSheets, skinSheet];
let source = "";
let applied = "";

function rememberedSkin(): RememberedSkin | null {
    try {
        const value = JSON.parse(localStorage.getItem(SKIN_FILES_KEY) ?? "null");
        return typeof value?.executable === "string" && typeof value.revision === "string" ? value : null;
    } catch {
        return null;
    }
}

export function resolveSkin(choice: SkinChoice, status: GameSkinState["status"]): Skin {
    const skin = choice === "game" ? GAME_SKIN : choice;
    if (skin !== "sons" || status === "ready") return skin;
    // Without the game's files there is no frame to draw. A launch that read them last time keeps
    // the Sons skin while they are checked, rather than switching skins once they are.
    return remembered && status !== "unavailable" ? "sons" : "modstoke";
}

function applySkin(files: GameSkinFiles) {
    if (applied === files.revision) return;
    applied = files.revision;
    const url = (file: string) => `${convertFileSrc(file, "gameskin")}?v=${files.revision}`;
    skinSheet.replaceSync(`:root { --sons-frame-mask: url("${url("frame-mask.png")}"); --sons-backdrop: url("${url("title-backdrop.png")}"); }`);
    if (files.font) document.fonts.add(new FontFace("SonsRoboto", `url("${url("roboto.ttf")}")`, { weight: "400 900" }));
}

async function readGameSkin(executable: string): Promise<void> {
    source = executable;
    gameSkin.set({ status: "reading", problem: "" });
    try {
        const files = await invoke<GameSkinFiles>("read_game_skin", { executable });
        if (source !== executable) return;
        applySkin(files);
        localStorage.setItem(SKIN_FILES_KEY, JSON.stringify({ executable, ...files }));
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

if (remembered) {
    applySkin(remembered);
    void readGameSkin(remembered.executable);
}

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
