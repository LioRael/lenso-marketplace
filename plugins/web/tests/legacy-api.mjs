import assert from "node:assert/strict";

const base = process.env.MARKETPLACE_TEST_URL;
assert.ok(base);
const response = await fetch(`${base}/api/marketplace/v1/plugins`);
assert.equal(response.status, 200);
const catalog = await response.json();
assert.equal(catalog.releases.length, 1);
const [release] = catalog.releases;
assert.equal(release.availability, "listed");
assert.ok(release.artifact.digest.startsWith("sha256:"));
const exact = await fetch(
  `${base}/api/marketplace/v1/plugins/${release.plugin_id}/${release.version}`
);
assert.equal(exact.status, 200);
const exactResult = await exact.json();
assert.deepEqual(exactResult.release, release);
const sampleArt = await fetch(`${base}/sample-assets/projects.png`);
assert.equal(sampleArt.status, 200);
assert.ok(sampleArt.headers.get("content-type").startsWith("image/png"));
const missingArt = await fetch(`${base}/sample-assets/missing.png`);
assert.equal(missingArt.status, 404);
for (const query of [
  "publisher=missing",
  "license=missing",
  "ids=",
  "ids=missing%401.0.0",
  "q=missing",
]) {
  const filtered = await fetch(`${base}/api/marketplace/v1/plugins?${query}`);
  assert.equal(filtered.status, 200);
  const result = await filtered.json();
  assert.equal(result.total, 0);
  assert.deepEqual(result.releases, []);
  assert.deepEqual(result.publishers, ["test-publisher"]);
}
console.log(
  "PASS legacy native Directory API fixture, exact release and filters; independent of canonical v3 browser display"
);
