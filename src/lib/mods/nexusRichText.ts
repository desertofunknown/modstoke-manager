import { escapeAttribute, escapeHtml } from "./richText";

// Tags nested deeper than this are shown as text, so a hostile description can't exhaust the stack.
const NEXUS_RICH_MAX_NESTING = 32;
const NEXUS_RICH_BB_TAGS = new Set(["b", "i", "u", "s", "strike", "strikethrough", "del", "sub", "sup", "small", "big", "mark", "abbr", "acronym", "cite", "q", "url", "img", "image", "thumb", "thumbnail", "color", "colour", "background", "bgcolor", "bgcolour", "backgroundcolour", "bcolor", "highlight", "size", "center", "centre", "left", "right", "justify", "align", "indent", "code", "pre", "tt", "kbd", "samp", "var", "quote", "spoiler", "collapse", "details", "accordion", "accordionitem", "font", "heading", "h", "header", "title", "subtitle", "caption", "h1", "h2", "h3", "h4", "h5", "h6", "float", "clear", "anchor", "bookmark", "target", "goto", "jump", "youtube", "video", "media", "embed", "columns", "cols", "column", "col", "nextcol", "tabs", "tab", "dl", "dt", "dd", "note", "info", "warning", "important", "tip", "box", "panel", "fieldset", "notice", "success", "danger", "error"]);
const NEXUS_SAFE_COLOR_NAMES = new Set(["black", "white", "gray", "grey", "silver", "red", "maroon", "orange", "yellow", "olive", "lime", "green", "aqua", "cyan", "teal", "blue", "navy", "fuchsia", "magenta", "purple", "pink"]);

type NexusRichNode =
    | { kind: "text"; value: string }
    | { kind: "tag"; name: string; attr?: string; rawAttrs?: string; children: NexusRichNode[] };
type NexusBlockChunk =
    | { kind: "block"; value: string }
    | { kind: "line"; value: string; mono?: boolean }
    | { kind: "list"; value: string; ordered: boolean; style?: string; start?: number }
    | { kind: "table"; value: string }
    | { kind: "definition"; value: string };
type NexusPlainListLine = {
    explicit: boolean;
    indent: number;
    ordered: boolean;
    raw: string;
    start?: number;
    style?: string;
    value: string;
};
type NexusTableCell = {
    value: string;
    header: boolean;
    colspan?: number;
    rowspan?: number;
    width?: string;
    align?: "left" | "center" | "right" | "justify";
};

export function plainText(value?: string): string {
    return normalizeNexusMarkup(value)
        .replace(/\[img[^\]]*\][\s\S]*?\[\/img\]/gi, " ")
        .replace(/\[url=([^\]]+)\]([\s\S]*?)\[\/url\]/gi, "$2 ($1)")
        .replace(/\[url\]([\s\S]*?)\[\/url\]/gi, "$1")
        .replace(/\[\*\s*=([^\]]+)\]/g, (_match, label: string) => `\n- ${safeNexusListItemLabel(label)} `)
        .replace(/\[\*\]/g, "\n- ")
        .replace(/\[\s*li(?:\s[^\]]*|=[^\]]*)?\]/gi, "\n- ")
        .replace(/\[\/\s*li\]/gi, "\n")
        .replace(/\[\/?(?:table|tbody|thead|tfoot)[^\]]*\]/gi, "\n")
        .replace(/\[\/?tr[^\]]*\]/gi, "\n")
        .replace(/\[(?:td|th)[^\]]*\]/gi, "")
        .replace(/\[\/(?:td|th)\]/gi, " | ")
        .replace(/\[hr\s*\/?\]/gi, "\n---\n")
        .replace(/\[line\s*\/?\]/gi, "\n---\n")
        .replace(/\[(?:rule|divider|separator)\s*\/?\]/gi, "\n---\n")
        .replace(/\[(youtube|video|media|embed)[^\]]*\]([\s\S]*?)\[\/\1\]/gi, (_match, tag: string, body: string) => `${tag === "youtube" ? "YouTube" : "Media"}: ${body}`)
        .replace(/\[(?:nextcol|nextcolumn)\s*\/?\]/gi, "\n")
        .replace(/\[\/?(?:columns|cols|column|col|tabs|tab|note|info|warning|important|tip|box|panel|fieldset|notice|success|danger|error|collapse|details|accordion|accordionitem|hidden|spoilerblock|caption|dl|dt|dd)[^\]]*\]/gi, "\n")
        .replace(/\[\/?\s*(?:list|ul|ol|olist)[^\]]*\]/gi, "")
        .replace(/\[\/?(?:b|i|u|s|strike|del|sub|sup|small|big|mark|abbr|acronym|cite|q|size|color|background|bgcolor|highlight|font|center|left|right|justify|align|indent|quote|spoiler|code|pre|tt|kbd|samp|var|heading|h|header|title|subtitle|h[1-6]|float|clear|div|p|paragraph|span)[^\]]*\]/gi, "")
        .replace(/\[\/?[a-z0-9_-]+[^\]]*\]/gi, "")
        .replace(/[ \t]+/g, " ")
        .replace(/\n\s+/g, "\n")
        .replace(/\n{3,}/g, "\n\n")
        .trim();
}

export function renderNexusRichText(value?: string, fallback = ""): string {
    try {
        const normalized = normalizeNexusMarkup(value);
        if (!normalized) {
            return `<p>${escapeHtml(fallback)}</p>`;
        }

        return renderNexusBlocks(normalized, 0);
    } catch (error) {
        console.warn("Could not render Nexus text", error);
        return `<p>${escapeHtml(plainTextOr(value, fallback))}</p>`;
    }
}

export function renderOptionalNexusRichText(value?: string): string {
    try {
        const normalized = normalizeNexusMarkup(value);
        return normalized ? renderNexusBlocks(normalized, 0) : "";
    } catch (error) {
        console.warn("Could not render Nexus text", error);
        const plain = plainTextOr(value, "");
        return plain ? `<p>${escapeHtml(plain)}</p>` : "";
    }
}

function plainTextOr(value: string | undefined, fallback: string): string {
    try {
        return plainText(value) || fallback;
    } catch {
        return fallback;
    }
}

export function nexusRichTextImageUrls(source?: string): string[] {
    const normalized = normalizeNexusMarkup(source);
    if (!normalized) {
        return [];
    }

    const urls: string[] = [];
    const pairedImagePattern = /\[img([^\]]*)\]([\s\S]*?)\[\/img\]/gi;
    let pairedMatch: RegExpExecArray | null;
    while ((pairedMatch = pairedImagePattern.exec(normalized)) !== null) {
        const attrUrl = nexusImageUrlAttribute(pairedMatch[1], nexusBbTagAttribute("img", pairedMatch[1]));
        const bodyUrl = collectNexusNodeText(parseNexusRichNodes(pairedMatch[2]));
        urls.push(...[attrUrl, bodyUrl].filter((url): url is string => Boolean(url)));
    }

    const imageOpeningPattern = /\[img([^\]]*)\]/gi;
    let openingMatch: RegExpExecArray | null;
    while ((openingMatch = imageOpeningPattern.exec(normalized)) !== null) {
        const url = nexusImageUrlAttribute(openingMatch[1], nexusBbTagAttribute("img", openingMatch[1]));
        if (url) {
            urls.push(url);
        }
    }

    return uniquePreviewUrls(urls);
}

export function uniquePreviewUrls(values: Array<string | undefined>): string[] {
    const seen = new Set<string>();
    const urls: string[] = [];

    for (const value of values) {
        const url = normalizePreviewUrl(value);
        if (!url || seen.has(url)) {
            continue;
        }

        seen.add(url);
        urls.push(url);
    }

    return urls;
}

function normalizePreviewUrl(value?: string): string | undefined {
    return safeNexusUrl(value) ?? undefined;
}

