# G5 production preflight

Status: preparation, not production qualification or deployment authorization.
Read-only inventory taken on 2026-09-14 UTC. Recheck mutable deployment and gate
evidence immediately before rollout. See [the runbook](g5-rollout.md).

## Recommended first production scope

Deploy the existing Marketplace Directory and Web Plugins as a separate,
public-read Workers App. Recommend `marketplace.lenso.dev` and a distinct
`lenso-marketplace` Worker as **proposed names**, pending the domain owner's
confirmation and conflict check. Keep `catalog.lenso.dev`, the `lenso-catalog`
Worker, and the existing `/v1/plugins.json` client contract with their current
owner. Do not point the catalog hostname at the G3 Worker or replace the old
catalog envelope with a signed Marketplace envelope.

The first deployment can serve a deliberately reviewed empty signed catalog.
It must not label the Echo proof release or synthetic publisher/source metadata
as official. Real releases require the same archive admission and curated review
as any later publication. Signing stays in a controlled publisher operation;
public ingress receives only public trust configuration and storage bindings.

## Verified existing service

| Item | Observation | Evidence |
| --- | --- | --- |
| Repository | `LioRael/lenso-catalog-worker`, current `origin/main` `3328775aa3827d6f574b75eb85c3842d59a8c625`, 2026-09-02 | [Commit](https://github.com/LioRael/lenso-catalog-worker/commit/3328775aa3827d6f574b75eb85c3842d59a8c625) |
| Source ownership | Reviewed `catalogs/lenso-official-plugin-catalog.json` is authoritative; `schema: lenso.plugin-catalog.v1`, `plugins: []` | [Catalog](https://github.com/LioRael/lenso-catalog-worker/blob/3328775aa3827d6f574b75eb85c3842d59a8c625/catalogs/lenso-official-plugin-catalog.json), [README](https://github.com/LioRael/lenso-catalog-worker/blob/3328775aa3827d6f574b75eb85c3842d59a8c625/README.md) |
| Domain ownership | Cloudflare account `LioRael`; active zone `lenso.dev`; Custom Domain `catalog.lenso.dev` belongs to service `lenso-catalog`, production environment | Read-only Cloudflare account, zone and Workers-domain APIs |
| Active deployment | Deployment `52e65757-c540-4491-9e3d-39b924216a0a`; version `38dfe7d7-fbe1-4021-a8f0-2c58a2c38889`, 100%, created 2026-07-10T13:07:01.886592Z | Read-only `/accounts/{account}/workers/scripts/lenso-catalog/deployments` |
| Existing runtime config | Compatibility date `2026-07-03`, `nodejs_compat`, workers.dev enabled, preview URLs disabled, logs enabled; no bindings | Read-only settings/subdomain APIs and [source config](https://github.com/LioRael/lenso-catalog-worker/blob/3328775aa3827d6f574b75eb85c3842d59a8c625/wrangler.jsonc) |
| Live behavior | `/` advertises `/v1/modules.json` and 12 Modules; `/healthz` returns `ok: true`, `catalogVersion: 1`, `modules: 12`; `/v1/plugins.json` returns 404 `catalog.not_found` | Direct HTTPS reads, 2026-09-14 around 15:42–15:45 UTC |
| Repository CI | Quality and daily Bundle verification workflows exist; neither deploys or signs. Latest five scheduled runs failed | [Workflow files](https://github.com/LioRael/lenso-catalog-worker/tree/3328775aa3827d6f574b75eb85c3842d59a8c625/.github/workflows) |
| Exact latest CI failure | The no-downloadable-release check passed; upstream check failed because the admission archive reader differs from `lenso-cli main` | [Run 34825046268](https://github.com/LioRael/lenso-catalog-worker/actions/runs/34825046268) |
| Release environment | GitHub API returned zero repository Actions secrets and zero environments; README documents manual `pnpm deploy` | Read-only GitHub metadata; this does not inventory account-level or external secret custody |

The local catalog checkout was older than `origin/main`; the table uses GitHub
source at the exact current commit, not that checkout. The live Module service
is deployment drift, not evidence that current Plugin routes work. Repairing
that deployment and its upstream-contract check is a separate catalog-owner
change with its own compatibility checks. It must not be hidden in a Marketplace
domain switch. Preserve the observed baseline while preparing that correction.

Source-owned catalog endpoints are `/`, `/healthz`, `/v1/plugins.json` and
`/v1/plugins/:pluginId`, including exact `pluginId`/`version` and search filters.
Current source returns 410 for retired Module routes. Those source semantics and
the older deployed behavior must both be recorded before any catalog deployment.

## Existing Marketplace implementation and proof

The [Directory publisher](../../catalog/src/directory.rs) owns namespace claims,
immutable submissions, digest-bound review, transactional revision allocation,
signed bytes and audit records in private SQLite. Its `publish` method receives
an authorized actor, expected revision, timestamps, key ID and signing key. The
read-only Directory projection never initializes publisher state or receives a
private key. Actor strings in a fixture are not a production authentication path.

The [Workers adapter](../storage.mjs) stores a publication pointer in D1 and exact
signed bytes in R2. The Web consumer separately persists an accepted checkpoint
and envelope as one content-addressed R2 object, fenced by D1 compare-and-swap.
The accepted history includes identities omitted from the current snapshot.
Only shared Rust verifies signatures and release policy. Private schemas remain
owner implementation details; there is no public SQL or publishing endpoint.

Only `lenso-marketplace-g3-proof` matched Marketplace names in the accessible
Worker, D1 and R2 inventories. Its D1 UUID is
`2b913921-3f0a-43b4-ae8f-cb219c21db81`; its R2 bucket has the same proof name.
This name-filtered inventory does not prove that no differently named resources
exist. No production resources were identified by an owner.

The [proof config](../wrangler.jsonc) uses catalog ID `workers-g3-proof`, key ID
`test-key`, a deterministic test public key, and public proof diagnostics. These
are not production inputs. The [smoke script](../proof/smoke.mjs) deletes proof
rows and intentionally overwrites objects to test corruption; **never run it
against production or adapt its unconditional publication SQL for operations**.

Marketplace-owned HTTP routes are:

- `/`, `/marketplace.js`, `/marketplace.css`, and `/sample-assets/{name}`;
- `/api/marketplace/v1/snapshot` for exact stored signed-envelope bytes;
- `/api/marketplace/v1/plugins` for verified bounded search;
- `/api/marketplace/v1/plugins/{plugin_id}/{version}` for verified exact details.

Raw snapshot transport success does not establish trust or freshness. Acceptance
must include verification and the search/detail path. The first rollout exposes
no authenticated publishing, installer, or Console administration API.

## Gate evidence after qualification

| Gate | Established context | Still required for final acceptance |
| --- | --- | --- |
| G1 | Runtime owner records bounded reference qualification in `docs/evidence/workers-g1/acceptance.md`, including real cancellation, faults, memory replacement and measured reference workloads | Preserve those invariants in the exact deployed Marketplace cohort; reference measurements are not Marketplace capacity or an SLA |
| G2 | Shared bridge and deployed 30-vector Fetch corpus pass; 27 external network receipts pass with three recorded edge interceptions. Native regressions and seven shared boundary tests pass | External upload buffering, local disconnect propagation and intermittent local post-413 proxy stalls remain explicit transport limits; see Runtime PR #145 |
| G3 | [Final clean-build evidence](../evidence/README.md): 127 local and 127 remote public-read checks, same fixtures through native SQLite, real native Host/browser regression, real D1 CAS winner/stale-writer checks, and independent archive verification. Deployed version `60c5f58b-77af-41e1-bdea-0bd6837ec8ce` | Bounded Base64 resolved the large-response allocation failure. The one-active-event, 16-admission retirement profile is a qualified proof profile, not a production SLA or total-isolate peak-memory claim |
| G4 | Auth PR #103 records 59 real Worker checks for Account/OAuth Flow/Router/WebSession with actual controlled OIDC and Fetch; native workspace 77 tests, zero ignored. Extracted owner archives also compile for Workers | External IdPs and Password/Phone/Device/API Token/OIDC Provider remain unqualified. Final JS lifecycle correction is deployed as `97296988-1c51-4c72-91d3-c56385ab76ca`, with eight local lifecycle/package tests |
| G5 | Additive domain recommendation, explicit migration, frozen public artifact, verified source cohort, configuration renderer and rollback/observability runbook delivered | Production trust, private publication operation and custody remain unresolved. A separate consumer-state recovery rehearsal does not establish publisher backup or production RPO/RTO |

Proof URLs: `https://lenso-workers-g2-proof.lenso.workers.dev`,
`https://lenso-marketplace-g3-proof.lenso.workers.dev`, and
`https://lenso-workers-g4-proof.lenso.workers.dev`. The G4 proof uses a private
proof request guard. None is a production identity or service guarantee.
The linked receipts preserve exact deployment identities and limitations.
Production configuration must be reviewed separately before a domain switch.
Event ingress request IDs currently restart with each request App; they prove
replacement of untrusted IDs, not global request-ID uniqueness. Account for that
scope when choosing operational correlation fields.
Cloudflare's current [memory limit](https://developers.cloudflare.com/workers/platform/limits/#memory)
is 128 MB per isolate including JavaScript and Wasm. Even successful local status
checks cannot qualify a workload whose retained capacity exceeds that budget.

## Required production inputs

Every blank requires an owner-supplied value or an implemented operation. Existing
OAuth access proves tool access, not signing custody or production authority.

| Input / decision | Existing value | Production requirement |
| --- | --- | --- |
| Domain and Worker | `catalog.lenso.dev` is occupied; `marketplace.lenso.dev` is only proposed | Domain owner approves an additive hostname and Worker name. DNS lookup for the proposed hostname returned API 403, so availability is unverified |
| Deployment owner | Repository and Cloudflare account owners are known | Name operator, reviewer, rollback operator, release environment and bounded deployment credentials; do not use a personal OAuth session as unattended CI design |
| Catalog identity and trust | Only deterministic proof identity/key found | Approved stable catalog ID, key ID, public key/fingerprint, out-of-band consumer bootstrap, private-key custodian and audited signing procedure; never generate or import a key silently |
| Rotation/revocation | One key ID/public key in current Web configuration | Document coordinated trust update and incident procedure preserving checkpoints. Overlapping keys would need a reviewed configuration change; do not assume transparent rotation exists |
| Authoritative publication | Legacy deployed Module catalog; current reviewed Plugin catalog empty; fixture-only signed publications | Identify actual publisher database/export and review authority, or explicitly approve a new empty catalog. Do not convert the 12 Module entries into Plugin Releases |
| Production storage | Only named G3 proof resources identified | Separate D1 ID, private R2 bucket, binding map, approved location/retention, protection against overwrite/delete, budget owner and migration receipt |
| Publication operation | Private Rust domain exists; proof SQL is destructive test setup | Operator entrypoint enforcing review, immutable upload and conditional pointer publication, with receipt/reconciliation. No such production command or automation was established by this inventory |
| Expiry renewal | Protocol permits at most seven days validity | Named operator, schedule, validity interval, alert lead time, failed-renewal escalation and key-unavailable procedure. Renewal must publish a higher revision through the owner |
| Artifact cohort | [Pinned cohort](../evidence/cohort.json) and [frozen artifact](../evidence/artifact.json) verified; exact owner commits are available remotely | Preserve and qualify the final production configuration with the frozen bytes; experimental cohort validation is not registry publication or an unrestricted production support claim |
| Recovery | [Isolated consumer recovery](../recovery/RESULTS.md) passed six live checks and five durable observations: paired revision-8 restoration, rollback rejection, corruption rejection and exact recovery; source unchanged | Private publisher/audit backup, production coherent snapshot procedure, retention and RPO/RTO decisions remain required; retain the highest known history across recovery |
| Observability and capacity | Proof diagnostics; experimental per-event limits | Deployment/version identity in private logs, HTTP failure/freshness alerts, CPU/memory/storage measurements, approved admission and cost limits, no secrets or raw credentials in logs |
| Final action | No production mutation performed | Reviewable manifest of hostname, Worker version, resource IDs, migration, public-key fingerprint, publication revision/digest/expiry, monitoring and rollback target |

Public reads require G1–G3 evidence and these operational inputs. G4 qualifies
a representative Auth composition; it does not add Auth to anonymous browse
or establish an authenticated publishing service. Publisher self-service,
namespace administration UI, submission revisions, remote installation and
Console contribution are optional later product work. A controlled signing,
renewal and publication operation, safe recovery and current signed content are
mandatory even when that UI is deferred.
