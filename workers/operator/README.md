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

Run `node --test operator/promote.test.mjs` from the Workers directory. These
storage fault tests inject a verifier; the Catalog process tests independently
exercise the real Rust `verify` command.

`node --test operator/promote-runtime.test.mjs` additionally runs the operation
in local workerd through Miniflare using real local D1 and R2 bindings. It
checks create-only R2 conditions, exact bytes, D1 competing updates, stale
pointers and preservation of consumer acceptance. The fixture verifier is a
stub; this is storage integration evidence, not signature or deployed Cloudflare
qualification. It creates no remote resources and disposes its isolated runtime.

## Protected host to Cloudflare

`node operator/promote-cloudflare.mjs CONFIG.json` exports the latest committed
publication from the configured Rust publisher database, verifies it using the
same binary and configured trust, then invokes conditional promotion against D1
REST and R2 S3. Run this command on the protected operator host, outside the
public Worker. It never initializes a database, signs a new revision or accepts
an arbitrary envelope file. First use the existing Rust `publish` operation to
commit an authorized publication; after a network failure, reconcile before
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
