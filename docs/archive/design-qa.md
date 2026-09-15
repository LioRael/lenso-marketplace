# Catalog integration review

Acceptance is Catalog adopting the existing Lenso UI design system and components.
It is not image reproduction and not an extension to the Lenso UI library.

Catalog page composition remains local in `web/ui`.
Existing library components and semantic tokens provide controls and styling.
The earlier Recipe, registry, documentation and tests added to Lenso UI were reverted.
See `ui-review.md` for the current boundary and validation scope.

## Release presentation verification — 2026-09-13

- Accepted compact typography, 56px header, 56px artwork and 96px catalog rows remain unchanged. The current sample catalog was checked in the in-app browser after rebuilding the embedded frontend.
- New optional publisher presentation travels through catalog validation, signature verification and the existing list/detail API. The real Echo fixture was submitted, approved and published to a fresh local proof database; its getting-started text was verified in the running UI.
- Thirteen catalog/cache/domain tests pass. The separate real-bundle acceptance test passes with the existing packed Echo archive; it covers presentation after database restart. Rust-to-Node signature conformance passes.
- TypeScript/Vite build and Oxlint pass. The full browser regression passes, including intercepted publisher media, missing-image fallbacks, literal HTML-like text, narrow screens and existing catalog interactions. Publisher media tests use intercepted responses, not production image hosting.
- React Doctor's changed-scope command scanned zero files because this feature is still untracked; its score service was also unreachable. No health-score claim is made. Type checking, lint, domain tests and rendered browser checks provide the verification above.
- Local preview: port 63729. Proof data and screenshots: `/tmp/lenso-marketplace-presentation-proof`. Existing proof data was preserved. This is a local development fixture, not a public marketplace publication.

## Installation ownership correction (2026-09-13)

The Marketplace-to-Agent installation experiment was rejected and removed.
The accepted compact catalog layout remains; connection controls, credentials,
installation panels, configured Agent targets and forwarding routes are gone.
The Agent experiment is test-only and has no production HTTP routes.

Its prior archive, exact identity, candidate activation and failure tests remain
useful component evidence. They do not prove the intended Console Agent tool
workflow. The subsequent Console Agent tool acceptance now proves signed metadata over
HTTP, exact installation and invocation, retry/restart receipts, upgrade, failed
activation and removal. Runtime fixes cover canonical JSON and unique Process
wire correlation IDs. This remains a development source cohort, not a registry
release. Marketplace's real-Host test verifies the original signed snapshot and
confirms that Agent connection/mutation proxy routes return 404.
