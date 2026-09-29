import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { once } from "node:events";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, resolve } from "node:path";

import { chromium } from "playwright";

// The fixture exercises browser display, not Sigstore verification or adoption.
const root = resolve(import.meta.dirname, "../../..");
const original = JSON.parse(
  await readFile(resolve(root, "tools/keyless/catalog.json"), "utf-8")
);
let fixture = structuredClone(original);
let unavailable = false;
const requests = [];
const fixtureBytes = () => Buffer.from(JSON.stringify(fixture));
const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");
const server = createServer(async (request, response) => {
  try {
    const path = new URL(request.url, "http://localhost").pathname;
    if (path.startsWith("/api/")) {
      requests.push(path);
    }
    if (path === "/api/marketplace/v3/current") {
      if (unavailable) {
        response.writeHead(503);
        response.end();
        return;
      }
      const bytes = fixtureBytes();
      const digest = sha(bytes);
      response.setHeader("cache-control", "no-store");
      response.setHeader("content-type", "application/json");
      response.end(
        JSON.stringify({
          bundle: {
            path: `/api/marketplace/v3/objects/${"a".repeat(64)}.json`,
            sha256: "a".repeat(64),
            size: 100,
          },
          catalog: {
            path: `/api/marketplace/v3/objects/${digest}.json`,
            sha256: digest,
            size: bytes.length,
          },
          catalog_id: fixture.catalog_id,
          provenance: {
            ref: "refs/heads/main",
            repository: "LioRael/lenso-marketplace",
            source_sha: "bda1f4f56d299a6a5e09fe94f14cd685c36c539e",
            workflow: ".github/workflows/publish-keyless-catalog.yml",
          },
          revision: fixture.revision,
          schema: "lenso.marketplace.keyless-current.v1",
        })
      );
      return;
    }
    if (path === `/api/marketplace/v3/objects/${sha(fixtureBytes())}.json`) {
      response.setHeader("content-type", "application/json");
      response.end(fixtureBytes());
      return;
    }
    const dist = resolve(
      process.env.MARKETPLACE_UI_DIST || resolve(root, "plugins/web/ui/dist")
    );
    const file = resolve(dist, `.${path === "/" ? "/index.html" : path}`);
    assert.ok(file.startsWith(`${dist}/`));
    const types = {
      ".css": "text/css",
      ".html": "text/html",
      ".js": "text/javascript",
      ".png": "image/png",
    };
    response.setHeader(
      "content-type",
      types[extname(file)] || "application/octet-stream"
    );
    response.end(await readFile(file));
  } catch {
    response.writeHead(404);
    response.end();
  }
});
server.listen(0, "127.0.0.1");
await once(server, "listening");
const base = `http://127.0.0.1:${server.address().port}`;
let browser;
try {
  browser = await chromium.launch({
    ...(process.env.MARKETPLACE_CHROMIUM_PATH
      ? { executablePath: process.env.MARKETPLACE_CHROMIUM_PATH }
      : {}),
    args: ["--no-sandbox"],
  });
  const page = await browser.newPage({
    viewport: { height: 900, width: 1280 },
  });
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (
      message.type() === "error" &&
      !message.text().includes("Failed to load resource")
    ) {
      errors.push(message.text());
    }
  });
  await page.goto(base);
  await page.getByText("6 plugins", { exact: true }).waitFor();
  assert.equal(page.url(), `${base}/`);
  assert.equal(await page.title(), "Lenso Plugin Marketplace");
  assert.equal(await page.locator("vite-error-overlay").count(), 0);
  if (process.env.MARKETPLACE_BROWSER_EVIDENCE) {
    await page.screenshot({
      path: resolve(process.env.MARKETPLACE_BROWSER_EVIDENCE, "desktop.png"),
    });
  }
  const catalog = page.getByRole("region", { name: "Plugin catalog" });
  const [first] = original.releases;
  const echo = catalog.getByRole("link", {
    exact: true,
    name: `${first.record.title} ${first.version}`,
  });
  await echo.focus();
  await echo.press("Enter");
  await page
    .getByRole("heading", { exact: true, level: 1, name: first.record.title })
    .waitFor();
  await page.reload();
  await page
    .getByRole("heading", { exact: true, level: 1, name: first.record.title })
    .waitFor();
  const picker = page.getByRole("combobox", { name: "Version" });
  await picker.focus();
  await picker.press("ArrowDown");
  await page
    .getByRole("option", { exact: true, name: first.version })
    .waitFor();
  await page.keyboard.press("Escape");

  for (const release of original.releases) {
    const title =
      release.channel === "release_content"
        ? release.record.metadata.title
        : release.record.title;
    await page.goto(
      `${base}/?plugin=${encodeURIComponent(release.plugin_id)}&version=${release.version}`
    );
    await page
      .getByRole("heading", { exact: true, level: 1, name: title })
      .waitFor();
    await page
      .getByRole("button", { exact: true, name: "Release integrity" })
      .click();
    assert.equal(
      await page.getByText("Manifest SHA-256", { exact: true }).count(),
      release.channel === "portable" ? 1 : 0
    );
    assert.equal(
      await page.getByText("Package size", { exact: true }).count(),
      ["portable", "release_content"].includes(release.channel) ? 1 : 0
    );
    await page
      .getByText("The CLI verifies the publisher provenance before adoption.", {
        exact: false,
      })
      .waitFor();
  }
  await page.goBack();
  await page
    .getByRole("heading", {
      exact: true,
      level: 1,
      name: "Knowledge Base Starter",
    })
    .waitFor();
  await page.goForward();
  await page
    .getByRole("heading", {
      exact: true,
      level: 1,
      name: "Terminal CLI Development Support",
    })
    .waitFor();
  await page.goto(base);
  await page.getByRole("textbox", { name: "Search plugins" }).fill("Knowledge");
  await page.getByRole("button", { exact: true, name: "Search" }).click();
  await page.getByText("2 plugins", { exact: true }).waitFor();
  await page.goto(`${base}/?publisher=missing`);
  await page.getByText("No matching plugins", { exact: true }).waitFor();
  await page.goto(`${base}/?plugin=${first.plugin_id}&version=99.0.0`);
  await page
    .getByText("This exact release is not in the current catalog.", {
      exact: true,
    })
    .waitFor();

  fixture.statuses[0].state = "revoked";
  fixture.statuses[1].state = "yanked";
  await page.goto(
    `${base}/?plugin=${first.plugin_id}&version=${first.version}`
  );
  await page.getByText("This release is revoked", { exact: true }).waitFor();
  await page.goto(
    `${base}/?plugin=${fixture.statuses[1].plugin_id}&version=${fixture.statuses[1].version}`
  );
  await page.getByText("This release is yanked", { exact: true }).waitFor();
  unavailable = true;
  await page.evaluate(() =>
    document.dispatchEvent(new Event("visibilitychange"))
  );
  await page
    .getByRole("region", { name: "Release details" })
    .getByText(
      "The current catalog could not be confirmed. Check your connection and try again.",
      { exact: true }
    )
    .waitFor();
  assert.equal(
    await page
      .getByRole("heading", {
        exact: true,
        level: 1,
        name: original.releases[1].record.title,
      })
      .count(),
    0
  );
  unavailable = false;
  await page
    .getByRole("region", { name: "Release details" })
    .getByRole("button", { exact: true, name: "Try again" })
    .click();
  await page.getByText("This release is yanked", { exact: true }).waitFor();
  fixture = structuredClone(original);
  await page.setViewportSize({ height: 844, width: 390 });
  await page.goto(base);
  await page.getByText("6 plugins", { exact: true }).waitFor();
  assert.ok(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth + 1
    )
  );
  if (process.env.MARKETPLACE_BROWSER_EVIDENCE) {
    await page.screenshot({
      path: resolve(process.env.MARKETPLACE_BROWSER_EVIDENCE, "mobile.png"),
    });
  }
  assert.ok(
    requests.every(
      (path) =>
        path === "/api/marketplace/v3/current" ||
        /^\/api\/marketplace\/v3\/objects\/[a-f0-9]{64}\.json$/u.test(path)
    )
  );
  assert.deepEqual(errors, []);
  console.log(
    "PASS canonical v3 display-only browser: six records, four channels, exact/filter/history/status/currentness/retry/keyboard/mobile"
  );
} finally {
  await browser?.close();
  const closed = once(server, "close");
  server.close();
  await closed;
}
