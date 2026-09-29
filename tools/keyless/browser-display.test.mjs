import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  loadKeylessCatalog,
  parseHead,
  projectKeylessCatalog,
  selectCatalog,
} from "../../plugins/web/ui/keyless.ts";

const bytes = readFileSync(new URL("catalog.json", import.meta.url));
const catalog = JSON.parse(bytes);
const hash = createHash("sha256").update(bytes).digest("hex");
const head = {
  bundle: {
    path: `/api/marketplace/v3/objects/${"a".repeat(64)}.json`,
    sha256: "a".repeat(64),
    size: 100,
  },
  catalog: {
    path: `/api/marketplace/v3/objects/${hash}.json`,
    sha256: hash,
    size: bytes.length,
  },
  catalog_id: "lenso-official-v2",
  provenance: {
    ref: "refs/heads/main",
    repository: "LioRael/lenso-marketplace",
    source_sha: "bda1f4f56d299a6a5e09fe94f14cd685c36c539e",
    workflow: ".github/workflows/publish-keyless-catalog.yml",
  },
  revision: catalog.revision,
  schema: "lenso.marketplace.keyless-current.v1",
};

test("projects all six exact identities and honest four-channel integrity", () => {
  const display = projectKeylessCatalog(catalog, parseHead(head));
  assert.equal(display.releases.length, 6);
  assert.deepEqual(
    new Set(display.releases.map((release) => release.channel)),
    new Set(["portable", "linked_cargo", "package", "release_content"])
  );
  for (const release of display.releases) {
    assert.deepEqual(
      release.record,
      catalog.releases.find((item) => item.plugin_id === release.plugin_id)
        .record
    );
    if (release.channel !== "portable") {
      assert.equal(release.artifact, undefined);
    }
    if (release.channel === "package" || release.channel === "linked_cargo") {
      assert.equal(release.package_size, undefined);
    }
  }
  assert.equal(display.expires_at, undefined);
  assert.equal(display.cached, false);
});

test("current status owns display availability and version history", () => {
  const input = structuredClone(catalog);
  input.statuses[0].state = "revoked";
  const display = projectKeylessCatalog(input, parseHead(head));
  const id = input.statuses[0].plugin_id;
  const exact = selectCatalog(
    display,
    `/api/marketplace/v3/display/${id}/${input.statuses[0].version}`
  );
  assert.equal(exact.release.availability, "revoked");
  assert.equal(exact.versions[0].availability, "revoked");
  assert.equal(
    selectCatalog(display, "/api/marketplace/v3/display?q=Knowledge").total,
    2
  );
  assert.equal(
    selectCatalog(display, "/api/marketplace/v3/display?ids=").total,
    0
  );
  assert.equal(
    selectCatalog(display, "/api/marketplace/v3/display/missing/1.0.0").release,
    undefined
  );
});

test("rejects mismatched identities, incomplete statuses and fabricated channel fields", () => {
  for (const mutate of [
    (input) => {
      input.releases[0].record.version = "99.0.0";
    },
    (input) => {
      input.statuses.pop();
    },
    (input) => {
      input.statuses.push(input.statuses[0]);
    },
    (input) => {
      input.releases[1].record.crate_digest = "not-a-hash";
    },
    (input) => {
      input.releases[3].record.distributions[0].version = "99.0.0";
    },
    (input) => {
      input.releases[4].record.content[0].size = 0;
    },
    (input) => {
      input.releases[0].channel = "unknown";
    },
  ]) {
    const input = structuredClone(catalog);
    mutate(input);
    assert.throws(() => projectKeylessCatalog(input, parseHead(head)));
  }
});

test("display loader fetches only canonical head/hash object and rechecks complete head", async () => {
  const paths = [];
  const result = await loadKeylessCatalog(
    new AbortController().signal,
    (path, options) => {
      paths.push(path);
      assert.equal(options.redirect, "error");
      assert.equal(options.credentials, "omit");
      assert.equal(options.cache, "no-store");
      return Promise.resolve(
        new Response(path.endsWith("current") ? JSON.stringify(head) : bytes)
      );
    }
  );
  assert.equal(result.releases.length, 6);
  assert.deepEqual(paths, [
    "/api/marketplace/v3/current",
    head.catalog.path,
    "/api/marketplace/v3/current",
  ]);
  for (const change of [
    (next) => {
      next.revision += 1;
    },
    (next) => {
      next.provenance.source_sha = "c".repeat(40);
    },
    (next) => {
      next.bundle.size += 1;
    },
  ]) {
    let request = 0;
    await assert.rejects(
      loadKeylessCatalog(new AbortController().signal, () => {
        request += 1;
        const next = structuredClone(head);
        if (request === 3) {
          change(next);
        }
        return Promise.resolve(
          new Response(request === 2 ? bytes : JSON.stringify(next))
        );
      })
    );
  }
});

test("rejects substituted bytes, unbounded descriptors and noncanonical origins", async () => {
  await assert.rejects(
    loadKeylessCatalog(new AbortController().signal, (path) =>
      Promise.resolve(
        new Response(
          path.endsWith("current")
            ? JSON.stringify(head)
            : Buffer.alloc(bytes.length, 32)
        )
      )
    )
  );
  for (const change of [
    (next) => {
      next.catalog.path = `https://example.com/${hash}.json`;
    },
    (next) => {
      next.catalog.size = 4194305;
    },
    (next) => {
      next.provenance.repository = "other/repo";
    },
    (next) => {
      next.provenance.ref = "refs/heads/candidate";
    },
    (next) => {
      next.bundle.extra = true;
    },
  ]) {
    const next = structuredClone(head);
    change(next);
    assert.throws(() => parseHead(next));
  }
  await assert.rejects(
    loadKeylessCatalog(new AbortController().signal, () =>
      Promise.resolve(new Response(null, { status: 503 }))
    )
  );
});