export function normalizeNexusMarkup(value?: string): string {
    let text = (value ?? "")
        .replace(/\r\n?/g, "\n")
        .replace(/<br\s*\/?>/gi, "\n")
        .replace(/<details\b[^>]*>\s*<summary\b[^>]*>([\s\S]*?)<\/summary>([\s\S]*?)<\/details>/gi, (_match, label: string, body: string) => {
            const safeLabel = safeNexusBbLabel(plainText(label));
            return `\n\n[details${safeLabel ? `=${safeLabel}` : ""}]${body}[/details]\n\n`;
        })
        .replace(/<(iframe|embed|object|video|audio)\b([^>]*)>([\s\S]*?)<\/\1>/gi, (_match, tag: string, attrs: string, body: string) => {
            return nexusHtmlMediaMarkup(tag, attrs, body);
        })
        .replace(/<(iframe|embed)\b([^>]*)\/?>/gi, (_match, tag: string, attrs: string) => {
            return nexusHtmlMediaMarkup(tag, attrs);
        })
        .replace(/<h([1-6])\b[^>]*>([\s\S]*?)<\/h\1>/gi, "\n\n[heading=$1]$2[/heading]\n\n")
        .replace(/<blockquote\b([^>]*)>/gi, (_match, attrs: string) => {
            const cite = safeNexusLabel(htmlAttribute(attrs, "cite") ?? "");
            return cite ? `\n\n[quote=${cite}]` : "\n\n[quote]";
        })
        .replace(/<\/blockquote>/gi, "[/quote]\n\n")
        .replace(/<hr\b[^>]*\/?>/gi, "\n\n[hr]\n\n")
        .replace(/<pre\b[^>]*>/gi, "\n\n[code]")
        .replace(/<\/pre>/gi, "[/code]\n\n")
        .replace(/<code\b[^>]*>/gi, "[code]")
        .replace(/<\/code>/gi, "[/code]")
        .replace(/<(kbd|samp|tt|var)\b[^>]*>/gi, "[code]")
        .replace(/<\/(?:kbd|samp|tt|var)>/gi, "[/code]")
        .replace(/<center\b[^>]*>/gi, "\n\n[center]")
        .replace(/<\/center>/gi, "[/center]\n\n")
        .replace(/<(p|div|section|article)\b([^>]*)>([\s\S]*?)<\/\1>/gi, (_match, _tag: string, attrs: string, body: string) => {
            const alignment = htmlAlignmentAttribute(attrs);
            const indent = htmlIndentLevelAttribute(attrs);
            const styled = hasNexusBlockishMarkup(body) ? body : nexusHtmlStyledInline(attrs, body);
            const aligned = alignment ? `[align=${alignment}]${styled}[/align]` : styled;
            const indented = indent ? `[indent=${indent}]${aligned}[/indent]` : aligned;
            return `\n\n${indented}\n\n`;
        })
        .replace(/<font\b([^>]*)>([\s\S]*?)<\/font>/gi, (_match, attrs: string, body: string) => {
            return nexusHtmlStyledInline(attrs, body);
        })
        .replace(/<span\b([^>]*)>([\s\S]*?)<\/span>/gi, (_match, attrs: string, body: string) => {
            return nexusHtmlStyledInline(attrs, body);
        })
        .replace(/<small\b[^>]*>([\s\S]*?)<\/small>/gi, "[small]$1[/small]")
        .replace(/<big\b[^>]*>([\s\S]*?)<\/big>/gi, "[big]$1[/big]")
        .replace(/<(abbr|acronym)\b([^>]*)>([\s\S]*?)<\/\1>/gi, (_match, _tag: string, attrs: string, body: string) => {
            const title = safeNexusLabel(htmlAttribute(attrs, "title") ?? "");
            return title ? `[abbr=${title}]${body}[/abbr]` : body;
        })
        .replace(/<cite\b[^>]*>([\s\S]*?)<\/cite>/gi, "[cite]$1[/cite]")
        .replace(/<q\b[^>]*>([\s\S]*?)<\/q>/gi, "[q]$1[/q]")
        .replace(/<mark\b([^>]*)>([\s\S]*?)<\/mark>/gi, (_match, attrs: string, body: string) => {
            const background = htmlBackgroundColorAttribute(attrs) ?? "yellow";
            return `[highlight=${background}]${body}[/highlight]`;
        })
        .replace(/<sub\b[^>]*>([\s\S]*?)<\/sub>/gi, "[sub]$1[/sub]")
        .replace(/<sup\b[^>]*>([\s\S]*?)<\/sup>/gi, "[sup]$1[/sup]")
        .replace(/<strong\b[^>]*>|<b\b[^>]*>/gi, "[b]")
        .replace(/<\/strong>|<\/b>/gi, "[/b]")
        .replace(/<em\b[^>]*>|<i\b[^>]*>/gi, "[i]")
        .replace(/<\/em>|<\/i>/gi, "[/i]")
        .replace(/<u\b[^>]*>/gi, "[u]")
        .replace(/<\/u>/gi, "[/u]")
        .replace(/<s\b[^>]*>|<strike\b[^>]*>/gi, "[s]")
        .replace(/<\/s>|<\/strike>/gi, "[/s]")
        .replace(/<del\b[^>]*>/gi, "[s]")
        .replace(/<\/del>/gi, "[/s]")
        .replace(/<figure\b[^>]*>/gi, "\n\n[box=Media]\n")
        .replace(/<\/figure>/gi, "\n[/box]\n\n")
        .replace(/<figcaption\b[^>]*>([\s\S]*?)<\/figcaption>/gi, "\n[caption]$1[/caption]\n")
        .replace(/<dl\b[^>]*>/gi, "\n\n[dl]\n")
        .replace(/<\/dl>/gi, "\n[/dl]\n\n")
        .replace(/<dt\b[^>]*>/gi, "\n[dt]")
        .replace(/<\/dt>/gi, "[/dt]\n")
        .replace(/<dd\b[^>]*>/gi, "[dd]")
        .replace(/<\/dd>/gi, "[/dd]\n")
        .replace(/<table\b[^>]*>/gi, "\n\n[table]\n")
        .replace(/<\/table>/gi, "\n[/table]\n\n")
        .replace(/<\/?(?:tbody|thead|tfoot)\b[^>]*>/gi, "")
        .replace(/<tr\b[^>]*>/gi, "\n[tr]")
        .replace(/<\/tr>/gi, "[/tr]\n")
        .replace(/<th\b([^>]*)>/gi, (_match, attrs: string) => `[th${nexusHtmlTableCellAttributes(attrs)}]`)
        .replace(/<\/th>/gi, "[/th]")
        .replace(/<td\b([^>]*)>/gi, (_match, attrs: string) => `[td${nexusHtmlTableCellAttributes(attrs)}]`)
        .replace(/<\/td>/gi, "[/td]")
        .replace(/<ul\b([^>]*)>/gi, (_match, attrs: string) => nexusHtmlListOpeningTag(attrs, false))
        .replace(/<\/ul>/gi, "\n[/list]\n")
        .replace(/<ol\b([^>]*)>/gi, (_match, attrs: string) => nexusHtmlListOpeningTag(attrs, true))
        .replace(/<\/ol>/gi, "\n[/olist]\n")
        .replace(/<li\b[^>]*>/gi, "\n[*]")
        .replace(/<\/li>/gi, "\n")
        .replace(/<\/p>/gi, "\n\n")
        .replace(/<p\b[^>]*>/gi, "");

    text = text
        .replace(/<a\b([^>]*)>([\s\S]*?)<\/a>/gi, (_match, attrs: string, label: string) => {
            const href = htmlAttribute(attrs, "href");
            return href ? `[url=${href}]${label}[/url]` : label;
        })
        .replace(/<img\b([^>]*)>/gi, (_match, attrs: string) => {
            const src = htmlAttribute(attrs, "src");
            return src ? `[img${nexusHtmlImageAttributes(attrs)}]${src}[/img]` : "";
        })
        .replace(/<[^>]*>/g, " ");

    return decodeHtmlEntities(text)
        .replace(/\[\/\*\]/g, "")
        .replace(/\[(?:br|break|nl|newline)\s*\/?\]/gi, "\n")
        .replace(/\[(?:pre|raw|noparse|codebox|plaintext|plain|fixed)(?:=[^\]]+|[ \t][^\]]*)?\]/gi, "[code]")
        .replace(/\[\/(?:pre|raw|noparse|codebox|plaintext|plain|fixed)\]/gi, "[/code]")
        .replace(/\[(?:p|paragraph)\]/gi, "\n\n")
        .replace(/\[\/(?:p|paragraph)\]/gi, "\n\n")
        .replace(/\[(?:imgleft|imageleft)([^\]]*)\]/gi, (_match, attrs: string) => nexusImageAliasOpeningTag("left", attrs))
        .replace(/\[\/(?:imgleft|imageleft)\]/gi, "[/img]")
        .replace(/\[(?:imgright|imageright)([^\]]*)\]/gi, (_match, attrs: string) => nexusImageAliasOpeningTag("right", attrs))
        .replace(/\[\/(?:imgright|imageright)\]/gi, "[/img]")
        .replace(/\[(?:imgcenter|imagecenter)([^\]]*)\]/gi, (_match, attrs: string) => nexusImageAliasOpeningTag("center", attrs))
        .replace(/\[\/(?:imgcenter|imagecenter)\]/gi, "[/img]")
        .replace(/\[(?:thumb|thumbnail|image)([^\]]*)\]/gi, (_match, attrs: string) => normalizedNexusImageOpeningTag(attrs))
        .replace(/\[\/(?:thumb|thumbnail|image)\]/gi, "[/img]")
        .replace(/\[\s*li(?=[\s=\]/])([^\]]*)\]/gi, (_match, attrs: string) => {
            const label = safeNexusListItemLabel(nexusBbTagAttribute("li", attrs));
            return label ? `\n[*][b]${label}[/b] ` : "\n[*]";
        })
        .replace(/\[\/\s*li\]/gi, "\n")
        .replace(/\[(?:centre)([^\]]*)\]/gi, (_match, attrs: string) => `[center${attrs ?? ""}]`)
        .replace(/\[\/(?:centre)\]/gi, "[/center]")
        .replace(/\[(?:colour)([^\]]*)\]/gi, (_match, attrs: string) => `[color${attrs ?? ""}]`)
        .replace(/\[\/(?:colour)\]/gi, "[/color]")
        .replace(/\[(?:bgcolour|backgroundcolour|bcolor)([^\]]*)\]/gi, (_match, attrs: string) => `[background${attrs ?? ""}]`)
        .replace(/\[\/(?:bgcolour|backgroundcolour|bcolor)\]/gi, "[/background]")
        .replace(/\[(?:strikethrough|strikeout)\]/gi, "[s]")
        .replace(/\[\/(?:strikethrough|strikeout)\]/gi, "[/s]")
        .replace(/\[dl[^\]]*\]/gi, "\n\n[dl]\n")
        .replace(/\[\/dl\]/gi, "\n[/dl]\n\n")
        .replace(/\[dt[^\]]*\]/gi, "\n[dt]")
        .replace(/\[\/dt\]/gi, "[/dt]\n")
        .replace(/\[dd[^\]]*\]/gi, "[dd]")
        .replace(/\[\/dd\]/gi, "[/dd]\n")
        .replace(/\[h([1-6])\]/gi, "[heading=$1]")
        .replace(/\[\/h[1-6]\]/gi, "[/heading]")
        .replace(/\[(?:headline|subhead|subheading)(?:=([^\]]+))?\]/gi, (_match, level: string | undefined) => `[heading=${safeNexusHeadingLevel(level ?? "3")}]`)
        .replace(/\[\/(?:headline|subhead|subheading)\]/gi, "[/heading]")
        .replace(/\[(url|img|color|background|bgcolor|highlight|size|align|indent|quote|spoiler|collapse|details|accordion|accordionitem|heading|h|header|title|subtitle|caption|float|youtube|video|media|embed|list|olist|ol|ul|columns|cols|column|col|tabs|tab|note|info|warning|important|tip|box|panel|fieldset|notice|success|danger|error|small|big|mark|abbr|acronym)([ \t][^\]]+)\]/gi, (_match, tag: string, attrs: string) => normalizeNexusBbOpeningTag(tag, attrs))
        .replace(/\[(?:h|header|title)(?:=([^\]]+))?\]/gi, (_match, level: string | undefined) => `[heading=${safeNexusHeadingLevel(level ?? "2")}]`)
        .replace(/\[\/(?:h|header|title)\]/gi, "[/heading]")
        .replace(/\[subtitle(?:=[^\]]+)?\]/gi, "[heading=4]")
        .replace(/\[\/subtitle\]/gi, "[/heading]")
        .replace(/\[(?:ul|list)(?:=([^\]]+))?\]/gi, (_match, attr: string | undefined) => {
            if (isOrderedNexusListAttr(attr)) {
                const type = nexusOrderedListType(attr);
                return `\n[olist${type ? `=${type}` : ""}]\n`;
            }

            return "\n[list]\n";
        })
        .replace(/\[ol(?:=([^\]]+))?\]/gi, (_match, attr: string | undefined) => {
            const type = nexusOrderedListType(attr);
            return `\n[olist${type ? `=${type}` : ""}]\n`;
        })
        .replace(/\[\/(?:ul|ol|list|olist)\]/gi, "\n[/list]\n")
        .replace(/\[\*\s*=([^\]]+)\]/g, (_match, label: string) => {
            const cleanLabel = safeNexusListItemLabel(label);
            return cleanLabel ? `\n[*][b]${cleanLabel}[/b] ` : "\n[*]";
        })
        .replace(/\[\*\]/g, "\n[*]")
        .replace(/\[(columns|cols)([^\]]*)\]/gi, (_match, tag: string, attrs: string) => `\n\n[${tag.toLowerCase()}${attrs ?? ""}]\n`)
        .replace(/\[\/(columns|cols)\]/gi, (_match, tag: string) => `\n[/${tag.toLowerCase()}]\n\n`)
        .replace(/\[(?:column|col)(?=[\s=\]/\]])([^\]]*)\]/gi, (_match, attrs: string) => `\n[column${attrs ?? ""}]\n`)
        .replace(/\[\/(?:column|col)\]/gi, "\n[/column]\n")
        .replace(/\[(?:nextcol|nextcolumn)\s*\/?\]/gi, "\n[nextcol/]\n")
        .replace(/\[tabs([^\]]*)\]/gi, (_match, attrs: string) => `\n\n[tabs${attrs ?? ""}]\n`)
        .replace(/\[\/tabs\]/gi, "\n[/tabs]\n\n")
        .replace(/\[(note|info|warning|important|tip)([^\]]*)\]/gi, (_match, tag: string, attrs: string) => `\n\n[${tag.toLowerCase()}${attrs ?? ""}]\n`)
        .replace(/\[\/(note|info|warning|important|tip)\]/gi, (_match, tag: string) => `\n[/${tag.toLowerCase()}]\n\n`)
        .replace(/\[(box|panel|fieldset|notice|success|danger|error)([^\]]*)\]/gi, (_match, tag: string, attrs: string) => `\n\n[${tag.toLowerCase()}${attrs ?? ""}]\n`)
        .replace(/\[\/(box|panel|fieldset|notice|success|danger|error)\]/gi, (_match, tag: string) => `\n[/${tag.toLowerCase()}]\n\n`)
        .replace(/\[(collapse|details|accordion|accordionitem)([^\]]*)\]/gi, (_match, tag: string, attrs: string) => `\n\n[${tag.toLowerCase()}${attrs ?? ""}]\n`)
        .replace(/\[\/(collapse|details|accordion|accordionitem)\]/gi, (_match, tag: string) => `\n[/${tag.toLowerCase()}]\n\n`)
        .replace(/\[(?:hidden|spoilerblock)([^\]]*)\]/gi, (_match, attrs: string) => `\n\n[spoiler${attrs ?? ""}]\n`)
        .replace(/\[\/(?:hidden|spoilerblock)\]/gi, "\n[/spoiler]\n\n")
        .replace(/\[caption([^\]]*)\]/gi, (_match, attrs: string) => `\n[caption${attrs ?? ""}]`)
        .replace(/\[\/caption\]/gi, "[/caption]\n")
        .replace(/\[(?:row)\]/gi, "[tr]")
        .replace(/\[\/(?:row)\]/gi, "[/tr]")
        .replace(/\[(?:cell)([^\]]*)\]/gi, (_match, attrs: string) => `[td${attrs ?? ""}]`)
        .replace(/\[\/(?:cell)\]/gi, "[/td]")
        .replace(/\[table[^\]]*\]/gi, "\n\n[table]\n")
        .replace(/\[\/table\]/gi, "\n[/table]\n\n")
        .replace(/\[\/?(?:tbody|thead|tfoot)[^\]]*\]/gi, "")
        .replace(/\[tr[^\]]*\]/gi, "[tr]")
        .replace(/\[\/tr\]/gi, "[/tr]\n")
        .replace(/\[(td|th)([^\]]*)\]/gi, (_match, cell: string, attrs: string) => `[${cell.toLowerCase()}${attrs ?? ""}]`)
        .replace(/\[\/(td|th)\]/gi, (_match, cell: string) => `[/${cell.toLowerCase()}]`)
        .replace(/\[(?:hr|line|rule|divider|separator|hrule|horizontalrule)([^\]]*)\]/gi, (_match, attrs: string) => nexusRuleMarkup(attrs))
        .replace(/\[\/(?:hr|line|rule|divider|separator|hrule|horizontalrule)\]/gi, "\n")
        .replace(/\[clear\s*\/?\]/gi, "\n[clear/]\n")
        .replace(/(^|\n)[ \t]*(?:-{3,}|={3,}|_{3,}|\*{3,})[ \t]*(?=\n|$)/g, "$1\n\n[hr]\n\n")
        .replace(/\[(list|olist)([^\]]*)\]\n{2,}/gi, (_match, tag: string, attr: string) => `[${tag.toLowerCase()}${attr ?? ""}]\n`)
        .replace(/\n{2,}\[\/list\]/gi, "\n[/list]")
        .replace(/([^\n])\[list\]/gi, "$1\n\n[list]")
        .replace(/([^\n])\[olist([^\]]*)\]/gi, "$1\n\n[olist$2]")
        .replace(/\[\/list\]([^\n])/gi, "[/list]\n\n$1")
        .replace(/([^\n])\n\[list\]/gi, "$1\n\n[list]")
        .replace(/([^\n])\n\[olist([^\]]*)\]/gi, "$1\n\n[olist$2]")
        .replace(/\[\/list\]\n(?!\n|$)/gi, "[/list]\n\n")
        .replace(/[ \t]+\n/g, "\n")
        .replace(/\n{4,}/g, "\n\n\n")
        .trim();
}

function nexusRuleMarkup(rawAttrs?: string): string {
    const label = safeNexusBbLabel(
        nexusBbAttribute(rawAttrs, "title")
        ?? nexusBbAttribute(rawAttrs, "label")
        ?? nexusBbAttribute(rawAttrs, "name")
        ?? nexusBbTagAttribute("hr", rawAttrs)
        ?? nexusBbFirstAttributeValue(rawAttrs)
    );
    return label ? `\n\n[hr]\n[heading=4]${label}[/heading]\n\n` : "\n\n[hr]\n\n";
}

function renderNexusBlocks(text: string, depth: number): string {
    if (depth > 3) {
        return `<p>${renderNexusInline(text)}</p>`;
    }

    return nexusBlockChunks(text)
        .map(chunk => renderNexusBlock(chunk, depth))
        .join("");
}

function nexusBlockChunks(text: string): NexusBlockChunk[] {
    const chunks: NexusBlockChunk[] = [];
    const structuralPattern = /\[(table|list|olist|dl|quote|spoiler|collapse|details|accordion|accordionitem|indent|center|left|right|align|justify|code|heading|float|youtube|video|media|embed|columns|cols|tabs|note|info|warning|important|tip|box|panel|fieldset|notice|success|danger|error)([^\]]*)\]/gi;
    let cursor = 0;
    let match: RegExpExecArray | null;

    while ((match = structuralPattern.exec(text)) !== null) {
        const tag = match[1].toLowerCase();
        const rawAttrs = match[2] ?? "";
        const attr = nexusBbTagAttribute(tag, rawAttrs);
        const before = text.slice(cursor, match.index);
        if (!isNexusStructuralBoundary(before)) {
            continue;
        }

        const close = findNexusStructureClose(text, structuralPattern.lastIndex, tag);
        if (!close) {
            const remainder = text.slice(structuralPattern.lastIndex).trim();
            if ((tag === "list" || tag === "olist") && /\[\*\]/.test(remainder)) {
                const ordered = tag === "olist" || isOrderedNexusListAttr(attr);
                appendNexusTextChunks(chunks, before);
                chunks.push({
                    kind: "list",
                    value: remainder,
                    ordered,
                    style: nexusListMarkerStyle(attr, ordered) ?? undefined,
                    start: ordered ? nexusOrderedListStart(rawAttrs, attr) : undefined
                });
                cursor = text.length;
                structuralPattern.lastIndex = text.length;
                break;
            }

            continue;
        }

        appendNexusTextChunks(chunks, before);
        const value = text.slice(structuralPattern.lastIndex, close.start).trim();
        if (tag === "table") {
            chunks.push({ kind: "table", value });
        } else if (tag === "dl") {
            chunks.push({ kind: "definition", value });
        } else if (tag === "list" || tag === "olist") {
            const ordered = tag === "olist" || isOrderedNexusListAttr(attr);
            chunks.push({
                kind: "list",
                value,
                ordered,
                style: nexusListMarkerStyle(attr, ordered) ?? undefined,
                start: ordered ? nexusOrderedListStart(rawAttrs, attr) : undefined
            });
        } else {
            chunks.push({ kind: "block", value: text.slice(match.index, close.end).trim() });
        }

        cursor = close.end;
        structuralPattern.lastIndex = close.end;
    }

    appendNexusTextChunks(chunks, text.slice(cursor));
    return chunks;
}

