# Upstream Lineage

OpenACAI Mod Manager is an OpenACAI-maintained fork of Toni Macaroni's RedManager with the main installation flow changed to install and maintain OpenACAI Endnight Loader packages.

## Primary Upstream Project

- **RedManager** by Toni Macaroni: `https://github.com/ToniMacaroni/RedManager`
  - Provides the original Tauri/Svelte manager codebase and RedLoader-oriented install flow.
  - License: Apache-2.0.

## Managed Loader Upstreams

The loader package installed by this manager is built around these open-source projects:

- **BepInEx** by the BepInEx team: `https://github.com/BepInEx/BepInEx`
  - License: LGPL-2.1.

- **RedLoader/SonsSdk** by Toni Macaroni: `https://github.com/ToniMacaroni/RedLoader`
  - License: LGPL-2.1 according to the upstream repository license file.

## OpenACAI Changes

OpenACAI-authored work includes the OpenACAI Endnight Loader install flow, public package validation, OpenACAI branding, safer zip handling, broader mod package scanning, and mod catalog stability improvements.

The private OpenACAI anti-cheat/admin mod is intentionally not part of this public repository.
