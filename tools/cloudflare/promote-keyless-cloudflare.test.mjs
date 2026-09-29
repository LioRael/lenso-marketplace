import assert from "node:assert/strict";
import { test } from "node:test";

import { validateOperatorConfig } from "./promote-keyless-cloudflare.mjs";

const fixture = () => ({
  accountId: "d".repeat(32),
  bucketName: "lenso-marketplace-v2-production",
  bundlePath: "/scratch/bundle.json",
  catalogId: "lenso-official-v2",
  catalogPath: "/scratch/catalog.json",
  databaseId: "ff75e080-5957-4d2e-9a42-8729b4d602a4",
  deploymentConfig: "/scratch/wrangler.json",
  expected: null,
  reviewedBundleSha256: "b".repeat(64),
  reviewedCatalogSha256: "a".repeat(64),
  sourceSha: "c".repeat(40),
});

test("operator requires exact reviewed immutable identities and absolute paths", () => {
  validateOperatorConfig(fixture());
  for (const field of ["catalogPath", "bundlePath", "deploymentConfig"]) {
    assert.throws(() =>
      validateOperatorConfig({ ...fixture(), [field]: "relative.json" })
    );
  }
  for (const field of [
    "reviewedCatalogSha256",
    "reviewedBundleSha256",
    "sourceSha",
  ]) {
    assert.throws(() =>
      validateOperatorConfig({ ...fixture(), [field]: "main" })
    );
  }
});

test("operator configuration cannot carry credentials or caller verifier receipts", () => {
  for (const field of [
    "token",
    "privateKey",
    "accessKeyId",
    "verificationReceipt",
  ]) {
    assert.throws(() =>
      validateOperatorConfig({ ...fixture(), [field]: "not-accepted" })
    );
  }
});

test("operator cannot silently switch authority or omit expected head", () => {
  assert.throws(() =>
    validateOperatorConfig({ ...fixture(), catalogId: "other" })
  );
  const config = fixture();
  delete config.expected;
  assert.throws(() => validateOperatorConfig(config));
  validateOperatorConfig({
    ...fixture(),
    expected: { catalog_digest: "a".repeat(64), revision: 1 },
  });
});
