# Local publication operator

`lenso-marketplace-publisher` is a native operator executable owned by the
Catalog publisher. It calls the existing Directory transaction and does not
implement an alternative signing format, public HTTP API or direct SQL writer.
Removing the Catalog package removes this entrypoint.

The trusted boundary is a controlled local user or an approved CI environment
with access to the private database, configuration and signing-key input.
An actor argument is an audit identity checked against configured reviewers;
it is not remote authentication. Never expose this command to public request
parameters or let an untrusted caller edit its configuration. The public Worker
must not receive the database, reviewers or private key.

Configuration is a JSON file containing `database` (absolute private SQLite
path), `catalog_id`, `reviewers` (actor IDs), `key_id`, and `public_key_hex`.
The operator does not generate keys, choose production identities or create
parent directories. Prepare and protect those outside the command.

Build with `cargo build --locked --manifest-path
tools/publisher/Cargo.toml --bin lenso-marketplace-publisher`.
Invoke the built executable as follows:

```sh
lenso-marketplace-publisher operator.json initialize
lenso-marketplace-publisher operator.json publish reviewer 0 604800 < signing-key.bin
lenso-marketplace-publisher operator.json export
```

`initialize` explicitly creates a new database and refuses existing paths.
It creates no namespace, submission, approved release or key. A publication of
an empty new database is an explicit empty catalog; fixtures are never promoted
into official releases. Existing reviewed submissions must come from the
Catalog admission/review domain before publication.

`publish` requires an existing database, the exact expected revision, and
validity of 1–604800 seconds. It reads exactly 32 raw Ed25519 seed bytes from
stdin, checks the derived public key against configuration, and uses the current
system time. The process inherits the operator's local authority; secure CI
must obtain this key from its protected environment, not a repository file or
command-line argument. Do not persist stdin in logs.

The single stdout JSON receipt contains `envelope` (the exact signed UTF-8
string) and `digest`. Decode the string to bytes when uploading; do not parse and
reserialize the envelope. If stdout fails after commit, publication remains
durable. Run `export` and reconcile the receipt; retrying the old expected
revision is rejected. Export can return an expired historical publication for
recovery and does not claim that it is currently acceptable to consumers.

Renewal is another `publish` using the current revision. The database retains
publication history and audit records. A timestamp or key mismatch is not
permission to reset that history. Back up this publisher database independently
of consumer D1/R2 state.

This executable does not upload to R2, mutate the D1 publication pointer,
configure renewal scheduling, or deploy Workers. Those operations must preserve
create-only object identity and compare-and-swap publication, and verify the
public consumer receipt before declaring an end-to-end publication complete.
The production key custodian and initial reviewed catalog remain launch inputs.

`verify` reads a bounded raw envelope from stdin and emits verified catalog ID,
revision and expiry using configured public trust and the current clock. It does
not require a publisher database to exist. This is a cryptographic admission
operation, not an upload, historical checkpoint check or publication authority.

## Consistent publisher backup

`lenso-marketplace-publisher CONFIG backup /absolute/new-backup.sqlite3` writes
an SQLite online-backup image from a read-only, catalog-checked connection. The
destination must not exist. It includes committed WAL state, all publication
history, submissions, namespace ownership and audit tables. It validates SQLite
integrity and flushes the image and parent directory before printing its receipt.
On Unix the new file is owner-readable/writable only. Backups contain private
review data and must remain in protected storage.

The receipt's `publication_digest` identifies the latest envelope in the copied
image, not a hash of the entire database. It is null for an unpublished directory.
Record a separate whole-file checksum when transferring the backup. Do not copy
only the live SQLite main file or use a fresh `initialize` as a restore operation.

A busy or failed backup leaves its new destination for inspection; it is not a
successful backup and the command will not overwrite it. Retry with another path
after diagnosing the failure. Broken stdout does not remove a completed image.
The single backup step holds a source read lock; run during a controlled operator
window rather than using it as a high-frequency backup service.

For recovery, preserve the original image and configure a separate protected copy
as the publisher database. Check the catalog identity, full image checksum and
exported signed envelope, and reconcile its revision/history with the highest
known publication and consumer state before authorizing a new publication. A
valid older backup cannot prove that no later revision exists. Never reset D1
acceptance or promote an old pointer to make restoration appear successful.

The process regression exercises a live WAL backup, exact export recovery, retained
snapshot history, rejection of stale publication, and forward publication from an
isolated restored image without changing the original database. This qualifies the
local backup primitive; remote backup retention, storage protection, transfer and
unattended Actions restore remain deployment work.

## Receive and review author submissions

Use the author's [preparation workflow](../../../docs/publishing.md). Authenticate
the contributor through the agreed handoff channel and verify namespace ownership
before assigning an actor. Actor strings below are local audit identities, never
credentials provided by a public caller.

```sh
lenso-marketplace-publisher operator.json claim REVIEWER example PUBLISHER AUTHOR
lenso-marketplace-publisher operator.json submit AUTHOR /absolute/submission-1.0.0
lenso-marketplace-publisher operator.json inspect REVIEWER SUBMISSION_ID
lenso-marketplace-publisher operator.json approve REVIEWER SUBMISSION_ID EXPECTED_DIGEST REVIEW_POLICY
```

`claim` is a one-time reviewer action. `submit` reads `release.json` and
`plugin.lenso-plugin`, verifies the exact archive into a private temporary extraction
using the shared CLI archive verifier, and calls Directory admission. It does not
execute the plugin. It returns `submission_id`, `proposal_digest` and `state`;
identical retries return the existing submission, while changed immutable releases
fail. `inspect` is restricted to the publisher actor or configured reviewers.
Review source and permissions independently of format verification. `approve`
requires the digest from that inspection and changes only review state.

These commands require an existing catalog database. They never sign, upload or
silently publish a candidate. After approval, make the exact archive available at
its immutable artifact URL, verify downloaded bytes against its release digest,
then use the existing `publish` and conditional Cloudflare promotion workflow.
Verify the public exact release, signed snapshot and Agent installation before
reporting publication complete. The same sequence handles each new version;
namespace claims are not repeated. Preserve the submitted files until publication
and backup are confirmed; the publisher database stores metadata, not archive bytes.

### Public submission tracking

The GitHub plugin submission issue is the review conversation. Record the internal
submission ID and proposal digest there after import, request missing evidence,
and report the exact public release URL and catalog revision after verification.
Do not treat opening/closing an issue, repository membership or form fields as
Directory approval. Never run author commands, source builds or plugins from an
issue inside a privileged publication job. Downloaded candidate bytes still pass
the bounded archive verifier and namespace authorization before review.
