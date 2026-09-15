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
