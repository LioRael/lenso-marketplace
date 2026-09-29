import assert from "node:assert/strict";
import { chmod, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { isAbsolute, join } from "node:path";
import { pathToFileURL } from "node:url";

import { loadReviewedCatalog } from "../keyless/catalog.mjs";
import { digest, readBoundedFile, verifyArtifact } from "../keyless/verify.mjs";
import { promoteKeyless } from "./promote-keyless.mjs";

export function validateOperatorConfig(config) {
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
  assert.deepEqual(Object.keys(config).sort(), allowed.sort());
  for (const key of ["catalogPath", "bundlePath", "deploymentConfig"]) {
    assert.ok(
      typeof config[key] === "string" && isAbsolute(config[key]),
      "Absolute input paths required"
    );
  }
  for (const key of ["reviewedCatalogSha256", "reviewedBundleSha256"])
    assert.match(config[key], /^[a-f0-9]{64}$/u);
  assert.match(config.sourceSha, /^[a-f0-9]{40}$/u);
  assert.match(config.accountId, /^[a-f0-9]{32}$/u);
  assert.match(
    config.databaseId,
    /^[a-f0-9]{8}-(?:[a-f0-9]{4}-){3}[a-f0-9]{12}$/u
  );
  assert.match(config.bucketName, /^[a-z0-9][a-z0-9-]{1,61}[a-z0-9]$/u);
  assert.equal(config.catalogId, "lenso-official-v2");
  if (config.expected !== null) {
    assert.deepEqual(Object.keys(config.expected).sort(), [
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
}

export async function promoteFromConfig(config) {
  validateOperatorConfig(config);
  const deployment = JSON.parse(
    (await readBoundedFile(config.deploymentConfig)).toString("utf8")
  );
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
    catalogBytes,
    bundleBytes,
    sourceSha: config.sourceSha,
    expected: config.expected,
    ...cloudflareStorage({
      accountId: config.accountId,
      databaseId: config.databaseId,
      bucketName: config.bucketName,
      token: process.env.MARKETPLACE_D1_TOKEN,
      accessKeyId: process.env.MARKETPLACE_R2_ACCESS_KEY_ID,
      secretAccessKey: process.env.MARKETPLACE_R2_SECRET_ACCESS_KEY,
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
}

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
    const config = JSON.parse(
      (await readBoundedFile(process.argv[2])).toString("utf8")
    );
    console.log(JSON.stringify(await promoteFromConfig(config)));
  } catch {
    console.error(
      "Keyless promotion failed or is unconfirmed; inspect immutable objects and current head before retrying"
    );
    process.exitCode = 1;
  }
}
