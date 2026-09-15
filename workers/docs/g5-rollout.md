# Marketplace Workers rollout and rollback

This is a proposed operations runbook for the additive public-read service.
It does not authorize production writes, select a trust root, or claim the proof
resources are production-ready. Complete [the preflight inputs](g5-preflight.md)
and record the final approved action before running mutation steps.

## 1. Freeze the release and ownership record

Record the Marketplace, CLI protocol, Web, Auth (if separately deployed), and
Runtime source commits, dependency versions, lockfile hashes, Rust/Wrangler/
wasm-bindgen versions, compatibility flags, generated JS and Wasm hashes, and
production configuration digest. The [cohort bootstrap](cohort-bootstrap.md) can
recreate the current experimental relative paths from reviewed commits without
copying working-tree contents. Use its reviewed exact pins and preserve remote fetch proof;
it does not turn the experiments into a production dependency package. Rebuild
from those clean checkouts and preserve the deployable upload artifact.
Generated linkage and reset support must come from the pinned generator; do not
edit generated bindings to reconcile source cohorts.
Resolve each shared generated Capability crate to one source identity across the
linked App. Equal crate versions from registry and Git/path sources can still be
distinct Rust types and fail in-process endpoint dispatch; the G4 integration
encountered this boundary. The clean cohort build must prove actual invocation.

Attach G1/G2/G3 final receipts and the separate G4 scope/limitations. Re-run
relevant native regressions and deployed staging checks against these exact
bytes. For G3, use populated signed fixtures and prove immutable identity after
omission, accepted-object corruption rejection, expiry, competing acceptance,
storage failure, cancellation/uncertain cleanup and persistence across Wasm
recreation. Measure the actual Marketplace App's CPU/memory and admitted load;
the two-Plugin G1 reference is not its capacity measurement.

Snapshot the current `catalog.lenso.dev` deployment/version and endpoint responses.
Keep its route and service unchanged. Its source/deployment drift and failed
upstream admission check require their own catalog-owner repair, not a switch to
an incompatible envelope. Record separately whether that repair is in scope for
the release; do not describe currently missing `/v1/plugins.json` as healthy.

## 2. Prepare separate storage and explicit migrations

Use separate staging and production Worker, D1 and private R2 resources. Supply
explicit resource IDs; never infer the target from the current Wrangler directory
or reuse `lenso-marketplace-g3-proof`. Public code holds no signing private key.

Apply the reviewed [public-read migration](../migrations/0001_public_reads.sql)
explicitly before admission. It creates publication and accepted-checkpoint
pointers. Plugin prepare does not migrate storage. Record migration checksum,
database ID, applied version and verification query results. Rehearse both the
new build and rollback build against the resulting schema. Future changes must
remain compatible with the rollback build or use an independently reviewed data
migration plan.

