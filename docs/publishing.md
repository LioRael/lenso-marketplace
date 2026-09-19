# Publish a plugin

Authors prepare a verified submission without a Marketplace private key, operator
configuration or database. A maintainer verifies namespace ownership and reviews
the exact release before publishing it. Submission transport is currently a
GitHub Issue review, not a public upload API or self-service Marketplace account system.

## Install the preparation tool

Prebuilt downloads are produced by the `author-v*` release workflow for macOS
Apple Silicon and Linux x64 (Ubuntu 24.04 or newer). The first distribution tag is
`author-v0.1.0`; the commands below require that release to be published. Check the
[release page](https://github.com/LioRael/lenso-marketplace/releases) for availability.

With GitHub CLI installed:

```sh
version=author-v0.1.0
archive="lenso-marketplace-author-$(uname -s)-$(uname -m).tar.gz"
download_dir="$(mktemp -d)"
gh release download "$version" --repo LioRael/lenso-marketplace \
  --pattern "$archive" --pattern "$archive.sha256" --dir "$download_dir"
(cd "$download_dir" && shasum -a 256 -c "$archive.sha256" && tar -xzf "$archive")
mkdir -p "$HOME/.local/bin"
install -m 755 "$download_dir/lenso-marketplace-author" "$HOME/.local/bin/lenso-marketplace-author"
"$HOME/.local/bin/lenso-marketplace-author" --version
```

Run the install only after download and checksum verification succeed. On Linux,
`sha256sum -c` can replace `shasum -a 256 -c`. Ensure `$HOME/.local/bin` is on PATH.
Checksums detect changed downloads; they are not a separate code-signing system.
macOS builds are not notarized; do not disable Gatekeeper globally to install one.
Intel macOS and other platforms currently require a source build (Rust 1.94):

```sh
cargo install --locked --path tools/publisher --bin lenso-marketplace-author
```

Use the released `lenso` CLI for checking and packaging your Plugin repository.

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

## Submit and track review

Host both prepared files at public immutable HTTPS URLs, for example assets of a
versioned GitHub Release in your plugin repository. The archive URL must match the
one in `release-metadata.json`; prepare again into a new directory if it changes.

```sh
lenso-marketplace-author submission-url ./submission-1.0.0
```

Open the printed URL to the [plugin submission form](https://github.com/LioRael/lenso-marketplace/issues/new?template=plugin-submission.yml).
Identity, source and archive digest are prefilled from the verified submission.
Add the immutable `release.json` URL and namespace ownership evidence, then submit.
The command does not open a browser or send an issue. A GitHub account is required
to file and follow the issue; no separate Marketplace account or private key is needed.

The issue is the public tracking record. A maintainer replies with the internal
submission ID and review state: received, needs changes, approved, then published
with an exact release URL and catalog revision. Approval is not publication. A
closed issue can also mean declined or withdrawn; consult its recorded outcome.
Ownership evidence is reviewed independently; a GitHub display name is not a
publisher identity. Existing namespace authorization is still checked on import.

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
