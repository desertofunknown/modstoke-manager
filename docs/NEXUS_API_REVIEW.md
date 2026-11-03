# Nexus Mods integration

**Modstoke Manager by OpenACAI** is an open-source Windows toolkit for Sons of the Forest. Its Nexus integration lets players browse mods, inspect files and requirements, track mods, endorse them, and hand selected downloads to Vortex.

Players sign in through their browser with OAuth 2.0 and PKCE, as a public desktop application. The Manager never asks for or stores a personal API key.

## Source and review build

- [Current source](https://github.com/desertofunknown/modstoke-manager/tree/openacai-tauri2-svelte5-shell)
- [Build instructions](NEXUS_SOURCE_AND_BUILD.md)
- [Application icon](../src-tauri/icons/128x128@2x.png)
- [License](../LICENSE) and [third-party notices](../THIRD_PARTY_NOTICES.md)

Build the `openacai-tauri2-svelte5-shell` branch with `npm ci` and `npm run tauri build`. The executable is `src-tauri/target/release/OpenACAI Mod Manager.exe`. Its filename is retained for installation compatibility; the app displays Modstoke Manager.

In the app, open **Browse mods**, choose **Nexus & Vortex** and select **Sign in with Nexus**. Approve Modstoke Manager on Nexus Mods, then let the browser open the Manager. The signed 0.2.0 downloads in `prebuilt/` are an older baseline, not this review build.

## Requests and account data

Catalog requests use the signed-in account's access token. The Manager does not retry a failed account catalog request anonymously or substitute a different feed. An empty response remains empty. Nexus controls which results its API returns; the Manager has not checked that every website preference is applied.

In-memory responses expire after ten minutes and are discarded when the connection changes. The local catalog snapshot belongs to a validated Nexus user ID and expires after 24 hours. Successful catalog refreshes replace that snapshot, including empty results. Previous accounts' rows are not merged into it. Offline rows are labelled as cached and may predate changes to website preferences.

The access token stays in memory and is refreshed shortly before it expires. Only the refresh token is saved, in Windows Credential Manager. Tokens are never sent to an OpenACAI service or written to catalog storage. If Nexus refuses the refresh token, for example after the player revokes the Manager's access, the Manager treats the player as signed out. Sign out deletes the saved token, asks Nexus to revoke it, and removes the local catalog snapshot. A personal API key saved by an earlier build is deleted, not used. Requests carry `Application-Name: Modstoke` and an `Application-Version` derived from the Cargo package version. They have a timeout and do not follow redirects with credentials.

Browsing and refreshes follow user navigation or an explicit refresh action. There is no periodic background catalog polling or automatic endorsement. Tracking and endorsements use explicit controls. Selected file downloads are handed to Vortex through `nxm://`; the Manager does not bypass Nexus download entitlements or manage Vortex's deployment itself.

The API implementation is in [nexus.rs](../src-tauri/src/nexus.rs), the request cache in [nexus.ts](../src/lib/mods/nexus.ts), and the account and catalog UI in [NexusVortex.svelte](../src/pages/NexusVortex.svelte).

## OAuth registration

The application is registered as **Modstoke**, published **by OpenACAI**, for Modstoke Manager. It replaces the earlier OpenACAI Mod Manager name and its `openacai-mod-manager` slug. It is a public client: each sign-in is proven with PKCE, and the Manager carries no client secret, since anyone could read one out of the executable.

| Item | Value |
| --- | --- |
| Application | Public desktop application for Windows, using PKCE |
| Client ID | `modstoke` |
| Callback | `modstoke-manager://oauth/callback` |
| Scopes | `openid public` |
| Purpose | Nexus API access on behalf of the signed-in player |

The sign-in follows the Nexus [OAuth 2.0 and PKCE guide](https://modding.wiki/en/api/oauth2-guide):

- The Manager creates a random 43-character code verifier and a state value for each attempt. It opens `users.nexusmods.com/oauth/authorize` with the S256 code challenge in the player's default browser.
- After approval, Nexus redirects to the `modstoke-manager://` callback. Windows hands that link to the running Manager. The installer registers the scheme, and portable and development builds register it for the current user when they start.
- The callback must carry the state of the waiting attempt and arrive within ten minutes. It is accepted once. A callback with any other state is ignored.
- The code and verifier are exchanged at `users.nexusmods.com/oauth/token`. The access token goes in the `Authorization: Bearer` header for API v1 and v2 requests. `users/validate.json` confirms the account and reports the rate limits.

These values are set in [nexus_auth.rs](../src-tauri/src/nexus_auth.rs). A refresh token Nexus refuses ends the sign-in, while throttling and network failures are retried on the next request.

Unauthenticated browsing on the Nexus website will not apply the user's blocked categories or other account preferences.

Nexus's [API acceptable use policy](https://help.nexusmods.com/article/114-api-acceptable-use-policy) describes application review, identification headers and the limits on personal-key testing. Registration does not imply Nexus endorsement.
