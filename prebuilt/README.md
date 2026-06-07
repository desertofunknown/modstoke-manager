# Prebuilt Installers

This folder contains the current Windows desktop builds of OpenACAI Mod Manager.

- `OpenACAI-Mod-Manager-0.2.0-x64-portable.exe`: direct portable app executable.
- `OpenACAI-Mod-Manager-0.2.0-x64-setup.exe`: NSIS setup executable.
- `OpenACAI-Mod-Manager.ico`: OpenACAI Windows icon used by the app and installer.
- `SHA256SUMS.txt`: checksums for the portable executable, setup installer, and icon.

Version `0.2.0` moves loader updates to GitHub Release assets, adds live SHA256 verification progress, caches the latest release manifest for last-known-good comparisons, updates the manager for the .NET 11 loader preview track, refreshes the UI toward a darker Sons-style options shell, links the Main tab to the public loader repository, and merges the SOTF Mods and Nexus/Vortex storefronts into one top-level Mods workspace with an All Stores mode that opens by default on fresh launches, session-local source switch, measured shared source preview panes, compact All Stores SOTF and Nexus/Vortex preview chrome, bottom safe-area scroll spacing, and shared cross-source search. The current signed prebuilts include path-aware loader health checks, a validated/icon path selector, Nexus category taxonomy caching, category alias canonicalization, local-taxonomy/inferred category source badges, profile/endorsement/tracking/changelog controls, confirmation guards before Nexus account write actions, a visible Nexus manual-refresh cooldown, hourly/daily Nexus API rate-limit meters, catalog loaded timestamps, auto-endorse queue state, dependency readiness with v3 file-ID resolution, compact dependency readiness chips for API rows/author hints/nested checks, materialized dependency lookup plus range-definition fallback, author requirement hints from API-provided author text, selected-file notes, and changelog text, source-labeled matched-text snippets for inferred author runtime hints, compatibility/removal warning detection for runtime/library mentions, local runtime/library phrase detection for BepInEx, RedLoader, SonsSdk, Harmony, .NET, and OpenACAI loader mentions, dependency detail drill-down, local conflict detection with grouped review panels, direct Review Conflict actions from rows and detail drawers, source-aware state actions, detail-drawer conflict panels for matched Nexus mods, installed-row Nexus detail drawers, optional conflict-row Nexus detail actions where local metadata supports them, status-badged Nexus detail surface links for Description/Files/Images/Posts/Bugs, safe BBCode/HTML formatting for Nexus descriptions, changelogs, and selected-file notes with nested heading/list/table/indent layout, naked BBCode list-item runs, authored unordered marker styles, inferred plain lists, ordered-list start numbers, real BBCode/HTML definition-list layouts, dense line-layout blocks, column/tab/callout layout fallbacks, labeled BBCode list-item variants, malformed-list recovery, multi-paragraph quote/spoiler/alignment/indent/float/code blocks, loose or partially closed table-row recovery, author divider lines and standalone section labels, safe table cell spans plus width/alignment hints, safe font-family hints, legacy centre/colour/strikethrough/newline aliases, attribute-style link/image tags, protocol-relative rich-text URLs, floated/aligned image hints, highlight/background spans, alignment/spoiler/rule/media handling, compact detail drawers that keep mod identity/media/description first, provide an in-drawer section rail for Description/Files/Dependencies/Install Plan/Changelog/Deployment, collect install/track/open controls in a bottom deployment footer, expose a bottom File/Deps/Target/Deploy readiness checklist before Vortex handoff, surface compact author instruction snippets in install plans, expose local-only inventory refresh actions in the account row and deployment footer after Vortex handoff, reactive catalog search/category/install/mod-type filters with live per-option counts, persisted local UI state/category/type restoration and a clear-filters control, measured Nexus catalog/detail layout sizing with automatic tight catalog density when the mod-list pane is short, compact catalog preview images with protocol-relative/nested media URL normalization, bundled local fallback imagery, clearer image-count/cycle controls, API-provided catalog/detail preview cycling, detail thumbnail rails with icon controls and current-preview opening for single-image and multi-image API payloads, safe detail preview enrichment from author descriptions/file notes/changelogs, and image fallback handling, catalog attention filters/sorts with active health-card shortcuts, a Nexus action queue with endorsement/tracked/update/disabled/conflict shortcuts, semver-aware Nexus update verdicts with row/detail reasons, installed-row detail metadata backfill, recommended-first file choices with compact selected/review summary chips, archived-file review badges, Use Recommended recovery, and bottom file-review shortcuts, selected-file notes/changelog rendering before Vortex handoff, install-plan previews with selected-file metadata, explicit BepInEx/RedLoader/manual placement overrides, and reactive action labels before Vortex handoff, generated NXM link preview/copy fallback, installed SOTF Mods local search/facet persistence with clearable filters, stale-request-safe shared search clearing, result counts, a measured SOTF Mods scroller with dense horizontal rows, and stricter Vortex-managed update guards.

