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

## Add a documentation revision to published release details

`submit-details` remains immutable for one Plugin ID and version. To add a new
documentation identity (`id` plus `revision`) to already published release
details, submit the complete next `ReleaseDetails` JSON instead. Preserve the
base-release identity, every distribution and every previously published
documentation entry exactly; only append new documentation entries. The
Directory rejects a second pending revision for the same release.

```sh
lenso-marketplace-publisher operator.json submit-details-revision AUTHOR /absolute/revised-details.json
lenso-marketplace-publisher operator.json inspect-details-revision REVIEWER REVISION_ID
lenso-marketplace-publisher operator.json approve-details-revision REVIEWER REVISION_ID EXPECTED_DIGEST POLICY
lenso-marketplace-publisher operator.json publish-details REVIEWER EXPECTED_REVISION 604800 < signing-key.bin
lenso-marketplace-publisher operator.json export-details
```

The first three commands require the existing protected publisher database;
they do not publish or grant authority from JSON. Inspection returns the exact
proposed body and digest for review. `publish-details` signs one new snapshot
in the same transaction that marks the amendment published; prior submissions,
amendments and signed snapshots stay in the database. A stale expected revision
is rejected. Verify the new envelope and its checkpoint against the previous
one before promoting it publicly.

This is metadata revision support, not document-byte hosting. A signed
documentation entry names an HTTPS URL, SHA-256 digest, size and Markdown media
type, but this publisher does not upload, fetch, sanitize or serve those bytes.
Review the exact bounded Markdown bytes and their immutable URL separately;
never infer safety or successful hosting from a signed metadata snapshot.

## Source-only linked Cargo release

This separate channel accepts an exact registry `.crate` as a Host build input;
it does not turn that crate into a loadable portable Bundle. The JSON release
names the Plugin ID, package/version, publisher, source revision, registry URL,
archive SHA-256, supported targets, and integration kind (`linked_plugin` or
`host_provided`). `host_provided` requires a product Host-specific adapter and
must not be offered as a generic `lenso app add` candidate. The operator verifies the archive digest
and its `Cargo.toml` package name, version and Lenso Plugin ID before creating a
submission. Review must still establish namespace ownership, source provenance,
registry availability and the actual Host build/linked factory behavior. The
archive is not stored in the publisher database. Authors can create a checked
`release.json` plus exact `plugin.crate` directory with
`lenso-marketplace-author prepare-linked-cargo` and recheck it with
`check-linked-cargo`. The tool derives Plugin ID, package name, version, and
SHA-256 from the `.crate`; it does not authenticate the registry or source commit.

```sh
lenso-marketplace-publisher operator.json submit-linked-cargo AUTHOR /absolute/release.json /absolute/plugin.crate
lenso-marketplace-publisher operator.json inspect-linked-cargo REVIEWER SUBMISSION_ID
lenso-marketplace-publisher operator.json approve-linked-cargo REVIEWER SUBMISSION_ID EXPECTED_DIGEST POLICY
lenso-marketplace-publisher operator.json publish-linked-cargo REVIEWER EXPECTED_REVISION 604800 < signing-key.bin
lenso-marketplace-publisher operator.json export-linked-cargo
```

The signed source-only snapshot has independent revision, signature domain and
publication pointer from the portable catalog and release-details snapshots.
`verify-linked-cargo` reads an exact envelope from stdin using configured public
trust. An identical retry returns the existing submission; changed content under
the same Plugin ID/version is rejected. To append documentation to an already
published source-only version, submit the complete linked release JSON with all
existing fields and documents unchanged, adding only new document identities:

```sh
lenso-marketplace-publisher operator.json submit-linked-cargo-docs-revision AUTHOR /absolute/revised-linked-release.json
lenso-marketplace-publisher operator.json inspect-linked-cargo-docs-revision REVIEWER REVISION_ID
lenso-marketplace-publisher operator.json approve-linked-cargo-docs-revision REVIEWER REVISION_ID EXPECTED_DIGEST POLICY
lenso-marketplace-publisher operator.json publish-linked-cargo REVIEWER EXPECTED_REVISION 604800 < signing-key.bin
lenso-marketplace-publisher operator.json export-linked-cargo
```

The revision is append-only and requires the same publisher ownership and
reviewer approval as the original release. It does not replace the old signed
snapshot or check the remote Markdown bytes. Review the exact document digest,
size and URL separately before publication. A portable base release with that identity
must instead use the existing release-details channel for a linked distribution.
The development CLI accepts an exact signed local snapshot and matching `.crate`
through `lenso app add PLUGIN_ID@VERSION --linked-snapshot ... --trust ... --crate ...`.
It does not fetch the archive or prove registry provenance automatically; reviewers
must verify registry availability and the generated Host build separately.

