const STORE_LINK_BASE = "https://sotf-mods.com";

type RichBlock =
    | { kind: "paragraph"; value: string }
    | { kind: "line"; value: string }
    | { kind: "list"; value: string; ordered: boolean; start?: number }
    | { kind: "quote"; value: string }
    | { kind: "code"; value: string; label?: string }
    | { kind: "heading"; value: string; level: number }
    | { kind: "align"; value: string; align: "left" | "center" | "right" | "justify" }
    | { kind: "table"; value: string };

export function renderStoreRichText(value?: string, fallback = ""): string {
    const normalized = normalizeStoreMarkup(value);
    if (!normalized) {
        return `<p>${escapeHtml(fallback)}</p>`;
    }

    return storeRichBlocks(normalized)
        .map(renderStoreBlock)
        .join("");
}

export function storeRichTextPlainText(value?: string, fallback = ""): string {
    const normalized = normalizeStoreMarkup(value);
    const plain = normalized
        .replace(/\[img[^\]]*\]([\s\S]*?)\[\/img\]/gi, " ")
        .replace(/\[url=([^\]]+)\]([\s\S]*?)\[\/url\]/gi, "$2 ($1)")
        .replace(/\[url\]([\s\S]*?)\[\/url\]/gi, "$1")
        .replace(/\[\*\s*(?:=[^\]]+)?\]/g, "\n- ")
        .replace(/\[\/?(?:table|tr|td|th|list|olist|quote|align|center|left|right|heading|h[1-6]|b|i|u|s|strike|del|code|pre|size|color|colour|background|bgcolor|highlight|font|small|big|sub|sup)[^\]]*\]/gi, " ")
        .replace(/\[\/?[a-z0-9_-]+[^\]]*\]/gi, " ")
        .replace(/[ \t]+/g, " ")
        .replace(/\n\s+/g, "\n")
        .replace(/\n{3,}/g, "\n\n")
        .trim();

    return plain || fallback;
}

