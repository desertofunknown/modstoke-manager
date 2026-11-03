import { normalizeNexusMarkup, plainText, safeNexusUrl } from "./nexusRichText";
import { describeInstallSource, type InstalledInventoryEntry } from "./modInventory";

const MAX_AUTHOR_REQUIREMENT_LINKS = 6;
const MAX_AUTHOR_REQUIREMENT_HINTS = 8;
const MAX_AUTHOR_INSTRUCTION_HINTS = 5;

type AuthorRequirementKind = "link" | "runtime";
type AuthorRequirementStatus = "detected" | "review" | "warning";
export type AuthorRequirementLink = {
    key: string;
    label: string;
    url?: string;
    source: string;
    kind: AuthorRequirementKind;
    status: AuthorRequirementStatus;
    detail?: string;
    excerpt?: string;
    match?: InstalledInventoryEntry | null;
    mod_id?: number;
};
export type AuthorRequirementSource = {
    key: string;
    label: string;
    value: string;
};
type AuthorInstructionKind = "install" | "configure" | "usage" | "warning" | "requirement";
type AuthorInstructionTarget = "description" | "files" | "changelog";
export type AuthorInstructionHint = {
    key: string;
    source: string;
    title: string;
    detail: string;
    kind: AuthorInstructionKind;
    target: AuthorInstructionTarget;
};

export function extractAuthorInstructions(sources: AuthorRequirementSource[]): AuthorInstructionHint[] {
    const hints: AuthorInstructionHint[] = [];
    const seen = new Set<string>();

    for (const source of sources) {
        const text = plainText(source.value);
        if (!text) {
            continue;
        }

        const blocks = authorInstructionBlocks(text);
        for (let index = 0; index < blocks.length; index += 1) {
            if (hints.length >= MAX_AUTHOR_INSTRUCTION_HINTS) {
                return hints;
            }

            const current = blocks[index];
            if (!current) {
                continue;
            }

            const next = blocks[index + 1] ?? "";
            const kind = authorInstructionKind(current, next);
            if (!kind) {
                continue;
            }

            if (source.key.includes("changelog") && !authorInstructionHeadingLike(current)) {
                continue;
            }

            const detail = authorInstructionExcerpt(current, next, kind);
            if (!detail || detail.length < 18) {
                continue;
            }

            const dedupeKey = detail.toLowerCase().replace(/[^a-z0-9\u4e00-\u9fff]+/g, " ").trim().slice(0, 180);
            if (!dedupeKey || seen.has(dedupeKey)) {
                continue;
            }

            seen.add(dedupeKey);
            hints.push({
                key: `${source.key}:${index}:${dedupeKey}`,
                source: source.label,
                title: authorInstructionTitle(kind, current),
                detail,
                kind,
                target: authorInstructionTarget(source.key)
            });
        }
    }

    return hints;
}

function authorInstructionBlocks(text: string): string[] {
    return text
        .replace(/\n\s*[-*]\s+/g, "\n- ")
        .split(/\n{2,}|\n(?=(?:install(?:ation)?|setup|usage|how to|requirements?|warning|important|配置|安装|使用|说明|要求|注意)[:：]?)/i)
        .map(block => block.replace(/\s+/g, " ").trim())
        .filter(Boolean);
}

