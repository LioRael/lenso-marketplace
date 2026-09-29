# Keyless catalog publication

The v1 keyless catalog binds exact release records and availability decisions in
one artifact. It is not a legacy Ed25519 envelope, has no `expires_at`, and does
not treat an audit digest as publishing authority. Package, Bundle, source-content,
documentation, integration and target fields remain in the original typed records.

Publication requires a reviewed main commit and SHA-256 of the original checked-in
`catalog.json` bytes. The manual workflow cannot fetch a caller-supplied catalog,
run on a fork or non-main ref, or accept a static signing key. GitHub OIDC creates
short-lived attestation credentials. The workflow uses only its scoped ephemeral
GitHub token, not a stored personal-access token.

The resulting provenance must pass the native `gh attestation verify` verifier
with the exact repository, workflow, main ref, issuer, source and signer commit.
Self-hosted runner attestations are rejected. Do not enable custom trust roots or
disable Sigstore public-good verification. Certificate expiry does not itself
expire a properly verified historical provenance bundle.

```sh
node tools/keyless/catalog.mjs tools/keyless/catalog.json REVIEWED_SHA256
node tools/keyless/verify.mjs catalog.json bundle.json SOURCE_COMMIT REVIEWED_SHA256
```

The workflow publishes provenance and retains candidate bytes; it does not deploy
a Worker or update the current pointer. Promotion must separately verify this
bundle, write immutable digest-addressed objects, and atomically update the single
current catalog pointer. HTTPS current status is an online authority assumption,
not cryptographic freshness protection against a compromised Marketplace.

The migration input must be constructed from the four independently verified
legacy payloads. `legacy_sources` records their schemas, revisions and SHA-256
digests as audit evidence. No fixture or unsigned flattened Site export is a
production input. Validation retains all record fields and applies bounded typed
checks; consumer-specific permission, target and archive checks remain mandatory.
