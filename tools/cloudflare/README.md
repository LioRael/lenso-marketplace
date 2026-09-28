# Private publication promotion

`promotePublication` owns the Marketplace publication pointer transition. It
receives primary D1 and R2 bindings from a protected operator composition. It
has no public route, credentials, key generation, scheduler or direct deployment
side effect. The public read Worker does not import this module.

Supply `catalogId`, exact UTF-8 `envelope`, and `expected` (null for an explicitly
empty publication slot, otherwise the complete previous `revision`, `object_key`
and `digest`). `verify` must invoke the shared Rust verifier with independently
configured public trust. It must not be supplied by a request or implemented as
JSON parsing. `lenso-marketplace-publisher CONFIG verify` reads envelope bytes
from stdin and returns verified `catalog_id`, `revision`, and `expires_at`.
It checks signature, schema, catalog identity and current validity; without an
accepted checkpoint it does not establish historical release identity continuity.
Only promote publications from the authoritative publisher database, whose
review and immutable identity rules remain the source of that authority.

Objects use a content-addressed key and create-only R2 conditions. Existing
objects and upload results are read back and compared with the exact bytes.
D1 updates compare the complete old pointer, not just a revision. The operation
never writes `marketplace_accepted` or resets consumer history. Concurrent losers
fail without overwriting the winner. Repeating an exact successful candidate
with its original expected pointer returns `already_published` only after object
verification. A later revision superseding it is a conflict, not a retry target.

If a D1 response is lost, a primary read can confirm the candidate and return
`reconciled`. Otherwise the result stays uncertain: inspect durable state before
any retry. An unavailable reconciliation read also fails. No mutation is retried
inside the operation. Expiry is checked again after object I/O and before the
pointer transition; expiry after commit leaves durable state for reconciliation
and renewal, never rollback.

The returned receipt describes storage publication only. Complete release
acceptance must separately read the public Directory/search/detail endpoints
and verify the exact signed bytes through the consumer, then record its accepted
revision. Artifact archive admission/upload, production binding credentials,
protected execution, renewal scheduling and production rollout remain separate
work. This module is not a publicly callable publishing service.

Run `node --test tools/cloudflare/promote.test.mjs tools/cloudflare/promote-cloudflare.test.mjs`
from the repository root. These storage fault and protected-host dispatch tests
inject a verifier; the Catalog process tests independently exercise the real
Rust `verify` command.

`node --test tools/cloudflare/promote-runtime.test.mjs` additionally runs the operation
in local workerd through Miniflare using real local D1 and R2 bindings. It
checks create-only R2 conditions, exact bytes, D1 competing updates, stale
pointers and preservation of consumer acceptance. The fixture verifier is a
stub; this is storage integration evidence, not signature or deployed Cloudflare
qualification. It creates no remote resources and disposes its isolated runtime.
`node --test tools/cloudflare/release-content-runtime.test.mjs` checks the
separate v2 pointer against local D1/R2, including competing updates and
preservation of portable publication and consumer acceptance rows.

## Protected host to Cloudflare

`node tools/cloudflare/promote-cloudflare.mjs CONFIG.json` exports the latest committed
publication from the configured Rust publisher database, verifies it using the
same binary and configured trust, then invokes conditional promotion against D1
REST and R2 S3. Run this command on the protected operator host, outside the
public Worker. It never initializes a database, signs a new revision or accepts
an arbitrary envelope file. First use the matching Rust `publish`,
`publish-details`, `publish-linked-cargo`, `publish-package` or `publish-release-content` operation
to commit an authorized publication; after a network failure, reconcile before
repeating promotion. Do not publish another revision just to retry transport.

The public configuration has this shape (replace every placeholder):

```json
{
  "accountId": "<32 hexadecimal account ID>",
  "databaseId": "<D1 UUID>",
  "bucketName": "<private R2 bucket>",
  "catalogId": "<approved catalog identity>",
  "publisherBinary": "/absolute/path/to/lenso-marketplace-publisher",
  "publisherConfig": "/absolute/path/to/publisher.json",
  "expected": null
}
```

