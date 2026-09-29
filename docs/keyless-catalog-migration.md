# Keyless catalog migration

The service remains `https://marketplace.lenso.dev`. The catalog ID remains
`lenso-official-v2`. The `/api/marketplace/v3/` path is a protocol version, not a
new domain or a replacement product.

## Trust boundary

GitHub OIDC and Sigstore attest the exact catalog bytes. Verification requires
the official repository, publishing workflow, main ref, source commit and
GitHub issuer; self-hosted signing runners are rejected. The catalog retains
exact artifact versions, hashes and original release metadata.

Catalog provenance does not expire every week. Its signature proves origin,
not current availability. A non-cached HTTPS head selects a catalog and bundle
atomically. Consumers must verify that bundle and current release status before
adoption. A verified historical release may remain visible without being
currently available. An already locked App does not need an online catalog to
keep running.

There is no new owner-held signing private key. Registry publication continues
to use existing Trusted Publisher identities; catalog attestation is separate
from npm/crates.io publication. Sigstore public trust roots and verification
tools remain software dependencies, not secrets the owner must back up.

## Implemented foundation

- Reviewed migration input for six original releases, verified against the old
  Ed25519 trust policy and delivered Site checkpoints.
- Manual same-main-commit, exact-digest attestation workflow.
- Bounded proof/object serving and a primary, non-cached atomic current head.
- Protected promotion adapter: real proof verification before remote writes,
  immutable objects and release records, terminal revocation, conditional head
  update and explicit reconciliation on an unconfirmed result.
- Native read-only provenance inspection using the mature GitHub verifier.
- Site historical/currentness separation without relaxing v1 ingestion.

These are migration foundations, not a completed production cutover. The native
inspector is an operator interface; its tool and public-root flags are not the
intended user-facing adoption experience.

## Remaining cutover gates

1. Pass the exact candidate CI gates and land those exact commits.
2. Produce the first real attestation and validate it with the native consumer.
3. Integrate keyless verification, managed public roots, anti-rollback state and
   HTTPS currentness into normal `lenso app add` and Site ingestion. No manual
   root/hash flags should be required for normal adoption.
4. Validate actual adoption of all four channels, including removal, then
   separately authorize any new CLI package versions required for delivery.
5. Apply the D1 migration, deploy/promote at the canonical domain, and verify
   production consumers before retiring legacy signing and renewal machinery.

No automatic legacy signature renewal is introduced. No legacy key, database,
endpoint or trust checkpoint is removed by the foundation change.