R2 stores exact UTF-8 signed envelopes and immutable complete accepted states.
Enable and test protection of immutable object prefixes against accidental
overwrite/delete, or document an equivalent enforced protection. Content hashes
detect corruption but do not recover destroyed historical checkpoint bytes.
Cloudflare [R2 bucket locks](https://developers.cloudflare.com/r2/buckets/bucket-locks/)
provide retention controls; choose the actual policy before production and
include its impact on cleanup and costs. Do not apply proof corruption operations
to locked production data.

## 3. Establish the publication and historical checkpoint

Identify the authoritative catalog and signing owner before importing anything.
The official repository's unsigned `lenso.plugin-catalog.v1` file and the live
legacy Module catalog are not signed Marketplace snapshots. Mapping entries
requires an explicit reviewed migration with real source/archive identity; an
empty source does not authorize creating a new signing root.

For an existing signed catalog, export exact envelope bytes and the latest trusted
consumer checkpoint together, including every historical `release_identities`
entry. Back up the private publisher database, namespace/submission/reviewer and
audit state as well. Verify signature, catalog identity, revision, expiry, archive
digests and checkpoint consistency with the shared Rust protocol. The latest
envelope alone cannot reconstruct identities omitted by earlier snapshots.
Recover a complete history from authoritative audit/publication records if no
checkpoint is available; do not reset trust by starting with an empty checkpoint.

Import historical accepted state through a reviewed migration tool using the
same content encoding, whole-state content hash and pointer semantics as the
adapter. Preserve the migration receipt and compare a read-back verification
against the source. A database copy without referenced R2 objects is incomplete.
When initializing an explicitly approved new catalog, document that it has no
previous history; allow the first verified read to create its accepted state.

Keep a single controlled publisher authoritative during cutover. This public-read
rollout does not move namespace/review/signing authority into D1 or the fetch
handler. The existing private `Directory::publish` allocates the next revision
(`expected_revision + 1`), signs and commits audit/snapshot state atomically in its SQLite database.
A production operator wrapper around that owner path remains a preflight input.

## 4. Publish immutable bytes, then the reviewed pointer

The controlled publisher operation must carry authenticated operator/reviewer
identity, reviewed submission digests, expected catalog revision, key identity,
validity window and an audit/change reference. Review and verify every real
archive through the canonical bounded archive and Bundle verifier before release.
Never reassign an existing Plugin/version to a different artifact, publisher or
source identity. Proof actor strings and proof fixtures cannot supply provenance.

1. Read the current publication receipt. Produce the next signed envelope using
   the owning publisher, preserving exact payload/envelope bytes. Verify it
   against the configured trust and the last accepted history. Record revision,
   issued/expiry times, envelope digest and all artifact identities.
2. Upload artifacts and envelope to unique content-addressed R2 keys with
   create-only conditions. On an existing key, require exact matching content;
   never overwrite. Read back and verify before exposing any pointer.
3. Change the one D1 publication row conditionally: insert only if the approved
   initial row is absent, otherwise compare the expected revision and prior
   pointer/digest. Require one affected row. The proposed new revision must be
   greater than the old revision and must match the signed envelope. The public
   read adapter does not enforce operator publication policy on your behalf.
4. If the conditional change loses a race, keep the winner authoritative and
   reconcile. If a write result is uncertain, read the durable pointer and exact
   object to establish whether it committed before retrying anything. No blind
   repeat, unconditional upsert or revision reuse is permitted.
5. Read the public snapshot and compare exact bytes/digest; then verify search and
   exact details through Web Ingress and the generated Directory contract. Record
   the accepted checkpoint pointer and response results. An uploaded object or a
   snapshot transport 200 alone is not a successful publication receipt.

Consumers own `marketplace_accepted`. Ordinary publication must not delete or
rewrite that row to make a changed snapshot pass. A failed publication-pointer
update leaves the old pointer authoritative; an unreferenced immutable upload is
an orphan to inventory later, not permission to weaken the conditional update.

## 5. Rehearse staging, then approve the concrete production action

Use controlled staging resources, the release artifact, production-shaped
configuration and non-production trust. Run the owner qualification suites there;
the existing G3 smoke is explicitly destructive and restricted to its proof
resources. Production verification consists of non-destructive public reads and
receipt checks, never corrupting storage or deliberately publishing rollback
snapshots.

Before final approval, provide the exact production hostname and Worker identity,
artifact digest/version, binding IDs, migration checksum, approved catalog ID,
public-key fingerprint, initial revision/digest/expiry, publication operator,
monitoring owner, prior compatible Worker version and rollback steps. Include the
staging evidence and any remaining platform divergences. Do not deploy the
deterministic test key, expose proof headers, or include the fixture signing key
in a production artifact. Inspect the final bundle/configuration for that boundary.

Once the concrete action is approved, provision/migrate/publish only the named
resources, attach the approved new Custom Domain, and retain a deployment receipt.
Cloudflare [Custom Domains](https://developers.cloudflare.com/workers/configuration/routing/custom-domains/)
bind a hostname to a Worker; verify the exact mapping and TLS after the change.
Never replace the existing catalog domain as a shortcut. Validate each owned
Marketplace route, exact signature bytes, current verified content and unchanged
catalog-service baseline, then monitor the agreed observation window.

## 6. Renew expiry and monitor useful behavior

The shared protocol enforces `issued_at <= now < expires_at`, a maximum validity
of seven days, monotonic revisions, same-revision payload consistency and immutable
release identities. The Web event path checks expiry again after storage I/O.
Do not cache a successful verification indefinitely, lengthen expiry in JSON, or
serve expired content to hide a failed renewal.

Choose and record a renewal interval and alert lead time before launch. A practical
proposal is daily renewal with seven-day validity and escalation well before the
remaining window is exhausted; this is an operational proposal, not an already
approved schedule. Each renewal uses the publisher's next revision even when
releases are unchanged, passes the same upload/conditional-pointer procedure and
produces an audit receipt. Keep the last successful expiry visible to the operator.
If the key or publisher is unavailable, escalate while content is still valid;
after expiry the service must fail closed until a valid forward publication exists.

Monitor verified search/detail status as well as raw envelope delivery, remaining
validity, published/accepted revision, signature/integrity rejection, storage
errors, CAS contention, 503 admission/deadline rates, unconfirmed cleanup,
CPU/memory and R2/D1 usage. Attach deployment identity to private diagnostics.
Do not log credentials, private keys, raw Auth records or sensitive request bodies.
Define alert ownership and capacity/cost thresholds from the staged workload.

D1/R2 operations may commit after event cancellation. Bounded settlement can
return `storage_cleanup_unconfirmed`; that must remain a 503 and an operational
uncertainty, not a clean receipt or automatic replay. Generation replacement
invalidates stale Rust callbacks; it does not roll back a durable write. A
Directory storage failure retires the affected provider/consumer under current
Kernel semantics and may yield 503 despite a persisted checkpoint. Preserve that
checkpoint and investigate the storage failure; do not promise a warm fallback 200.

## 7. Roll back code without rolling back trust

For a code regression, stop promotion and return traffic to the last verified,
schema-compatible version of the **new Marketplace Worker**, preserving its D1,
R2, publication and accepted history. On the first deployment with no healthy
prior version, withdraw the new domain/traffic or return an explicit unavailable
response; leave the old catalog service intact. Recheck domain mapping, public
content and alert state after the operation.

Cloudflare [Worker rollbacks](https://developers.cloudflare.com/workers/versions-and-deployments/rollbacks/)
restore a Worker version; binding changes can constrain rollback. They are not a
catalog database restore. Keep rollback resources and their compatible schema
available throughout the observation window.

For bad catalog content, **do not point back to an older revision or clear the
accepted checkpoint**. Clients that observed the new revision must reject that
rollback. Use a reviewed higher-revision corrective publication through the owner,
preserving immutable historical identities. If the required withdrawal/revocation
operation is not implemented, keep the affected path unavailable while the owner
provides a reviewed recovery; do not hand-edit private publisher tables or silently
substitute a different archive for the same version.

For key compromise, disable affected acceptance, follow the named custody/rotation
procedure, distribute the new public trust out of band and retain historical
checkpoints. Current Web configuration accepts one configured key; any overlap or
new rotation mechanism needs its own reviewed change. A production private key
must never be recovered from the public Worker or generated as an incident shortcut.

## 8. Back up and rehearse recovery

Before first publication and each schema change, record the D1 bookmark and export,
both pointer tables, every referenced immutable R2 object and its digest, trusted
public-key configuration, the complete highest accepted checkpoint, publisher
database/audit history and artifact/deployment receipt. Maintain independent
retention for historical identity state and signing-key custody. Record the
approved retention, RPO/RTO and backup operator; no values are implied by this plan.

Cloudflare provides [D1 export/import](https://developers.cloudflare.com/d1/best-practices/import-export-data/)
and [Time Travel](https://developers.cloudflare.com/d1/reference/time-travel/).
A Time Travel restore overwrites database state and interrupts in-flight queries;
it is an incident operation, not routine code rollback. Rehearse using an isolated
recovery database and copied immutable objects before launch.

Restore publication and accepted state consistently, then reconcile against the
highest trustworthy revision/history observed before the incident. A stale backup
must not lower a rollback fence. If the latest complete history cannot be recovered,
keep acceptance closed and reconstruct it from authoritative records; current
snapshot signatures alone cannot prove omitted historical identities. Verify
object hashes, signatures, catalog ID, monotonic revision, historical identities,
expiry and actual Web responses before returning traffic. If the recovered
publication has expired, issue a reviewed forward renewal before restoration is
declared successful. Preserve the incident and reconciliation receipt.

## Executed consumer recovery rehearsal

The [isolated recovery receipt](../recovery/RESULTS.md) records paired D1/R2 restoration using the frozen public artifact. Six live checks and five durable observations passed, including rollback rejection and recovery from a corrupted copied checkpoint. The original source remained unchanged. This closes the experimental consumer-state restore check; private publisher backup, production custody and operational recovery targets remain launch inputs.