function authorInstructionKind(block: string, nextBlock: string): AuthorInstructionKind | null {
    const headingLike = authorInstructionHeadingLike(block);
    const value = headingLike ? `${block} ${nextBlock.slice(0, 180)}` : block;

    if (/^(?:install(?:ation|ing)?|setup|how to install|deploy|安装|安装步骤|安装方法|部署)\b|(?:\b(?:extract|copy|place|put|drop|move|drag)\b.{0,80}\b(?:plugins?|mods?|libs?|folder|directory|game folder|\.dll|zip)\b)|(?:\bdownload\b.{0,80}\b(?:bepinex|redloader|zip|archive|\.dll)\b)|(?:下载.{0,80}(?:BepInEx|RedLoader|zip|\.dll|压缩包)|解压|(?:复制|移动).{0,80}(?:文件夹|目录|BepInEx|RedLoader|plugins?|mods?|\.dll)|放入|放到|拖入|找到游戏文件夹)/i.test(value)) {
        return "install";
    }

    if (/^(?:usage|use|how to use|controls?|使用|说明|教程)\b|(?:\b(?:press|open menu|launch|start|toggle|command)\b.{0,80}\b(?:menu|game|mod|feature|tab|key)\b)|(?:按.{0,12}打开|启动|切换|命令)/i.test(value)) {
        return "usage";
    }

    if (/\b(?:warning|important|caution|do not|don't|dont|must not|should not|before installing|before install|remove before|delete before|uninstall|disable)\b|(?:注意|警告|重要|请勿|不要|不可|先删除|先移除|卸载|禁用)/i.test(value)) {
        return "warning";
    }

    if (/^(?:requirements?|dependencies?|prereq(?:uisites?)?|要求|依赖|前置)\b|(?:\b(?:requires?|needed|must have|needs)\b.{0,80}\b(?:runtime|framework|library|bepinex|redloader|sonssdk|harmony|\.net|dotnet)\b)|(?:需要.{0,30}(?:运行库|框架|BepInEx|RedLoader|SonsSdk))/i.test(value)) {
        return "requirement";
    }

    if (/^(?:config(?:ure|uration)?|settings?|options?|配置|设置|选项)\b|(?:\b(?:ini|json|toml|yaml|keybind|hotkey)\b)|(?:快捷键|热键)/i.test(value)) {
        return "configure";
    }

    return null;
}

function authorInstructionHeadingLike(block: string): boolean {
    return block.length <= 90
        && /^(?:install(?:ation|ing)?|setup|usage|use|how to|requirements?|dependencies?|config(?:ure|uration)?|settings?|warning|important|note|安装|部署|使用|说明|教程|要求|依赖|前置|配置|设置|注意|警告|重要)[:：]?$/i.test(block.trim());
}

function authorInstructionExcerpt(block: string, nextBlock: string, kind: AuthorInstructionKind): string {
    const headingLike = authorInstructionHeadingLike(block);
    const value = headingLike && nextBlock
        ? `${block.replace(/[:：]?\s*$/, "")}: ${nextBlock}`
        : block;
    const context = !headingLike && value.length > 320
        ? authorInstructionContext(value, kind)
        : value;
    return context
        .replace(/\s+-\s+/g, "; ")
        .replace(/\s+/g, " ")
        .trim()
        .slice(0, 260);
}

function authorInstructionContext(value: string, kind: AuthorInstructionKind): string {
    const pattern = {
        configure: /\b(?:config(?:ure|uration)?|settings?|options?|ini|json|toml|yaml|keybind|hotkey)\b|(?:配置|设置|选项|快捷键|热键)/i,
        install: /\b(?:install(?:ation|ing)?|setup|deploy|extract|copy|place|put|drop|move|drag)\b.{0,90}\b(?:plugins?|mods?|libs?|folder|directory|game folder|\.dll|zip)\b|(?:安装步骤|安装方法|部署|下载.{0,80}(?:BepInEx|RedLoader|zip|\.dll|压缩包)|解压|(?:复制|移动).{0,80}(?:文件夹|目录|BepInEx|RedLoader|plugins?|mods?|\.dll)|放入|放到|拖入|找到游戏文件夹)/i,
        requirement: /\b(?:requirements?|dependencies?|requires?|needed|must have|runtime|framework|library|bepinex|redloader|sonssdk|harmony|\.net|dotnet)\b|(?:要求|依赖|前置|需要|运行库|框架)/i,
        usage: /\b(?:usage|use|how to|controls?|open menu|press|launch|start|toggle|command)\b|(?:使用|说明|教程|按.{0,12}打开|启动|切换|命令)/i,
        warning: /\b(?:warning|important|caution|do not|don't|dont|must not|before install|uninstall|disable)\b|(?:注意|警告|重要|请勿|不要|不可|卸载|禁用)/i
    }[kind];
    const match = value.match(pattern);
    const index = match?.index ?? -1;
    if (index < 0) {
        return value;
    }

    const sentence = authorInstructionSentenceContext(value, index, match?.[0]?.length ?? 0);
    if (sentence) {
        return sentence;
    }

    const start = Math.max(0, index - 60);
    const end = Math.min(value.length, index + (match?.[0]?.length ?? 0) + 160);
    return `${start > 0 ? "... " : ""}${value.slice(start, end)}${end < value.length ? " ..." : ""}`;
}

function authorInstructionSentenceContext(value: string, index: number, matchLength: number): string | null {
    const before = value.slice(0, index);
    const startBoundary = Math.max(
        before.lastIndexOf("。"),
        before.lastIndexOf("."),
        before.lastIndexOf(";"),
        before.lastIndexOf("；"),
        before.lastIndexOf("\n")
    );
    const afterStart = index + matchLength;
    const after = value.slice(afterStart);
    const endMatch = after.match(/[。.!?；;]\s*/);
    const start = Math.max(0, startBoundary + 1);
    const end = endMatch?.index !== undefined
        ? Math.min(value.length, afterStart + endMatch.index + endMatch[0].length)
        : Math.min(value.length, afterStart + 150);
    const sentence = value.slice(start, end).trim();
    return sentence.length >= 16 ? sentence : null;
}

function authorInstructionTitle(kind: AuthorInstructionKind, block: string): string {
    const label = block
        .replace(/[:：].*$/, "")
        .replace(/^\W+|\W+$/g, "")
        .trim();
    if (label && label.length <= 42 && authorInstructionKind(label, "")) {
        return label;
    }

    switch (kind) {
        case "configure":
            return "Configuration note";
        case "usage":
            return "Usage note";
        case "warning":
            return "Author warning";
        case "requirement":
            return "Requirement note";
        default:
            return "Install instruction";
    }
}

function authorInstructionTarget(sourceKey: string): AuthorInstructionTarget {
    if (sourceKey.startsWith("selected-file")) {
        return "files";
    }

    if (sourceKey.startsWith("mod-changelog")) {
        return "changelog";
    }

    return "description";
}

export function extractAuthorRequirements(sources: AuthorRequirementSource[], currentModId: number | undefined, entries: InstalledInventoryEntry[]): AuthorRequirementLink[] {
    const candidates: AuthorRequirementLink[] = [];
    const seen = new Set<string>();
    let linkedCount = 0;

    const addLink = (sourceLabel: string, urlValue: string, labelValue?: string) => {
        const url = safeNexusUrl(urlValue);
        if (!url || seen.has(url) || linkedCount >= MAX_AUTHOR_REQUIREMENT_LINKS) {
            return;
        }

        const nexusModId = nexusSotfModIdFromUrl(url);
        if (currentModId && nexusModId === currentModId) {
            return;
        }

        const label = authorRequirementLabel(labelValue, url, nexusModId);
        const linkContext = `${label} ${url}`;
        if (!nexusModId && !looksLikeRequirementContext(linkContext)) {
            return;
        }

        seen.add(url);
        linkedCount += 1;
        candidates.push({
            key: `${nexusModId ?? "link"}:${url}`,
            label,
            url,
            source: nexusModId ? `${sourceLabel} Nexus link` : `${sourceLabel} link`,
            kind: "link",
            status: "review",
            detail: `Found in ${sourceLabel}. Open and confirm whether this author-linked target is required for this file.`,
            mod_id: nexusModId ?? undefined
        });
    };

    for (const source of sources) {
        const normalized = normalizeNexusMarkup(source.value);
        if (!normalized) {
            continue;
        }

        const linkedPattern = /\[url=([^\]]+)\]([\s\S]*?)\[\/url\]/gi;
        let linkedMatch: RegExpExecArray | null;
        while ((linkedMatch = linkedPattern.exec(normalized)) !== null) {
            addLink(source.label, linkedMatch[1], plainText(linkedMatch[2]));
        }

        const plainUrlPattern = /\[url\]([\s\S]*?)\[\/url\]|https?:\/\/[^\s<>"'\]]+/gi;
        let plainMatch: RegExpExecArray | null;
        while ((plainMatch = plainUrlPattern.exec(normalized)) !== null) {
            const raw = plainMatch[1] ?? plainMatch[0];
            addLink(source.label, raw);
        }

        for (const hint of inferAuthorRuntimeRequirements(normalized, source.label, entries)) {
            if (candidates.length >= MAX_AUTHOR_REQUIREMENT_HINTS) {
                break;
            }

            if (seen.has(hint.key)) {
                continue;
            }

            seen.add(hint.key);
            candidates.push(hint);
        }
    }

    return candidates.slice(0, MAX_AUTHOR_REQUIREMENT_HINTS);
}

function inferAuthorRuntimeRequirements(source: string, sourceLocation: string, entries: InstalledInventoryEntry[]): AuthorRequirementLink[] {
    const text = plainText(source);
    if (!text) {
        return [];
    }

    const hints: AuthorRequirementLink[] = [];
    const seen = new Set<string>();
    const addRuntimeHint = (
        key: string,
        label: string,
        kindLabel: string,
        pattern: RegExp,
        matchPredicate: (entry: InstalledInventoryEntry) => boolean,
        reviewDetail: string
    ) => {
        const matched = text.match(pattern);
        if (!matched || seen.has(key)) {
            return;
        }

        seen.add(key);
        const context = authorRequirementMatchContext(text, matched);
        if (!context.warning && !authorRequirementContextLooksLikeActionableDependency(context.excerpt ?? matched[0] ?? "")) {
            return;
        }

        const warningDetail = authorRequirementCompatibilityWarningDetail(label, sourceLocation, context.excerpt);
        const match = entries.find(matchPredicate) ?? null;
        hints.push({
            key: `runtime:${key}`,
            label,
            source: kindLabel,
            kind: "runtime",
            status: warningDetail ? "warning" : match ? "detected" : "review",
            match,
            detail: warningDetail
                ? `${warningDetail}${match ? ` Local match detected as ${describeInstallSource(match)} in ${match.expectedLocation}.` : ""}`
                : match
                ? `Found in ${sourceLocation}. Detected locally as ${describeInstallSource(match)} in ${match.expectedLocation}.`
                : `Found in ${sourceLocation}. ${reviewDetail}`,
            excerpt: context.excerpt
        });
    };

    addRuntimeHint(
        "bepinex",
        "BepInEx / IL2CPP runtime",
        "Author runtime hint",
        /\b(?:bepinex|bepex|il2cpp|unity\.?il2cpp|bepinex\s*6|bepinex6)\b/i,
        entry => entry.loaderType === "bepinex-plugin" || /bepinex|il2cpp|openacai\s*loader|modstoke\s*loader|redloaderbepinexcompat/i.test(authorRequirementEntryText(entry)),
        "Author text mentions BepInEx or IL2CPP. Confirm the BepInEx runtime is installed before Vortex deployment."
    );
    addRuntimeHint(
        "redloader",
        "RedLoader",
        "Author runtime hint",
        /\b(?:red\s*loader|redloader)\b/i,
        entry => entry.loaderType === "redloader-mod" || entry.loaderType === "redloader-library" || /red\s*loader|redloader/i.test(authorRequirementEntryText(entry)),
        "Author text mentions RedLoader. Confirm RedLoader-compatible loader support is installed before Vortex deployment."
    );
    addRuntimeHint(
        "sonssdk",
        "SonsSdk",
        "Author library hint",
        /\b(?:sons\s*sdk|sonssdk|sons\s+sdk)\b/i,
        entry => /sons\s*sdk|sonssdk/i.test(authorRequirementEntryText(entry)),
        "Author text mentions SonsSdk. Confirm the required library or bundled loader support is present."
    );
    addRuntimeHint(
        "harmony",
        "Harmony",
        "Author library hint",
        /\b(?:harmony|0harmony)\b/i,
        entry => /harmony|0harmony/i.test(authorRequirementEntryText(entry)),
        "Author text mentions Harmony. Confirm it is bundled by the selected file or already present locally."
    );
    addRuntimeHint(
        "dotnet",
        ".NET runtime",
        "Author runtime hint",
        /\b(?:\.net|dotnet|net\s*(?:6|7|8|9|10|11)|netstandard)\b/i,
        entry => /\.net|dotnet|net\s*(?:6|7|8|9|10|11)|openacai\s*loader|modstoke\s*loader/i.test(authorRequirementEntryText(entry)),
        "Author text mentions a .NET runtime requirement. Confirm the loader/runtime track matches this game install."
    );
    addRuntimeHint(
        "openacai-loader",
        "Modstoke Loader",
        "Author loader hint",
        /\b(?:openacai(?:\s+endnight)?\s+loader|openacailoader|modstoke\s+loader|modstokeloader)\b/i,
        entry => /openacai\s*(?:endnight)?\s*loader|openacailoader|modstoke\s*loader/i.test(authorRequirementEntryText(entry)),
        "Author text mentions Modstoke Loader support. Confirm the public loader is installed and healthy in Overview."
    );

    return hints;
}

function authorRequirementMatchContext(text: string, match: RegExpMatchArray): { excerpt?: string; warning: boolean } {
    const index = match.index ?? -1;
    const phrase = match[0]?.trim();
    if (index < 0 || !phrase) {
        return { excerpt: phrase, warning: false };
    }

    const start = Math.max(0, index - 70);
    const end = Math.min(text.length, index + phrase.length + 70);
    const compact = text.slice(start, end).replace(/\s+/g, " ").trim();
    if (!compact) {
        return { warning: false };
    }

    const excerpt = `${start > 0 ? "... " : ""}${compact}${end < text.length ? " ..." : ""}`.slice(0, 220);
    return {
        excerpt,
        warning: authorRequirementContextLooksLikeWarning(compact)
    };
}

function authorRequirementContextLooksLikeWarning(value: string): boolean {
    return /\b(?:cannot|can't|cant|must\s+not|should\s+not|do\s+not|don't|dont|not\s+compatible|incompatible|conflicts?|conflicting|cannot\s+coexist|can't\s+coexist|will\s+not\s+work|won't\s+work|does\s+not\s+work|remove|delete|uninstall|disable|only\s+one|not\s+supported|must\s+first\s+delete|must\s+first\s+remove)\b|(?:不能|不可|不兼容|冲突|删除|移除|卸载|禁用|无法|不会|只允许|必须先删除|必须先移除)/i.test(value);
}

function authorRequirementContextLooksLikeActionableDependency(value: string): boolean {
    if (/\b(?:credits?|credit\s+to|thanks?|thank\s+you|contributors?|attribution|acknowledgements?|license|licensed|pardeike|used\s+throughout\s+many\s+unity\s+modding)\b|(?:鸣谢|致谢|贡献者|署名|许可)/i.test(value)) {
        return false;
    }

    return /\b(?:requires?|requirements?|dependenc(?:y|ies)|prereq(?:uisites?)?|needed|needs|must\s+have|install\s+first|depends?\s+on|before\s+install(?:ing)?|download\s+and\s+install|install(?:ed|ing)?\s+(?:bepinex|redloader|sonssdk|harmony|doorstop|\.net|dotnet|runtime|framework|library|loader)|runtime|framework|library|required\s+loader|loader\s+required)\b|(?:要求|依赖|前置|需要|必须|安装|下载|运行库|框架|库文件)/i.test(value);
}

function authorRequirementCompatibilityWarningDetail(label: string, sourceLocation: string, excerpt?: string): string | null {
    if (!excerpt || !authorRequirementContextLooksLikeWarning(excerpt)) {
        return null;
    }

    return `Found in ${sourceLocation}. Author text mentions ${label} in an incompatibility, removal, or conflict context; review before deployment.`;
}

function authorRequirementEntryText(entry: InstalledInventoryEntry): string {
    return [
        entry.id,
        entry.name,
        entry.author,
        entry.manifestType,
        entry.expectedLocation,
        entry.store,
        entry.vortexPackage,
        entry.packagePath,
        entry.assemblyPath,
        ...entry.matchKeys
    ].filter(Boolean).join(" ");
}

function looksLikeRequirementContext(value: string): boolean {
    return /\b(require(?:d|ment|s)?|dependenc(?:y|ies)|prereq(?:uisite)?|needed|install first|runtime|framework|library|bepinex|redloader|sonssdk|harmony|doorstop|\.net|dotnet)\b/i.test(value);
}

function authorRequirementLabel(value: string | undefined, url: string, nexusModId: number | null): string {
    const cleaned = plainText(value)
        .replace(/^https?:\/\/\S+$/i, "")
        .replace(/\s+/g, " ")
        .trim();

    if (cleaned) {
        return cleaned.slice(0, 90);
    }

    if (nexusModId) {
        return `Nexus mod ${nexusModId}`;
    }

    try {
        const parsed = new URL(url);
        return parsed.hostname.replace(/^www\./i, "");
    } catch {
        return "Author requirement";
    }
}

function nexusSotfModIdFromUrl(value: string): number | null {
    try {
        const url = new URL(value);
        const host = url.hostname.replace(/^www\./i, "").toLowerCase();
        if (host !== "nexusmods.com") {
            return null;
        }

        const match = url.pathname.match(/^\/sonsoftheforest\/mods\/(\d+)(?:\/|$)/i);
        return match ? Number.parseInt(match[1], 10) : null;
    } catch {
        return null;
    }
}
