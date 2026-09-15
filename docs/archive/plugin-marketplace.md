# Console Plugin marketplace

Status: identity and artifact contract design; marketplace UI and service not yet shipped.

The [marketplace v1 implementation proposal](plugin-marketplace-v1.md) develops
the public website, publisher submission/review, target installation transaction,
and delivery gates. It proposes a portable App Plugin as an earlier delivery
slice while retaining the Observe scenario below for Console extensions.

## Decision

Lenso should maintain an official directory for discovery and risk context, but
the directory is not the only source-code host, artifact host, or installation
authority. GitHub is a preferred public development source, not a package
identity system.

The marketplace answers **what exists, who claims it, what it requires, where
its immutable artifacts are, and whether a risk notice applies**. The target
Host independently decides whether an exact release may be downloaded,
verified, installed, configured, and executed.

## Separate identities

| Identity | Meaning |
| --- | --- |
| Plugin ID | Stable product contract namespace, for example `com.example.projects` |
| Release | Exact semantic version of one Plugin contract |
| Publisher | Directory identity making the release claim |
| Source | Human-reviewable repository and exact source revision |
| Artifact | Immutable Bundle bytes identified by media type, size, target, and digest |
| Catalog entry | Directory metadata about one release; not executable authority |
| Installation target | One explicit App or Console extension authority |
| Instance | Target-local configured selection of a release |

A repository rename does not rename a Plugin. A mutable branch/tag is not an
artifact identity. A publisher may provide multiple runtime/host-target
implementations under the existing Bundle contract; the Host still resolves
one exact implementation before constructing the Plan.

## Catalog release record

The first signed catalog schema records:

- schema version, Plugin ID, release version, title, summary, categories;
- publisher ID, display name, identity-verification state, and support URL;
- source repository URL, source revision, license identifier, documentation URL;
- one or more immutable Bundle locations plus digest, size, and media type;
- root Slot/install target (`app` roles or `console-workspaces`), runtime profile,
  host targets, and Console Workspace API compatibility when applicable;
- provided/required Capability IDs and descriptor versions derived from the Bundle;
- human-readable permission/risk explanations tied to machine-checkable requirements;
- publication time, superseded/yanked state, and security/advisory references.

The catalog does not duplicate the executable Plugin descriptor as hand-edited
truth. On ingestion, the directory verifies the Bundle and derives contract,
implementation, Capability, target, and digest facts. Publisher-authored prose
cannot override them.

## Trust states

The UI renders independent facts, never one ambiguous trusted badge:

- **identity verified**: control of the publisher identity was verified;
- **source linked**: the claimed repository/revision is reachable and corresponds
  to submitted provenance;
- **artifact verified**: bytes match the recorded digest and Bundle structure;
- **reviewed**: a named review policy and review revision completed;
- **official**: maintained by the Lenso project;
- **advisory**: a version-specific risk or vulnerability notice exists;
- **yanked**: excluded from new selection but retained for reproducibility;
- **revoked**: known unsafe; target policy decides blocking and remediation.

Official, reviewed, verified, and safe are not synonyms. Catalog inclusion never
grants runtime permissions or business authorization.

## Distribution

The official directory publishes a signed, versioned snapshot and bounded
incremental updates. Artifact URLs may point to approved HTTPS or OCI
distribution, including third-party and organization-private origins. Every
download is admitted by expected digest and size before Bundle verification.

Installed Apps pin the selected release, implementation, and artifact digest.
They continue to run when the marketplace is offline. Cached catalog data is
marked with its snapshot time and signature state. Installation never resolves
`latest`, a branch, or a mutable tag at execution time.

Private organizations may configure additional signed catalogs. Search results
preserve catalog provenance and do not merge same-ID releases silently. Direct
Bundle installation remains possible only through an explicit trusted-package
authority and the same review screen; it is not mislabeled as marketplace-listed.

## Installation targets and review

Installation is requested in Console Agent through its Plugin management tools.
Marketplace only discovers and presents releases; it does not connect to Agents,
accept control credentials or proxy installation. The review below belongs to
the Console Agent workflow.


The user selects the target before review:

- **App Plugin** changes one App's Plugin Root and capabilities;
- **Console extension** changes Console's own Plugin Root and may contribute Workspaces;
- **companion product** is two explicit proposed installations, never one implicit
  cross-target mutation.

The review screen shows exact release/digest, source and publisher, target,
root Slot, implementations, capabilities, Workspace API range, permission/risk
explanations, configuration fields, dependencies, advisories, and update policy.
Approval authorizes only the proposed target and exact candidate. The Host
re-resolves the complete candidate App, stages it, and switches only after its
Generation reaches Ready. Failure preserves the previous active Generation and
returns a durable operation receipt from the target authority.

Disablement removes selection/routing but retains data. Removal is recoverable
by default and has a separate explicit data-purge action owned by the Plugin.

## Update and withdrawal

Automatic discovery of an update is not authority to install it. Update policy
may propose compatible releases, but any new Capability, permission class,
publisher, source, runtime profile, root Slot, or target requires renewed review.
Artifact digest changes without a release identity change are rejected.

Yanking prevents new installs while preserving existing lock resolution.
Revocation is a signed high-priority notice. The UI identifies affected targets
and offers an explicit disable, remove, or reviewed replacement action; the
marketplace cannot silently mutate Apps.

## First tracer

The first marketplace tracer uses the first-party Observe release:

1. fetch and verify a signed official catalog snapshot;
2. open Observe details and distinguish official, artifact-verified, and reviewed facts;
3. choose **Console extension** as the target;
4. download an immutable Bundle by digest and run existing Bundle verification;
5. show the complete candidate and obtain installation approval;
6. publish through Console's configuration/package authorities;
7. wait for Ready, then show the Observe Workspace in the far-left rail;
8. disable it and prove the Workspace disappears while retained telemetry data remains;
9. operate the installed Workspace while the catalog is unavailable.

This tracer cannot ship until Console has its own durable Plugin Root authority
and a runtime path for the selected packaged implementation. The current linked
Workspace Host proves contribution composition, not marketplace installation.

## Contract ownership and implementation handoff

The catalog schema belongs to the marketplace product; Bundle and Plugin
contract schemas remain owned by the packaging/framework repositories. Before
publication, owners must review how catalog records reference the current V4
Bundle rather than forking it.

Concrete first artifacts:

- signed catalog snapshot and signature-envelope schemas with canonical bytes;
- publisher/source/provenance and advisory schemas;
- a read-only catalog client with bounded cache and offline behavior;
- Console search/detail/install-review Workspace;
- an adapter from verified catalog artifacts into the existing trusted Bundle
  installation proposal, not a second installer;
- conformance fixtures for digest mismatch, ID/version mismatch, ambiguous
  catalogs, stale signatures, yanks, revocations, incompatible targets, and
  offline installed operation.

Do not publish a generic `lenso.marketplace.*` Capability until the directory
service boundary and authentication model are concrete. The first local fixture
may be a signed static catalog; its schema and verifier must be production-shaped.

## Deferred

- ratings, rankings, payments, recommendations, and dependency popularity;
- mandatory central artifact hosting or mandatory GitHub source;
- silent updates or global kill switches;
- multi-user organization billing and publisher self-service portals.

Release artwork and usage instructions are specified in [Marketplace release presentation](plugin-marketplace-presentation.md).
