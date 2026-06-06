# Prebuilt Installers

This folder contains the current Windows desktop builds of OpenACAI Mod Manager.

- `OpenACAI-Mod-Manager-0.2.0-x64-portable.exe`: direct portable app executable.
- `OpenACAI-Mod-Manager-0.2.0-x64-setup.exe`: NSIS setup executable.
- `OpenACAI-Mod-Manager.ico`: OpenACAI Windows icon used by the app and installer.
- `SHA256SUMS.txt`: checksums for the portable executable, setup installer, and icon.

Version `0.2.0` moves loader updates to GitHub Release assets, adds live SHA256 verification progress, caches the latest release manifest for last-known-good comparisons, updates the manager for the .NET 11 loader preview track, refreshes the UI toward a darker Sons-style options shell, links the Main tab to the public loader repository, and merges the SOTF Mods and Nexus/Vortex storefronts into one top-level Mods workspace with a local source switch. The current signed prebuilts include path-aware loader health checks, a validated/icon path selector, Nexus category taxonomy caching, profile/endorsement/tracking/changelog controls, confirmation guards before Nexus account write actions, a visible Nexus manual-refresh cooldown, hourly/daily Nexus API rate-limit meters, catalog loaded timestamps, auto-endorse queue state, dependency readiness with v3 file-ID resolution, compact dependency readiness chips for API rows/author hints/nested checks, materialized dependency lookup plus range-definition fallback, author-linked requirement hints from API-provided author text, selected-file notes, and changelog text, dependency detail drill-down, local conflict detection with grouped review panels, source-aware state actions, detail-drawer conflict panels for matched Nexus mods, installed-row Nexus detail drawers, optional conflict-row Nexus detail actions where local metadata supports them, Nexus detail links for Description/Files/Posts/Images/Bugs, safe BBCode/HTML formatting for Nexus descriptions, changelogs, and selected-file notes with nested heading/list/table/indent layout, column/tab/callout layout fallbacks, labeled BBCode list-item variants, malformed-list recovery, multi-paragraph quote/spoiler/alignment/indent/float/code blocks, author divider lines and standalone section labels, safe table cell spans, attribute-style link/image tags, protocol-relative rich-text URLs, floated/aligned image hints, highlight/background spans, alignment/spoiler/rule/media handling, compact detail drawers that keep mod identity/media/description first, provide an in-drawer section rail for Description/Files/Dependencies/Install Plan/Changelog/Deployment, and collect install/track/open controls in a bottom deployment footer, reactive catalog search/category/install/mod-type filters with persisted local UI state/category/type restoration and a clear-filters control, measured Nexus catalog/detail layout sizing, compact catalog preview images with protocol-relative/nested media URL normalization, bundled local fallback imagery, clearer image-count/cycle controls, API-provided catalog/detail preview cycling, detail thumbnail rails for multi-image API payloads, and image fallback handling, catalog attention filters/sorts with active health-card shortcuts, a Nexus action queue with endorsement/tracked/update/disabled/conflict shortcuts, semver-aware Nexus update verdicts with row/detail reasons, installed-row detail metadata backfill, recommended-first file choices with compact selected/review summary chips and archived-file review badges, selected-file notes/changelog rendering before Vortex handoff, install-plan previews with selected-file metadata, explicit BepInEx/RedLoader/manual placement overrides, and reactive action labels before Vortex handoff, generated NXM link preview/copy fallback, installed SOTF Mods local search/facet persistence with clearable filters, result counts, a measured SOTF Mods scroller with dense horizontal rows, and stricter Vortex-managed update guards.

The current BBCode renderer also preserves additional author-layout cues for Nexus descriptions, selected-file notes, and changelogs, including box/panel/collapsible sections, captions, BBCode and HTML definition-list fallbacks, row/cell table aliases, styled HTML spans/divs, single-newline structural breaks, image alignment aliases, clear markers, light whitespace cues, rgb color hints, and relative size hints while still escaping unsupported markup.

Dependency normalization also accepts alternate wrapped, camelCase, nodes, and edges payload shapes from supported Nexus dependency responses before falling back from materialized candidates to range definitions.

The Nexus detail drawer now allocates more height to Files, selected-file notes, Dependencies, and Changelog, stacks earlier on medium-width windows, and wraps Nexus page-section links below the primary deployment controls so the install action stays readable.

MSI builds are intentionally not published while the current MSI launch issue is investigated.

These builds are signed with the OpenACAI Inc Azure Trusted Signing certificate. Windows SmartScreen and some browsers may still warn until publisher reputation builds, so keep checksums published and verify signatures before release.

Dependency/source links:

- OpenACAI Endnight Loader package: `https://github.com/desertofunknown/openacai-loader`
- BepInEx: `https://github.com/BepInEx/BepInEx`
- RedLoader/SonsSdk rewrite branch: `https://github.com/ToniMacaroni/RedLoader/tree/rewrite`
- Original RedManager upstream: `https://github.com/ToniMacaroni/RedManager`

The private OpenACAI anti-cheat/admin mod is not included in this manager or its installers.