function normalizeStoreMarkup(value?: string): string {
    let text = (value ?? "")
        .replace(/\r\n?/g, "\n")
        .replace(/<!--[\s\S]*?-->/g, "")
        .replace(/<script\b[\s\S]*?<\/script>/gi, "")
        .replace(/<style\b[\s\S]*?<\/style>/gi, "")
        .replace(/<br\s*\/?>/gi, "\n")
        .replace(/<h([1-6])\b[^>]*>([\s\S]*?)<\/h\1>/gi, "\n\n[heading=$1]$2[/heading]\n\n")
        .replace(/<blockquote\b[^>]*>/gi, "\n\n[quote]")
        .replace(/<\/blockquote>/gi, "[/quote]\n\n")
        .replace(/<pre\b[^>]*>([\s\S]*?)<\/pre>/gi, "\n\n[code]$1[/code]\n\n")
        .replace(/<code\b[^>]*>([\s\S]*?)<\/code>/gi, "[code]$1[/code]")
        .replace(/<strong\b[^>]*>|<b\b[^>]*>/gi, "[b]")
        .replace(/<\/strong>|<\/b>/gi, "[/b]")
        .replace(/<em\b[^>]*>|<i\b[^>]*>/gi, "[i]")
        .replace(/<\/em>|<\/i>/gi, "[/i]")
        .replace(/<u\b[^>]*>/gi, "[u]")
        .replace(/<\/u>/gi, "[/u]")
        .replace(/<(s|strike|del)\b[^>]*>/gi, "[s]")
        .replace(/<\/(?:s|strike|del)>/gi, "[/s]")
        .replace(/<font\b([^>]*)>([\s\S]*?)<\/font(?:\s+color)?>/gi, (_match, attrs: string, body: string) => {
            const color = htmlAttribute(attrs, "color");
            return color ? `[color=${color}]${body}[/color]` : body;
        })
        .replace(/<ul\b[^>]*>/gi, "\n[list]\n")
        .replace(/<\/ul>/gi, "\n[/list]\n")
        .replace(/<ol\b([^>]*)>/gi, (_match, attrs: string) => {
            const start = htmlAttribute(attrs, "start");
            const safeStart = safeListStart(start ?? undefined);
            return `\n[olist${safeStart ? ` start="${safeStart}"` : ""}]\n`;
        })
        .replace(/<\/ol>/gi, "\n[/list]\n")
        .replace(/<li\b[^>]*>/gi, "\n[*]")
        .replace(/<\/li>/gi, "\n")
        .replace(/<table\b[^>]*>/gi, "\n\n[table]\n")
        .replace(/<\/table>/gi, "\n[/table]\n\n")
        .replace(/<\/?(?:tbody|thead|tfoot)\b[^>]*>/gi, "")
        .replace(/<tr\b[^>]*>/gi, "\n[tr]")
        .replace(/<\/tr>/gi, "[/tr]\n")
        .replace(/<th\b[^>]*>/gi, "[th]")
        .replace(/<\/th>/gi, "[/th]")
        .replace(/<td\b[^>]*>/gi, "[td]")
        .replace(/<\/td>/gi, "[/td]")
        .replace(/<a\b([^>]*)>([\s\S]*?)<\/a>/gi, (_match, attrs: string, label: string) => {
            const href = htmlAttribute(attrs, "href");
            return href ? `[url=${href}]${label}[/url]` : label;
        })
        .replace(/<img\b([^>]*)>/gi, (_match, attrs: string) => {
            const src = htmlAttribute(attrs, "src");
            const alt = htmlAttribute(attrs, "alt") ?? htmlAttribute(attrs, "title");
            return src ? `[img${alt ? ` alt="${safeInlineLabel(alt)}"` : ""}]${src}[/img]` : "";
        })
        .replace(/<\/?(?:p|div|section|article|figure|figcaption)\b[^>]*>/gi, "\n\n")
        .replace(/<[^>]*>/g, " ");

    return decodeHtmlEntities(text)
        .replace(/\[(?:br|break|nl|newline)\s*\/?\]/gi, "\n")
        .replace(/\[(?:centre)\]/gi, "[center]")
        .replace(/\[\/(?:centre)\]/gi, "[/center]")
        .replace(/\[(?:colour)([^\]]*)\]/gi, (_match, attrs: string) => `[color${attrs ?? ""}]`)
        .replace(/\[\/(?:colour)\]/gi, "[/color]")
        .replace(/\[(?:h|header|title)(?:=([^\]]+))?\]/gi, (_match, level: string | undefined) => `[heading=${safeHeadingLevel(level ?? "2")}]`)
        .replace(/\[\/(?:h|header|title)\]/gi, "[/heading]")
        .replace(/\[h([1-6])\]/gi, "[heading=$1]")
        .replace(/\[\/h[1-6]\]/gi, "[/heading]")
        .replace(/(^|\n)(#{1,6})[ \t]+(.+?)(?=\n|$)/g, (_match, prefix: string, markers: string, label: string) => {
            return `${prefix}[heading=${Math.min(6, markers.length)}]${label.trim()}[/heading]`;
        })
        .replace(/\[(?:ul|list)(?:=[^\]]+)?\]/gi, "\n[list]\n")
        .replace(/\[ol(?:=([^\]]+))?\]/gi, (_match, attr: string | undefined) => `\n[olist${safeListStart(attr) ? ` start="${safeListStart(attr)}"` : ""}]\n`)
        .replace(/\[\/(?:ul|ol|list|olist)\]/gi, "\n[/list]\n")
        .replace(/\[\*\s*=([^\]]+)\]/g, (_match, label: string) => {
            const clean = safeInlineLabel(label);
            return clean ? `\n[*][b]${clean}[/b] ` : "\n[*]";
        })
        .replace(/\[\*\]/g, "\n[*]")
        .replace(/\[(?:hr|line|rule|divider|separator)\s*\/?\]/gi, "\n\n---\n\n")
        .replace(/(^|\n)[ \t]*(?:-{3,}|={3,}|_{3,}|\*{3,})[ \t]*(?=\n|$)/g, "$1\n\n---\n\n")
        .replace(/[ \t]+\n/g, "\n")
        .replace(/\n{4,}/g, "\n\n\n")
        .trim();
}