Nexus catalog thumbnails now expose compact open-preview actions for already-loaded real row images, while local fallback thumbnails remain passive so no extra Nexus API requests are introduced. Catalog rows also reuse already-loaded preview URLs discovered by detail drawers from API-provided descriptions, selected-file notes, or changelogs, even when Nexus detail payloads omit their mod IDs.

Nexus catalog scrolling now reserves a measured bottom guard inside the list viewport so row install/detail/open controls stay above the lower frame mask in both dedicated Nexus/Vortex and All Stores layouts.

The frameless desktop shell now starts with a larger comfort cap on roomy displays and widens the shared content/detail rails, keeping the same small-screen minimums while giving dense catalogs and detail drawers more width before users resort to fullscreen.

SOTF Mods and Nexus/Vortex catalog rows now use row-aware proximity snapping so the merged All Stores workspace is less likely to stop with row titles or install/detail actions clipped under an embedded section header after wheel scrolling.

The All Stores Nexus/Vortex preview keeps the full controls in the dedicated source while hiding low-priority account actions, rate meters, summary cards, feed/filter controls, and empty queue chrome in the merged preview so Nexus rows appear sooner.

When a Nexus account is already connected, the All Stores embedded Nexus/Vortex preview also hides the redundant connected-account panel so row content starts higher; disconnected previews still show the login/API access controls.

Embedded Nexus/Vortex rows in All Stores also use tighter thumbnails, one-line summaries, and preview-scoped primary actions so install/detail/open controls remain visible in the shared workspace instead of dropping below the lower frame.

The All Stores SOTF Mods preview now measures the shared panel height and uses preview-only compact rows, hiding duplicate filters only when no hidden SOTF facet is active so complete native Install/Details/Open Page rows appear before the Nexus/Vortex preview without removing full SOTF controls from the dedicated source.

SOTF Mods rows now show compact image counts and left/right preview controls when the live storefront payload includes gallery image records, cycling only already-loaded image URLs without making extra store requests. Their denser row action column also exposes Open Page beside Install and Details so native-store rows match the Nexus row action model in All Stores.

SOTF Mods details now open in a native drawer that keeps the preview image, mod name, category/type/author context, and description first, then summarizes source state, install target, dependency records, and bottom deployment/page actions without moving install controls above the mod identity.

SOTF Mods dependency fields are normalized when the live storefront returns either a single mod ID string or an array, keeping detail dependency rows and recursive dependency installs from splitting one ID into characters.

SOTF Mods dependency rows now resolve matching store metadata in the detail drawer, show available/installed/review state, and expose Details/Open Page actions so users can inspect a dependency before installing it.

The Nexus action queue now collapses to a slim `Queue clear` strip when endorsement, tracking, update, disabled, and conflict counts are all zero, but restores the full shortcut chips whenever an action exists or one of those filters is selected.

The connected Nexus account row now uses tighter button spacing in compact catalog layouts, keeping auto-endorse, Nexus, Vortex staging, inventory refresh, and disconnect controls readable without taking extra list height.

The Nexus account summary now treats Premium, Supporter, and tier values returned by Nexus as account status, with tooltip/context text that download entitlement, speed, and queue behavior remain handled by Nexus/Vortex while tokens stay local and API requests stay cached.

The install plan now also shows dependency handoff readiness in the deployment area itself, including API row count, author instruction snippets, author hint warning/review/detected summaries, nested-check state, lookup-in-progress or lookup-failed notes, informational no-API-row guidance, a bottom dependency-review shortcut when requirements need attention, and status-aware dependency row actions for installing, updating, or reviewing linked dependency mods before the user sends a selected file to Vortex.

Inferred author runtime/library hints now require requirement, install, dependency, runtime/framework/library, or incompatibility warning context, so ordinary support/compatibility descriptions and credits/attribution text do not become dependency review items.

