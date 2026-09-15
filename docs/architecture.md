# Architecture

## Product and ownership

Marketplace publishes and displays Plugin releases. It does not install them.
Console Agent selects a target, verifies current signed metadata and artifacts,
and owns authorization and installation. The shared `lenso-plugin-catalog`
package owns wire format and signature verification; Marketplace imports it
directly, without a local protocol facade.

| Owner | Owned state and lifecycle | Collaboration |
| --- | --- | --- |
| Directory Plugin | Namespace ownership, immutable submissions, reviews, audit and publication; SQLite handles close with native instance | Provides Directory read Capability; event host injects only `PublishedStorage` |
| Web Plugin | Search/detail, static UI, verified browsing checkpoint and expiry notices; cache belongs to consumer instance/storage binding | Requires Directory Capability; event host injects only accepted-cache storage, clock and diagnostics |
| Native App | Plugin Root, host catalog, binding and HTTP ingress | Composes the two Plugins using generated factories |
| Workers App | Event host, runtime admission and D1/R2 transport | Implements each Plugin's own storage port separately; does not decide signatures or catalog admission |
| Publisher CLI | Operator configuration and stdin/key boundary | Calls Directory-owned publishing API; no HTTP route and no Web dependency |
| Cloudflare tools | Upload immutable publication and advance conditional pointer | Calls the strict Rust verifier before promotion |

The Directory Capability returns the signed publication. Directory never imports
Web caches, and Web never imports Directory publishing/database code. Their
private storage ports are distinct: published reads cannot accept or overwrite a
Web checkpoint. Workers happens to adapt both ports to D1/R2 in one host.

## Three flows

1. **Publish:** operator -> Directory publishing model -> signed envelope ->
   native publication database or private Cloudflare promotion -> published pointer.
2. **Browse:** HTTP ingress -> Web -> bound Directory Capability -> signature and
   history verification -> Web checkpoint/cache -> search/detail response. Expiry
   is shown explicitly; integrity failures are rejected.
3. **Install:** Console Agent -> original signed snapshot -> strict current-time,
   release availability and artifact verification -> target-owned installation.
   A successful browse response is never an installation grant.

## Composition and removal

Native and Workers compose the same Web/Directory Plugin identities. Removing
Web removes Marketplace UI/HTTP contributions; it does not uninstall target Apps.
Directory requires an explicit binding; no fallback to another directory is
introduced. Cloudflare storage adapters are host mechanics, not new business
Plugins. Native-only SQLite publishing is feature-gated out of the Workers graph.

## Build boundary

The root workspace pins common host dependencies and selects native packages by
default. Build Workers with `-p lenso-marketplace-workers-host --target
wasm32-unknown-unknown`; do not enable native feature sets for the whole workspace
on that target. The two executable test Plugin projects stay excluded because
they exercise external Plugin packaging rather than application composition.

Historical G-stage receipts and recovery experiments live under `docs/archive`.
They retain original paths/digests as evidence. Current commands live in README,
operations and package scripts; archive procedures are not production defaults.