function isNexusStructuralBoundary(value: string): boolean {
    return !value.trim() || /\n\s*$/.test(value);
}

function appendNexusTextChunks(chunks: NexusBlockChunk[], value: string) {
    for (const block of value.split(/\n{2,}/)) {
        const trimmed = block.trim();
        if (trimmed) {
            if (looksLikeNexusLooseTable(trimmed)) {
                chunks.push({ kind: "table", value: trimmed });
                continue;
            }

            if (looksLikeNexusPipeTable(trimmed)) {
                chunks.push({ kind: "table", value: nexusPipeTableToBbcode(trimmed) });
                continue;
            }

            const mixedChunks = nexusMixedPlainListChunks(trimmed);
            if (mixedChunks) {
                chunks.push(...mixedChunks);
                continue;
            }

            const listChunk = nexusPlainListChunk(trimmed);
            chunks.push(listChunk ?? nexusTextBlockChunk(trimmed));
        }
    }
}

function nexusTextBlockChunk(value: string): NexusBlockChunk {
    if (!shouldPreserveNexusLineLayout(value)) {
        return { kind: "block", value };
    }

    return {
        kind: "line",
        value,
        mono: shouldUseNexusMonospaceLineLayout(value)
    };
}

function nexusPlainListChunk(value: string): NexusBlockChunk | null {
    if (hasNexusBlockStructure(value)) {
        return null;
    }

    const lines = value
        .split("\n")
        .map(line => line.trim())
        .filter(Boolean);
    if (lines.length < 2) {
        return null;
    }

    const parsedLines = lines.map(line => nexusPlainListLine(line));
    if (parsedLines.every(Boolean)) {
        const first = parsedLines[0];
        if (!first) {
            return null;
        }

        return {
            kind: "list",
            ordered: first.ordered,
            start: first.start,
            style: first.style,
            value: nexusPlainListLinesToBbcode(parsedLines as NexusPlainListLine[])
        };
    }

    return null;
}

function nexusMixedPlainListChunks(value: string): NexusBlockChunk[] | null {
    if (hasNexusBlockStructure(value)) {
        return null;
    }

    const lines = value.split("\n");
    if (lines.length < 2 && !/\[\*(?:\s*=[^\]]+)?\]/i.test(value)) {
        return null;
    }

    const chunks: NexusBlockChunk[] = [];
    let textLines: string[] = [];
    let listLines: NexusPlainListLine[] = [];

    const pushTextLines = () => {
        const text = textLines.join("\n").trim();
        textLines = [];
        if (!text) {
            return;
        }

        chunks.push(nexusTextBlockChunk(text));
    };

    const pushListLines = () => {
        if (listLines.length === 0) {
            return;
        }

        const shouldRenderList = listLines.length >= 2 || listLines.some(line => line.explicit);
        if (!shouldRenderList) {
            textLines.push(...listLines.map(line => line.raw.trim()).filter(Boolean));
            listLines = [];
            return;
        }

        pushTextLines();
        const first = listLines[0];
        chunks.push({
            kind: "list",
            ordered: first.ordered,
            start: first.start,
            style: first.style,
            value: listLines.map(line => `[*]${line.value}`).join("\n")
        });
        listLines = [];
    };

    for (const line of lines) {
        const parsed = nexusPlainListLine(line);
        if (!parsed) {
            pushListLines();
            textLines.push(line);
            continue;
        }

        if (listLines.length === 0) {
            pushTextLines();
            listLines.push(parsed);
            continue;
        }

        const first = listLines[0];
        const baseIndent = Math.min(...listLines.map(line => line.indent));
        if (parsed.indent > baseIndent || (first.ordered === parsed.ordered && first.style === parsed.style)) {
            listLines.push(parsed);
        } else {
            pushListLines();
            listLines.push(parsed);
        }
    }

    pushListLines();
    pushTextLines();

    return chunks.some(chunk => chunk.kind === "list") ? chunks : null;
}

function nexusPlainListLine(line: string): NexusPlainListLine | null {
    const trimmed = line.trim();
    if (!trimmed) {
        return null;
    }

    const explicit = trimmed.match(/^\[\*(?:\s*=[^\]]+)?\]\s*(.+)$/i);
    if (explicit?.[1]?.trim()) {
        return {
            explicit: true,
            indent: nexusPlainListIndentLevel(line),
            ordered: false,
            raw: line,
            value: explicit[1].trim()
        };
    }

    const bullet = trimmed.match(/^(?:[-*+]|\u2022)\s+(.+)$/);
    if (bullet?.[1]?.trim()) {
        return {
            explicit: false,
            indent: nexusPlainListIndentLevel(line),
            ordered: false,
            raw: line,
            value: bullet[1].trim()
        };
    }

    const numbered = trimmed.match(/^(\d+)[.)]\s+(.+)$/);
    if (numbered?.[2]?.trim()) {
        const start = Number.parseInt(numbered[1], 10);
        return {
            explicit: false,
            indent: nexusPlainListIndentLevel(line),
            ordered: true,
            raw: line,
            start: Number.isFinite(start) && start > 1 ? Math.min(999, start) : undefined,
            style: "1",
            value: numbered[2].trim()
        };
    }

    const alpha = trimmed.match(/^([a-z])[.)]\s+(.+)$/i);
    if (alpha?.[2]?.trim()) {
        return {
            explicit: false,
            indent: nexusPlainListIndentLevel(line),
            ordered: true,
            raw: line,
            style: alpha[1] === alpha[1].toUpperCase() ? "A" : "a",
            value: alpha[2].trim()
        };
    }

    return null;
}

function nexusPlainListIndentLevel(line: string): number {
    const leading = line.match(/^[\t ]*/)?.[0] ?? "";
    const width = leading.replace(/\t/g, "    ").length;
    return Math.max(0, Math.min(4, Math.floor(width / 2)));
}

function nexusPlainListLinesToBbcode(lines: NexusPlainListLine[]): string {
    if (lines.length === 0) {
        return "";
    }

    const minIndent = Math.min(...lines.map(line => line.indent));
    const parts: string[] = [];
    const nestedLists: Array<{ ordered: boolean; style?: string }> = [];
    let previousLevel = 0;

    for (const [index, line] of lines.entries()) {
        let level = Math.max(0, line.indent - minIndent);
        if (index === 0) {
            level = 0;
        }

        level = Math.min(3, Math.min(level, previousLevel + 1));

        while (nestedLists.length > level) {
            parts.push("[/list]");
            nestedLists.pop();
        }

        while (nestedLists.length < level) {
            parts.push(nexusPlainListOpeningTag(line));
            nestedLists.push({ ordered: line.ordered, style: line.style });
        }

        if (level > 0) {
            const active = nestedLists[level - 1];
            if (active && (active.ordered !== line.ordered || active.style !== line.style)) {
                parts.push("[/list]");
                nestedLists.pop();
                parts.push(nexusPlainListOpeningTag(line));
                nestedLists.push({ ordered: line.ordered, style: line.style });
            }
        }

        parts.push(`[*]${line.value}`);
        previousLevel = level;
    }

    while (nestedLists.length > 0) {
        parts.push("[/list]");
        nestedLists.pop();
    }

    return parts.join("\n");
}

function nexusPlainListOpeningTag(line: NexusPlainListLine): string {
    if (line.ordered) {
        if (line.start) {
            return `[olist${line.style ? ` type="${line.style}"` : ""} start="${line.start}"]`;
        }

        return `[olist${line.style ? `=${line.style}` : ""}]`;
    }

    return `[list${line.style ? `=${line.style}` : ""}]`;
}

function shouldPreserveNexusLineLayout(value: string): boolean {
    if (hasNexusBlockStructure(value)) {
        return false;
    }

    const lines = value.split("\n");
    const nonEmpty = lines.map(line => line.trim()).filter(Boolean);
    if (nonEmpty.length < 3) {
        return false;
    }

    const structuralLines = lines.filter(line => {
        const trimmed = line.trim();
        return /^\s{2,}\S/.test(line)
            || /\S\s{2,}\S/.test(line)
            || /^[A-Za-z0-9][A-Za-z0-9 /&+_.()'-]{1,42}:\s+\S/.test(trimmed)
            || /^(?:[A-Z][A-Z0-9 _/&+.'()-]{3,}|[=*_ -]{4,})$/.test(trimmed)
            || /(?:[A-Za-z]:\\|\.{0,2}\/|\\)[^\s]+/.test(trimmed)
            || /(?:\.dll|\.json|\.cfg|\.ini|\.zip|\.rar|\.7z)\b/i.test(trimmed);
    }).length;

    if (structuralLines >= 2) {
        return true;
    }

    const averageLength = nonEmpty.reduce((total, line) => total + line.length, 0) / nonEmpty.length;
    return nonEmpty.length >= 4 && averageLength <= 64 && !/[.!?]\s+[A-Z]/.test(nonEmpty.join(" "));
}

function shouldUseNexusMonospaceLineLayout(value: string): boolean {
    const lines = value.split("\n").filter(line => line.trim());
    if (lines.length < 2) {
        return false;
    }

    const tabbedLines = lines.filter(line => /\S\t+\S/.test(line)).length;
    const spacedColumnLines = lines.filter(line => /\S[ \t]{2,}\S/.test(line)).length;
    const fileOrPathLines = lines.filter(line => /(?:[A-Za-z]:\\|\.{0,2}\/|\\)[^\s]+|(?:\.dll|\.json|\.cfg|\.ini|\.zip|\.rar|\.7z)\b/i.test(line)).length;
    const keyValueGridLines = lines.filter(line => /^[A-Za-z0-9][A-Za-z0-9 /&+_.()'-]{1,32}:\s{2,}\S/.test(line.trim())).length;

    return tabbedLines > 0
        || spacedColumnLines >= 2
        || fileOrPathLines >= 2
        || keyValueGridLines >= 2;
}

function findNexusStructureClose(text: string, startIndex: number, tag: string): { start: number; end: number } | null {
    const escapedTag = tag.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const pattern = tag === "list" || tag === "olist"
        ? /\[\/?(?:list|olist)(?:[^\]]*)\]/gi
        : new RegExp(`\\[\\/?${escapedTag}(?:[^\\]]*)\\]`, "gi");
    pattern.lastIndex = startIndex;
    let depth = 1;
    let match: RegExpExecArray | null;

    while ((match = pattern.exec(text)) !== null) {
        if (match[0].startsWith("[/")) {
            depth -= 1;
            if (depth === 0) {
                return { start: match.index, end: pattern.lastIndex };
            }
        } else {
            depth += 1;
        }
    }

    return null;
}

function renderNexusBlock(chunk: NexusBlockChunk, depth: number): string {
    const block = chunk.value;
    if (!block) {
        return "";
    }

    if (chunk.kind === "list") {
        const listItems = nexusListItems(block);
        if (listItems.length === 0) {
            return "";
        }

        const tag = chunk.ordered ? "ol" : "ul";
        const typeAttribute = chunk.ordered && chunk.style ? ` type="${escapeAttribute(chunk.style)}"` : "";
        const startAttribute = chunk.ordered && chunk.start ? ` start="${chunk.start}"` : "";
        const classAttribute = !chunk.ordered && chunk.style ? ` class="nexus-rich-list-${escapeAttribute(chunk.style)}"` : "";
        return `<${tag}${typeAttribute}${startAttribute}${classAttribute}>${listItems.map(item => `<li>${renderNexusListItem(item, depth)}</li>`).join("")}</${tag}>`;
    }

    if (chunk.kind === "table") {
        return renderNexusTable(block, depth);
    }

    if (chunk.kind === "definition") {
        return renderNexusDefinitionList(block, depth);
    }

    if (chunk.kind === "line") {
        const classes = ["nexus-rich-line-block", chunk.mono ? "nexus-rich-line-block-mono" : ""]
            .filter(Boolean)
            .join(" ");
        return `<div class="${classes}">${renderNexusInline(block)}</div>`;
    }

    const columns = block.match(/^\[(columns|cols)(?:=([^\]]+))?\]([\s\S]*?)\[\/\1\]$/i);
    if (columns) {
        return renderNexusColumns(columns[3], depth);
    }

    const tabs = block.match(/^\[tabs(?:=([^\]]+))?\]([\s\S]*?)\[\/tabs\]$/i);
    if (tabs) {
        return renderNexusTabs(tabs[2], depth);
    }

    const media = block.match(/^\[(youtube|video|media|embed)([^\]]*)\]([\s\S]*?)\[\/\1\]$/i);
    if (media) {
        const rendered = renderNexusMediaBlock(media[1], media[2], media[3]);
        if (rendered) {
            return rendered;
        }
    }

    const callout = block.match(/^\[(note|info|warning|important|tip)(?:=([^\]]+))?\]([\s\S]*?)\[\/\1\]$/i);
    if (callout) {
        const kind = safeNexusCalloutKind(callout[1]);
        const label = safeNexusLabel(callout[2]) || nexusCalloutLabel(kind);
        return `<div class="nexus-rich-callout nexus-rich-callout-${kind}"><span>${escapeHtml(label)}</span>${renderNexusBlocks(callout[3], depth + 1)}</div>`;
    }

    const box = block.match(/^\[(box|panel|fieldset|notice|success|danger|error)(?:=([^\]]+))?\]([\s\S]*?)\[\/\1\]$/i);
    if (box) {
        const kind = safeNexusBoxKind(box[1]);
        const label = safeNexusLabel(box[2]) || nexusBoxLabel(kind);
        return `<div class="nexus-rich-box nexus-rich-box-${kind}">${label ? `<span>${escapeHtml(label)}</span>` : ""}${renderNexusBlocks(box[3], depth + 1)}</div>`;
    }

    const collapsible = block.match(/^\[(collapse|details|accordion|accordionitem)(?:=([^\]]+))?\]([\s\S]*?)\[\/\1\]$/i);
    if (collapsible) {
        const label = safeNexusLabel(collapsible[2]) || "Details";
        return renderNexusDisclosure(label, collapsible[3], depth + 1, "details");
    }

    if (/^\[hr\]$/i.test(block)) {
        return `<hr class="nexus-rich-rule" />`;
    }

    const heading = block.match(/^\[heading(?:=([1-6]))?\]([\s\S]*?)\[\/heading\]$/i);
    if (heading) {
        const level = safeNexusHeadingLevel(heading[1]);
        return `<p class="nexus-rich-heading nexus-rich-heading-${level}">${renderNexusInline(heading[2])}</p>`;
    }

    const standaloneHeading = nexusStandaloneSectionHeading(block);
    if (standaloneHeading) {
        return `<p class="nexus-rich-heading nexus-rich-heading-3">${renderNexusInline(standaloneHeading)}</p>`;
    }

    const code = block.match(/^\[code(?:=([^\]]+)|[ \t]([^\]]+))?\]([\s\S]*?)\[\/code\]$/i);
    if (code) {
        const label = safeNexusLabel(code[1] ?? code[2]);
        return `<pre${label ? ` data-nexus-label="${escapeAttribute(label)}"` : ""}><code>${escapeHtml(code[3])}</code></pre>`;
    }

    const directAligned = block.match(/^\[(center|centre|left|right)(?:=[^\]]+|[ \t][^\]]*)?\]([\s\S]*?)\[\/\1\]$/i);
    const attributedAligned = directAligned ? null : block.match(/^\[align(?:=([^\]]+)|[ \t]([^\]]+))?\]([\s\S]*?)\[\/align\]$/i);
    if (directAligned || attributedAligned) {
        const tagAlignment = directAligned?.[1]?.toLowerCase() === "centre" ? "center" : directAligned?.[1];
        const alignment = safeNexusAlignment(attributedAligned?.[1] ?? attributedAligned?.[2] ?? tagAlignment);
        const body = directAligned?.[2] ?? attributedAligned?.[3] ?? "";
        return `<div class="nexus-rich-align-${alignment}">${renderNexusBlocks(body, depth + 1)}</div>`;
    }

    const quote = block.match(/^\[quote(?:=([^\]]+))?\]([\s\S]*?)\[\/quote\]$/i);
    if (quote) {
        const cite = safeNexusLabel(quote[1]);
        return `<blockquote>${cite ? `<cite>${escapeHtml(cite)}</cite>` : ""}${renderNexusBlocks(quote[2], depth + 1)}</blockquote>`;
    }

    const spoiler = block.match(/^\[spoiler(?:=([^\]]+))?\]([\s\S]*?)\[\/spoiler\]$/i);
    if (spoiler) {
        const label = safeNexusLabel(spoiler[1]) || "Spoiler";
        return renderNexusDisclosure(label, spoiler[2], depth + 1, "spoiler");
    }

    const indented = block.match(/^\[indent(?:=([^\]]+))?\]([\s\S]*?)\[\/indent\]$/i);
    if (indented) {
        const level = safeNexusIndentLevel(indented[1]);
        return `<div class="nexus-rich-indent nexus-rich-indent-${level}">${renderNexusBlocks(indented[2], depth + 1)}</div>`;
    }

    const floated = block.match(/^\[float(?:=([^\]]+))?\]([\s\S]*?)\[\/float\]$/i);
    if (floated) {
        return `<div class="nexus-rich-float nexus-rich-float-${safeNexusFloat(floated[1])}">${renderNexusBlocks(floated[2], depth + 1)}</div>`;
    }

    const justified = block.match(/^\[justify\]([\s\S]*?)\[\/justify\]$/i);
    if (justified) {
        return `<div class="nexus-rich-align-justify">${renderNexusBlocks(justified[1], depth + 1)}</div>`;
    }

    return `<p>${renderNexusInline(block)}</p>`;
}

