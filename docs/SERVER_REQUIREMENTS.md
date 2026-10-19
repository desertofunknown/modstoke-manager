# Server requirements

Modstoke Manager, by OpenACAI, can prepare and compare a portable list of native
packages for Sons of the Forest. This document describes preparation metadata;
it is not a server configuration or an admission message.

```json
{
  "format": "modstoke.server-requirements",
  "schemaVersion": 1,
  "name": "Community server",
  "packages": [
    {
      "loader": "modstoke",
      "id": "example.community-map",
      "version": "1.2.0"
    }
  ]
}
```

The example package is illustrative. Importing a document does not fetch,
install or execute anything. Existing installation profiles, catalog lockfiles,
recovery data and update addresses are unchanged.

## Fields and limits

- `format` and `schemaVersion` must match the example exactly.
- `name` contains 1–120 Unicode characters.
- `packages` contains 1–128 explicitly selected requirements.
- `loader` is `modstoke`. Other ecosystems require a future format revision.
- `id` contains 1–256 characters and must be unique after conversion to lowercase,
  following native inventory matching. It identifies the manifest package.
- `version` contains 1–128 characters. Comparison uses the exact string; `1.2`,
  `1.2.0` and `v1.2.0` are different. Version ranges are not accepted as ranges.

Text fields cannot contain control characters or leading or trailing whitespace.
The input and formatted export are limited to 64 KiB of UTF-8 JSON. Unknown and
duplicate fields, unsupported schemas and duplicate package IDs are rejected.
Documents contain no paths, URLs, hashes, ownership declarations or recovery state.

## Interpreting a comparison

The Manager reads native package manifests and files in the selected installation.
Only unambiguous load candidates or libraries with the requested exact version
receive **Files match**. Disabled paths, metadata without an entrypoint,
incomplete files and conflicting entries remain separate results. When a scan
has reported unreadable or excluded data, an absent ID is **Not established**
instead of **Missing**. Read the inventory notes before relying on the result.

This is a snapshot of local files. Other programs can change them afterward.
It does not establish successful loading, compatible dependencies, authenticated
client participation or an honest client. Manager's comparison does not read
loaded-mod reports or activate server enforcement. The SDK and server must
select their admission policy separately. Selecting server-only packages as
client requirements is an operator decision; the Manager does not infer it.

Use **Save JSON** to preserve a list before navigating away. Switching installation
profiles keeps the current list and clears its previous comparison. Reading a new
profile compares the same list against that installation. Requirements preparation
itself neither reconciles package transactions nor changes installed mods.
