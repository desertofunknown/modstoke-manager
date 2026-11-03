import type { Mod, ModVersion } from "./mods";
import { renderStoreRichText } from "./richText";
import { formatCount, formatDate } from "./format";

export type SotfCatalogSort = "updated" | "downloads" | "favorites" | "comments" | "rating" | "name";

export function modIdentityKey(mod: Mod): string {
    return mod.mod_id || mod.slug || mod.name;
}

export function detailAuthorName(mod: Mod): string {
    return mod.user?.name ?? mod.installedMod?.manifest?.author ?? "Unknown";
}

export function detailCategoryLabel(mod: Mod): string {
    return mod.category?.name ?? mod.installedMod?.manifest?.type ?? "-";
}

export function detailSourceLabel(mod: Mod): string {
    if (!mod.installedMod) {
        return "Online";
    }

    if (mod.installedMod.installSource === "vortex") {
        return "Vortex";
    }

    if (mod.installedMod.installSource === "native") {
        return "Native";
    }

    return "Manual";
}

export function detailLoaderLabel(mod: Mod): string {
    if (mod.installedMod?.loaderType === "bepinex-plugin") {
        return "BepInEx plugin";
    }

    if (mod.installedMod?.loaderType === "redloader-library" || mod.type === "Library") {
        return "RedLoader library";
    }

    if (mod.type === "Build") {
        return "Build";
    }

    return "RedLoader mod";
}

export function detailCompatibilityLabel(mod: Mod): string {
    if (mod.requiresAllPlayers) {
        return "Requires all players";
    }

    if (mod.isMultiplayerCompatible) {
        return "Multiplayer compatible";
    }

    if (mod.modSide === "client") {
        return "Client-side";
    }

    return "Review";
}

export function detailVersions(mod: Mod): ModVersion[] {
    return [...(mod.versions ?? [])]
        .sort((left, right) => {
            if (left.isLatest !== right.isLatest) {
                return left.isLatest ? -1 : 1;
            }

            return dateValue(right.createdAt ?? right.updatedAt) - dateValue(left.createdAt ?? left.updatedAt);
        })
        .slice(0, 5);
}

export function dateValue(value?: string): number {
    if (!value) {
        return 0;
    }

    const time = new Date(value).getTime();
    return Number.isFinite(time) ? time : 0;
}

export function detailVersionDate(version: ModVersion): string {
    return version.createdAt || version.updatedAt ? formatDate(version.createdAt ?? version.updatedAt ?? "") : "-";
}

export function detailVersionDownloads(version: ModVersion): string {
    return formatCount(version._count?.downloads);
}

export function detailVersionChangelogHtml(version: ModVersion): string {
    return renderStoreRichText(version.changelog, "No changelog text.");
}

export function sortMetricLabel(sort: SotfCatalogSort): string {
    if (sort === "downloads") {
        return "Downloads";
    }

    if (sort === "favorites") {
        return "Favorites";
    }

    if (sort === "comments") {
        return "Comments";
    }

    if (sort === "rating") {
        return "Rating";
    }

    return "";
}

export function rowSortMetricValue(mod: Mod, sort: SotfCatalogSort): string {
    if (sort === "downloads") {
        return formatCount(mod.downloads ?? mod.lastWeekDownloads);
    }

    if (sort === "favorites") {
        return formatCount(mod.favoritesCount);
    }

    if (sort === "comments") {
        return formatCount(mod.commentsCount);
    }

    if (sort === "rating") {
        return detailRatingLabel(mod.averageRating);
    }

    return "";
}

export function detailRatingLabel(value?: number): string {
    if (typeof value !== "number" || value <= 0) {
        return "No rating";
    }

    return `${value.toFixed(1)} / 5`;
}

export function trustedAuthorLabel(mod: Mod): string {
    return mod.user?.isTrusted ? "Trusted" : "Standard";
}

export function normalizedSourceUrl(mod: Mod): string {
    const raw = mod.sourceUrl?.trim();
    if (!raw) {
        return "";
    }

    try {
        const url = new URL(raw);
        return url.protocol === "http:" || url.protocol === "https:" ? url.toString() : "";
    } catch {
        return "";
    }
}

export function detailInstallStateLabel(mod: Mod): string {
    if (!mod.isInstalled) {
        return "Not installed";
    }

    if (mod.installedMod?.installSource === "vortex") {
        return "Managed by Vortex";
    }

    return mod.installedMod?.isEnabled ? "Installed enabled" : "Installed disabled";
}