Use `expected: null` only for an explicitly reviewed empty remote publication
slot. Otherwise provide the full observed revision, object_key and digest.
`MARKETPLACE_D1_TOKEN`, `MARKETPLACE_R2_ACCESS_KEY_ID` and
`MARKETPLACE_R2_SECRET_ACCESS_KEY` are injected environment credentials. Restrict
them to the selected account/database and bucket using available provider scopes;
they are distinct from the Ed25519 signing key and from a Wrangler OAuth login.
Never place them in this configuration or commit them. The command does not
create credentials, migrations, buckets or environments.

For signed release-content v2, set `"kind": "release-content"` in the same
configuration. The protected host then calls the publisher's
`export-release-content` and `verify-release-content` operations, and promotes
the verified bytes under the `release-content/` R2 prefix through the independent
`marketplace_release_content` D1 pointer. The Rust verifier owns the
`lenso.marketplace.release-content.v2` schema and separate signature context;
this JavaScript adapter does not reinterpret or re-sign the envelope. Existing
portable (`kind` omitted), `release-details` and `linked-cargo` kinds retain
their own pointers and object namespaces. A successful v2 promotion does not
update them or `marketplace_accepted`.

For npm-only package releases, set `"kind": "package"` and use a publisher
built with `--features package-publication` after the signed Rust package
protocol is available in the Market lock. The protected adapter exports and
verifies the exact signed package envelope before conditionally promoting it
under `packages/` through `marketplace_packages`. Apply
`apps/workers/migrations/d1/0005_packages.sql` explicitly first, and review
the full prior pointer or confirmed empty slot. This adds no public writer
route; the read-only Worker serves `/api/marketplace/v1/package`.

## Retain exact published Markdown bodies

For `release-details`, `linked-cargo` and `package` promotions, the protected
host requires every document in the Rust-verified snapshot to exist in the
Marketplace R2 bucket before advancing the D1 publication pointer. Supply
missing bodies as an optional `documentFiles` object in the same protected
promotion config, keyed by signed `sha256:<hex>` digest with absolute local
Markdown paths as values. Each file is bounded to 1 MiB and checked against
the signed size and SHA-256 before a create-only upload to
`documents/sha256/<hex>.md`. An already uploaded object is read back and
verified, so renewals do not need to resupply its file. An unrelated file key,
missing body, changed object or uncertain write fails before pointer promotion.
Do not place private documents in this public signed-directory workflow.

The public Worker serves `GET` and `HEAD /documents/sha256/<hex>.md` only when
that digest appears in the exact envelope named by a current D1 publication
pointer. It verifies the envelope object against the pointer digest, then the
Markdown body against its signed size and digest. The response carries
`X-Lenso-Document-Digest` and `text/markdown; charset=utf-8`; it is never an
MDX/JS execution path. A published body's bytes remain readable after the
envelope's freshness window expires, while catalog freshness and adoption are
separate consumer decisions. A later publication that omits the document no
longer authorizes this route; the R2 object remains retained but unserved.
Existing external signed URLs are not automatically mirrored: the operator
must provide their exact bytes once. Protect the bucket from lifecycle deletion
and back it up independently of the publisher database; this code never
deletes published document objects.

Apply `apps/workers/migrations/d1/0004_release_content.sql` explicitly to the
selected D1 database before first v2 promotion. The promoter deliberately does
not create tables or infer an empty pointer from a missing migration. Review
the current row and provide its complete pointer as `expected`, or `null` only
for a confirmed empty v2 slot. Do not use the portable kind to publish v2 bytes;
the independent Rust verifier operation and pointer are part of the trust
boundary.

The adapter signs R2 requests with aws4fetch `sign()` and performs one fetch;
it does not use the SDK retrying fetch helper. Requests have a 30-second timeout,
reject redirects, bound D1 response bodies and suppress provider response bodies
in errors. Transport failures remain uncertain until the promotion operation
reconciles durable state. Exact successful pointer receipts are printed to stdout;
a broken stdout does not roll back committed storage.

The intended production custody environment is `marketplace-production`, with
an additive `marketplace.lenso.dev` service and a reviewed empty first directory.
A durable publisher database and backup/restore ownership must be wired before
unattended renewal. An ephemeral Actions checkout is not the publisher database;
this command alone does not configure or qualify an unattended workflow.
Cloudflare account execution and consumer endpoint acceptance remain pending.

Protocol references: [D1 query API](https://developers.cloudflare.com/api/resources/d1/subresources/database/methods/query/)
and [R2 aws4fetch](https://developers.cloudflare.com/r2/examples/aws/aws4fetch/).
