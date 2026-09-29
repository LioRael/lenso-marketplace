import assert from "node:assert/strict";
import { chmod, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { isAbsolute, join } from "node:path";
import { pathToFileURL } from "node:url";

import { loadReviewedCatalog } from "../keyless/catalog.mjs";
import { digest, readBoundedFile, verifyArtifact } from "../keyless/verify.mjs";
import { promoteKeyless } from "./promote-keyless.mjs";

export const validateOperatorConfig = (config) => {
  const allowed = [
    "catalogPath",
    "bundlePath",
    "deploymentConfig",
    "reviewedCatalogSha256",
    "reviewedBundleSha256",
    "sourceSha",
    "expected",
    "accountId",
    "databaseId",
    "bucketName",
    "catalogId",
  ];
  assert.ok(config && typeof config === "object" && !Array.isArray(config));
  assert.deepEqual(Object.keys(config).toSorted(), allowed.toSorted());
  for (const key of ["catalogPath", "bundlePath", "deploymentConfig"]) {
    assert.ok(
      typeof config[key] === "string" && isAbsolute(config[key]),
      "Absolute input paths required"
    );
  }
  for (const key of ["reviewedCatalogSha256", "reviewedBundleSha256"]) {
    assert.match(config[key], /^[a-f0-9]{64}$/u);
  }
  assert.match(config.sourceSha, /^[a-f0-9]{40}$/u);
  assert.match(config.accountId, /^[a-f0-9]{32}$/u);
  assert.match(
    config.databaseId,
    /^[a-f0-9]{8}-(?:[a-f0-9]{4}-){3}[a-f0-9]{12}$/u
  );
  assert.match(config.bucketName, /^[a-z0-9][a-z0-9-]{1,61}[a-z0-9]$/u);
  assert.equal(config.catalogId, "lenso-official-v2");
  if (config.expected !== null) {
    assert.deepEqual(Object.keys(config.expected).toSorted(), [
      "catalog_digest",
      "revision",
    ]);
    assert.ok(
      Number.isSafeInteger(config.expected.revision) &&
        config.expected.revision > 0
    );
    assert.match(config.expected.catalog_digest, /^[a-f0-9]{64}$/u);
  }
  return config;
};

export const promoteFromConfig = async (config) => {
  validateOperatorConfig(config);
  const deploymentBytes = await readBoundedFile(config.deploymentConfig);
  const deployment = JSON.parse(deploymentBytes.toString("utf-8"));
  assert.equal(deployment.name, "lenso-marketplace");
  assert.equal(deployment.account_id, config.accountId);
  assert.equal(deployment.vars.CATALOG_ID, config.catalogId);
  assert.equal(deployment.d1_databases.length, 1);
  assert.equal(deployment.r2_buckets.length, 1);
  assert.equal(deployment.d1_databases[0].database_id, config.databaseId);
  assert.equal(deployment.r2_buckets[0].bucket_name, config.bucketName);
  assert.equal(deployment.routes.length, 1);
  assert.equal(deployment.routes[0].pattern, "marketplace.lenso.dev");
  assert.equal(deployment.routes[0].custom_domain, true);
  const catalogBytes = await readBoundedFile(config.catalogPath);
  const bundleBytes = await readBoundedFile(config.bundlePath);
  loadReviewedCatalog(catalogBytes, config.reviewedCatalogSha256);
  assert.equal(
    digest(bundleBytes),
    config.reviewedBundleSha256,
    "Reviewed bundle digest mismatch"
  );
  const directory = await mkdtemp(join(tmpdir(), "lenso-keyless-reviewed-"));
  const catalogPath = join(directory, "catalog.json");
  const bundlePath = join(directory, "bundle.json");
  await writeFile(catalogPath, catalogBytes, { flag: "wx", mode: 0o400 });
  await writeFile(bundlePath, bundleBytes, { flag: "wx", mode: 0o400 });
  await chmod(directory, 0o500);
  const { cloudflareStorage } = await import("./cloudflare.mjs");
  return promoteKeyless({
    bundleBytes,
    catalogBytes,
    expected: config.expected,
    sourceSha: config.sourceSha,
    ...cloudflareStorage({
      accessKeyId: process.env.MARKETPLACE_R2_ACCESS_KEY_ID,
      accountId: config.accountId,
      bucketName: config.bucketName,
      databaseId: config.databaseId,
      secretAccessKey: process.env.MARKETPLACE_R2_SECRET_ACCESS_KEY,
      token: process.env.MARKETPLACE_D1_TOKEN,
    }),
    verify: async (input) => {
      assert.equal(input.sourceSha, config.sourceSha);
      assert.equal(input.catalogDigest, config.reviewedCatalogSha256);
      assert.ok(
        input.catalogBytes.equals(catalogBytes) &&
          input.bundleBytes.equals(bundleBytes)
      );
      await verifyArtifact(
        catalogPath,
        bundlePath,
        config.sourceSha,
        config.reviewedCatalogSha256
      );
    },
  });
};

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  try {
    assert.equal(process.argv.length, 3);
    assert.ok(
      isAbsolute(process.argv[2]),
      "Absolute operator configuration path required"
    );
    const configBytes = await readBoundedFile(process.argv[2]);
    const config = JSON.parse(configBytes.toString("utf-8"));
    console.log(JSON.stringify(await promoteFromConfig(config)));
  } catch {
    console.error(
      "Keyless promotion failed or is unconfirmed; inspect immutable objects and current head before retrying"
    );
    process.exitCode = 1;
  }
}
