import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  closeSync,
  fstatSync,
  openSync,
  readFileSync,
  readSync,
} from "node:fs";
import { isAbsolute } from "node:path";

import { cloudflareStorage } from "./cloudflare.mjs";
import {
  assertParallelRoot,
  assertStableDomainRoot,
} from "./parallel-root.mjs";
import {
  promotePublication,
  promoteReleaseDetails,
  promoteLinkedCargo,
  promotePackage,
  promoteReleaseContent,
} from "./promote.mjs";

const readDocument = (path) => {
  const descriptor = openSync(path, "r");
  try {
    assert.ok(
      fstatSync(descriptor).isFile(),
      "document must be a regular file"
    );
    const buffer = Buffer.alloc(1024 * 1024 + 1);
    let size = 0;
    while (size < buffer.byteLength) {
      const read = readSync(
        descriptor,
        buffer,
        size,
        buffer.byteLength - size,
        null
      );
      if (read === 0) {
        break;
      }
      size += read;
    }
    assert.ok(size > 0 && size <= 1024 * 1024, "document size exceeds bound");
    return buffer.subarray(0, size);
  } finally {
    closeSync(descriptor);
  }
};

const validateParallelPromotion = (config) => {
  assert.ok(
    config.deploymentTrack === undefined ||
      config.deploymentTrack === "parallel-new-root" ||
      config.deploymentTrack === "stable-domain-new-root",
    "unknown deployment track"
  );
  if (
    config.deploymentTrack !== "parallel-new-root" &&
    config.deploymentTrack !== "stable-domain-new-root"
  ) {
    return;
  }
  assert.ok(
    isAbsolute(config.deploymentConfig),
    "parallel root requires an absolute deployment config path"
  );
  const deployment = JSON.parse(readFileSync(config.deploymentConfig, "utf-8"));
  assert.equal(deployment.d1_databases?.length, 1);
  assert.equal(deployment.r2_buckets?.length, 1);
  assert.equal(deployment.routes?.length, 1);
  assert.equal(deployment.routes[0].custom_domain, true);
  const rootInput = {
    bucket_name: deployment.r2_buckets[0].bucket_name,
    catalog_id: deployment.vars?.CATALOG_ID,
    database_id: deployment.d1_databases[0].database_id,
    database_name: deployment.d1_databases[0].database_name,
    deployment_track: config.deploymentTrack,
    hostname: deployment.routes[0].pattern,
    key_id: deployment.vars?.CATALOG_KEY_ID,
    legacy_public_key_hex: config.legacyPublicKeyHex,
    public_key_hex: deployment.vars?.CATALOG_PUBLIC_KEY,
    worker: deployment.name,
  };
  if (config.deploymentTrack === "parallel-new-root") {
    assertParallelRoot(rootInput);
  } else {
    assertStableDomainRoot(rootInput);
  }
  assert.equal(config.accountId, deployment.account_id);
  assert.equal(config.databaseId, deployment.d1_databases[0].database_id);
  assert.equal(config.bucketName, deployment.r2_buckets[0].bucket_name);
  assert.equal(config.catalogId, deployment.vars.CATALOG_ID);
  const publisherConfig = JSON.parse(
    readFileSync(config.publisherConfig, "utf-8")
  );
  assert.ok(
    isAbsolute(publisherConfig.database),
    "parallel root requires an absolute private publisher database path"
  );
  assert.equal(publisherConfig.catalog_id, deployment.vars.CATALOG_ID);
  assert.equal(publisherConfig.key_id, deployment.vars.CATALOG_KEY_ID);
  assert.equal(
    publisherConfig.public_key_hex,
    deployment.vars.CATALOG_PUBLIC_KEY
  );
};

// Run only in the protected operator host. CONFIG contains public identifiers,
// absolute publisher paths and the operator-reviewed expected remote pointer.
const main = async () => {
  assert.equal(
    process.argv.length,
    3,
    "usage: node tools/cloudflare/promote-cloudflare.mjs CONFIG.json"
  );
  const config = JSON.parse(readFileSync(process.argv[2], "utf-8"));
  assert.ok(
    isAbsolute(config.publisherBinary) && isAbsolute(config.publisherConfig),
    "absolute publisher paths required"
  );
  validateParallelPromotion(config);
  const publisher = (operation, input) => {
    const result = spawnSync(
      config.publisherBinary,
      [config.publisherConfig, operation],
      { encoding: "utf-8", input, maxBuffer: 16 * 1024 * 1024, timeout: 30000 }
    );
    assert.ok(
      !result.error && result.status === 0,
      `publisher ${operation} failed; inspect its durable state`
    );
    return JSON.parse(result.stdout);
  };
  // Export cannot create or renew a publication. Its bytes come from the
  // authoritative durable database, not a user-supplied envelope file.
  const details = config.kind === "release-details";
  const linked = config.kind === "linked-cargo";
  const packaged = config.kind === "package";
  const content = config.kind === "release-content";
  assert.ok(
    config.kind === undefined || details || linked || packaged || content,
    "kind must be omitted, release-details, linked-cargo, package or release-content"
  );
  let exportOperation = "export";
  let verifyOperation = "verify";
  let promote = promotePublication;
  if (details) {
    exportOperation = "export-details";
    verifyOperation = "verify-details";
    promote = promoteReleaseDetails;
  } else if (linked) {
    exportOperation = "export-linked-cargo";
    verifyOperation = "verify-linked-cargo";
    promote = promoteLinkedCargo;
  } else if (packaged) {
    exportOperation = "export-package";
    verifyOperation = "verify-package";
    promote = promotePackage;
  } else if (content) {
    exportOperation = "export-release-content";
    verifyOperation = "verify-release-content";
    promote = promoteReleaseContent;
  }
  const publication = publisher(exportOperation);
  const documentBodies = new Map();
  if (details || linked || packaged || content) {
    for (const [digest, path] of Object.entries(config.documentFiles ?? {})) {
      assert.match(digest, /^sha256:[a-f0-9]{64}$/u);
      assert.ok(isAbsolute(path), "absolute document path required");
      documentBodies.set(digest, () => readDocument(path));
    }
  } else {
    assert.equal(
      config.documentFiles,
      undefined,
      "this channel has no documents"
    );
  }
  const receipt = await promote({
    ...cloudflareStorage({
      ...config,
      accessKeyId: process.env.MARKETPLACE_R2_ACCESS_KEY_ID,
      secretAccessKey: process.env.MARKETPLACE_R2_SECRET_ACCESS_KEY,
      token: process.env.MARKETPLACE_D1_TOKEN,
    }),
    catalogId: config.catalogId,
    documentBodies,
    envelope: publication.envelope,
    expected: config.expected,
    verify: (envelope) => publisher(verifyOperation, envelope),
  });
  process.stdout.write(`${JSON.stringify(receipt)}\n`);
};
try {
  await main();
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
