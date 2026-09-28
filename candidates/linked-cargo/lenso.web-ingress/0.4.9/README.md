# Web Ingress 0.4.9 directory candidate

This author metadata is for the exact linked Cargo release from the consolidated
`LioRael/lenso` repository. It is not a signed Directory entry, a portable
Plugin, or proof of public catalog availability. The `.crate` is a Host build
input. Its `host_provided` integration requires a product-specific Host adapter;
generic `lenso app add` must not offer it as an installable linked Plugin.

The `lenso-web-ingress-plugin-v0.4.9` tag resolves to commit
`954a00b89b095f20fafb68071b7252d5d2c2e67b`. The exact public registry
archive has SHA-256
`942631263f7e574e3f6b7fcce73e797e4d2c8edf52428889ca8bfbe7d41032fc`.
The documentation reference points to `docs/components/web.md` at that same
commit; its digest and size in `metadata.json` match those exact Markdown bytes.
The older 0.4.5 candidate remains a separate historical release proposal.

With the exact registry archive, prepare and check a local submission:

```sh
lenso-marketplace-author prepare-linked-cargo \
  lenso-web-ingress-plugin-0.4.9.crate \
  candidates/linked-cargo/lenso.web-ingress/0.4.9/metadata.json \
  /path/to/new-submission-directory
lenso-marketplace-author check-linked-cargo /path/to/new-submission-directory
```

An operator must independently review namespace ownership, source provenance,
Host adapter behavior, supported targets, and documentation bytes before
signing or publishing. No production catalog write is implied by this
candidate.
