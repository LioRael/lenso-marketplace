---
name: land
description: >-
  Land the current Marketplace change when the user explicitly requests
  landing, merging, or the Land Changes action. Do not invoke this skill for
  review, preparation, passing checks, or skill installation alone.
metadata:
  delta-action: land
---

# Land Lenso Marketplace changes

Run this workflow only after an explicit landing request. A direct `/land`
invocation, `Land changes from this thread`, or an equivalent explicit merge
request already supplies merge intent; do not ask for confirmation again.

The repository's landing policy is in `AGENTS.md:5-9`. Required delivery
checks are `catalog`, `event-host`, and `quality`; inspect failures and never
remove a required check. Do not claim deployment from a merge or build.

## Conflict policy

The project preference is to resolve clearly mechanical conflicts
automatically. Preserve both sides when the intended result is obvious, then
continue the merge. Stop and report a failure when a conflict changes
product behavior, ownership, security, signed fixtures, migrations, or any
other intent that is not mechanically decidable.

## 1. Establish the change and preserve unrelated work

Set:

```sh
repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"
```

Inspect the current branch and complete working-tree state:

```sh
git status --short --branch
git diff --stat
git diff --cached --stat
git ls-files --others --exclude-standard
git log --oneline --decorate -12
```

Do not use `git add .`, `git clean`, `git reset --hard`, or a broad deletion to
silence unrelated work. Do not add credentials, private keys, publisher
databases, generated `target/` trees, local `.wrangler/` state, build output,
or local fixture databases. If any untracked or modified path outside known
generated output is unrelated to the requested change, stop and ask the user.

If the requested change is already committed, use those commits. If relevant
changes are uncommitted, inspect every path, run the checks below, stage only
the intended paths, and create a Conventional Commit with a concise
`feat:`, `fix:`, `perf:`, `test:`, `docs:`, or `chore:` subject. Run
`git diff --check` before committing. Never rewrite an already-published topic
branch with a force push.

## 2. Resolve the destination and local integration branch

Verify the configured remotes before publishing:

```sh
git remote -v
gh repo view --json nameWithOwner,defaultBranchRef
```

For this repository, publish through the GitHub `origin` remote and target its
`main` default branch. Follow Zed's documented worktree model: review the
thread's isolated diff, then incorporate it through normal local Git workflow.
This skill does not create, update, or merge a pull request.

Fetch the destination before preparing the topic branch:

```sh
git fetch origin main
```

Save the requested source commit before changing branches:

```sh
source_ref="$(git rev-parse HEAD)"
```

Create a unique local integration branch from the latest destination:

```sh
git switch -c "land/<short-change-name>" origin/main
```

Incorporate the saved source commit through a normal local merge:

```sh
GIT_EDITOR=true git merge --no-ff "$source_ref" \
  -m "<final Conventional Commit title>"
```

During the merge, resolve only clear mechanical conflicts. After each
resolution, inspect the hunk and stage the exact file before completing the
merge:

```sh
git add <resolved-path>
GIT_EDITOR=true git commit --no-edit
```

Stop on ambiguous intent, conflict in signed evidence or migrations, failed
integration, or any request to force-push. Never rewrite or force-push
`origin/main`.

## 3. Run repository and change-specific verification

Use Rust 1.94. The repository requires the UI build before Rust checks because
the embedded UI has a source fingerprint (`AGENTS.md:5`; `plugins/web/build.rs`).
Honor an explicit caller-provided `CARGO`; otherwise use the installed `cargo`
executable:

```sh
CARGO="${CARGO:-cargo}"
```

Install the frozen Node graph and build the UI:

```sh
pnpm install --frozen-lockfile
pnpm build
```

These commands are defined by `.github/workflows/marketplace.yml:19-20` and
`package.json:6-8`.

Run the foundation and required local checks:

```sh
CARGO="$CARGO" pnpm test:native
CARGO="$CARGO" pnpm test:browser
pnpm test:workers
pnpm lint
pnpm format:check
```

