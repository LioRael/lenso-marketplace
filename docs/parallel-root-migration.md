# Parallel Marketplace trust root

The existing `lenso-official` catalog at `marketplace.lenso.dev` is a legacy
service, not a source for a new signing revision. Its authoritative publisher
database and signing key are unavailable. Keep its Worker, D1, R2, route,
signed revision 4 and consumer checkpoints unchanged. Do not initialize a
replacement database under that catalog ID, rotate its key in place or treat a
copy of the public envelope as reviewed publisher state.

The parallel root is a separate catalog and public origin. Its exact catalog
ID, key ID, Worker name, custom hostname, D1 ID/name, R2 bucket, reviewer
identities and custody owners are launch decisions; no production values are
chosen by this repository. The new public key must be derived from a newly
custodied private seed outside the repository. The old public key must first be
read back from the deployed legacy Worker configuration or another verified
consumer trust record. Both public keys are required to render a parallel
configuration so equality can be rejected. Neither private seed is an input to
the renderer.

## Local configuration candidate

Copy the public values below into an untracked operator input, replacing every
placeholder with reviewed values. `legacy_public_key_hex` is a public
verification key, not a private seed. The known old production Worker, domain,
D1, R2, catalog and key identifiers are rejected in this track. Use a separate
Worker and custom domain even if both roots remain in the same Cloudflare
account. Do not reuse the old bucket under a new object prefix.

```json
{
  "deployment_track": "parallel-new-root",
  "environment": "production",
  "account_id": "<32 hex Cloudflare account ID>",
  "worker": "<new Worker name>",
  "hostname": "<new custom hostname>",
  "database_name": "<new D1 name>",
  "database_id": "<new D1 UUID>",
  "bucket_name": "<new private R2 bucket>",
  "catalog_id": "<new catalog ID>",
  "key_id": "<new key ID>",
  "legacy_public_key_hex": "<verified old 64 hex public key>",
  "public_key_hex": "<new 64 hex public key>",
  "cpu_ms": 1000,
  "workers_dev": false
}
```

Run `node tools/cloudflare/deployment-config.mjs INPUT.json OUTPUT.json` and
review the generated, create-only `OUTPUT.json`. The generated Wrangler file
contains only the new public trust and bindings, with Worker-first API,
artifact and document routes. The old public key and track marker are input
checks, not deployed variables. The renderer does not create resources or
deploy. Build the existing UI and Worker, then use a Wrangler dry run against
this exact config; a dry run is not a deployed or signed-catalog receipt.

The private publisher configuration must use a **new**, absolute, protected
SQLite path and match the new catalog ID, key ID and public key exactly. The
publisher's `initialize` operation creates a new empty database; it does not
recover old submissions or sign a release. Review/admit each new-root release
from its exact public archive and registry bytes under the new identity. Keep
the database, signed envelopes, audit record and backups under named operator
custody; do not put a database or signing seed in Actions or a repository.

For protected promotion, set `deploymentTrack: "parallel-new-root"`,
`deploymentConfig` to the absolute path of the reviewed generated Wrangler
file, and `legacyPublicKeyHex` to the independently verified old public key in
the operator promotion config. The promoter cross-checks its account, D1, R2
and catalog against that file, and checks the private publisher config's
catalog, key and public trust before any publisher execution or network write.
All other promotion fields (`kind`, exact expected pointer and document bytes)
retain their existing rules. Never set `expected: null` without confirming that
the selected **new D1 channel** is empty.

## Launch gates

1. Record the old Worker version, route, resource IDs and verified public key.
   Confirm the new names, catalog ID, key ID and public key differ. The old
   service must remain reachable throughout rollout.
2. Assign a signer custodian, at least one independent reviewer, protected
   publisher host, and backup/restore owner. Create the new key in that custody
   system; do not print or commit it. Prove a publisher SQLite online backup,
   whole-file checksum and restoration in a separate protected location before
   first publication. A CI checkout is not durable custody.
3. After the exact new cloud resources are approved, create only those new
   resources. Apply D1 migrations `0001` through `0005` to the new D1 and
   verify their tables before promotion. Establish an independent D1 SQL export
   with a retained off-service copy and test an isolated restore. D1 Time Travel
   covers only a rolling 7- or 30-day window by plan; it is not durable
   publisher custody or a long-term backup.
4. Protect the new R2 artifact, envelope and Markdown objects against deletion
   with reviewed bucket-lock retention and an independent copy/restore check.
   A lock rule can be removed by an authorized administrator; it does not
   replace backup. Avoid lifecycle deletion of published objects.
5. Review the new Worker version and configuration before activation. Do not
   use `wrangler secret put` as a preflight: it creates and deploys a Worker
   version. The public read Worker needs no signing or promotion credential.
   Keep D1/R2 promotion credentials in the protected operator host, scoped to
   the new resources only.
6. Publish reviewed, current signed feeds to the new empty channels, verify
   exact bytes from the new public origin, and perform Site/CLI consumer
   acceptance against the **new** catalog/key/public-key triple. Do not copy
   the old catalog's accepted checkpoint into the new root. Promote Site only
   after its signed-feed and deployed-output checks; do not redirect or remove
   the old domain as part of this launch.

Cloudflare references: [Custom Domains](https://developers.cloudflare.com/workers/configuration/routing/custom-domains/),
[D1 Time Travel](https://developers.cloudflare.com/d1/reference/time-travel/),
[D1 export](https://developers.cloudflare.com/d1/best-practices/import-export-data/),
and [R2 bucket locks](https://developers.cloudflare.com/r2/buckets/bucket-locks/).
