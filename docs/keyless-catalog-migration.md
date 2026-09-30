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

## Delivery receipts and pending work

Marketplace commit `2ed7e6ac4024262231916d13f7720638f4fd725c` passed its exact Foundation, Quality and Workers gates before landing. The production Worker version is `e90bfc3c-c69a-4077-950a-965125566ea3` at the unchanged canonical domain. The D1 migration and protected promotion established keyless revision 1. Its non-cached current head and exact immutable objects were read back after deployment.

The real attestation binds source commit `bda1f4f56d299a6a5e09fe94f14cd685c36c539e` to the six-record catalog. The retained catalog digest is `sha256:cf96b4caf1fb0005f05bb9553427313ed112f543b66e49b2477003078ae8eb3d` (11,128 bytes); the bundle digest is `sha256:b1de046e5676593a4582c476acd98206d7511cc7573792b910069d080a3c0eb1` (11,463 bytes). Policy-only changes do not require signing new catalog bytes.

Published `@lenso/cli@0.17.4` contains native CLI `0.6.4`. [Native AMD64 acceptance](https://github.com/LioRael/lenso-js/actions/runs/36654935991) passed at exact workflow commit `04a40669c46356323f0bad65cedd2752b046e52b`, subsequently landed: the public npm archive and native executable matched reviewed hashes, then normal `--marketplace` adoption previewed and copied `lenso.reference.knowledge-base-starter@0.1.0` with automatically managed verification. This receipt covers content-only adoption without executing copied content. It does not claim a new end-to-end runtime test of every channel; original channel qualification and typed consumer checks are separate evidence.

Site's final build, production switch and production consumer readback remain pending. Do not describe the whole cutover as complete until those receipts exist. Ordinary adoption and the deployed Marketplace browser no longer depend on legacy weekly catalog expiry. No legacy signing renewal is required for the keyless current head.

No automatic legacy signature renewal is introduced. Legacy guides and test flows remain labeled historical references. This code change does not delete legacy keys, private databases or trust checkpoints.