## npm-only signed package release (staged)

The package-only channel has no fabricated Portable base. Its release JSON uses
the `PackageRelease` schema from the new `lenso-plugin-catalog::package` module:
one exact Plugin ID/version, publisher, source commit, and one to sixteen npm
distributions. Each distribution has an ID, exact npm name and version,
credential-free HTTPS registry URL, and SHA-256 `.tgz` digest. The current
Marketplace dependency pin predates this protocol. This operator path is a
local candidate until that Rust commit is available remotely and the Market
manifest and lock are advanced together.

```sh
lenso-marketplace-publisher operator.json submit-package AUTHOR /absolute/release.json npm /absolute/package.tgz
lenso-marketplace-publisher operator.json inspect-package REVIEWER SUBMISSION_ID
lenso-marketplace-publisher operator.json approve-package REVIEWER SUBMISSION_ID EXPECTED_DIGEST POLICY
lenso-marketplace-publisher operator.json publish-package REVIEWER EXPECTED_REVISION 604800 < signing-key.bin
lenso-marketplace-publisher operator.json export-package
```

For multiple distributions, append one `ID ARCHIVE` pair per distribution.
Submission checks each bounded archive's SHA-256 and `package/package.json`
name/version before any database write. The reviewer must independently check
that the named registry serves those same bytes: local archive inspection does
not establish registry provenance or availability. The signed snapshot has its
own revision and signature domain; `verify-package` reads an exact envelope
from stdin with configured public trust. The publisher rejects reuse of any
Plugin ID/version already recorded in Portable, linked Cargo, or package-only
submission history. Package publication remains local here: no public
directory read route, App installation, upload, or deployment is implied.

## Optional signed release content

Editable templates and development extensions use a separate
`lenso.marketplace.release-content.v2` snapshot. They attach to one already
published `portable` or `linked_cargo` base with the same Plugin ID and exact
version. Neither the portable nor linked Cargo v1 signed payload is changed.
Each content entry identifies a credential-free HTTPS `.tar.gz`, compressed
size and SHA-256. The operator checks the supplied exact archive bytes and
rejects links, path traversal, duplicate paths and excessive expansion, but
does not store the archive or prove its remote URL serves those bytes. Review
the URL and hosting separately. A signed listing does not execute the archive.
For `development_extension`, submission additionally requires exactly one root
Bun or Cargo Plugin source with the signed ID/version and nonempty convention
declarations. Composite/workspace discovery layouts are conservatively rejected
by this channel; publish an adoptable simple root source archive instead.

Use `release-content-base KIND RELEASE_JSON` to derive the immutable v1 base
identity; do not hash a modified release or its current availability field.
The content JSON has `plugin_id`, `version`, `base_kind`,
`base_release_identity` and a `content` array of `id`, `kind`
(`editable_template` or `development_extension`), `url`, `digest`, `size`.
Pass archives in that array's order:

```sh
lenso-marketplace-publisher operator.json release-content-base linked_cargo /absolute/published-linked-release.json
lenso-marketplace-publisher operator.json submit-release-content AUTHOR /absolute/release-content.json /absolute/template.tar.gz /absolute/extension.tar.gz
lenso-marketplace-publisher operator.json inspect-release-content REVIEWER SUBMISSION_ID
lenso-marketplace-publisher operator.json approve-release-content REVIEWER SUBMISSION_ID EXPECTED_DIGEST POLICY
lenso-marketplace-publisher operator.json publish-release-content REVIEWER EXPECTED_REVISION 604800 < signing-key.bin
lenso-marketplace-publisher operator.json export-release-content
```

`verify-release-content` checks an exact envelope from stdin against configured
trust. The content publication has independent reviewer approval, revision CAS,
signature domain and read-only `/api/marketplace/v1/release-content` endpoint.
The same ID/version cannot be resubmitted with changed content. App owners must
explicitly preview and copy selected content from verified local bytes; copied
templates become App-owned and later updates never overwrite user edits.
Development extensions remain inert until separately selected as a local source;
listing or copying one grants no runtime permission.

### Public submission tracking

The GitHub plugin submission issue is the review conversation. Record the internal
submission ID and proposal digest there after import, request missing evidence,
and report the exact public release URL and catalog revision after verification.
Do not treat opening/closing an issue, repository membership or form fields as
Directory approval. Never run author commands, source builds or plugins from an
issue inside a privileged publication job. Downloaded candidate bytes still pass
the bounded archive verifier and namespace authorization before review.
