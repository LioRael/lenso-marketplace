import assert from "node:assert/strict";
import { createPublicKey, verify } from "node:crypto";
import { pathToFileURL } from "node:url";

import {
  channels,
  legacySchemas,
  schema,
  validateCatalog,
} from "./catalog.mjs";
import { digest, readBoundedFile } from "./verify.mjs";

const key = createPublicKey({
  format: "der",
  key: Buffer.from(
    "302a300506032b6570032100f7e627ec01f22224328c194fd957689fede66419fb26e9b1dc1d6b4347920865",
    "hex"
  ),
  type: "spki",
});

export const verifyLegacyEnvelope = (bytes, channel, now) => {
  assert.ok(bytes.length <= 4 * 1024 * 1024);
  assert.ok(channels.includes(channel));
  const envelope = JSON.parse(bytes.toString("utf-8"));
  assert.deepEqual(Object.keys(envelope).toSorted(), [
    "key_id",
    "payload_base64",
    "signature_base64",
  ]);
  assert.equal(envelope.key_id, "lenso-marketplace-v2-2026");
  const payload = Buffer.from(envelope.payload_base64, "base64");
  const signature = Buffer.from(envelope.signature_base64, "base64");
  assert.equal(payload.toString("base64"), envelope.payload_base64);
  assert.equal(signature.toString("base64"), envelope.signature_base64);
  assert.equal(signature.length, 64);
  assert.ok(
    verify(
      null,
      Buffer.concat([
        Buffer.from(`${legacySchemas[channel]}\0${envelope.key_id}\0`),
        payload,
      ]),
      key,
      signature
    ),
    "Legacy publisher signature does not verify"
  );
  const snapshot = JSON.parse(payload.toString("utf-8"));
  assert.equal(snapshot.schema, legacySchemas[channel]);
  assert.equal(snapshot.catalog_id, "lenso-official-v2");
  assert.ok(Number.isSafeInteger(snapshot.revision) && snapshot.revision > 0);
  assert.ok(
    Number.isSafeInteger(snapshot.issued_at) && snapshot.issued_at <= now
  );
  assert.ok(
    Number.isSafeInteger(snapshot.expires_at) && snapshot.expires_at > now
  );
  assert.ok(Array.isArray(snapshot.releases));
  return { payload, snapshot };
};

export const migrateVerifiedSources = (sources, now, checkpoint) => {
  assert.equal(checkpoint.schema, "lenso.site.catalog-checkpoints.v1");
  assert.equal(checkpoint.catalog_id, "lenso-official-v2");
  const releases = [];
  const legacySources = {};
  for (const channel of channels) {
    const { payload, snapshot } = verifyLegacyEnvelope(
      sources[channel],
      channel,
      now
    );
    legacySources[channel] = {
      payload_digest: `sha256:${digest(payload)}`,
      revision: snapshot.revision,
      schema: snapshot.schema,
    };
    const prior = checkpoint[channel];
    assert.ok(prior, "Authoritative channel history checkpoint is required");
    assert.equal(prior.catalog_id, snapshot.catalog_id);
    assert.equal(
      prior.revision,
      snapshot.revision,
      "Initial migration requires the reviewed history revision"
    );
    assert.equal(
      prior.payload_digest,
      legacySources[channel].payload_digest,
      "Reviewed payload history changed"
    );
    assert.deepEqual(
      Object.keys(prior.release_identities).toSorted(),
      snapshot.releases
        .map((record) => `${record.plugin_id}@${record.version}`)
        .toSorted(),
      "Initial migration must preserve the complete recorded release history"
    );
    for (const record of snapshot.releases) {
      releases.push({
        channel,
        plugin_id: record.plugin_id,
        record,
        version: record.version,
      });
    }
  }
  assert.equal(
    releases.length,
    6,
    "Initial migration must preserve the six reviewed identities"
  );
  const statuses = releases.map(({ plugin_id, version, record }) => ({
    plugin_id,
    state: record.availability || "listed",
    version,
  }));
  return validateCatalog({
    catalog_id: "lenso-official-v2",
    issued_at: now,
    legacy_sources: legacySources,
    releases,
    revision: 1,
    schema,
    statuses,
  });
};

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  const [
    portable,
    linked,
    packagePath,
    content,
    checkpointPath,
    nowText,
    ...extra
  ] = process.argv.slice(2);
  assert.equal(extra.length, 0);
  const paths = {
    linked_cargo: linked,
    package: packagePath,
    portable,
    release_content: content,
  };
  const sources = Object.fromEntries(
    await Promise.all(
      channels.map(async (channel) => [
        channel,
        await readBoundedFile(paths[channel]),
      ])
    )
  );
  const checkpointBytes = await readBoundedFile(checkpointPath);
  const checkpoint = JSON.parse(checkpointBytes.toString("utf-8"));
  console.log(
    JSON.stringify(
      migrateVerifiedSources(sources, Number(nowText), checkpoint),
      null,
      2
    )
  );
}
