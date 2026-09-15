# Released Workers dependency adoption

The Workers host consumes the registry versions of Native Adapter 0.3.14,
Workers Driver 0.1.0 and Web Ingress 0.4.3. The shared HTTP and WebSocket
Capability crates resolve from crates.io once; no Runtime or Web Git patches
remain in this host workspace. Marketplace owner crates remain local private
implementations. The CLI protocol dependency retains its existing immutable pin.

CI installs npm Runtime 0.1.1 from the frozen lockfile, builds the UI, checks the
Wasm composition and storage boundaries, then builds and bundles an actual
Workers artifact. It no longer installs Runtime from a sibling Git checkout.
The exact fresh owner release is exempted from the package-age delay only in
this isolated workspace, after registry/archive integrity verification.

Wrangler dry runs use the existing proof configuration for packaging validation.
They do not create a production deployment or qualify production trust. The
production publication operator, key custody, renewal and rollout inputs remain
separate launch requirements in the G5 runbook.
