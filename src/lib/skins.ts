import { writable } from "svelte/store";

export type Skin = "modstoke" | "sons";
export type SkinChoice = "game" | Skin;

export const SKIN_CHOICES: { id: SkinChoice; label: string; description: string }[] = [
    { id: "game", label: "Match the game", description: "Use the skin made for the game you're managing." },
    { id: "modstoke", label: "Modstoke", description: "Plain dark panels." },
    { id: "sons", label: "Sons of the Forest", description: "The ragged frame and title screen backdrop from the game's menus." },
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

export function resolveSkin(choice: SkinChoice): Skin {
    return choice === "game" ? GAME_SKIN : choice;
}
