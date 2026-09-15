# Lenso Plugin marketplace v1

Status: implementation proposal, 2026-09-13. No marketplace service, public site,
publisher workflow, or remote installation is delivered by this document.

Implementation has started in [the marketplace package](../README.md):
signed-catalog and durable publishing domain checks, a native directory read
Plugin with generated Rust/TypeScript contracts, and a real packaged Process
fixture are implemented. The shared archive verification API is implemented in
CLI source and passes a source handoff proof; its release is pending. Host/Auth,
HTTP/UI and target installation integration remain pending. This does not
complete gates 1–4 below.

This proposal develops [the marketplace contract](plugin-marketplace.md) and
[ADR-0007](https://github.com/LioRael/lenso-console/blob/main/docs/adr/0007-use-an-open-signed-plugin-marketplace.md). It preserves
[Shell/App separation](https://github.com/LioRael/lenso-console/blob/main/docs/adr/0010-separate-console-shell-from-app-assembly.md).
The first end-to-end delivery below precedes the existing Observe Workspace
tracer: Observe still requires a distributable implementation and Console target
authority. It remains the later Console-extension acceptance scenario.

## Product outcome

A developer can publish a versioned Plugin with useful documentation and
verifiable artifacts. A user can find it, understand what it does, select a
compatible App, and install that exact release with an observable result.

The marketplace is a public product with a website and a Console contribution.
Browsing does not require a connected App or login. Publishing requires a
publisher identity. Installation requires authorization at the selected App;
a marketplace account grants no App permissions.

The first release includes real publisher submission, review, publication,
search, details, installation, and an update to a second release. Local inventory
management alone does not satisfy marketplace delivery.

## Current foundations and gaps

These are source observations, not production service availability claims.

| Foundation | Reuse | Gap |
| --- | --- | --- |
| Agent Web `src/plugin_control.rs` | Local catalog, trusted local Bundle proposals, revision checks, target admission | Remote acquisition, signed release identity, durable resumable operation receipts |
| Agent ADR-0072 | Source-aware available/active entries and allowed actions | Public discovery and publisher authority |
| CLI Bundle packaging and installation | Existing Bundle verifier and Plugin Root authoring | Bind remote metadata and downloaded bytes to one reviewed proposal |
| Console contribution contracts and ADR-0010 | Plugin-owned page mounting; Shell independent of business providers | Marketplace contribution and website |
| `lenso-catalog-worker` | An existing curated distribution endpoint to assess | Legacy Module catalog is not assumed compatible with this signed Plugin catalog |

Do not wrap the Agent-only endpoint in a universal App label. The first supported
target is one local Agent App with an explicit installation authority. Other
Apps become installable when they expose the same reviewed target semantics.

## Information architecture

| Surface | Content and primary action |
| --- | --- |
| Discover | Search, curated categories, official Plugins, recently updated releases; open details |
| Search results | Title, purpose, publisher, category, latest listed version, distribution type; filter by target compatibility when a target is selected |
| Plugin details | Overview, screenshots, documentation, source/license/support, versions/changelog, publisher evidence, runtime and capability requirements; install or explain incompatibility |
| Publisher profile | Owned namespaces, verification evidence and public Plugins; never a blanket safety endorsement |
| Publish | Claim namespace, submit release manifest/artifact references, follow validation and review, correct rejection reasons |
| Installation review | Explicit App, release, effects, configuration, execution profile and permission changes; authorize one exact proposal |
| Operation result | Stage, receipt, failure reason and recovery action; open the installed functionality when supported |

The public website and Console reuse marketplace-owned components and data
contracts. The Console-specific adapter supplies explicit target selection and
authorized calls. Neither surface maintains a second installed-plugin database.

Search ranks exact Plugin IDs first, then title and description matches with a
stable tie-breaker. Editorial lists are labeled curated. Do not invent download
counts, popularity scores, compatibility, or review badges from missing data.

Without a target, details say “Choose an App to check compatibility.” With a
target, distinguish installable, already installed, update available, missing
requirements, unsupported runtime, requires Host rebuild, and unavailable target.
A listing can be useful documentation even when it cannot be installed online.

From the website, “Install in Console” transfers only a catalog identity and
exact release reference through a documented handoff. Console re-fetches and
verifies it. The handoff cannot carry credentials, arbitrary download commands,
or installation approval. If Console is unavailable, retain the release link
and explain how to open it; no unauthenticated loopback mutation.

## Publisher journey

1. Sign in through an existing Auth provider. Create a publisher and prove control
   of a namespace through a recorded maintainer review in v1. Repository control
   can be evidence, but does not itself establish ownership of every Plugin ID.
2. Build and verify the Bundle with existing authoring tools. Upload it to an
   admitted immutable artifact origin; the marketplace need not host all bytes.
3. Submit exact Plugin ID/version, artifact URLs/digests/sizes, source revision,
   license, docs, changelog, and screenshots. Declare execution requirements in
   the Bundle; descriptive text cannot override verified machine facts.
4. The ingestion worker fetches bounded bytes, verifies the Bundle, checks
   namespace ownership and existing releases, derives contract facts, and emits
   a validation report. A URL becoming unreachable produces an actionable error.
5. A maintainer reviews source/provenance, descriptive accuracy and declared
   runtime risk. Approval names the policy revision and reviewer. V1 publication
   is curated; it does not require a fully automated moderation system.
6. An authorized publish operation writes a signed snapshot that includes the
   immutable release. Search exposes only committed published releases.
7. The publisher submits a new version for executable changes. They may request
   withdrawal, with an auditable reason; security revocation uses a separate
   authorized review path. Namespace transfers never silently replace release
   provenance or imply existing users accepted a new publisher.

Submission states: `draft -> validating -> awaiting_review -> approved -> published`.
Validation/review may return `changes_requested`; resubmission creates a new
submission revision. Approval is bound to that revision and its artifact digests.
Only completed snapshot publication makes it public. Retries are idempotent.

Release availability is separate: `listed`, `yanked`, or `revoked`. Corrections to
descriptions and advisory metadata are revisioned; executable identity and bytes
are immutable. Reusing a Plugin ID/version for changed artifacts is rejected.

## Ownership and Plugin boundaries

Provisional names below describe implementation owners, not published packages.

| Owner | Facts, rules and deletion boundary | Collaboration |
| --- | --- | --- |
| Marketplace directory Plugin | Publishers/namespaces, submissions, release records, reviews, catalog revisions and advisories; removing it removes publishing and discovery | Requires Auth assertions; provides read and publishing roles |
| Marketplace Web Plugin | Website and Console contribution, browse/detail/publish/review interactions; removing it removes marketplace UI without uninstalling Apps | Requires one directory role; provides existing Web/UI contribution roles; does not connect to installation targets |
| Target installation authority | Target revision, candidate plan, authorization, acquisition policy, staging, operation receipt and readiness outcome | Existing Host/configuration provider extended through a narrow role; no access to marketplace private tables |
| Console Shell | Generic contribution mounting and subject context | No concrete marketplace imports, publisher schema, search rules or installer |
| Artifact hosting and signing storage | Byte transport and private implementation resources | Not additional business Plugins merely because they use separate infrastructure |

The marketplace App composes directory, Web and existing Auth providers. Its
workers and database remain private directory implementation details. Source is maintained in the independent `lenso-marketplace` repository. Do not add the business implementation to
the Console Shell crate.

Role handoff before public contract publication:

| Role | Consumer / provider / cardinality | Required operations |
| --- | --- | --- |
| Directory read | Marketplace Web / directory / one | Search with cursor, get exact release, get publisher, get signed snapshot |
| Publisher workflow | Publisher Web / directory / one | Submit immutable revision, read validation/review result, publish approved revision, request yank |
| Target installation | Console Agent tools / selected target authority / one explicit target per operation | Inspect support/revision, prepare exact candidate, apply authorized proposal, read operation receipt |

Directory roles belong to a marketplace-owned contract package if independently
consumed. Target installation belongs to the configuration/install contract
owner, not the marketplace. Reuse existing roles where equivalent; names,
schemas and authentication mapping must be reviewed before assigning public
Capability IDs. No global target registry or private-table import is required.

## Records and security model

The release key is `(catalog identity, Plugin ID, exact version)`. An artifact adds
digest, media type, byte size, implementation and supported targets. The catalog
identity is anchored in locally configured trust, not whatever key accompanies a
network response. Same-ID entries from different catalogs stay visibly distinct.

Submission, review and publication records include actor, immutable revision,
timestamp, decision/reason and artifact references. Private credentials and App
configuration never appear in catalog records. Publisher-authored Markdown and
screenshots are untrusted display content, not executable page contributions.

Proposed signed snapshot envelope: schema version, catalog ID, monotonically
increasing revision, issued/expiry times, payload digest and signing key ID, with
signatures over canonical bytes. Canonical encoding, algorithm/key rotation,
bootstrap trust and cross-language fixtures are contract gates before coding a
production verifier; this document does not invent a second Bundle signature
format. Clients reject rollback to an older accepted revision and unknown keys.

A valid cached snapshot may support a new install within its validity policy.
Expired metadata remains visibly stale for browsing but cannot authorize a new
install/update. Existing installations continue running independently. Signing
and risk metadata cannot introduce a startup dependency on marketplace uptime.

Downloader policy covers allowed origins, redirects, resolved network addresses,
timeouts, size limits and extraction limits. Public submissions cannot make the
ingestion service fetch private/loopback resources. Target-side acquisition has
its own policy; private catalogs are not implicitly authorized by public trust.

Describe native/process/Bun execution truthfully. A capability list is not an OS
sandbox. Show the actual execution boundary and host access before approval.
Identity verification, artifact integrity, named review and official ownership
remain separate evidence fields.

## Install and update transaction

1. Resolve an exact signed release and select an explicit target identity.
2. Ask the target to prepare a candidate. It checks authority and catalog policy,
   acquires and verifies bytes, evaluates runtime/Slot/Capability compatibility,
   and validates configuration against the complete App.
3. Return a proposal bound to target ID, base revision, release/artifact digests,
   configuration digest or secret references, candidate effects and expiry.
   Missing providers produce named requirements; v1 never silently installs an
   arbitrary matching provider. Any additional installation needs review.
4. Review that proposal in Console Agent. It shows permission/configuration effects;
   lower-level digests are available in technical details. Secrets are entered
   through target-owned facilities and never posted to the marketplace.
5. Apply with proposal digest and idempotency key. Recheck target authorization,
   current revision and release admission. Target changes or expired proposals
   require preparation again. Never redirect an old approval to the newly
   selected App.
6. Persist a receipt, stage the candidate, and switch only after Ready. Read
   progress by receipt after disconnect/restart; a UI timeout is not proof of
   failure and must not trigger a second independent install.

Receipt phases: preparing, awaiting approval, applying, waiting for Ready,
succeeded, failed, or recovery required. Include base/candidate revision,
target identity, exact release, failure stage and safe next action. Only the
target writes authoritative operation outcomes. Cancellation before commit may
discard staging; after commit, reversal is a separate reviewed operation.

Updates use the same transaction with an explicit old/new comparison. V1 prompts
for every update; it does not silently expand authority. Runtime changes,
publisher transfers, new requirements and data migrations receive specific
explanations. A failed candidate must preserve the previous active Generation.
That is not a promise to reverse arbitrary external side effects or database
migrations: incompatible migrations block v1 update unless the owning Plugin
supplies an explicit safe strategy. Removal retains data; purge is a separate
Plugin-owned action.

## First end-to-end slice and delivery gates

Use one small first-party portable Plugin with an observable operation and no
irreversible data migration. Select its concrete package only after verifying
it packs and runs on the chosen target; do not relabel a linked Workspace as a
portable artifact. V1 supports a verified local Agent App target first.

The slice has directory instance `official-directory`, Web instance
`marketplace-web`, the existing Auth provider, and one target installation
authority. Compose them through declared roles and existing Host policy. The
target Plugin Root gets exactly the reviewed sample instance and configuration.
Public-site browsing has no target dependency; installing requires one target.

| Gate | Artifact / owner | Completion evidence |
| --- | --- | --- |
| 1. Contract and artifact | Marketplace schema/verifier fixtures; existing Bundle and target-contract owners review references | Exact sample Bundle runs on target; stable identities; no forked Bundle schema |
| 2. Publish and discover | Directory Plugin, persistence, curated review operation, signed snapshot, minimal publisher form and public browse/detail | Publisher submits release; reviewer publishes; independent client verifies and finds it |
| 3. Install | Marketplace Web contribution and target acquisition/proposal/receipt support | A fresh target downloads, verifies, reviews, reaches Ready and successfully invokes sample behavior |
| 4. Update and failure | Second sample release, receipt recovery and compatibility fixtures | Upgrade succeeds; stale target/digest mismatch/failed Ready are rejected without losing prior active behavior |
| 5. Console extensions | Packaged contribution runtime and Console-owned target authority | Original Observe tracer succeeds without rebuilding the installed Console |

Gates 1–4 constitute marketplace v1, not four independently complete products.
Gate 5 is a separate extension milestone. A public listing demo alone does not
prove installation. Exact dates require the artifact and target-contract spike.

Implementation routing: Capability authoring owns reviewed role contracts;
Plugin authoring owns directory/Web behavior; App configuration owns declared
instances; runtime extension owns only missing acquisition/lifecycle mechanisms.
Real remote-artifact and real-target acceptance are required across those seams.

Focused failure proofs: unauthorized publisher cannot claim a namespace; artifact
mutation after approval fails; untrusted/expired/rolled-back snapshots cannot
authorize installation; App revision/identity changes reject stale approval;
disconnect/retry yields one operation; failed Ready preserves old behavior;
offline marketplace does not stop installed behavior; removing marketplace UI
leaves installed Plugins intact. Test each at its authoritative boundary.

## Explicitly later

Ratings, payments, recommendations, popularity telemetry, publisher billing,
automated review decisions, private-catalog administration UI, automatic updates,
multi-target atomic installation, and general native hot loading are outside v1.
The trust format preserves catalog provenance so private sources can be added
later without replacing release identity or installation authority.

## Installation through Console Agent

Marketplace is a discovery and release presentation surface. It has no Agent
connections, control credentials, target selector or installation proxy.
The user requests installation in Console Agent, which uses the existing
`list_available_plugins`, `check_plugin_install` and `apply_plugin_install`
tools with an explicit target and exact catalog entry. Review and authorization
belong to that Agent conversation; the target owns verification and execution.

Console Agent now queries the original signed envelope at
`/api/marketplace/v1/snapshot`, verifies it against its own configured keys and
retained checkpoint, then acquires and checks the exact archive. It prepares and
applies through its existing tools. `get_plugin_installation` reads the retained
result using the reviewed proposal digest. Only runtime activation proves success.
The real tool-provider acceptance covers network metadata, expired/wrong reviews,
installation, invocation, update, restart, failed activation and removal. Its
archive bytes are preverified test cache entries; HTTPS acquisition is tested
separately. No public registry availability is implied.

### Source-cohort validation and release order

Verified archive acquisition and signed catalog verification consume unpublished
source APIs. Release canonical Bundle JSON identity first, then the shared CLI
verification APIs, then update Agent and Marketplace consumers. Local Cargo
patches do not establish registry availability. No package release is claimed.
