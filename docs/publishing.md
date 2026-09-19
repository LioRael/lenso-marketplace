# Publish a plugin

Authors prepare a verified submission without a Marketplace private key, operator
configuration or database. A maintainer verifies namespace ownership and reviews
the exact release before publishing it. Submission transport is currently a
maintainer handoff, not a public upload API or self-service account system.

## Install the preparation tool

From a checkout of this repository (Rust 1.94):

```sh
cargo install --locked --path tools/publisher --bin lenso-marketplace-author
```

This tool is not yet published to a package registry. Use the released `lenso` CLI
for checking and packaging your Plugin repository.

## Check and package

```sh
lenso plugin check --repo-root ./my-plugin
lenso plugin pack --repo-root ./my-plugin --output ./plugin.lenso-plugin
```

The package owns the Plugin ID, exact version and executable metadata. Preparation
derives these from the verified bundle; they cannot be overridden by display JSON.
Use a clean source revision and an immutable artifact URL agreed with the maintainer.
The preparation tool never fetches or uploads that URL.

Create `release-metadata.json`:

```json
{
  "publisher_id": "example",
  "title": "My Plugin",
  "summary": "Describe the task this plugin performs.",
  "description": "Requirements and usage instructions.",
  "license": "MIT",
  "source_url": "https://github.com/example/my-plugin",
  "source_revision": "0123456789abcdef0123456789abcdef01234567",
  "artifact_url": "https://example.com/releases/my-plugin/1.0.0/plugin.lenso-plugin"
}
```

Replace all example values. Optional `presentation` supports the existing signed
icon, screenshots and getting-started metadata.

## Prepare and check the submission

```sh
lenso-marketplace-author prepare ./plugin.lenso-plugin ./release-metadata.json ./submission-1.0.0
lenso-marketplace-author check ./submission-1.0.0
```

The new directory contains `release.json` and `plugin.lenso-plugin`. Preparation
checks the bounded archive and complete bundle and derives archive size, SHA-256
and manifest digest. Existing output directories are refused. `check` detects
changes to the archive or mismatched release identity. Neither command executes
the plugin, publishes it or establishes publisher ownership.

Give both files to the catalog maintainer through your agreed contribution channel,
along with source and namespace ownership evidence. No private signing key is
needed from the author. The maintainer returns a submission ID and review state;
`awaiting_review` and `approved` do not mean the release is publicly available.

## Update a plugin

Increment the version in the Plugin source, commit it, and repeat check, pack and
prepare into a new submission directory with the new source revision and immutable
artifact URL. Keep the same Plugin ID and publisher. An exact retry returns the
existing submission; changed bytes or metadata cannot replace an existing version.
Publish a new version to correct an already submitted release.

The maintainer imports, reviews, publishes and verifies each update using the
[operator guide](../tools/publisher/docs/operator.md). Public browse defaults to the
latest listed stable version; old versions remain reachable from release details.
Users install or update through Console Agent tools.