function renderNexusDisclosure(label: string, body: string, depth: number, kind: "details" | "spoiler"): string {
    const bodyHtml = renderNexusBlocks(body, depth);
    return `<details class="nexus-rich-spoiler-block nexus-rich-disclosure nexus-rich-disclosure-${kind}"><summary>${escapeHtml(label)}</summary><div>${bodyHtml}</div></details>`;
}

function renderNexusListItem(item: string, depth: number): string {
    return hasNexusBlockStructure(item)
        ? renderNexusBlocks(item, depth + 1)
        : renderNexusInline(item);
}

function renderNexusColumns(value: string, depth: number): string {
    const columns = splitNexusColumns(value);
    if (columns.length < 2) {
        return renderNexusBlocks(value, depth + 1);
    }

    const count = Math.min(4, columns.length);
    return `<div class="nexus-rich-columns nexus-rich-columns-${count}">${columns.map(column => `<div>${renderNexusBlocks(column, depth + 1)}</div>`).join("")}</div>`;
}

function splitNexusColumns(value: string): string[] {
    const explicitColumns: string[] = [];
    const explicitPattern = /\[column(?=[\s=\]])[^\]]*\]([\s\S]*?)\[\/column\]/gi;
    let explicitMatch: RegExpExecArray | null;

    while ((explicitMatch = explicitPattern.exec(value)) !== null) {
        const column = explicitMatch[1].trim();
        if (column) {
            explicitColumns.push(column);
        }
    }

    if (explicitColumns.length > 0) {
        return explicitColumns;
    }

    const splitColumns = value
        .split(/\[nextcol\s*\/?\]/i)
        .map(column => column.trim())
        .filter(Boolean);
    return splitColumns.length > 0 ? splitColumns : value.trim() ? [value.trim()] : [];
}

function renderNexusTabs(value: string, depth: number): string {
    const tabs = splitNexusTabs(value);
    if (tabs.length === 0) {
        return renderNexusBlocks(value, depth + 1);
    }

    return `<div class="nexus-rich-tabs">${tabs.map(tab => `<section><b>${escapeHtml(tab.label)}</b>${renderNexusBlocks(tab.value, depth + 1)}</section>`).join("")}</div>`;
}

function splitNexusTabs(value: string): Array<{ label: string; value: string }> {
    const tabs: Array<{ label: string; value: string }> = [];
    const tabPattern = /\[tab(?:=([^\]]+))?\]([\s\S]*?)\[\/tab\]/gi;
    let tabMatch: RegExpExecArray | null;

    while ((tabMatch = tabPattern.exec(value)) !== null) {
        const body = tabMatch[2].trim();
        if (!body) {
            continue;
        }

        tabs.push({
            label: safeNexusLabel(tabMatch[1]) || `Section ${tabs.length + 1}`,
            value: body
        });
    }

    return tabs;
}

function renderNexusDefinitionList(value: string, depth: number): string {
    const entries = nexusDefinitionListEntries(value);
    if (entries.length === 0) {
        return renderNexusBlocks(value, depth + 1);
    }

    return `<dl class="nexus-rich-definition-list">${entries.map(entry => {
        const details = entry.details.length > 0 ? entry.details : [""];
        return `<div><dt>${renderNexusInline(entry.term)}</dt>${details.map(detail => `<dd>${detail ? renderNexusBlocks(detail, depth + 1) : ""}</dd>`).join("")}</div>`;
    }).join("")}</dl>`;
}

function nexusDefinitionListEntries(value: string): Array<{ term: string; details: string[] }> {
    const entries: Array<{ term: string; details: string[] }> = [];
    const entryPattern = /\[dt(?:[^\]]*)\]([\s\S]*?)(?:\[\/dt\])?([\s\S]*?)(?=\[dt(?:[^\]]*)\]|$)/gi;
    let entryMatch: RegExpExecArray | null;

    while ((entryMatch = entryPattern.exec(value)) !== null) {
        const term = entryMatch[1].trim();
        const body = entryMatch[2] ?? "";
        if (!term) {
            continue;
        }

        const details: string[] = [];
        const detailPattern = /\[dd(?:[^\]]*)\]([\s\S]*?)(?:\[\/dd\]|(?=\[dd(?:[^\]]*)\]|\[dt(?:[^\]]*)\]|$))/gi;
        let detailMatch: RegExpExecArray | null;
        while ((detailMatch = detailPattern.exec(body)) !== null) {
            const detail = detailMatch[1].trim();
            if (detail) {
                details.push(detail);
            }
        }

        if (details.length === 0) {
            const fallback = body
                .replace(/\[\/?dd(?:[^\]]*)\]/gi, "")
                .trim();
            if (fallback) {
                details.push(fallback);
            }
        }

        entries.push({ term, details });
    }

    return entries;
}

function renderNexusMediaBlock(tag: string, rawAttrs: string, body: string): string {
    const media = nexusMediaRenderData(tag, rawAttrs, nexusBbTagAttribute(tag, rawAttrs), collectNexusNodeText(parseNexusRichNodes(body)));
    if (!media) {
        return "";
    }

    return `<div class="nexus-rich-media-block"><span>${escapeHtml(media.label)}</span><a href="${escapeAttribute(media.url)}" target="_blank" rel="noreferrer noopener">${escapeHtml(media.urlLabel)}</a></div>`;
}

function hasNexusBlockStructure(value: string): boolean {
    return /\[(?:table|list|olist|dl|quote|spoiler|collapse|details|accordion|accordionitem|indent|center|left|right|align|justify|code|heading|float|youtube|video|media|embed|columns|cols|tabs|note|info|warning|important|tip|box|panel|fieldset|notice|success|danger|error)(?:=[^\]]+|[ \t][^\]]*)?\]/i.test(value)
        || looksLikeNexusLooseTable(value);
}

function looksLikeNexusLooseTable(value: string): boolean {
    return (/\[tr(?:[^\]]*)\][\s\S]*?(?:\[\/tr\]|\[tr(?:[^\]]*)\]|\[\/table\]|$)/i.test(value)
        && /\[(?:td|th)(?:[^\]]*)\][\s\S]*?(?:\[\/(?:td|th)\]|\[(?:td|th)(?:[^\]]*)\]|\[\/tr\]|$)/i.test(value)
    ) || looksLikeNexusCellOnlyTable(value);
}

function looksLikeNexusCellOnlyTable(value: string): boolean {
    if (/\[tr(?:[^\]]*)\]/i.test(value)) {
        return false;
    }

    const cellMatches = value.match(/\[(?:td|th)(?:[^\]]*)\]/gi) ?? [];
    return cellMatches.length >= 2
        && /\[\/(?:td|th)\]/i.test(value);
}

function looksLikeNexusPipeTable(value: string): boolean {
    if (hasNexusBlockStructure(value)) {
        return false;
    }

    const lines = value.split("\n").map(line => line.trim()).filter(Boolean);
    if (lines.length < 2 || lines.length > 16) {
        return false;
    }

    const tableRows = lines.filter(line => splitNexusPipeTableLine(line).length >= 2);
    const separatorRows = lines.filter(isNexusPipeTableSeparator).length;
    return tableRows.length >= 2 && tableRows.length + separatorRows === lines.length;
}

function nexusPipeTableToBbcode(value: string): string {
    const lines = value.split("\n").map(line => line.trim()).filter(Boolean);
    const hasHeader = lines.length > 1 && isNexusPipeTableSeparator(lines[1]);
    const rows = lines
        .filter(line => !isNexusPipeTableSeparator(line))
        .map((line, index) => {
            const cellTag = hasHeader && index === 0 ? "th" : "td";
            return `[tr]${splitNexusPipeTableLine(line).map(cell => `[${cellTag}]${cell}[/${cellTag}]`).join("")}[/tr]`;
        });
    return rows.join("\n");
}

function splitNexusPipeTableLine(line: string): string[] {
    return line
        .replace(/^\|/, "")
        .replace(/\|$/, "")
        .split("|")
        .map(cell => cell.trim())
        .filter(Boolean);
}

function isNexusPipeTableSeparator(line: string): boolean {
    const cells = splitNexusPipeTableLine(line);
    return cells.length >= 2 && cells.every(cell => /^:?-{3,}:?$/.test(cell));
}

function nexusStandaloneSectionHeading(block: string): string | null {
    const trimmed = block.trim();
    if (!trimmed || trimmed.includes("\n") || trimmed.length > 160) {
        return null;
    }

    const wrapped = trimmed.match(/^(?:\[(?:b|u|size(?:=[^\]]+)?|color(?:=[^\]]+)?|background(?:=[^\]]+)?|bgcolor(?:=[^\]]+)?|highlight(?:=[^\]]+)?)\]\s*)+([\s\S]*?)(?:\s*\[\/(?:b|u|size|color|background|bgcolor|highlight)\])+$/i);
    const framed = trimmed.match(/^(?:={2,}|#{1,3}\s+)(.+?)(?:\s*={2,})?$/);
    const labeled = nexusStandaloneLabelHeading(trimmed);
    const candidate = wrapped?.[1] ?? framed?.[1] ?? labeled;
    if (!candidate) {
        return null;
    }

    const label = collectNexusNodeText(parseNexusRichNodes(candidate)).replace(/\s+/g, " ").trim();
    if (label.length < 3 || label.length > 80 || /[.!?]\s*$/.test(label)) {
        return null;
    }

    return candidate.trim();
}

function nexusStandaloneLabelHeading(value: string): string | null {
    const label = value.replace(/:$/, "").trim();
    if (label.length < 3 || label.length > 80 || /[.!?]\s*$/.test(label)) {
        return null;
    }

    if (!/^[a-z0-9][a-z0-9 /&+_.()'-]+:?$/i.test(value)) {
        return null;
    }

    const knownSection = /^(requirements?|installation|install|setup|usage|features?|compatibility|load order|known issues?|changelog|credits?|permissions?|recommended|optional|manual install|vortex install|uninstall|troubleshooting|faq)$/i.test(label);
    const letters = label.replace(/[^a-z]/gi, "");
    const uppercase = label.replace(/[^A-Z]/g, "");
    const uppercaseRatio = letters.length > 0 ? uppercase.length / letters.length : 0;
    return knownSection || uppercaseRatio >= 0.62 ? label : null;
}

function renderNexusTable(block: string, depth: number): string {
    const rows = nexusTableRows(block);
    if (rows.length === 0) {
        const fallback = block
            .replace(/\[\/?tr\]/gi, "\n")
            .replace(/\[(?:td|th)[^\]]*\]/gi, "")
            .replace(/\[\/(?:td|th)\]/gi, " | ")
            .trim();
        return fallback ? `<p>${renderNexusInline(fallback)}</p>` : "";
    }

    return `<div class="nexus-rich-table-wrap"><table class="nexus-rich-table"><tbody>${rows.map(row => {
        const cells = row.cells.map(cell => {
            const tag = cell.header ? "th" : "td";
            const spanAttributes = [
                cell.colspan ? ` colspan="${cell.colspan}"` : "",
                cell.rowspan ? ` rowspan="${cell.rowspan}"` : ""
            ].join("");
            const style = [
                cell.width ? `width: ${cell.width}` : "",
                cell.align ? `text-align: ${cell.align}` : ""
            ].filter(Boolean).join("; ");
            return `<${tag}${spanAttributes}${style ? ` style="${escapeAttribute(style)}"` : ""}>${renderNexusBlocks(cell.value, depth + 1)}</${tag}>`;
        }).join("");
        return `<tr>${cells}</tr>`;
    }).join("")}</tbody></table></div>`;
}

function nexusTableRows(block: string): Array<{ cells: NexusTableCell[] }> {
    const rows: Array<{ cells: NexusTableCell[] }> = [];
    const rowPattern = /\[tr\]([\s\S]*?)(?:\[\/tr\]|(?=\[tr\]|\[\/table\]|$))/gi;
    let rowMatch: RegExpExecArray | null;

    while ((rowMatch = rowPattern.exec(block)) !== null) {
        const cells = nexusTableCells(rowMatch[1]);
        if (cells.length > 0) {
            rows.push({ cells });
        }
    }

    if (rows.length === 0) {
        return nexusCellOnlyTableRows(block);
    }

    return rows;
}

function nexusCellOnlyTableRows(block: string): Array<{ cells: NexusTableCell[] }> {
    if (!looksLikeNexusCellOnlyTable(block)) {
        return [];
    }

    const groups = block
        .split(/\n{2,}/)
        .map(group => group.trim())
        .filter(Boolean);
    const groupedRows = groups
        .map(group => ({ cells: nexusTableCells(group) }))
        .filter(row => row.cells.length > 0);

    if (groupedRows.length > 1 && groupedRows.some(row => row.cells.length > 1)) {
        return groupedRows;
    }

    const cells = nexusTableCells(block);
    return cells.length > 0 ? [{ cells }] : [];
}

function nexusTableCells(value: string): NexusTableCell[] {
    const cells: NexusTableCell[] = [];
    const cellPattern = /\[(td|th)([^\]]*)\]([\s\S]*?)(?:\[\/\1\]|(?=\[(?:td|th)(?:[^\]]*)\]|\[\/tr\]|$))/gi;
    let cellMatch: RegExpExecArray | null;

    while ((cellMatch = cellPattern.exec(value)) !== null) {
        const attrs = cellMatch[2] ?? "";
        cells.push({
            value: cellMatch[3].trim(),
            header: cellMatch[1].toLowerCase() === "th",
            colspan: nexusTableCellColspan(attrs),
            rowspan: safeNexusTableSpan(nexusBbAttribute(attrs, "rowspan") ?? nexusBbAttribute(attrs, "row")),
            width: nexusTableCellWidth(attrs),
            align: nexusTableCellAlignment(attrs)
        });
    }

    return cells;
}

