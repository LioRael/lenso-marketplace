# Keyless catalog migration

The service remains `https://marketplace.lenso.dev`. The catalog ID remains `lenso-official-v2`. The `/api/marketplace/v3/` path is a protocol version, not a new domain or a replacement product.

## Trust boundary

GitHub OIDC and Sigstore attest the exact catalog bytes. Verification requires the official repository, publishing workflow, main ref, source commit and GitHub issuer; self-hosted signing runners are rejected. The catalog retains exact artifact versions, hashes and original release metadata.

Catalog provenance does not expire every week. Its signature proves origin, not current availability. A non-cached HTTPS head selects a catalog and bundle atomically. Consumers must verify that bundle and current release status before adoption. A verified historical release may remain visible without being currently available. An already locked App does not need an online catalog to keep running.

There is no new owner-held signing private key. Registry publication continues to use existing Trusted Publisher identities; catalog attestation is separate from npm/crates.io publication. Sigstore public trust roots and verification tools remain software dependencies, not secrets the owner must back up.

## Implemented paths

- Reviewed migration input for six original releases, verified against the old Ed25519 trust policy and delivered Site checkpoints.
- Manual same-main-commit, exact-digest attestation workflow.
- Bounded proof/object serving and a primary, non-cached atomic current head.
- Protected promotion adapter: real proof verification before remote writes, immutable objects and release records, terminal revocation, conditional head update and explicit reconciliation on an unconfirmed result.
- Native inspection and normal `lenso app add PLUGIN_ID@VERSION --marketplace` with a managed, fixed official verifier, automatic public roots, current-head admission and local anti-rollback history. No daily tool/root/hash flags.
- Site keyless ingestion with real publisher verification and separate historical/current availability. Legacy v1 ingestion is not relaxed.
- Canonical browser browsing reads the same-origin v3 current head, checks the exact catalog digest/size and rechecks the complete head. Visibility refresh failures clear the current display instead of retaining a confirmed claim. The browser does not verify Sigstore: actual adoption remains the CLI's job.
- Portable, linked Cargo, npm package and release-content records keep their original metadata. The display shows only real channel-specific integrity fields; it does not invent manifest digests or package sizes for other kinds.

The inspector's manual tool/root options remain an operator interface, not the normal adoption experience. Implemented paths are not a production cutover receipt. Managed verifier support currently covers macOS ARM64/x64 and Linux GNU ARM64/x64; unsupported platforms fail closed.

## Remaining cutover gates

1. Pass the exact candidate CI gates and land those exact commits.
2. Retain the real attestation, exact catalog/bundle digests and source-SHA verification receipt; policy-only changes do not require signing new bytes.
3. Publish the authorized necessary CLI versions and validate ordinary adoption of all four channels, including removal.
4. Apply the D1 migration, deploy/promote at the canonical domain, and verify production consumers before retiring legacy signing and renewal machinery.

No automatic legacy signature renewal is introduced. Legacy guides and test flows remain labeled historical references. This code change does not delete legacy keys, private databases or trust checkpoints.
