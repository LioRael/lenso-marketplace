# Web Ingress 0.4.5 directory candidate

This is author metadata for an exact linked Cargo release, not a signed Directory entry or an installable portable Plugin. The package is a Host build input. `host_provided` means a product-specific Host adapter must supply the Web Ingress factory; generic `lenso app add` must not offer this release.

The `lenso-web-ingress-plugin-v0.4.5` source tag points to `LioRael/lenso-web` commit `0e93f1149ac1b0905a51d76692d369a028f3a532`. The 0.4.5 registry crate's SHA-256 is `76cf4784b0706b269b0a3d475aafca181853c1c1085f0bc3715d93facbedad36`, matching the crates.io version checksum. The versioned documentation points to the README at that commit; the digest in `metadata.json` is for those exact Markdown bytes. Current development has moved to `LioRael/lenso` under `crates/lenso-web-ingress-plugin`; this historical commit remains the proposed source identity for 0.4.5 pending operator provenance review.

On an Apple Silicon Host with the exact registry archive, prepare and check a local submission:

```sh
lenso-marketplace-author prepare-linked-cargo \
  lenso-web-ingress-plugin-0.4.5.crate \
  candidates/linked-cargo/lenso.web-ingress/0.4.5/metadata.json \
  /path/to/new-submission-directory
lenso-marketplace-author check-linked-cargo /path/to/new-submission-directory
```

The operator must independently review namespace ownership, source provenance, Host adapter behavior and documentation bytes before signing or publishing. No production catalog write is implied by this candidate.
