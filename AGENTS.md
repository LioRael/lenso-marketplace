# Lenso Marketplace

Marketplace owns its Web and Directory Plugins, publisher, native App and Workers host. Console is a consumer and must not be a build-time source dependency. Reuse released Lenso UI and runtime packages. Installation authority stays in Console Agent tools.

Use Rust 1.94 and frozen dependency locks. Under the Lenso sibling workspace, run Cargo through `.lenso-tools/bin/lenso-cargo` in the framework root. Run `pnpm build` before Rust checks; embedded UI has a source fingerprint.

Required delivery checks are `catalog`, `event-host` and `quality`. Inspect failures; never remove a required check to merge. Preserve immutable evidence and exact signed fixtures. Local proof scripts must not target production resources. Never commit credentials, private keys or local publisher databases.

Repository artifacts are written in English. For delivery recovery or CI monitoring use the installed code-delivery skill. Do not claim deployment from a merge or build alone.
