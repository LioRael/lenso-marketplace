# Operations

## Public deployment

Build from the repository root with `pnpm build` and `CARGO=cargo pnpm
build:workers`. The only public entrypoint is `apps/workers/worker.mjs`. A reviewed
environment configuration must set the Worker/account, hostname, D1 and private
R2 bindings, catalog ID, trusted key ID and public key. Never supply signing keys
to the public Worker. The default config contains no test identity or resources.

Generate a configuration with `node tools/cloudflare/deployment-config.mjs
INPUT.json OUTPUT.json`. Review its absolute entrypoint, migrations path, resource
IDs and public trust before using `pnpm exec wrangler deploy --config OUTPUT.json`.
A build or dry run is not a deployment receipt. Observe the deployed version and
verify search/detail and exact signed snapshot responses after deployment.

Use `tools/publisher` for consistent publisher backup and signed output, and
`tools/cloudflare/promote-cloudflare.mjs` for conditional publication promotion.
See their operator guides. Preserve the original publication bytes, expected
pointer, signature verification and confirmation of uncertain writes. Signing
credentials stay in operator storage, never repository files or the read Host.

Expired verified catalogs remain browsable with a stale notice. New installation
requires current metadata. Renew with a new signed revision; do not edit expiry
inside a signed envelope. Monitor expiry before installations are interrupted.
Signature, rollback, immutable identity and corrupted-storage failures remain
errors even when a cached copy exists.

## Local proof

The explicit test entrypoint and config are in `tests/workers`. Run commands from
that directory when using `proof/smoke.mjs`; its Wrangler commands discover the
local test config there. Never substitute public deployment resources. Generate
fixtures from the root with `cargo run --locked -p marketplace-test-support
--example workers_fixtures -- /tmp/NEW_FIXTURE_DIRECTORY`.

## Historical recovery

`docs/archive/recovery` preserves the executed recovery experiment and immutable
receipts. It is not a general production restore command. For a real recovery,
restore the approved publisher backup, verify signed publication history and
resource bindings, advance through an authorized forward publication, then verify
public responses before returning traffic. Never replay archived test credentials,
resource mutations or expired signatures as a production recovery procedure.