function storeRichBlocks(value: string): RichBlock[] {
    const blocks: RichBlock[] = [];
    const structuralPattern = /\[(list|olist|quote|code|heading|align|center|left|right|table)([^\]]*)\]/gi;
    let cursor = 0;
    let match: RegExpExecArray | null;

    while ((match = structuralPattern.exec(value)) !== null) {
        const tag = match[1].toLowerCase();
        const attrs = match[2] ?? "";
        const before = value.slice(cursor, match.index);
        if (!isStructuralBoundary(before)) {
            continue;
        }

        const close = findClose(value, structuralPattern.lastIndex, tag === "olist" ? "list" : tag);
        if (!close) {
            continue;
        }

        appendPlainBlocks(blocks, before);
        const body = value.slice(structuralPattern.lastIndex, close.start).trim();
        if (tag === "list" || tag === "olist") {
            blocks.push({
                kind: "list",
                ordered: tag === "olist",
                start: tag === "olist" ? safeListStart(attrs) : undefined,
                value: body
            });
        } else if (tag === "quote") {
            blocks.push({ kind: "quote", value: body });
        } else if (tag === "code") {
            blocks.push({ kind: "code", value: body, label: safeInlineLabel(tagAttribute(attrs)) });
        } else if (tag === "heading") {
            blocks.push({ kind: "heading", value: body, level: safeHeadingLevel(tagAttribute(attrs)) });
        } else if (tag === "table") {
            blocks.push({ kind: "table", value: body });
        } else {
            blocks.push({ kind: "align", value: body, align: safeAlignment(tag === "align" ? tagAttribute(attrs) : tag) });
        }

        cursor = close.end;
        structuralPattern.lastIndex = close.end;
    }

    appendPlainBlocks(blocks, value.slice(cursor));
    return blocks;
}

function appendPlainBlocks(blocks: RichBlock[], value: string) {
    for (const block of value.split(/\n{2,}/)) {
        const trimmed = block.trim();
        if (!trimmed) {
            continue;
        }

        if (trimmed === "---") {
            blocks.push({ kind: "line", value: "---" });
            continue;
        }

        if (looksLikePipeTable(trimmed)) {
            blocks.push({ kind: "table", value: pipeTableToBbcode(trimmed) });
            continue;
        }

        const list = plainListBlock(trimmed);
        blocks.push(list ?? { kind: "paragraph", value: trimmed });
    }
}

function plainListBlock(value: string): RichBlock | null {
    const lines = value.split("\n").map(line => line.trim()).filter(Boolean);
    if (lines.length < 2) {
        return null;
    }

    const parsed = lines.map(line => {
        const bullet = line.match(/^(?:[-*+]|\u2022)\s+(.+)$/);
        if (bullet?.[1]?.trim()) {
            return { ordered: false, value: bullet[1].trim() };
        }

        const numbered = line.match(/^(\d+)[.)]\s+(.+)$/);
        if (numbered?.[2]?.trim()) {
            return {
                ordered: true,
                start: Number.parseInt(numbered[1], 10),
                value: numbered[2].trim()
            };
        }

        const explicit = line.match(/^\[\*\]\s*(.+)$/);
        if (explicit?.[1]?.trim()) {
            return { ordered: false, value: explicit[1].trim() };
        }

        return null;
    });

    if (!parsed.every(Boolean)) {
        return null;
    }

    const first = parsed[0];
    if (!first) {
        return null;
    }
    const firstStart = "start" in first ? first.start : undefined;

    return {
        kind: "list",
        ordered: first.ordered,
        start: first.ordered && Number.isFinite(firstStart) && firstStart && firstStart > 1 ? firstStart : undefined,
        value: parsed.map(item => `[*]${item?.value ?? ""}`).join("\n")
    };
}

function renderStoreBlock(block: RichBlock): string {
    switch (block.kind) {
        case "list": {
            const items = listItems(block.value);
            if (items.length === 0) {
                return "";
            }

            const tag = block.ordered ? "ol" : "ul";
            const start = block.ordered && block.start ? ` start="${block.start}"` : "";
            return `<${tag}${start}>${items.map(item => `<li>${renderStoreInline(item)}</li>`).join("")}</${tag}>`;
        }
        case "quote":
            return `<blockquote>${renderStoreRichText(block.value)}</blockquote>`;
        case "code":
            return `<pre${block.label ? ` data-store-label="${escapeAttribute(block.label)}"` : ""}><code>${escapeHtml(block.value)}</code></pre>`;
        case "heading":
            return `<p class="store-rich-heading store-rich-heading-${block.level}">${renderStoreInline(block.value)}</p>`;
        case "align":
            return `<div class="store-rich-align store-rich-align-${block.align}">${renderStoreRichText(block.value)}</div>`;
        case "table":
            return renderStoreTable(block.value);
        case "line":
            return `<hr />`;
        default:
            return `<p>${renderStoreInline(block.value).replace(/\n/g, "<br />")}</p>`;
    }
}

