import assert from "node:assert/strict";
import { test } from "node:test";
import { digest, verificationArguments } from "./verify.mjs";

test("verification pins repository, workflow, ref, exact source and signer", () => {
  const commit = "a".repeat(40);
  const args = verificationArguments("/tmp/catalog.json", "/tmp/catalog.bundle.json", commit);
  assert.equal(args[args.indexOf("--repo") + 1], "LioRael/lenso-marketplace");
  assert.equal(args[args.indexOf("--source-digest") + 1], commit);
  assert.equal(args[args.indexOf("--signer-digest") + 1], commit);
  assert.equal(args[args.indexOf("--source-ref") + 1], "refs/heads/main");
  assert.equal(args[args.indexOf("--cert-identity") + 1],
    "https://github.com/LioRael/lenso-marketplace/.github/workflows/publish-keyless-catalog.yml@refs/heads/main");
  assert.ok(args.includes("--deny-self-hosted-runners"));
  assert.ok(!args.includes("--no-public-good"));
  assert.ok(!args.includes("--custom-trusted-root"));
});

test("verification rejects non-exact source and ambiguous local paths", () => {
  for (const source of ["main", "HEAD", "A".repeat(40), "a".repeat(39)]) {
    assert.throws(() => verificationArguments("catalog.json", "bundle.json", source));
  }
  assert.throws(() => verificationArguments("--help", "bundle.json", "a".repeat(40)));
  assert.throws(() => verificationArguments("catalog.json", "", "a".repeat(40)));
});

test("reviewed digest binds the original bytes, not parsed JSON", () => {
  assert.notEqual(digest(Buffer.from('{"revision":1}')), digest(Buffer.from('{ "revision": 1 }')));
});