function nexusTableCellColspan(attrs: string): number | undefined {
    const explicit = nexusBbAttribute(attrs, "colspan") ?? nexusBbAttribute(attrs, "col");
    if (explicit) {
        return safeNexusTableSpan(explicit);
    }

    const direct = nexusBbFirstAttributeValue(attrs);
    if (!direct || !/^\d{1,2}$/.test(direct.trim())) {
        return undefined;
    }

    return safeNexusTableSpan(direct);
}

function nexusTableCellWidth(attrs: string): string | undefined {
    const direct = nexusBbFirstAttributeValue(attrs);
    const candidate = nexusBbAttribute(attrs, "width")
        ?? nexusBbAttribute(attrs, "w")
        ?? (direct && (!/^\d{1,2}$/.test(direct.trim()) || Number.parseInt(direct, 10) > 6) ? direct : undefined);
    return safeNexusCssLength(candidate) ?? undefined;
}

function nexusTableCellAlignment(attrs: string): "left" | "center" | "right" | "justify" | undefined {
    const candidate = nexusBbAttribute(attrs, "align")
        ?? nexusBbAttribute(attrs, "text-align")
        ?? nexusBbAttribute(attrs, "halign");
    if (!candidate) {
        return undefined;
    }

    return safeNexusAlignment(candidate);
}

function nexusListItems(block: string): string[] {
    const cleaned = block
        .replace(/^\[(?:list|olist)(?:=[^\]]+|[ \t][^\]]*)?\]\s*/i, "")
        .replace(/\s*\[\/list\]$/i, "")
        .replace(/\s*\[\/olist\]$/i, "")
        .trim();
    const markedItems = splitNexusMarkedListItems(cleaned);
    if (markedItems.length > 0) {
        return markedItems;
    }

    const lines = cleaned.split("\n").map(line => line.trim()).filter(Boolean);
    if (lines.length > 0 && lines.every(line => /^(?:\[\*\]|[-*]\s+|\d+[.)]\s+)/.test(line))) {
        return lines.map(line => line.replace(/^(?:\[\*\]|[-*]\s+|\d+[.)]\s+)/, "").trim()).filter(Boolean);
    }

    return lines.length > 1 ? lines : cleaned ? [cleaned] : [];
}

function splitNexusMarkedListItems(value: string): string[] {
    const items: string[] = [];
    const tokenPattern = /\[\*(?:\s*=[^\]]+)?\]|\[\/?(?:list|olist|table)(?:=[^\]]+|[ \t][^\]]*)?\]/gi;
    let listDepth = 0;
    let tableDepth = 0;
    let currentStart: number | null = null;
    let match: RegExpExecArray | null;

    while ((match = tokenPattern.exec(value)) !== null) {
        const token = match[0].toLowerCase();
        if (token.startsWith("[*")) {
            if (listDepth === 0 && tableDepth === 0) {
                if (currentStart !== null) {
                    const item = value.slice(currentStart, match.index).trim();
                    if (item) {
                        items.push(item);
                    }
                }

                currentStart = tokenPattern.lastIndex;
            }
            continue;
        }

        if (token.startsWith("[/table")) {
            tableDepth = Math.max(0, tableDepth - 1);
        } else if (token.startsWith("[table")) {
            tableDepth += 1;
        } else if (token.startsWith("[/list") || token.startsWith("[/olist")) {
            listDepth = Math.max(0, listDepth - 1);
        } else {
            listDepth += 1;
        }
    }

    if (currentStart !== null) {
        const item = value.slice(currentStart).trim();
        if (item) {
            items.push(item);
        }
    }

    return items;
}

function renderNexusInline(value: string): string {
    const inlineValue = value
        .replace(/\[\/?(?:ul|ol|olist|list)(?:=[^\]]+|[ \t][^\]]*)?\]/gi, "\n")
        .replace(/\[\*\]/g, "\n- ");
    return renderNexusNodes(parseNexusRichNodes(inlineValue));
}

function parseNexusRichNodes(value: string): NexusRichNode[] {
    const root: { children: NexusRichNode[] } = { children: [] };
    const stack: Array<{ name: string; children: NexusRichNode[] }> = [{ name: "root", children: root.children }];
    const tagPattern = /\[(\/?)([a-z][a-z0-9_-]*)([^\]]*)\]/gi;
    let lastIndex = 0;
    let match: RegExpExecArray | null;

    while ((match = tagPattern.exec(value)) !== null) {
        if (match.index > lastIndex) {
            stack[stack.length - 1].children.push({ kind: "text", value: value.slice(lastIndex, match.index) });
        }

        const full = match[0];
        const name = match[2].toLowerCase();
        const isClosing = match[1] === "/";
        const rawAttrs = match[3] ?? "";
        const isSelfClosing = /\/\s*$/.test(rawAttrs) || isNexusVoidRichTag(name, rawAttrs);

        if (!NEXUS_RICH_BB_TAGS.has(name) || (!isClosing && !isSelfClosing && stack.length > NEXUS_RICH_MAX_NESTING)) {
            stack[stack.length - 1].children.push({ kind: "text", value: full });
        } else if (isClosing) {
            if (stack.length > 1 && stack[stack.length - 1].name === name) {
                stack.pop();
            } else {
                const openTagIndex = lastNexusOpenTagIndex(stack, name);
                if (openTagIndex > 0) {
                    stack.length = openTagIndex;
                } else if (isNexusLooseClosingTag(name)) {
                    // Standalone Nexus image aliases often leave a harmless closing token behind.
                }
            }
        } else {
            const node: NexusRichNode = { kind: "tag", name, attr: nexusBbTagAttribute(name, rawAttrs), rawAttrs, children: [] };
            stack[stack.length - 1].children.push(node);
            if (!isSelfClosing) {
                stack.push(node);
            }
        }

        lastIndex = tagPattern.lastIndex;
    }

    if (lastIndex < value.length) {
        stack[stack.length - 1].children.push({ kind: "text", value: value.slice(lastIndex) });
    }

    return root.children;
}

function lastNexusOpenTagIndex(stack: Array<{ name: string; children: NexusRichNode[] }>, name: string): number {
    for (let index = stack.length - 1; index > 0; index -= 1) {
        if (stack[index].name === name) {
            return index;
        }
    }

    return -1;
}

function isNexusVoidRichTag(name: string, rawAttrs?: string): boolean {
    if (name === "clear" || name === "nextcol") {
        return true;
    }

    if ((name === "anchor" || name === "bookmark" || name === "target") && nexusBbTagAttribute(name, rawAttrs)) {
        return true;
    }

    if (name !== "img" && name !== "image" && name !== "thumb" && name !== "thumbnail") {
        return false;
    }

    const direct = nexusBbDirectAttribute(rawAttrs);
    return Boolean(
        safeNexusUrl(nexusBbAttribute(rawAttrs, "src"))
        ?? safeNexusUrl(nexusBbAttribute(rawAttrs, "url"))
        ?? safeNexusUrl(direct)
    );
}

function isNexusLooseClosingTag(name: string): boolean {
    return name === "img"
        || name === "image"
        || name === "thumb"
        || name === "thumbnail"
        || name === "clear"
        || name === "nextcol"
        || name === "anchor"
        || name === "bookmark"
        || name === "target";
}

function renderNexusNodes(nodes: NexusRichNode[]): string {
    return nodes.map(node => renderNexusNode(node)).join("");
}

function renderNexusNode(node: NexusRichNode): string {
    if (node.kind === "text") {
        return renderNexusTextNode(node.value);
    }

    const inner = renderNexusNodes(node.children);
    switch (node.name) {
        case "b":
            return `<strong>${inner}</strong>`;
        case "i":
            return `<em>${inner}</em>`;
        case "u":
            return `<u>${inner}</u>`;
        case "s":
        case "strike":
        case "strikethrough":
        case "del":
            return `<s>${inner}</s>`;
        case "sub":
            return `<sub>${inner}</sub>`;
        case "sup":
            return `<sup>${inner}</sup>`;
        case "small":
            return `<span class="nexus-rich-size-small">${inner}</span>`;
        case "big":
            return `<span class="nexus-rich-size-large">${inner}</span>`;
        case "mark": {
            const color = safeNexusColor(node.attr) ?? "yellow";
            return `<span class="nexus-rich-highlight" style="background-color: ${escapeAttribute(color)}">${inner}</span>`;
        }
        case "abbr":
        case "acronym": {
            const title = safeNexusLabel(node.attr);
            return title ? `<abbr class="nexus-rich-abbr" title="${escapeAttribute(title)}">${inner}</abbr>` : `<abbr class="nexus-rich-abbr">${inner}</abbr>`;
        }
        case "cite":
            return `<cite class="nexus-rich-cite">${inner}</cite>`;
        case "q":
            return `<q>${inner}</q>`;
        case "url": {
            const url = safeNexusUrl(node.attr ?? collectNexusNodeText(node.children));
            return url ? `<a href="${escapeAttribute(url)}" target="_blank" rel="noreferrer noopener">${inner || escapeHtml(url)}</a>` : inner;
        }
        case "img":
        case "image":
        case "thumb":
        case "thumbnail": {
            const text = collectNexusNodeText(node.children);
            const attrUrl = safeNexusUrl(nexusImageUrlAttribute(node.rawAttrs, node.attr));
            const textUrl = safeNexusUrl(text);
            const url = attrUrl ?? textUrl;
            const alignment = nexusImageAlignment(node.rawAttrs, node.attr);
            const style = nexusImageStyleAttribute(node.rawAttrs);
            const imageClass = ["nexus-rich-image", alignment ? `nexus-rich-image-${alignment}` : ""].filter(Boolean).join(" ");
            const alt = safeNexusImageAlt(nexusBbAttribute(node.rawAttrs, "alt") ?? nexusBbAttribute(node.rawAttrs, "title"));
            const title = safeNexusImageAlt(nexusBbAttribute(node.rawAttrs, "title") ?? nexusBbAttribute(node.rawAttrs, "alt"));
            return url ? `<img class="${imageClass}" src="${escapeAttribute(url)}" alt="${escapeAttribute(alt)}"${title ? ` title="${escapeAttribute(title)}"` : ""}${style} loading="lazy" />${attrUrl && inner ? inner : ""}` : inner;
        }
        case "color": {
            const color = safeNexusColor(node.attr);
            return color ? `<span style="color: ${escapeAttribute(color)}">${inner}</span>` : inner;
        }
        case "colour": {
            const color = safeNexusColor(node.attr);
            return color ? `<span style="color: ${escapeAttribute(color)}">${inner}</span>` : inner;
        }
        case "background":
        case "bgcolor":
        case "bgcolour":
        case "backgroundcolour":
        case "bcolor":
        case "highlight": {
            const color = safeNexusColor(node.attr) ?? (node.name === "highlight" ? "yellow" : null);
            return color ? `<span class="nexus-rich-highlight" style="background-color: ${escapeAttribute(color)}">${inner}</span>` : inner;
        }
        case "size":
            return `<span class="${nexusSizeClass(node.attr)}">${inner}</span>`;
        case "center":
        case "centre":
        case "left":
        case "right":
            return `<span class="nexus-rich-align-${node.name === "centre" ? "center" : node.name}">${inner}</span>`;
        case "justify":
            return `<span class="nexus-rich-align-justify">${inner}</span>`;
        case "align":
            return `<span class="nexus-rich-align-${safeNexusAlignment(node.attr)}">${inner}</span>`;
        case "indent": {
            const level = safeNexusIndentLevel(node.attr);
            return `<span class="nexus-rich-indent-inline nexus-rich-indent-${level}">${inner}</span>`;
        }
        case "float":
            return `<span class="nexus-rich-float nexus-rich-float-${safeNexusFloat(node.attr)}">${inner}</span>`;
        case "clear":
            return `<span class="nexus-rich-clear"></span>${inner}`;
        case "columns":
        case "cols":
            return `<span class="nexus-rich-columns-inline">${inner}</span>`;
        case "column":
        case "col":
        case "tab":
        case "dl":
        case "dt":
        case "dd":
            return inner;
        case "nextcol":
            return "<br>";
        case "note":
        case "info":
        case "warning":
        case "important":
        case "tip": {
            const kind = safeNexusCalloutKind(node.name);
            const label = safeNexusLabel(node.attr) || nexusCalloutLabel(kind);
            return `<span class="nexus-rich-callout-inline nexus-rich-callout-${kind}"><b>${escapeHtml(label)}</b> ${inner}</span>`;
        }
        case "code":
        case "pre":
        case "tt":
        case "kbd":
        case "samp":
        case "var":
            return `<code>${escapeHtml(collectNexusNodeText(node.children))}</code>`;
        case "quote": {
            const cite = safeNexusLabel(node.attr);
            return `<blockquote>${cite ? `<cite>${escapeHtml(cite)}</cite>` : ""}${inner}</blockquote>`;
        }
        case "spoiler":
            return `<span class="nexus-rich-spoiler">${safeNexusLabel(node.attr) ? `<b>${escapeHtml(safeNexusLabel(node.attr) ?? "")}</b> ` : ""}${inner}</span>`;
        case "collapse":
        case "details":
        case "accordion":
        case "accordionitem":
            return `<span class="nexus-rich-spoiler">${safeNexusLabel(node.attr) ? `<b>${escapeHtml(safeNexusLabel(node.attr) ?? "")}</b> ` : ""}${inner}</span>`;
        case "heading":
            return `<span class="nexus-rich-heading nexus-rich-heading-${safeNexusHeadingLevel(node.attr)}">${inner}</span>`;
        case "h":
        case "header":
        case "title":
            return `<span class="nexus-rich-heading nexus-rich-heading-2">${inner}</span>`;
        case "subtitle":
            return `<span class="nexus-rich-heading nexus-rich-heading-4">${inner}</span>`;
        case "caption":
            return `<span class="nexus-rich-caption">${inner}</span>`;
        case "anchor":
        case "bookmark":
        case "target": {
            const label = safeNexusLabel(node.attr);
            return `<span class="nexus-rich-anchor"${label ? ` title="${escapeAttribute(label)}"` : ""}>${inner}</span>`;
        }
        case "goto":
        case "jump":
            return `<span class="nexus-rich-anchor-ref">${inner || escapeHtml(safeNexusLabel(node.attr) || "Jump")}</span>`;
        case "h1":
        case "h2":
        case "h3":
        case "h4":
        case "h5":
        case "h6":
            return `<span class="nexus-rich-heading nexus-rich-heading-${node.name.slice(1)}">${inner}</span>`;
        case "youtube":
        case "video":
        case "media":
        case "embed": {
            const media = nexusMediaRenderData(node.name, node.rawAttrs, node.attr, collectNexusNodeText(node.children));
            return media ? `<a class="nexus-rich-media-link" href="${escapeAttribute(media.url)}" target="_blank" rel="noreferrer noopener">${escapeHtml(media.inlineLabel)}</a>` : inner;
        }
        case "font": {
            const color = safeNexusColor(nexusBbAttribute(node.rawAttrs, "color"));
            const background = safeNexusColor(
                nexusBbAttribute(node.rawAttrs, "background")
                ?? nexusBbAttribute(node.rawAttrs, "background-color")
                ?? nexusBbAttribute(node.rawAttrs, "bgcolor")
            );
            const size = nexusBbAttribute(node.rawAttrs, "size")
                ?? nexusBbAttribute(node.rawAttrs, "font-size");
            const fontClass = safeNexusFontClass(
                nexusBbAttribute(node.rawAttrs, "face")
                ?? nexusBbAttribute(node.rawAttrs, "font")
                ?? nexusBbAttribute(node.rawAttrs, "font-family")
                ?? node.attr
            );
            const classes = ["nexus-rich-font", fontClass ? `nexus-rich-font-${fontClass}` : "", size ? nexusSizeClass(size) : ""]
                .filter(Boolean)
                .join(" ");
            const style = [
                color ? `color: ${color}` : "",
                background ? `background-color: ${background}` : ""
            ].filter(Boolean).join("; ");
            return style || fontClass || size ? `<span class="${classes}"${style ? ` style="${escapeAttribute(style)}"` : ""}>${inner}</span>` : inner;
        }
        case "box":
        case "panel":
        case "fieldset":
        case "notice":
        case "success":
        case "danger":
        case "error": {
            const kind = safeNexusBoxKind(node.name);
            const label = safeNexusLabel(node.attr) || nexusBoxLabel(kind);
            return `<span class="nexus-rich-box-inline nexus-rich-box-${kind}">${label ? `<b>${escapeHtml(label)}</b> ` : ""}${inner}</span>`;
        }
        default:
            return inner;
    }
}

