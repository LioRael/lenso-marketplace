# Workers package bootstrap and historical cohort

[`proof/cohort.py`](../proof/cohort.py) recreates the historical sibling layout used
by the initial Workers experiments. It fetches exact committed Git objects and
checks out detached HEADs; it never copies a developer's working tree, builds,
deploys, publishes, creates keys, or reads credentials. Git may use the caller's
existing credential helper when fetching private repositories; credentials are
not placed in the manifest, receipt or error output.

This is a reproducible experimental bootstrap, not a production dependency
package, release workflow or qualification receipt. The relative paths and
experimental generated-factory linkage apply only to that historical pinned cohort.
The tool does not replace published contracts or make uncommitted changes usable.

## Current package integration

The Host now imports `@lenso/workers-runtime` and the `lenso-workers-driver` crate.
Its factory wiring uses public package `link_plugin()` and explicit
`with_factory_override`. Current builds do not import experimental Runtime source
or depend on sibling worktree names.

Before the new packages are published, install reviewed npm archives explicitly:

```sh
npm install --no-save --package-lock=false /absolute/path/to/lenso-workers-runtime-0.1.0.tgz
CARGO=/absolute/path/to/lenso-cargo bash build.sh --config /absolute/path/to/cohort.toml
```

The Cargo config supplies `[patch.crates-io]` entries for the reviewed unpublished
Runtime and Web crates. Paths are supplied by the caller and may point to extracted
archives; the build entry does not infer a checkout layout. Once the cohort is
released, consume published versions and regenerate registry-backed lockfiles.
Do not commit a lockfile pointing to temporary local archives.

The shared event scope owns R2/D1 cancellation settlement and Wasm fencing. The
Marketplace still owns its business storage operations; six storage/cleanup tests
and the package-based Wasm build pass. These are build and integration results,
not a production deployment or registry publication.

## Historical manifest and review boundary

[`proof/cohort.json`](../proof/cohort.json) has one versioned repository list.
Each entry has `name`, credential-free HTTPS `.git` `url`, a two-component relative
`path`, and a full lowercase 40-hex commit SHA. The path restores:

```text
DESTINATION/
  lenso-cli/feat-portable-catalog-protocol/
  lenso-web/feat-workers-http-parity/
  lenso-runtime-rust/design-workers-compatibility/
  lenso-auth-plugin/feat-workers-auth-storage/
  lenso-console/feat-workers-marketplace/
```

The initial CLI pin is `798d58b9aae2f90ed29e17285b0693d10cb8eabd`.
Other `null` commits are explicitly pending. **The entire manifest is refused
until every pin is filled.** Branch names, `HEAD`, tags, abbreviated SHAs,
non-commit object IDs, duplicate entries and path traversal are rejected.
Commit existence does not establish review or remote publication; the release
owner must attach those proofs separately.

Commit each source payload first, then freeze the complete cohort manifest as a
reviewed release input. A Console commit cannot contain its own final SHA. Its
entry should pin the completed source commit; the later manifest may be a separate
reviewed file or a subsequent metadata-only commit. Use `--manifest` to select
that frozen input explicitly. Do not resolve `main` dynamically or substitute a
different commit when the pinned one cannot be fetched.

## Materialize exact remote commits

After the owner fills and reviews the manifest, run from this directory's parent
(`plugins/marketplace/workers`):

```sh
python3 -B proof/cohort.py \
  --manifest proof/cohort.json \
  --destination /absolute/path/to/empty-cohort
```

The destination must be absent or empty, and cannot itself be a symlink. Existing
checkouts, dirty or clean, are not reused or updated. The tool validates all pins
and paths before creating destination content, then fetches each SHA with no
tags, checks its object type, checks out that exact commit and verifies a clean
tree. It does not initialize submodules or install dependencies. Each checkout's
`origin` remains its reviewed HTTPS URL.

Successful materialization writes `DESTINATION/.lenso-cohort.json` containing the
input manifest SHA-256, every exact checkout and whether a local override was used.
The receipt is written only after every checkout succeeds. Fetch failures preserve
any partial destination for inspection; there is no automatic deletion or retry
into that tree. Select another empty destination after resolving the failure.
This receipt establishes checkout identity, not build or production readiness.

## Validate unpublished commits through local sources

An explicit source override permits a local Git object database while preserving
the manifest's exact commit and official `origin`:

```sh
python3 -B proof/cohort.py \
  --manifest /absolute/path/to/reviewed-cohort.json \
  --destination /absolute/path/to/empty-local-cohort \
  --source lenso-cli=/absolute/path/to/clean-cli-checkout \
  --source lenso-web=/absolute/path/to/clean-web-checkout \
  --source lenso-runtime-rust=/absolute/path/to/clean-runtime-checkout \
  --source lenso-auth-plugin=/absolute/path/to/clean-auth-checkout \
  --source lenso-console=/absolute/path/to/clean-console-checkout
```

Overrides must name an existing clean repository root or bare Git repository,
contain the exact commit, and use a known unique repository name. Tracked edits
and untracked files make a working-tree source ineligible; ignored build artifacts
are not copied. Local validation does not prove that a commit is remotely
available. Delivery requires a second materialization without local overrides.

Build and qualify through the exact owner instructions from the resulting trees,
using the framework Cargo wrapper where applicable. Preserve the cohort receipt
with build/deployment receipts. All production trust, storage, domain, publication
and approval requirements in [G5 preflight](g5-preflight.md) still apply.

## Focused self-test

```sh
python3 -B proof/cohort.py --self-test
```

Five isolated temporary-Git tests prove that an older exact pin is selected even
when source HEAD advances; source-local files are not copied; pending/symbolic/
abbreviated refs and tag objects are refused; traversal, absolute paths and
symlink destinations are refused; and dirty source/nonempty destination contents
are preserved. They use no network or real workspace mutations and remove only
their own temporary fixtures.
