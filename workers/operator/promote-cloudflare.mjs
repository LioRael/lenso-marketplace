import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { isAbsolute } from "node:path";

import { cloudflareStorage } from "./cloudflare.mjs";
import { promotePublication } from "./promote.mjs";

// Run only in the protected operator host. CONFIG contains public identifiers,
// absolute publisher paths and the operator-reviewed expected remote pointer.
const main = async () => {
  assert.equal(
    process.argv.length,
    3,
    "usage: node operator/promote-cloudflare.mjs CONFIG.json"
  );
  const config = JSON.parse(readFileSync(process.argv[2], "utf-8"));
  assert.ok(
    isAbsolute(config.publisherBinary) && isAbsolute(config.publisherConfig),
    "absolute publisher paths required"
  );
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
  const publication = publisher("export");
  const receipt = await promotePublication({
    ...cloudflareStorage({
      ...config,
      accessKeyId: process.env.MARKETPLACE_R2_ACCESS_KEY_ID,
      secretAccessKey: process.env.MARKETPLACE_R2_SECRET_ACCESS_KEY,
      token: process.env.MARKETPLACE_D1_TOKEN,
    }),
    catalogId: config.catalogId,
    envelope: publication.envelope,
    expected: config.expected,
    verify: (envelope) => publisher("verify", envelope),
  });
  process.stdout.write(`${JSON.stringify(receipt)}\n`);
};
try {
  await main();
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
