# Environment Secrets 0.1.7 directory candidate

This author metadata proposes the exact public `lenso-secrets-env-plugin`
0.1.7 archive as a generic `linked_plugin`. It is not a signed Directory
entry, a portable runtime artifact, or proof of public catalog availability.
An App must select an exact signed linked snapshot, supply this matching
`.crate`, configure named references, and build its own Host. The Plugin's
linked factory provides `lenso.secrets@1` through the native adapter; it does
not grant unrestricted environment access. An App owner must independently
review every reference-to-environment-variable mapping and Host permission.

The public, non-yanked crates.io archive has SHA-256
`6dd6a27548c53acaa56d5b65faa6aab3c755e1fb5b00365c406b47aed79c4787`.
Its Cargo metadata names Plugin ID `lenso.secrets.env`, root Slot `secrets`,
MIT license, and Rust 1.94. Its `.cargo_vcs_info.json` records source commit
`43591a4374cf2a6ec586ba5c365bdf41eff1beb4`; the packaged manifest,
configuration schema, and two source files match that commit byte-for-byte.
The later `lenso-secrets-env-plugin@0.1.7` tag points to the docs-only commit
`23bbd7778dfa24cd72453ad48dd7832a4c4deb81`; it does not change this
package's source. The versioned guide is `README.md` at the archive's source
commit, SHA-256
`38904fbf66d398777eb0f78d8d08d3433609f609f255151a926bb05a829b0d0b`
and 4592 bytes, matching the public raw URL in `metadata.json`.

Prior test-only signed `app add --no-install` accepted this exact archive on
Apple Silicon, but did not build or run a macOS Host. A separate offline Linux
ARM64 gate compiled it into a source-deleted Host and exercised background
processing. The single listed target is bounded to that Host evidence. These
tests do not make the test signature official or prove every possible Host
configuration. The archive itself does not declare a target or a portable
ABI/permission manifest. The native descriptor and configuration are checked
again when the target Host is built and resolved.

With the exact public registry archive, prepare and check a local submission:

```sh
lenso-marketplace-author prepare-linked-cargo \
  lenso-secrets-env-plugin-0.1.7.crate \
  candidates/linked-cargo/lenso.secrets.env/0.1.7/metadata.json \
  /path/to/new-submission-directory
lenso-marketplace-author check-linked-cargo /path/to/new-submission-directory
```

An operator must still verify namespace ownership, registry/source provenance,
target support, documentation bytes, dependency closure, and Host behavior
before signing or publishing. No production catalog write is implied.