Detail drawers now move the Dependencies section above selected-file notes whenever dependency warnings, reviews, nested checks, or author hints exist, keeping requirement work visible in both layout and navigation order before long file changelogs or notes consume the side pane.

When Nexus returns no structured dependency rows, author requirement hints now appear before generic no-API dependency helper text so warning and review snippets stay visible first.

The Files panel now exposes `Use Recommended` whenever a nonrecommended file is selected, adds All/current-Main/Review/Selected filters for mixed file sets, and the deployment footer shows `Review File Choice` when the selected file is archived, old, or removed so users can recover before Vortex handoff without leaving the detail drawer.

Nexus detail file rows now use a denser adaptive compact grid on wider detail panes, forming up to three file-choice columns so more returned choices stay visible before Dependencies and Deployment.

The Nexus/Vortex detail and account actions now include a local-only `Refresh Inventory` scan that reconciles Vortex-managed packages, native installs, and manual/local installs after the user finishes deployment in Vortex without making another Nexus API request.

The current BBCode renderer also preserves additional author-layout cues for Nexus descriptions, selected-file notes, and changelogs, including box/panel sections, native expandable spoiler/details/collapse/accordion sections, captions, real BBCode and HTML definition-list layouts, naked BBCode list-item runs, authored unordered marker styles, inferred plain bullet/number/letter lists with indentation kept as nested lists and ordered starts preserved when authors begin after `1`, dense line-layout blocks for path/version/table-like author notes, row/cell table aliases, loose, cell-only, or partially closed table-row recovery, table cell width/alignment hints, styled HTML spans/divs, safe monospace/serif/sans font hints, BBCode font-size hints, single-newline structural breaks, standalone image aliases plus alignment/alt/title labels preserved through BBCode and HTML normalization, semantic abbreviation/citation/inline-quote hints, quote citations, labeled pre/code blocks, attribute-bearing alignment sections, clear markers, light whitespace cues, rgb color hints, relative size hints, inline media links, and standalone media blocks for supported video/embed/audio/object markup while still escaping unsupported markup.

Rich-text blocks, tables, and inline images are also bounded for narrow detail panes so preserved author formatting wraps or scrolls inside the drawer instead of pushing Files, Dependencies, or deployment controls off-screen.

It also recovers common Nexus shorthand such as thumbnail/image BBCode aliases, standalone `[img=...]` media tags, relative Nexus links, spaced labeled list markers, indented plain lists, anchor/bookmark/jump references, simple pipe-separated or BBCode-cell-only author note tables, labeled horizontal-rule sections, codebox/plain/fixed preformatted aliases, hidden/spoilerblock aliases, legacy horizontal-rule tags, relative/point size hints, HTML/CSS list marker styles, malformed nested closing tags, legacy `centre`/`colour`/strikethrough/newline aliases, and monospaced column-style line blocks so downloaded descriptions keep more of the original author layout.

Dependency normalization also accepts alternate wrapped, camelCase, nodes, and edges payload shapes from supported Nexus dependency responses, preserves Nexus v3/global file IDs when the API exposes them, and falls back from materialized candidates to range definitions or original game-scoped file IDs when needed.

The Nexus detail drawer now allocates more height to Files, selected-file notes, Dependencies, and Changelog, stacks earlier on medium-width windows, and wraps status-badged Nexus page-section links below the primary deployment controls so the install action stays readable while unsupported community surfaces remain clear external links.

Empty Nexus changelog responses now collapse into a compact detail-drawer status strip instead of reserving a large empty changelog pane; real API changelog entries still render in the full formatted section.

MSI builds are intentionally not published while the current MSI launch issue is investigated.

These builds are signed with the OpenACAI Inc Azure Trusted Signing certificate. Windows SmartScreen and some browsers may still warn until publisher reputation builds, so keep checksums published and verify signatures before release.

Dependency/source links:

- OpenACAI Endnight Loader package: `https://github.com/desertofunknown/openacai-loader`
- BepInEx: `https://github.com/BepInEx/BepInEx`
- RedLoader/SonsSdk rewrite branch: `https://github.com/ToniMacaroni/RedLoader/tree/rewrite`
- Original RedManager upstream: `https://github.com/ToniMacaroni/RedManager`

The private OpenACAI anti-cheat/admin mod is not included in this manager or its installers.