function renderStoreInline(value: string, depth = 0): string {
    if (depth > 4) {
        return escapeHtml(stripUnknownBbcode(value));
    }

    const tokens: string[] = [];
    const reserve = (html: string) => {
        tokens.push(html);
        return `\u0000${tokens.length - 1}\u0000`;
    };

    let text = value
        .replace(/\[code(?:=[^\]]+)?\]([\s\S]*?)\[\/code\]/gi, (_match, body: string) => reserve(`<code>${escapeHtml(body)}</code>`))
        .replace(/`([^`\n]+)`/g, (_match, body: string) => reserve(`<code>${escapeHtml(body)}</code>`))
        .replace(/\[img([^\]]*)\]([\s\S]*?)\[\/img\]/gi, (_match, attrs: string, body: string) => {
            const url = safeUrl(body.trim());
            if (!url) {
                return "";
            }

            const label = safeInlineLabel(attributeValue(attrs, "alt") ?? attributeValue(attrs, "title") ?? "Preview image");
            return reserve(`<span class="store-rich-media"><img src="${escapeAttribute(url)}" alt="${escapeAttribute(label)}" loading="lazy" /></span>`);
        })
        .replace(/\[url=([^\]]+)\]([\s\S]*?)\[\/url\]/gi, (_match, href: string, label: string) => {
            const url = safeUrl(href);
            return url
                ? reserve(`<a href="${escapeAttribute(url)}" target="_blank" rel="noreferrer">${renderStoreInline(label, depth + 1)}</a>`)
                : renderStoreInline(label, depth + 1);
        })
        .replace(/\[url\]([\s\S]*?)\[\/url\]/gi, (_match, href: string) => {
            const url = safeUrl(href);
            return url
                ? reserve(`<a href="${escapeAttribute(url)}" target="_blank" rel="noreferrer">${escapeHtml(url)}</a>`)
                : escapeHtml(href);
        })
        .replace(/(^|[\s(])((?:https?:\/\/|\/\/)[^\s<\]]+)/g, (_match, prefix: string, href: string) => {
            const cleanHref = href.replace(/[),.;:!?]+$/, "");
            const url = safeUrl(cleanHref);
            const trailing = href.slice(cleanHref.length);
            return url
                ? `${prefix}${reserve(`<a href="${escapeAttribute(url)}" target="_blank" rel="noreferrer">${escapeHtml(url)}</a>`)}${trailing}`
                : `${prefix}${href}`;
        });

    text = escapeHtml(text);
    text = replaceInlinePair(text, "b", "strong");
    text = replaceInlinePair(text, "i", "em");
    text = replaceInlinePair(text, "u", "u");
    text = replaceInlinePair(text, "(?:s|strike|del)", "s");
    text = replaceInlinePair(text, "sub", "sub");
    text = replaceInlinePair(text, "sup", "sup");
    text = text
        .replace(/\[(?:small)\]([\s\S]*?)\[\/(?:small)\]/gi, "<small>$1</small>")
        .replace(/\[(?:big)\]([\s\S]*?)\[\/(?:big)\]/gi, "<span class=\"store-rich-big\">$1</span>")
        .replace(/\[(?:color|colour)=([^\]]+)\]([\s\S]*?)\[\/(?:color|colour)\]/gi, (_match, color: string, body: string) => {
            const safe = safeColor(color);
            return safe ? `<span style="color: ${escapeAttribute(safe)}">${body}</span>` : body;
        })
        .replace(/\[(?:background|bgcolor|highlight)=?([^\]]*)\]([\s\S]*?)\[\/(?:background|bgcolor|highlight)\]/gi, (_match, color: string, body: string) => {
            const safe = safeColor(color || "#f6d365");
            return safe ? `<span class="store-rich-highlight" style="background-color: ${escapeAttribute(safe)}">${body}</span>` : `<span class="store-rich-highlight">${body}</span>`;
        })
        .replace(/\[size=([^\]]+)\]([\s\S]*?)\[\/size\]/gi, (_match, size: string, body: string) => {
            const safe = safeFontSize(size);
            return safe ? `<span style="font-size: ${escapeAttribute(safe)}">${body}</span>` : body;
        })
        .replace(/\[\/?[a-z0-9_-]+[^\]]*\]/gi, "");

    return text.replace(/\u0000(\d+)\u0000/g, (_match, index: string) => tokens[Number(index)] ?? "");
}

function renderStoreTable(value: string): string {
    const rows = tableRows(value);
    if (rows.length === 0) {
        return "";
    }

    return `<div class="store-rich-table-wrap"><table>${rows.map(row => {
        const cells = tableCells(row);
        return cells.length > 0
            ? `<tr>${cells.map(cell => `<${cell.header ? "th" : "td"}>${renderStoreInline(cell.value)}</${cell.header ? "th" : "td"}>`).join("")}</tr>`
            : "";
    }).join("")}</table></div>`;
}

function tableRows(value: string): string[] {
    const tagged = Array.from(value.matchAll(/\[tr\]([\s\S]*?)\[\/tr\]/gi)).map(match => match[1].trim()).filter(Boolean);
    if (tagged.length > 0) {
        return tagged;
    }

    return value.split("\n").map(line => line.trim()).filter(Boolean);
}

function tableCells(row: string): Array<{ value: string; header: boolean }> {
    const tagged = Array.from(row.matchAll(/\[(td|th)[^\]]*\]([\s\S]*?)\[\/\1\]/gi))
        .map(match => ({ header: match[1].toLowerCase() === "th", value: match[2].trim() }))
        .filter(cell => cell.value);

    if (tagged.length > 0) {
        return tagged;
    }

    return row
        .split("|")
        .map(cell => cell.trim())
        .filter(Boolean)
        .map(value => ({ header: false, value }));
}

function looksLikePipeTable(value: string): boolean {
    const rows = value.split("\n").map(row => row.trim()).filter(Boolean);
    return rows.length >= 2 && rows.every(row => row.includes("|"));
}

function pipeTableToBbcode(value: string): string {
    return value
        .split("\n")
        .map(row => `[tr]${row.split("|").map(cell => `[td]${cell.trim()}[/td]`).join("")}[/tr]`)
        .join("\n");
}

function listItems(value: string): string[] {
    return value
        .split(/\[\*\]/)
        .map(item => item.trim())
        .filter(Boolean);
}

function isStructuralBoundary(value: string): boolean {
    return !value.trim() || /\n\s*$/.test(value);
}

function findClose(value: string, startIndex: number, tag: string): { start: number; end: number } | null {
    const pattern = tag === "list"
        ? /\[\/?(?:list|olist)(?:[^\]]*)\]/gi
        : new RegExp(`\\[\\/?${tag}(?:[^\\]]*)\\]`, "gi");
    pattern.lastIndex = startIndex;
    let depth = 1;
    let match: RegExpExecArray | null;

    while ((match = pattern.exec(value)) !== null) {
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

function replaceInlinePair(value: string, bbTag: string, htmlTag: string): string {
    return value.replace(new RegExp(`\\[${bbTag}\\]([\\s\\S]*?)\\[\\/${bbTag}\\]`, "gi"), `<${htmlTag}>$1</${htmlTag}>`);
}

function stripUnknownBbcode(value: string): string {
    return value.replace(/\[\/?[a-z0-9_-]+[^\]]*\]/gi, "");
}

function htmlAttribute(attrs: string, name: string): string | null {
    const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const match = attrs.match(new RegExp(`${escaped}\\s*=\\s*(?:"([^"]*)"|'([^']*)'|([^\\s>]+))`, "i"));
    return match ? decodeHtmlEntities(match[1] ?? match[2] ?? match[3] ?? "") : null;
}

