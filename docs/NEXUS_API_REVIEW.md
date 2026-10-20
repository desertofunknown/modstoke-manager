# Nexus Mods integration

**Modstoke Manager by OpenACAI** is an open-source Windows toolkit for Sons of the Forest. Its Nexus integration lets players browse mods, inspect files and requirements, track mods, endorse them, and hand selected downloads to Vortex.

Public account sign-in is awaiting registration with Nexus Mods. The current development build accepts a personal API key for review and testing; it is not a substitute for registered application access.

## Source and review build

- [Current source](https://github.com/desertofunknown/openacai-mod-manager/tree/openacai-tauri2-svelte5-shell)
- [Build instructions](NEXUS_SOURCE_AND_BUILD.md)
- [Application icon](../src-tauri/icons/128x128@2x.png)
- [License](../LICENSE) and [third-party notices](../THIRD_PARTY_NOTICES.md)

Build the `openacai-tauri2-svelte5-shell` branch with `npm ci` and `npm run tauri build`. The executable is `src-tauri/target/release/OpenACAI Mod Manager.exe`. Its filename is retained for installation compatibility; the app displays Modstoke Manager.

In the app, open **Browse mods**, choose **Nexus & Vortex**, and expand **Development testing** to connect a reviewer's own personal API key. Never send a key with a bug report. The signed 0.2.0 downloads in `prebuilt/` are an older baseline, not this review build.

## Requests and account data

Catalog requests use the connected account's API key. The Manager does not retry a failed account catalog request anonymously or substitute a different feed. An empty response remains empty. Nexus controls which results its API returns; this change does not claim that every website preference has been independently verified.

In-memory responses expire after ten minutes and are discarded when the connection changes. The local catalog snapshot belongs to a validated Nexus user ID and expires after 24 hours. Successful catalog refreshes replace that snapshot, including empty results. Previous accounts' rows are not merged into it. Offline rows are labelled as cached and may predate changes to website preferences.

Keys remain in the operating system's credential vault. The Manager does not send them to an OpenACAI service or include them in catalog storage. Disconnect removes the saved key and local catalog snapshot. Requests carry the existing `Application-Name: OpenACAI Mod Manager` identity and an `Application-Version` derived from the Cargo package version. Requests have a timeout and do not follow redirects with credentials.

Browsing and refreshes follow user navigation or an explicit refresh action. There is no periodic background catalog polling or automatic endorsement. Tracking and endorsements use explicit controls. Selected file downloads are handed to Vortex through `nxm://`; the Manager does not bypass Nexus download entitlements or manage Vortex's deployment itself.

The API implementation is in [main.rs](../src-tauri/src/main.rs), the request cache in [nexus.ts](../src/lib/nexus.ts), and the account and catalog UI in [NexusVortex.svelte](../src/pages/NexusVortex.svelte).

## OAuth registration proposal

The requested public application name is **Modstoke Manager**, published **by OpenACAI**. The earlier application name was OpenACAI Mod Manager; its request identity and credential location are retained until replacement identifiers are agreed with Nexus.

For registration discussion:

| Item | Proposed value |
| --- | --- |
| Application | Public desktop application for Windows |
| Callback | `modstoke://oauth/nexus/callback` |
| Scopes | `public openid profile` |
| Purpose | Nexus API access and identifying the connected account |

These values are proposals, not an approved or implemented OAuth contract. Nexus must confirm support for the desktop callback and issue the client registration details. The OAuth callback receiver, authorization flow and token lifecycle still need implementation once that contract is agreed. The retained legacy SSO code is not OAuth and its sign-in button is disabled while registration is pending.

The proposed callback is a desktop URI and does not depend on `modstoke.com` being deployed. The intended website remains separate work. Unauthenticated browsing on the Nexus website will not apply the user's blocked categories or other account preferences.

Nexus's [API acceptable use policy](https://help.nexusmods.com/article/114-api-acceptable-use-policy) describes application review, identification headers and the limits on personal-key testing. Registration does not imply Nexus endorsement.