function renderNexusTextNode(value: string): string {
    return escapeHtml(value.replace(/\t/g, "    "))
        .replace(/ {2,}/g, spaces => {
            const preserved = Math.min(spaces.length - 1, 8);
            return ` ${"&nbsp;".repeat(preserved)}${spaces.length - preserved - 1 > 0 ? " " : ""}`;
        })
        .replace(/\n/g, "<br>");
}

function normalizeNexusBbOpeningTag(tag: string, rawAttrs: string): string {
    const name = tag.toLowerCase();
    if (name === "img") {
        return normalizedNexusImageOpeningTag(rawAttrs);
    }

    const attr = nexusBbTagAttribute(name, rawAttrs);
    if (name === "ol" || name === "olist") {
        return normalizedNexusOrderedListOpeningTag(attr, rawAttrs);
    }

    if (name === "ul") {
        const style = nexusListMarkerStyle(attr, false);
        return `\n[list${style ? `=${style}` : ""}]\n`;
    }

    if (name === "list") {
        if (isOrderedNexusListAttr(attr)) {
            return normalizedNexusOrderedListOpeningTag(attr, rawAttrs);
        }

        const style = nexusListMarkerStyle(attr, false);
        return `\n[list${style ? `=${style}` : ""}]\n`;
    }

    return attr ? `[${name}=${attr}]` : `[${name}]`;
}

function normalizedNexusOrderedListOpeningTag(attr: string | undefined, rawAttrs?: string): string {
    const type = nexusOrderedListType(attr);
    const start = nexusOrderedListStart(rawAttrs);
    if (start) {
        return `\n[olist${type ? ` type="${type}"` : ""} start="${start}"]\n`;
    }

    return `\n[olist${type ? `=${type}` : ""}]\n`;
}

function nexusImageAliasOpeningTag(alignment: "left" | "right" | "center", rawAttrs: string): string {
    const attrs = (rawAttrs ?? "").trim();
    if (!attrs) {
        return `[img align="${alignment}"]`;
    }

    if (attrs.startsWith("=")) {
        const source = cleanNexusBbAttributeValue(attrs.slice(1));
        return source ? `[img align="${alignment}" src="${source}"]` : `[img align="${alignment}"]`;
    }

    return `[img align="${alignment}" ${attrs}]`;
}

function normalizedNexusImageOpeningTag(rawAttrs: string): string {
    const direct = nexusBbDirectAttribute(rawAttrs);
    const source = safeNexusUrl(nexusBbAttribute(rawAttrs, "src"))
        ?? safeNexusUrl(nexusBbAttribute(rawAttrs, "url"))
        ?? safeNexusUrl(direct)
        ?? undefined;
    const alignment = nexusImageAlignment(rawAttrs, direct);
    const width = safeNexusCssLength(nexusBbAttribute(rawAttrs, "width") ?? nexusBbAttribute(rawAttrs, "w"));
    const height = safeNexusCssLength(nexusBbAttribute(rawAttrs, "height") ?? nexusBbAttribute(rawAttrs, "h"));
    const alt = safeNexusImageAlt(nexusBbAttribute(rawAttrs, "alt"));
    const title = safeNexusImageAlt(nexusBbAttribute(rawAttrs, "title"));
    const attrs = [
        source ? `src="${source}"` : "",
        alignment ? `align="${alignment}"` : "",
        width ? `width="${width}"` : "",
        height ? `height="${height}"` : "",
        nexusSafeBbAttribute("alt", alt),
        nexusSafeBbAttribute("title", title)
    ].filter(Boolean);

    if (attrs.length > 0) {
        return `[img ${attrs.join(" ")}]`;
    }

    return direct ? `[img=${direct}]` : "[img]";
}

function nexusSafeBbAttribute(name: string, value?: string): string {
    const cleaned = value?.trim();
    return cleaned ? `${name}="${escapeHtml(cleaned)}"` : "";
}

function nexusBbTagAttribute(tag: string, rawAttrs?: string): string | undefined {
    const direct = nexusBbDirectAttribute(rawAttrs);
    switch (tag) {
        case "url":
            return nexusBbAttribute(rawAttrs, "href")
                ?? nexusBbAttribute(rawAttrs, "url")
                ?? direct
                ?? nexusBbFirstAttributeValue(rawAttrs);
        case "img":
            return nexusBbAttribute(rawAttrs, "src")
                ?? nexusBbAttribute(rawAttrs, "url")
                ?? direct
                ?? nexusBbFirstAttributeValue(rawAttrs);
        case "quote":
            return nexusBbAttribute(rawAttrs, "name")
                ?? nexusBbAttribute(rawAttrs, "author")
                ?? nexusBbAttribute(rawAttrs, "cite")
                ?? direct
                ?? nexusBbFirstAttributeValue(rawAttrs);
        case "spoiler":
        case "collapse":
        case "details":
        case "accordion":
        case "accordionitem":
            return nexusBbAttribute(rawAttrs, "title")
                ?? nexusBbAttribute(rawAttrs, "label")
                ?? nexusBbAttribute(rawAttrs, "name")
                ?? direct
                ?? nexusBbFirstAttributeValue(rawAttrs);
        case "anchor":
        case "bookmark":
        case "target":
        case "goto":
        case "jump":
            return nexusBbAttribute(rawAttrs, "id")
                ?? nexusBbAttribute(rawAttrs, "href")
                ?? nexusBbAttribute(rawAttrs, "name")
                ?? nexusBbAttribute(rawAttrs, "target")
                ?? nexusBbAttribute(rawAttrs, "title")
                ?? nexusBbAttribute(rawAttrs, "label")
                ?? direct
                ?? nexusBbFirstAttributeValue(rawAttrs);
        case "tab":
        case "note":
        case "info":
        case "warning":
        case "important":
        case "tip":
        case "box":
        case "panel":
        case "fieldset":
        case "notice":
        case "success":
        case "danger":
        case "error":
        case "li":
            return nexusBbAttribute(rawAttrs, "title")
                ?? nexusBbAttribute(rawAttrs, "label")
                ?? nexusBbAttribute(rawAttrs, "name")
                ?? direct
                ?? nexusBbFirstAttributeValue(rawAttrs);
        case "align":
        case "background":
        case "bgcolor":
        case "columns":
        case "cols":
        case "float":
        case "color":
        case "heading":
        case "highlight":
        case "indent":
        case "list":
        case "ol":
        case "olist":
        case "size":
        case "youtube":
        case "video":
        case "media":
        case "embed":
            return nexusBbAttribute(rawAttrs, "type")
                ?? nexusBbAttribute(rawAttrs, "src")
                ?? nexusBbAttribute(rawAttrs, "url")
                ?? nexusBbAttribute(rawAttrs, "href")
                ?? nexusBbAttribute(rawAttrs, "data")
                ?? nexusBbAttribute(rawAttrs, "style")
                ?? nexusBbAttribute(rawAttrs, "color")
                ?? nexusBbAttribute(rawAttrs, "background")
                ?? nexusBbAttribute(rawAttrs, "background-color")
                ?? nexusBbAttribute(rawAttrs, "align")
                ?? nexusBbAttribute(rawAttrs, "float")
                ?? nexusBbAttribute(rawAttrs, tag)
                ?? direct
                ?? nexusBbFirstAttributeValue(rawAttrs);
        default:
            return direct ?? nexusBbFirstAttributeValue(rawAttrs);
    }
}

function nexusBbDirectAttribute(rawAttrs?: string): string | undefined {
    const trimmed = (rawAttrs ?? "").trim().replace(/\/\s*$/, "").trim();
    if (!trimmed.startsWith("=")) {
        return undefined;
    }

    return cleanNexusBbAttributeValue(trimmed.slice(1));
}

function nexusBbAttribute(rawAttrs: string | undefined, name: string): string | undefined {
    const attrs = rawAttrs ?? "";
    const pattern = new RegExp(`${name}\\s*=\\s*(?:"([^"]*)"|'([^']*)'|([^\\s\\]]+))`, "i");
    const match = attrs.match(pattern);
    return match ? cleanNexusBbAttributeValue(match[1] ?? match[2] ?? match[3]) : undefined;
}

function nexusBbFirstAttributeValue(rawAttrs?: string): string | undefined {
    const trimmed = (rawAttrs ?? "")
        .trim()
        .replace(/\/\s*$/, "")
        .trim();
    if (!trimmed || trimmed.includes("=")) {
        return undefined;
    }

    const quoted = trimmed.match(/^"([^"]*)"|^'([^']*)'/);
    if (quoted) {
        return cleanNexusBbAttributeValue(quoted[1] ?? quoted[2]);
    }

    return cleanNexusBbAttributeValue(trimmed.split(/\s+/)[0]);
}