The native and browser workflows are the repository scripts referenced by
`.github/workflows/marketplace.yml:21-25`; the Workers test, lint, and format
commands are defined in `package.json:10-19`.

Run the Workers delivery-equivalent checks from
`.github/workflows/marketplace-workers.yml:22-31`:

```sh
pnpm build
cargo clippy --locked -p lenso-marketplace-workers-host \
  --target wasm32-unknown-unknown -- -D warnings
pnpm test:workers
cargo install wasm-bindgen-cli --version 0.2.127 --locked
CARGO="$CARGO" pnpm build:workers
pnpm exec wrangler deploy --dry-run --config apps/workers/wrangler.jsonc
```

If the change touches `apps/workers-sdk-prototype`, its scripts, or its
documentation, also run the pinned prototype proof from `README.md:81-92`:

```sh
cargo install worker-build --version 0.8.5 --locked
CARGO="$CARGO" pnpm test:workers:sdk
```

Inspect every failure. Do not weaken a check, remove a required check, skip a
failed test, or treat a dry-run as deployment.

## 4. Publish the integrated change directly

Before publishing, confirm the integration branch contains only the requested
change relative to the fetched destination:

```sh
git diff --check origin/main...HEAD
git diff --stat origin/main...HEAD
git status --short --branch
```

Push the local integration result directly to the remote default branch. This
is the only publication operation; do not push an intermediate topic branch
and do not open a PR:

```sh
git push origin HEAD:main
```

If the push is rejected because `main` advanced, fetch again, incorporate the
new `origin/main` into the local integration branch, rerun all checks, and
retry the non-force push. If direct publication is denied by permissions or
branch policy, report that the change has not landed; do not fall back to a
pull request.

## 5. Verify push checks before reporting a landing

Record the exact pushed commit:

```sh
landed_sha="$(git rev-parse HEAD)"
repo_slug="$(gh repo view --json nameWithOwner --jq .nameWithOwner)"
```

The three delivery jobs are defined in
`.github/workflows/marketplace.yml`,
`.github/workflows/marketplace-workers.yml`, and
`.github/workflows/quality.yml`. Query checks for the exact pushed SHA:

```sh
gh run list --commit "$landed_sha" --limit 50 \
  --json databaseId,name,status,conclusion,url,headSha
gh api "repos/$repo_slug/commits/$landed_sha/check-runs" \
  --paginate --jq '.check_runs[] |
    [.name,.status,.conclusion,.details_url] | @tsv'
```

Wait for the push-triggered runs and inspect failures. Do not report success
while `catalog`, `event-host`, or `quality` is missing, pending, failing, or
unverifiable. Preserve and inspect additional checks reported for this SHA,
including GitGuardian; a skipped unrelated release job is not a successful
required delivery check. Do not bypass branch policy, required checks, or
review requirements if the destination reports them.

## 6. Verify the direct landing

After all checks pass, verify that the pushed commit reached remote `main`:

```sh
git fetch origin main
git merge-base --is-ancestor "$landed_sha" origin/main
git show --stat --oneline "$landed_sha"
git status --short --branch
```

A prepared commit, pushed integration branch, passing local checks, or
successful workflow start is not a successful landing. Success requires the
exact commit to be verified as an ancestor of `origin/main`. Do not deploy or
claim deployment unless the user separately requests and completes the
documented deployment procedure.

## Outcome reporting

When running in a subthread and `report_subthread_status` is available, report
the result to the parent after verification:

- `status: "success"` only after the directly landed commit is verified on
  `origin/main`;
  link the verified short commit SHA and check/run URLs when GitHub exposed
  them. There is no PR URL in this workflow.
- `status: "failure"` when checks fail, publication is denied, conflicts are
  ambiguous, or the direct landing cannot be verified; state clearly whether
  the change has reached `main`, whether required checks passed, and identify
  the blocker.

Keep the title short and sentence-case, and keep the description to one short
line. If the status tool is unavailable, report the same facts directly in the
conversation.