function attributeValue(attrs: string, name: string): string | null {
    const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const match = attrs.match(new RegExp(`${escaped}\\s*=\\s*(?:"([^"]*)"|'([^']*)'|([^\\s\\]]+))`, "i"));
    return match ? decodeHtmlEntities(match[1] ?? match[2] ?? match[3] ?? "") : null;
}

function tagAttribute(attrs?: string): string {
    const raw = attrs ?? "";
    const equals = raw.match(/^\s*=\s*("?)(.*?)\1\s*$/);
    if (equals) {
        return equals[2];
    }

    return attributeValue(raw, "value")
        ?? attributeValue(raw, "align")
        ?? attributeValue(raw, "start")
        ?? raw.trim();
}

function safeHeadingLevel(value?: string): number {
    const parsed = Number.parseInt(value ?? "", 10);
    return Number.isFinite(parsed) ? Math.min(6, Math.max(1, parsed)) : 3;
}

function safeAlignment(value?: string): "left" | "center" | "right" | "justify" {
    const normalized = (value ?? "").trim().toLowerCase();
    if (normalized === "center" || normalized === "centre") {
        return "center";
    }

    if (normalized === "right" || normalized === "justify") {
        return normalized;
    }

    return "left";
}

function safeListStart(value?: string): number | undefined {
    const parsed = Number.parseInt(tagAttribute(value), 10);
    return Number.isFinite(parsed) && parsed > 1 ? Math.min(999, parsed) : undefined;
}

function safeInlineLabel(value?: string): string {
    return stripUnknownBbcode(decodeHtmlEntities(value ?? ""))
        .replace(/[<>]/g, "")
        .replace(/\s+/g, " ")
        .trim()
        .slice(0, 100);
}

function safeColor(value?: string): string | null {
    const color = decodeHtmlEntities(value ?? "").trim().replace(/^["']|["']$/g, "");
    if (/^#[0-9a-f]{3}(?:[0-9a-f]{3})?$/i.test(color)) {
        return color;
    }

    if (/^rgba?\(\s*\d{1,3}\s*,\s*\d{1,3}\s*,\s*\d{1,3}(?:\s*,\s*(?:0|1|0?\.\d+))?\s*\)$/i.test(color)) {
        return color;
    }

    if (/^(?:black|white|gray|grey|red|green|blue|yellow|orange|purple|pink|cyan|teal|lime|silver|gold)$/i.test(color)) {
        return color.toLowerCase();
    }

    return null;
}

function safeFontSize(value?: string): string | null {
    const size = decodeHtmlEntities(value ?? "").trim().replace(/^["']|["']$/g, "");
    const relative = Number.parseInt(size, 10);
    if (Number.isFinite(relative) && relative >= 1 && relative <= 7) {
        const scale = [0.82, 0.92, 1, 1.12, 1.22, 1.32, 1.42][relative - 1];
        return `${scale}em`;
    }

    const css = size.match(/^(\d+(?:\.\d+)?)(px|em|rem|%)$/i);
    if (!css) {
        return null;
    }

    const amount = Number(css[1]);
    const unit = css[2].toLowerCase();
    if (!Number.isFinite(amount)) {
        return null;
    }

    if (unit === "px") {
        return `${Math.min(28, Math.max(10, amount))}px`;
    }

    if (unit === "%") {
        return `${Math.min(160, Math.max(75, amount))}%`;
    }

    return `${Math.min(1.6, Math.max(0.75, amount))}${unit}`;
}

function safeUrl(value?: string): string | null {
    let raw = decodeHtmlEntities(value ?? "").trim().replace(/^["']|["']$/g, "");
    if (!raw) {
        return null;
    }

    if (raw.startsWith("//")) {
        raw = `https:${raw}`;
    } else if (raw.startsWith("/")) {
        raw = `${STORE_LINK_BASE}${raw}`;
    }

    try {
        const url = new URL(raw);
        return url.protocol === "http:" || url.protocol === "https:" ? url.toString() : null;
    } catch {
        return null;
    }
}

function decodeHtmlEntities(value: string): string {
    const named: Record<string, string> = {
        amp: "&",
        apos: "'",
        gt: ">",
        lt: "<",
        nbsp: " ",
        quot: "\""
    };

    return value.replace(/&(#x?[0-9a-f]+|[a-z]+);/gi, (match, entity: string) => {
        const lower = entity.toLowerCase();
        if (lower.startsWith("#x")) {
            const code = Number.parseInt(lower.slice(2), 16);
            return Number.isFinite(code) ? String.fromCodePoint(code) : match;
        }

        if (lower.startsWith("#")) {
            const code = Number.parseInt(lower.slice(1), 10);
            return Number.isFinite(code) ? String.fromCodePoint(code) : match;
        }

        return named[lower] ?? match;
    });
}

function escapeHtml(value: string): string {
    return value
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#39;");
}

function escapeAttribute(value: string): string {
    return escapeHtml(value).replace(/`/g, "&#96;");
}
