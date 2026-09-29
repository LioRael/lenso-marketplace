# Operations

## Public deployment

For a stable-domain cutover, use the [new-root cutover](parallel-root-migration.md)
gates. The `lenso-official` catalog and key describe retired trust material and
must not be used to initialize a new signer or overwrite their resources. The
new root is the only supported publication and consumer trust path, while
`marketplace.lenso.dev` remains the one public address.

Build from the repository root with `pnpm build` and `pnpm build:workers`.
The UI build writes `plugins/web/ui/dist`, which is deployed as
Cloudflare Static Assets in the same release; `/api/*`, `/artifacts/*`, and
`/documents/*` are routed through the Wasm Worker. The only public entrypoint is
`apps/workers/worker.mjs`. A reviewed environment configuration must set the
Worker/account, hostname, new D1 and private R2 bindings, catalog ID, trusted
key ID and public key. Never supply signing keys to the public Worker. The
default config contains no test identity or resources.

The Workers Rust backend alone can be checked without the UI source or build:
its Web Plugin dependency disables default features. That is not a deployable
UI handoff. Keep `pnpm build` and the existing Static Assets binding in the
deployment workflow until the Site owns the deployed browser surface.

Generate a configuration with `node tools/cloudflare/deployment-config.mjs
INPUT.json OUTPUT.json`. The input must explicitly include `environment:
"production"`, the 32-character Cloudflare `account_id`, a custom hostname, the
separate D1/R2 resource identifiers, the approved catalog/key identities and the
public verification key. The renderer rejects `proof`, `test`, `recovery`, a
`workers.dev` value in the canonical hostname, the legacy catalog hostname and
both known proof public keys. Set the optional `workers_dev` input to `true`
only when the deployed Worker must expose its stable Cloudflare `workers.dev`
origin as a direct Agent origin; the custom hostname remains the canonical
browser/catalog address. Review its absolute entrypoint, migrations path,
resource IDs, public trust and direct-origin choice before using `pnpm exec
wrangler deploy --config OUTPUT.json`.

The smallest reviewable input has this shape (replace every value with an
approved production value; this example is not deployable):

```json
{
  "account_id": "<32 hex Cloudflare account ID>",
  "environment": "production",
  "deployment_track": "stable-domain-new-root",
  "worker": "lenso-marketplace",
  "hostname": "marketplace.lenso.dev",
  "database_name": "<new D1 name>",
  "database_id": "<new D1 UUID>",
  "bucket_name": "<new private R2 bucket>",
  "catalog_id": "<new catalog ID>",
  "key_id": "<new key ID>",
  "legacy_public_key_hex": "<verified old 64 hex public key>",
  "public_key_hex": "<new 64 hex Ed25519 public key>",
  "cpu_ms": 1000,
  "workers_dev": true
}
```

The production Agent release uses the additive direct origin
`https://lenso-marketplace.lenso.workers.dev` for signed snapshot and immutable
artifact downloads. This avoids redirect-based acquisition; the custom domain
continues serving the same catalog and remains the human-facing address.

Run `node --test tools/cloudflare/deployment-config.test.mjs` to exercise the
boundary without contacting Cloudflare. Configuration generation never creates
resources, handles private signing keys or deploys.
A build or dry run is not a deployment receipt. Observe the deployed version and
verify search/detail and exact signed snapshot responses after deployment.

Use `tools/publisher` for consistent publisher backup and signed output, and
`tools/cloudflare/promote-cloudflare.mjs` for conditional publication promotion.
See their operator guides. Preserve the original publication bytes, expected
pointer, signature verification and confirmation of uncertain writes. Signing
credentials stay in operator storage, never repository files or the read Host.

Source-only linked Cargo releases use the separate
`apps/workers/migrations/d1/0003_linked_cargo.sql` pointer table. Apply that
migration to the intended D1 database before promoting or serving the channel.
Set `kind: "linked-cargo"` in the operator promotion config; the promoter exports
and verifies the independently signed envelope and conditionally updates only
the `marketplace_linked_cargo` pointer. Verify the public
`/api/marketplace/v1/linked-cargo` response against configured trust after
promotion. Building the Worker or creating a local publication does not prove
the migration, remote promotion, or public availability occurred.

Optional signed release content uses the separate
`apps/workers/migrations/d1/0004_release_content.sql` pointer. Apply it to the
intended D1 database before promoting or serving this channel. The operator
exports the exact v2 envelope; Cloudflare promotion must verify that envelope
and conditionally advance only `marketplace_release_content`. Verify the public
`/api/marketplace/v1/release-content` bytes and signature after promotion.
Neither the D1 migration nor a local publisher result is deployment evidence.

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

## Recovery

Restore the approved publisher backup, verify signed publication history and
resource bindings, advance through an authorized forward publication, then verify
public responses before returning traffic. Never replay test credentials, test
resource mutations or expired signatures as a production recovery procedure.
