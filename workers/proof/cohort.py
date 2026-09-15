#!/usr/bin/env python3
"""Materialize a pinned experimental source cohort; never build or deploy it."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest
from urllib.parse import urlsplit


class CohortError(Exception):
    pass


def git(*args, cwd=None):
    env = os.environ.copy()
    for key in (
        "GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ):
        env.pop(key, None)
    env["GIT_TERMINAL_PROMPT"] = "0"
    try:
        result = subprocess.run(
            ["git", "-c", "core.hooksPath=/dev/null", *args], cwd=cwd, env=env,
            text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            timeout=60, check=True,
        )
    except subprocess.TimeoutExpired as error:
        raise CohortError("Git operation exceeded 60 seconds") from error
    except subprocess.CalledProcessError as error:
        # Do not echo arbitrary remote output or credential-bearing Git config.
        raise CohortError("Git operation failed (exit %s)" % error.returncode) from error
    return result.stdout.strip()


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise CohortError("Duplicate manifest field: " + key)
        value[key] = item
    return value


def read_manifest(path):
    raw = Path(path).read_bytes()
    value = json.loads(raw, object_pairs_hook=unique_object)
    if not isinstance(value, dict) or set(value) != {"version", "repositories"}:
        raise CohortError("Expected manifest version and repositories")
    if type(value["version"]) is not int or value["version"] != 1:
        raise CohortError("Unsupported manifest version")
    repositories = value["repositories"]
    if not isinstance(repositories, list) or not repositories:
        raise CohortError("Expected a nonempty repository list")
    names, paths = set(), set()
    for repo in repositories:
        if not isinstance(repo, dict) or set(repo) != {"name", "path", "url", "commit"}:
            raise CohortError("Expected name, path, url and commit per repository")
        name, path, url, commit = (repo[k] for k in ("name", "path", "url", "commit"))
        if not isinstance(name, str) or not re.fullmatch(r"[a-z][a-z0-9-]*", name):
            raise CohortError("Invalid repository name")
        if name in names:
            raise CohortError("Duplicate repository name: " + name)
        names.add(name)
        # Exactly two portable components recreate the experiment's sibling layout.
        # Reject absolute paths, dot segments, backslashes, drive names and aliases.
        if not isinstance(path, str) or not re.fullmatch(
            r"[a-z0-9][a-z0-9._-]*/[a-z0-9][a-z0-9._-]*", path
        ) or path.split("/")[0] != name or path in paths:
            raise CohortError("Invalid or conflicting cohort path for " + name)
        paths.add(path)
        if commit is None:
            raise CohortError("Pending commit for " + name + "; fill the reviewed exact pin first")
        if not isinstance(commit, str) or not re.fullmatch(r"[0-9a-f]{40}", commit):
            raise CohortError("Expected a full lowercase 40-hex commit for " + name)
        if not isinstance(url, str):
            raise CohortError("Expected HTTPS repository URL for " + name)
        parsed = urlsplit(url)
        if (parsed.scheme != "https" or not parsed.hostname or parsed.username is not None
                or parsed.password is not None or parsed.query or parsed.fragment
                or not parsed.path.endswith(".git") or any(c.isspace() for c in url)):
            raise CohortError("Expected a credential-free HTTPS .git URL for " + name)
    return repositories, hashlib.sha256(raw).hexdigest()


def local_sources(repositories, overrides):
    names = {repo["name"] for repo in repositories}
    if set(overrides) - names:
        raise CohortError("Source override names an unknown repository")
    sources = {}
    for repo in repositories:
        name = repo["name"]
        if name not in overrides:
            sources[name] = repo["url"]
            continue
        source = Path(overrides[name]).expanduser().resolve(strict=True)
        if not source.is_dir():
            raise CohortError("Local source must be a Git repository directory: " + name)
        bare = git("rev-parse", "--is-bare-repository", cwd=source) == "true"
        if not bare:
            if Path(git("rev-parse", "--show-toplevel", cwd=source)).resolve() != source:
                raise CohortError("Local source must name the repository root: " + name)
            if git("status", "--porcelain=v1", "--untracked-files=all", cwd=source):
                raise CohortError("Dirty local source refused: " + name)
        if git("cat-file", "-t", repo["commit"], cwd=source) != "commit":
            raise CohortError("Pin is not a commit object: " + name)
        sources[name] = str(source)
    return sources


def empty_destination(path):
    destination = Path(path).expanduser().absolute()
    if destination.is_symlink():
        raise CohortError("Destination must not be a symlink")
    if destination.exists() and (
        not destination.is_dir() or any(destination.iterdir())
    ):
        raise CohortError("Destination must be absent or empty; existing trees are never reused")
    return destination


def materialize(manifest, destination, overrides=None, progress=True):
    repositories, manifest_digest = read_manifest(manifest)
    destination = empty_destination(destination)
    overrides = overrides or {}
    sources = local_sources(repositories, overrides)
    # All pins, paths and local-source checks precede the first destination write.
    destination.mkdir(parents=True, exist_ok=True)
    receipt = {"version": 1, "manifest_sha256": manifest_digest, "repositories": []}
    for repo in repositories:
        if progress:
            print("Fetching " + repo["name"] + " at " + repo["commit"], flush=True)
        target = destination / repo["path"]
        target.parent.mkdir()  # Unique repository names must not collide with another tree.
        target.mkdir()  # Never overwrite a tree created by another process.
        git("init", "--quiet", str(target))
        git("remote", "add", "origin", repo["url"], cwd=target)
        git("fetch", "--quiet", "--no-tags", "--depth=1", sources[repo["name"]],
            repo["commit"], cwd=target)
        if git("cat-file", "-t", repo["commit"], cwd=target) != "commit":
            raise CohortError("Fetched pin is not a commit: " + repo["name"])
        git("checkout", "--quiet", "--detach", repo["commit"], cwd=target)
        if git("rev-parse", "HEAD", cwd=target) != repo["commit"]:
            raise CohortError("Checkout does not match exact pin: " + repo["name"])
        if git("status", "--porcelain=v1", "--untracked-files=all", cwd=target):
            raise CohortError("Materialized tree is dirty: " + repo["name"])
        receipt["repositories"].append({**repo, "local_source_override": repo["name"] in overrides})
    # A receipt exists only after every checkout has verified successfully.
    with (destination / ".lenso-cohort.json").open("x") as output:
        json.dump(receipt, output, indent=2)
        output.write("\n")
    return receipt


class SelfTest(unittest.TestCase):
    """Prevent ref drift, source leakage and writes outside an empty destination."""

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="lenso-cohort-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "source"
        git("init", "--quiet", str(self.source))
        (self.source / "value.txt").write_text("pinned\n")
        git("add", "value.txt", cwd=self.source)
        self.commit("first")
        self.pin = git("rev-parse", "HEAD", cwd=self.source)
        (self.source / "value.txt").write_text("newer HEAD\n")
        git("add", "value.txt", cwd=self.source)
        self.commit("second")
        self.manifest = self.root / "cohort.json"
        self.repo = {"name": "example", "path": "example/pinned", "url":
                     "https://github.com/example/example.git", "commit": self.pin}
        self.write_manifest()
        self.destination = self.root / "output"

    def commit(self, message):
        # Plumbing avoids host commit-time integrations writing notes after a
        # temporary fixture has already started cleaning up.
        tree = git("write-tree", cwd=self.source)
        args = ["-c", "user.name=Cohort Test", "-c", "user.email=cohort@example.invalid",
                "-c", "commit.gpgsign=false", "commit-tree", tree, "-m", message]
        if getattr(self, "previous", None):
            args.extend(["-p", self.previous])
        self.previous = git(*args, cwd=self.source)
        git("update-ref", "HEAD", self.previous, cwd=self.source)

    def write_manifest(self):
        self.manifest.write_text(json.dumps({"version": 1, "repositories": [self.repo]}))

    def run_cohort(self):
        return materialize(self.manifest, self.destination, {"example": self.source}, False)

    def test_exact_commit_not_source_head_and_no_worktree_copy(self):
        (self.source / ".git" / "source-only").write_text("never copied")
        receipt = self.run_cohort()
        target = self.destination / "example/pinned"
        self.assertEqual((target / "value.txt").read_text(), "pinned\n")
        self.assertEqual(git("rev-parse", "HEAD", cwd=target), self.pin)
        self.assertFalse((target / ".git/source-only").exists())
        self.assertEqual(receipt["repositories"][0]["commit"], self.pin)
        self.assertTrue((self.destination / ".lenso-cohort.json").exists())

    def test_unpinned_refs_refused_before_writes(self):
        for pin in (None, "HEAD", "main", self.pin[:8], "a" * 39, "g" * 40):
            with self.subTest(pin=pin):
                self.repo["commit"] = pin
                self.write_manifest()
                with self.assertRaises(CohortError):
                    self.run_cohort()
                self.assertFalse(self.destination.exists())

    def test_traversal_and_absolute_paths_refused_before_writes(self):
        for path in ("../escape", "/tmp/escape", "example/../escape",
                     "example/../../escape", "example\\escape", "example/.", "example//pin"):
            with self.subTest(path=path):
                self.repo["path"] = path
                self.write_manifest()
                with self.assertRaises(CohortError):
                    self.run_cohort()
                self.assertFalse(self.destination.exists())

    def test_dirty_sources_and_existing_destinations_are_preserved(self):
        (self.source / "untracked").write_text("must not copy")
        with self.assertRaisesRegex(CohortError, "Dirty local source"):
            self.run_cohort()
        self.assertFalse(self.destination.exists())
        (self.source / "untracked").unlink()
        self.destination.mkdir()
        marker = self.destination / "keep"
        marker.write_text("untouched")
        with self.assertRaisesRegex(CohortError, "absent or empty"):
            self.run_cohort()
        self.assertEqual(marker.read_text(), "untouched")

    def test_symlink_destination_and_tag_object_refused(self):
        self.destination.symlink_to(self.source, target_is_directory=True)
        with self.assertRaisesRegex(CohortError, "symlink"):
            self.run_cohort()
        self.destination.unlink()
        git("-c", "user.name=Cohort Test", "-c", "user.email=cohort@example.invalid",
            "-c", "tag.gpgsign=false", "tag", "-a", "tagged", "-m", "tag", cwd=self.source)
        self.repo["commit"] = git("rev-parse", "tagged", cwd=self.source)
        self.write_manifest()
        with self.assertRaisesRegex(CohortError, "not a commit object"):
            self.run_cohort()
        self.assertFalse(self.destination.exists())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=Path(__file__).with_name("cohort.json"))
    parser.add_argument("--destination", type=Path, help="Absent or empty directory")
    parser.add_argument("--source", action="append", default=[], metavar="NAME=PATH",
                        help="Clean local Git source override; only the pinned commit is fetched")
    parser.add_argument("--self-test", action="store_true", help="Run isolated temporary Git tests")
    args = parser.parse_args()
    if args.self_test:
        return not unittest.TextTestRunner(verbosity=2).run(
            unittest.defaultTestLoader.loadTestsFromTestCase(SelfTest)
        ).wasSuccessful()
    if args.destination is None:
        parser.error("--destination is required unless --self-test is selected")
    overrides = {}
    try:
        for override in args.source:
            name, separator, path = override.partition("=")
            if not separator or not name or not path or name in overrides:
                raise CohortError("Expected a unique --source NAME=PATH")
            overrides[name] = path
        receipt = materialize(args.manifest, args.destination, overrides)
        print("Verified %d exact checkouts; receipt: %s" % (
            len(receipt["repositories"]), args.destination / ".lenso-cohort.json"))
        return 0
    except (CohortError, OSError, ValueError) as error:
        print("Cohort refused: " + str(error), file=sys.stderr)
        print("Any partial destination is preserved; inspect it and choose an empty destination.",
              file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