function cleanNexusBbAttributeValue(value?: string): string | undefined {
    const cleaned = decodeHtmlEntities(value ?? "")
        .replace(/^[\s"'=]+|[\s"']+$/g, "")
        .replace(/[\[\]\r\n]/g, " ")
        .replace(/\s+/g, " ")
        .trim()
        .slice(0, 500);
    return cleaned || undefined;
}

function collectNexusNodeText(nodes: NexusRichNode[]): string {
    return nodes.map(node => node.kind === "text" ? node.value : collectNexusNodeText(node.children)).join("");
}

function htmlAttribute(attrs: string, name: string): string | null {
    const pattern = new RegExp(`${name}\\s*=\\s*(?:"([^"]*)"|'([^']*)'|([^\\s>]+))`, "i");
    const match = attrs.match(pattern);
    return match ? decodeHtmlEntities(match[1] ?? match[2] ?? match[3] ?? "") : null;
}

function nexusHtmlMediaMarkup(tag: string, attrs: string, body = ""): string {
    const source = htmlMediaSource(attrs, body);
    if (!source) {
        return "";
    }

    const title = cleanNexusBbAttributeValue(
        htmlAttribute(attrs, "title")
        ?? htmlAttribute(attrs, "aria-label")
        ?? htmlAttribute(attrs, "alt")
        ?? ""
    );
    const normalizedTag = tag.toLowerCase() === "youtube" ? "youtube" : "media";
    const titleAttribute = title ? ` title="${title}"` : "";
    return `\n\n[${normalizedTag}${titleAttribute}]${source}[/${normalizedTag}]\n\n`;
}

function htmlMediaSource(attrs: string, body = ""): string | null {
    const direct = htmlAttribute(attrs, "src")
        ?? htmlAttribute(attrs, "data")
        ?? htmlAttribute(attrs, "href");
    if (direct) {
        return direct;
    }

    const source = body.match(/<(?:source|param)\b([^>]*)>/i);
    if (!source) {
        return null;
    }

    return htmlAttribute(source[1], "src")
        ?? htmlAttribute(source[1], "value")
        ?? htmlAttribute(source[1], "data");
}

function htmlAlignmentAttribute(attrs: string): "left" | "center" | "right" | "justify" | null {
    const direct = htmlAttribute(attrs, "align");
    const style = htmlAttribute(attrs, "style");
    const classes = htmlAttribute(attrs, "class") ?? "";
    const styled = style?.match(/text-align\s*:\s*([a-z-]+)/i)?.[1];
    const classed = classes.match(/(?:^|\s)(?:align|text)-(left|center|right|justify)(?:\s|$)/i)?.[1]
        ?? classes.match(/(?:^|\s)(left|center|right|justify)(?:\s|$)/i)?.[1];
    const raw = decodeHtmlEntities(direct ?? styled ?? classed ?? "").trim().toLowerCase();
    return raw === "left" || raw === "center" || raw === "right" || raw === "justify" ? raw : null;
}

function htmlColorAttribute(attrs: string): string | null {
    const direct = htmlAttribute(attrs, "color");
    const style = htmlAttribute(attrs, "style");
    const styled = style?.match(/(?:^|;)\s*color\s*:\s*([^;]+)/i)?.[1];
    return safeNexusColor(direct ?? styled ?? "");
}

function htmlBackgroundColorAttribute(attrs: string): string | null {
    const direct = htmlAttribute(attrs, "bgcolor");
    const style = htmlAttribute(attrs, "style");
    const styled = style?.match(/(?:^|;)\s*background(?:-color)?\s*:\s*([^;]+)/i)?.[1];
    return safeNexusColor(direct ?? styled ?? "");
}

function nexusHtmlStyledInline(attrs: string, body: string): string {
    const style = htmlAttribute(attrs, "style") ?? "";
    const color = htmlColorAttribute(attrs);
    const background = htmlBackgroundColorAttribute(attrs);
    const size = htmlSizeAttribute(attrs);
    const fontClass = htmlFontClassAttribute(attrs);
    const weight = style.match(/(?:^|;)\s*font-weight\s*:\s*([^;]+)/i)?.[1]?.trim().toLowerCase();
    const fontStyle = style.match(/(?:^|;)\s*font-style\s*:\s*([^;]+)/i)?.[1]?.trim().toLowerCase();
    const textDecoration = style.match(/(?:^|;)\s*text-decoration(?:-line)?\s*:\s*([^;]+)/i)?.[1]?.trim().toLowerCase() ?? "";
    const wrappers: Array<[string, string]> = [];

    if (fontClass) {
        wrappers.push([`[font=${fontClass}]`, "[/font]"]);
    }

    if (color) {
        wrappers.push([`[color=${color}]`, "[/color]"]);
    }

    if (background) {
        wrappers.push([`[background=${background}]`, "[/background]"]);
    }

    if (size) {
        wrappers.push([`[size=${size}]`, "[/size]"]);
    }

    if (weight && (weight === "bold" || weight === "bolder" || Number.parseInt(weight, 10) >= 600)) {
        wrappers.push(["[b]", "[/b]"]);
    }

    if (fontStyle === "italic" || fontStyle === "oblique") {
        wrappers.push(["[i]", "[/i]"]);
    }

    if (textDecoration.includes("underline")) {
        wrappers.push(["[u]", "[/u]"]);
    }

    if (textDecoration.includes("line-through")) {
        wrappers.push(["[s]", "[/s]"]);
    }

    return wrappers.reduce((text, [open, close]) => `${open}${text}${close}`, body);
}

function htmlFontClassAttribute(attrs: string): "mono" | "serif" | "sans" | null {
    const direct = htmlAttribute(attrs, "face")
        ?? htmlAttribute(attrs, "font")
        ?? htmlAttribute(attrs, "font-family");
    const style = htmlAttribute(attrs, "style");
    const styled = style?.match(/(?:^|;)\s*font-family\s*:\s*([^;]+)/i)?.[1];
    const classes = htmlAttribute(attrs, "class") ?? "";
    const classed = classes.match(/(?:^|\s)(mono|monospace|serif|sans|sans-serif|code|preformatted)(?:\s|$)/i)?.[1];
    return safeNexusFontClass(direct ?? styled ?? classed ?? "");
}

function hasNexusBlockishMarkup(value: string): boolean {
    return /<(?:blockquote|details|div|dl|figure|h[1-6]|ol|p|pre|section|table|ul)\b/i.test(value)
        || /\[(?:table|list|olist|dl|quote|spoiler|collapse|details|accordion|accordionitem|columns|cols|tabs|box|panel|fieldset|notice|note|info|warning|important|tip)(?:[=\s\]])/i.test(value);
}

function htmlSizeAttribute(attrs: string): string | null {
    const direct = htmlAttribute(attrs, "size");
    const style = htmlAttribute(attrs, "style");
    const styled = style?.match(/(?:^|;)\s*font-size\s*:\s*([^;]+)/i)?.[1];
    const raw = decodeHtmlEntities(direct ?? styled ?? "")
        .trim()
        .toLowerCase()
        .replace(/;+\s*$/g, "")
        .replace(/\s*!important\s*$/i, "");
    if (!raw) {
        return null;
    }

    const named = raw.match(/^(?:xx-small|x-small|small|medium|large|x-large|xx-large|smaller|larger)$/)?.[0];
    if (named) {
        return named;
    }

    const numeric = raw.match(/^(\d+(?:\.\d+)?)(px|em|rem|%)?$/);
    if (!numeric) {
        return null;
    }

    const value = Number.parseFloat(numeric[1]);
    if (!Number.isFinite(value)) {
        return null;
    }

    const unit = numeric[2] ?? "px";
    if (unit === "%") {
        return `${Math.max(60, Math.min(180, value))}%`;
    }

    if (unit === "em" || unit === "rem") {
        return `${Math.max(0.7, Math.min(1.8, value))}${unit}`;
    }

    return `${Math.max(9, Math.min(28, value))}px`;
}

function htmlIndentLevelAttribute(attrs: string): 1 | 2 | 3 | 4 | null {
    const style = htmlAttribute(attrs, "style") ?? "";
    const raw = style.match(/(?:^|;)\s*(?:margin-left|padding-left)\s*:\s*([^;]+)/i)?.[1];
    if (!raw) {
        return null;
    }

    const numeric = Number.parseFloat(raw);
    if (!Number.isFinite(numeric) || numeric <= 0) {
        return null;
    }

    if (raw.includes("%")) {
        return Math.max(1, Math.min(4, Math.round(numeric / 6))) as 1 | 2 | 3 | 4;
    }

    if (raw.includes("em") || raw.includes("rem")) {
        return Math.max(1, Math.min(4, Math.round(numeric / 1.5))) as 1 | 2 | 3 | 4;
    }

    return Math.max(1, Math.min(4, Math.round(numeric / 28))) as 1 | 2 | 3 | 4;
}

function nexusHtmlListOpeningTag(attrs: string, ordered: boolean): string {
    const style = htmlAttribute(attrs, "type") ?? htmlListStyleTypeAttribute(attrs) ?? "";
    if (ordered) {
        const type = nexusOrderedListType(style);
        const start = nexusOrderedListStart(` start="${htmlAttribute(attrs, "start") ?? ""}"`);
        if (start) {
            return `\n[olist${type ? ` type="${type}"` : ""} start="${start}"]\n`;
        }

        return `\n[olist${type ? `=${type}` : ""}]\n`;
    }

    const marker = nexusUnorderedListStyle(style);
    return `\n[list${marker ? `=${marker}` : ""}]\n`;
}

function htmlListStyleTypeAttribute(attrs: string): string | null {
    const style = htmlAttribute(attrs, "style") ?? "";
    const styled = style.match(/(?:^|;)\s*list-style(?:-type)?\s*:\s*([^;]+)/i)?.[1];
    const classes = htmlAttribute(attrs, "class") ?? "";
    const classed = classes.match(/(?:^|\s)(?:list-style-)?(disc|circle|square|none|dash|hyphen|check|checklist|decimal|lower-alpha|upper-alpha|lower-latin|upper-latin|lower-roman|upper-roman|roman)(?:\s|$)/i)?.[1];
    return styled ?? classed ?? null;
}

function nexusHtmlTableCellAttributes(attrs: string): string {
    const colspan = safeNexusTableSpan(htmlAttribute(attrs, "colspan") ?? htmlAttribute(attrs, "col") ?? undefined);
    const rowspan = safeNexusTableSpan(htmlAttribute(attrs, "rowspan") ?? htmlAttribute(attrs, "row") ?? undefined);
    const style = htmlAttribute(attrs, "style") ?? "";
    const styledWidth = style.match(/(?:^|;)\s*width\s*:\s*([^;]+)/i)?.[1];
    const width = safeNexusCssLength(htmlAttribute(attrs, "width") ?? styledWidth);
    const alignment = htmlAlignmentAttribute(attrs);
    return [
        colspan ? `colspan="${colspan}"` : "",
        rowspan ? `rowspan="${rowspan}"` : "",
        width ? `width="${width}"` : "",
        alignment ? `align="${alignment}"` : ""
    ].filter(Boolean).map(attribute => ` ${attribute}`).join("");
}

function nexusHtmlImageAttributes(attrs: string): string {
    const alignment = htmlImageAlignment(attrs);
    const width = safeNexusCssLength(htmlAttribute(attrs, "width"));
    const height = safeNexusCssLength(htmlAttribute(attrs, "height"));
    const alt = safeNexusImageAlt(htmlAttribute(attrs, "alt") ?? htmlAttribute(attrs, "aria-label") ?? undefined);
    const title = safeNexusImageAlt(htmlAttribute(attrs, "title") ?? htmlAttribute(attrs, "aria-label") ?? undefined);
    return [
        alignment ? ` align="${alignment}"` : "",
        width ? ` width="${width}"` : "",
        height ? ` height="${height}"` : "",
        alt ? ` alt="${escapeHtml(alt)}"` : "",
        title ? ` title="${escapeHtml(title)}"` : ""
    ].join("");
}

function htmlImageAlignment(attrs: string): "left" | "center" | "right" | null {
    const direct = htmlAttribute(attrs, "align");
    const style = htmlAttribute(attrs, "style");
    const floated = style?.match(/(?:^|;)\s*float\s*:\s*([a-z-]+)/i)?.[1];
    const textAlign = style?.match(/(?:^|;)\s*text-align\s*:\s*([a-z-]+)/i)?.[1];
    const marginAuto = style && /margin(?:-left)?\s*:\s*auto/i.test(style) && /margin(?:-right)?\s*:\s*auto/i.test(style);
    const raw = decodeHtmlEntities(direct ?? floated ?? textAlign ?? (marginAuto ? "center" : "")).trim().toLowerCase();
    return raw === "left" || raw === "center" || raw === "right" ? raw : null;
}

function nexusImageUrlAttribute(rawAttrs?: string, attr?: string): string | undefined {
    return nexusBbAttribute(rawAttrs, "src")
        ?? nexusBbAttribute(rawAttrs, "url")
        ?? (safeNexusUrl(attr) ? attr : undefined);
}

function nexusImageAlignment(rawAttrs?: string, attr?: string): "left" | "center" | "right" | null {
    const candidate = nexusBbAttribute(rawAttrs, "align")
        ?? nexusBbAttribute(rawAttrs, "float")
        ?? nexusBbAttribute(rawAttrs, "position")
        ?? (attr && !safeNexusUrl(attr) ? attr : undefined);
    const normalized = decodeHtmlEntities(candidate ?? "").trim().toLowerCase().replace(/^['"]|['"]$/g, "");
    return normalized === "left" || normalized === "center" || normalized === "right" ? normalized : null;
}

function nexusImageStyleAttribute(rawAttrs?: string): string {
    const width = safeNexusCssLength(nexusBbAttribute(rawAttrs, "width") ?? nexusBbAttribute(rawAttrs, "w"));
    const height = safeNexusCssLength(nexusBbAttribute(rawAttrs, "height") ?? nexusBbAttribute(rawAttrs, "h"));
    const style = [
        width ? `width: ${width}` : "",
        height ? `height: ${height}` : ""
    ].filter(Boolean).join("; ");
    return style ? ` style="${escapeAttribute(style)}"` : "";
}

function safeNexusFloat(value?: string): "left" | "right" | "center" {
    const normalized = decodeHtmlEntities(value ?? "").trim().toLowerCase().replace(/^['"]|['"]$/g, "");
    if (normalized === "right" || normalized === "center") {
        return normalized;
    }

    return "left";
}

function safeNexusCalloutKind(value?: string): "note" | "info" | "warning" | "important" | "tip" {
    const normalized = decodeHtmlEntities(value ?? "").trim().toLowerCase().replace(/^['"]|['"]$/g, "");
    if (normalized === "info" || normalized === "warning" || normalized === "important" || normalized === "tip") {
        return normalized;
    }

    return "note";
}

function nexusCalloutLabel(kind: "note" | "info" | "warning" | "important" | "tip"): string {
    switch (kind) {
        case "info":
            return "Info";
        case "warning":
            return "Warning";
        case "important":
            return "Important";
        case "tip":
            return "Tip";
        default:
            return "Note";
    }
}

function safeNexusBoxKind(value?: string): "box" | "panel" | "fieldset" | "notice" | "success" | "danger" | "error" {
    const normalized = decodeHtmlEntities(value ?? "").trim().toLowerCase().replace(/^['"]|['"]$/g, "");
    if (normalized === "panel" || normalized === "fieldset" || normalized === "notice" || normalized === "success" || normalized === "danger" || normalized === "error") {
        return normalized;
    }

    return "box";
}

function nexusBoxLabel(kind: "box" | "panel" | "fieldset" | "notice" | "success" | "danger" | "error"): string {
    switch (kind) {
        case "panel":
            return "Panel";
        case "fieldset":
            return "Section";
        case "notice":
            return "Notice";
        case "success":
            return "Confirmed";
        case "danger":
        case "error":
            return "Important";
        default:
            return "";
    }
}

function safeNexusCssLength(value?: string | null): string | null {
    const raw = decodeHtmlEntities(value ?? "")
        .trim()
        .toLowerCase()
        .replace(/;+\s*$/g, "")
        .replace(/\s*!important\s*$/i, "")
        .replace(/^['"]|['"]$/g, "");
    const percent = raw.match(/^(\d+(?:\.\d+)?)%$/);
    if (percent) {
        const numeric = Number.parseFloat(percent[1]);
        return Number.isFinite(numeric) ? `${Math.max(10, Math.min(100, numeric))}%` : null;
    }

    const pixels = raw.match(/^(\d+(?:\.\d+)?)(?:px)?$/);
    if (pixels) {
        const numeric = Number.parseFloat(pixels[1]);
        return Number.isFinite(numeric) ? `${Math.max(24, Math.min(900, numeric))}px` : null;
    }

    return null;
}

export function safeNexusUrl(value?: string): string | null {
    const trimmed = decodeHtmlEntities(value ?? "")
        .trim()
        .replace(/^['"]|['"]$/g, "");
    if (!trimmed || /[\r\n<>]/.test(trimmed)) {
        return null;
    }

    let candidate = trimmed;
    if (trimmed.startsWith("//")) {
        candidate = `https:${trimmed}`;
    } else if (/^www\./i.test(trimmed)) {
        candidate = `https://${trimmed}`;
    } else if (trimmed.startsWith("/")) {
        candidate = `https://www.nexusmods.com${trimmed}`;
    } else if (/^(?:sonsoftheforest|games|users|mods)\//i.test(trimmed)) {
        candidate = `https://www.nexusmods.com/${trimmed}`;
    }
    try {
        const url = new URL(candidate);
        return url.protocol === "https:" || url.protocol === "http:" ? url.toString() : null;
    } catch {
        return null;
    }
}

function safeNexusAlignment(value?: string): "left" | "center" | "right" | "justify" {
    const alignment = decodeHtmlEntities(value ?? "").trim().toLowerCase().replace(/^['"]|['"]$/g, "");
    if (alignment === "justify") {
        return "justify";
    }

    return alignment === "center" || alignment === "right" ? alignment : "left";
}

function safeNexusLabel(value?: string): string {
    return decodeHtmlEntities(value ?? "")
        .replace(/^['"]|['"]$/g, "")
        .replace(/\s+/g, " ")
        .trim()
        .slice(0, 80);
}

function safeNexusBbLabel(value?: string): string {
    return safeNexusLabel(value)
        .replace(/[\[\]\r\n]/g, "")
        .trim()
        .slice(0, 70);
}

function safeNexusListItemLabel(value?: string): string {
    return safeNexusLabel(value)
        .replace(/[\[\]\r\n]/g, "")
        .trim()
        .slice(0, 60);
}

function safeNexusImageAlt(value?: string): string {
    return safeNexusLabel(value)
        .replace(/[\[\]\r\n]/g, "")
        .trim()
        .slice(0, 120);
}

function safeNexusFontClass(value?: string): "mono" | "serif" | "sans" | null {
    const normalized = decodeHtmlEntities(value ?? "")
        .replace(/^['"]|['"]$/g, "")
        .replace(/\s*!important\s*$/i, "")
        .toLowerCase();
    if (!normalized) {
        return null;
    }

    if (/(?:consolas|courier|monaco|menlo|mono|fixed|terminal|cascadia|jetbrains|source code|lucida console|inconsolata)/.test(normalized)) {
        return "mono";
    }

    if (/(?:georgia|garamond|cambria|times|serif)/.test(normalized) && !/sans/.test(normalized)) {
        return "serif";
    }

    if (/(?:arial|verdana|tahoma|trebuchet|calibri|segoe|helvetica|sans)/.test(normalized)) {
        return "sans";
    }

    return null;
}

function isOrderedNexusListAttr(value?: string): boolean {
    const attr = decodeHtmlEntities(value ?? "").trim().replace(/^['"]|['"]$/g, "");
    if (!attr) {
        return false;
    }

    const lowered = attr.toLowerCase();
    if (nexusUnorderedListStyle(attr) || lowered === "bullet" || lowered === "bullets") {
        return false;
    }

    return Boolean(nexusOrderedListType(attr)) || lowered === "number" || lowered === "numbers" || lowered === "numeric";
}

function nexusListMarkerStyle(value: string | undefined, ordered: boolean): string | null {
    return ordered ? nexusOrderedListType(value) : nexusUnorderedListStyle(value);
}

function nexusUnorderedListStyle(value?: string): "disc" | "circle" | "square" | "none" | "dash" | "check" | null {
    const attr = nexusListStyleToken(value).toLowerCase();
    if (!attr || attr === "bullet" || attr === "bullets") {
        return null;
    }

    if (attr === "disc" || attr === "circle" || attr === "square") {
        return attr;
    }

    if (attr === "none" || attr === "plain" || attr === "no-bullet" || attr === "nobullet") {
        return "none";
    }

    if (attr === "dash" || attr === "hyphen") {
        return "dash";
    }

    if (attr === "check" || attr === "checklist" || attr === "checkbox" || attr === "tick") {
        return "check";
    }

    return null;
}

function nexusOrderedListType(value?: string): "1" | "a" | "A" | "i" | "I" | null {
    const attr = nexusListStyleToken(value);
    const lowered = attr.toLowerCase();

    if (attr === "1" || lowered === "decimal" || lowered === "decimal-leading-zero" || lowered === "number" || lowered === "numbers" || lowered === "numeric") {
        return "1";
    }

    if (attr === "a" || lowered === "lower-alpha" || lowered === "lower-latin" || lowered === "alpha") {
        return "a";
    }

    if (attr === "A" || lowered === "upper-alpha" || lowered === "upper-latin") {
        return "A";
    }

    if (attr === "i" || lowered === "lower-roman" || lowered === "roman") {
        return "i";
    }

    if (attr === "I" || lowered === "upper-roman") {
        return "I";
    }

    return null;
}

function nexusListStyleToken(value?: string): string {
    const normalized = decodeHtmlEntities(value ?? "")
        .trim()
        .replace(/^['"]|['"]$/g, "")
        .replace(/\s*!important\s*$/i, "");
    const styled = normalized.match(/(?:^|;)\s*list-style(?:-type)?\s*:\s*([^;]+)/i)?.[1];
    return (styled ?? normalized)
        .replace(/^['"]|['"]$/g, "")
        .replace(/;.*$/g, "")
        .trim();
}

function safeNexusHeadingLevel(value?: string): "1" | "2" | "3" | "4" | "5" | "6" {
    const level = decodeHtmlEntities(value ?? "").trim().replace(/^['"]|['"]$/g, "");
    return /^[1-6]$/.test(level) ? level as "1" | "2" | "3" | "4" | "5" | "6" : "3";
}

function safeNexusIndentLevel(value?: string): 1 | 2 | 3 | 4 {
    const numeric = Number.parseInt(decodeHtmlEntities(value ?? "").trim().replace(/^['"]|['"]$/g, ""), 10);
    if (!Number.isFinite(numeric)) {
        return 1;
    }

    return Math.max(1, Math.min(4, numeric)) as 1 | 2 | 3 | 4;
}

function nexusOrderedListStart(rawAttrs?: string, attr?: string): number | undefined {
    const raw = nexusBbAttribute(rawAttrs, "start")
        ?? nexusBbAttribute(rawAttrs, "value")
        ?? (attr && /^\d{1,3}$/.test(attr.trim()) ? attr : undefined);
    const numeric = Number.parseInt(decodeHtmlEntities(raw ?? "").trim().replace(/^['"]|['"]$/g, ""), 10);
    if (!Number.isFinite(numeric) || numeric <= 1) {
        return undefined;
    }

    return Math.min(999, numeric);
}

function safeNexusTableSpan(value?: string): number | undefined {
    const numeric = Number.parseInt(decodeHtmlEntities(value ?? "").trim().replace(/^['"]|['"]$/g, ""), 10);
    if (!Number.isFinite(numeric) || numeric <= 1) {
        return undefined;
    }

    return Math.min(6, numeric);
}

function safeNexusMediaUrl(kind: string, attr?: string, text?: string): string | null {
    const raw = cleanNexusBbAttributeValue(attr) || cleanNexusBbAttributeValue(text) || "";
    const url = safeNexusUrl(raw);
    if (url) {
        return url;
    }

    if (kind === "youtube" && /^[a-z0-9_-]{6,32}$/i.test(raw)) {
        return `https://www.youtube.com/watch?v=${encodeURIComponent(raw)}`;
    }

    return null;
}

function nexusMediaRenderData(kind: string, rawAttrs?: string, attr?: string, text?: string): { url: string; label: string; inlineLabel: string; urlLabel: string } | null {
    const tag = kind.toLowerCase();
    const rawUrl = nexusBbAttribute(rawAttrs, "src")
        ?? nexusBbAttribute(rawAttrs, "url")
        ?? nexusBbAttribute(rawAttrs, "href")
        ?? nexusBbAttribute(rawAttrs, "data")
        ?? (safeNexusUrl(attr) || (tag === "youtube" && /^[a-z0-9_-]{6,32}$/i.test(attr ?? "")) ? attr : undefined);
    const url = safeNexusMediaUrl(tag, rawUrl, text);
    if (!url) {
        return null;
    }

    const label = safeNexusLabel(
        nexusBbAttribute(rawAttrs, "title")
        ?? nexusBbAttribute(rawAttrs, "label")
        ?? nexusBbAttribute(rawAttrs, "name")
        ?? ""
    ) || (tag === "youtube" ? "YouTube Video" : tag === "video" ? "Video" : "Media");
    const inlineLabel = tag === "youtube" ? `Open ${label}` : `Open ${label}`;
    let urlLabel = url;
    try {
        const parsed = new URL(url);
        urlLabel = parsed.hostname.replace(/^www\./i, "") + parsed.pathname;
    } catch {
        urlLabel = url;
    }

    return { url, label, inlineLabel, urlLabel };
}

function safeNexusColor(value?: string): string | null {
    const color = decodeHtmlEntities(value ?? "")
        .trim()
        .replace(/;+\s*$/g, "")
        .replace(/\s*!important\s*$/i, "")
        .replace(/^['"]|['"]$/g, "");
    if (/^#[0-9a-f]{3}(?:[0-9a-f]{3})?(?:[0-9a-f]{2})?$/i.test(color)) {
        return color;
    }

    const rgb = color.match(/^rgba?\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})(?:\s*,\s*(0|1|0?\.\d+))?\s*\)$/i);
    if (rgb) {
        const red = Math.max(0, Math.min(255, Number.parseInt(rgb[1], 10)));
        const green = Math.max(0, Math.min(255, Number.parseInt(rgb[2], 10)));
        const blue = Math.max(0, Math.min(255, Number.parseInt(rgb[3], 10)));
        if (rgb[4] !== undefined) {
            const alpha = Math.max(0, Math.min(1, Number.parseFloat(rgb[4])));
            return `rgba(${red}, ${green}, ${blue}, ${alpha})`;
        }

        return `rgb(${red}, ${green}, ${blue})`;
    }

    const lowered = color.toLowerCase();
    return NEXUS_SAFE_COLOR_NAMES.has(lowered) ? lowered : null;
}

function nexusSizeClass(value?: string): string {
    const raw = decodeHtmlEntities(value ?? "")
        .trim()
        .toLowerCase()
        .replace(/;+\s*$/g, "")
        .replace(/\s*!important\s*$/i, "")
        .replace(/^['"]|['"]$/g, "");
    if (raw === "xx-small" || raw === "x-small" || raw === "tiny") {
        return "nexus-rich-size-tiny";
    }

    if (raw === "small" || raw === "smaller") {
        return "nexus-rich-size-small";
    }

    if (raw === "large" || raw === "larger" || raw === "x-large") {
        return "nexus-rich-size-large";
    }

    if (raw === "xx-large" || raw === "huge") {
        return "nexus-rich-size-xlarge";
    }

    const relative = raw.match(/^([+-])\s*(\d+(?:\.\d+)?)$/);
    if (relative) {
        const delta = Number.parseFloat(relative[2]);
        if (relative[1] === "-") {
            return delta >= 2 ? "nexus-rich-size-tiny" : "nexus-rich-size-small";
        }

        return delta >= 2 ? "nexus-rich-size-xlarge" : "nexus-rich-size-large";
    }

    const numeric = Number.parseFloat(raw);
    if (Number.isFinite(numeric)) {
        const looksLikePixels = raw.includes("px") || raw.includes("pt") || numeric > 7;
        const looksLikePercent = raw.includes("%");
        const looksLikeRelative = raw.includes("em") || raw.includes("rem");
        if (!looksLikePixels && !looksLikePercent && !looksLikeRelative) {
            if (numeric <= 1) {
                return "nexus-rich-size-tiny";
            }

            if (numeric <= 2) {
                return "nexus-rich-size-small";
            }

            if (numeric >= 7) {
                return "nexus-rich-size-xlarge";
            }

            if (numeric >= 5) {
                return "nexus-rich-size-large";
            }

            return "nexus-rich-size-medium";
        }

        if (looksLikePercent ? numeric <= 75 : looksLikeRelative ? numeric <= 0.8 : looksLikePixels ? numeric <= 9 : numeric <= 0.85) {
            return "nexus-rich-size-tiny";
        }

        if (looksLikePercent ? numeric <= 90 : looksLikeRelative ? numeric < 1 : looksLikePixels ? numeric <= 11 : numeric <= 2) {
            return "nexus-rich-size-small";
        }

        if (looksLikePercent ? numeric >= 150 : looksLikeRelative ? numeric >= 1.55 : looksLikePixels ? numeric >= 24 : numeric >= 1.45) {
            return "nexus-rich-size-xlarge";
        }

        if (looksLikePercent ? numeric >= 115 : looksLikeRelative ? numeric >= 1.2 : looksLikePixels ? numeric >= 18 : numeric >= 5) {
            return "nexus-rich-size-large";
        }
    }

    return "nexus-rich-size-medium";
}

function decodeHtmlEntities(value: string): string {
    return value
        .replace(/&#x([0-9a-f]+);/gi, (_match, hex: string) => decodeNumericEntity(Number.parseInt(hex, 16)))
        .replace(/&#(\d+);/g, (_match, decimal: string) => decodeNumericEntity(Number.parseInt(decimal, 10)))
        .replace(/&nbsp;/gi, " ")
        .replace(/&quot;/gi, "\"")
        .replace(/&#39;/g, "'")
        .replace(/&apos;/gi, "'")
        .replace(/&#92;/g, "\\")
        .replace(/&amp;/gi, "&")
        .replace(/&lt;/gi, "<")
        .replace(/&gt;/gi, ">");
}

function decodeNumericEntity(codePoint: number): string {
    try {
        return Number.isFinite(codePoint) && codePoint >= 0 && codePoint <= 0x10ffff
            ? String.fromCodePoint(codePoint)
            : "";
    } catch {
        return "";
    }
}
