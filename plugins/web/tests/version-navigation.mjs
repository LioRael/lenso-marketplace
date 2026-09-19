import assert from "node:assert/strict";

import { chromium } from "playwright";

const base = process.env.MARKETPLACE_TEST_URL;
assert.ok(base, "Set MARKETPLACE_TEST_URL to a running UI preview");
const browser = await chromium.launch();
try {
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  const release = {
    artifact: {
      digest: "sha256:fixture",
      manifest_digest: "sha256:fixture",
      size: 1,
    },
    availability: "listed",
    description: "Version navigation fixture",
    license: "MIT",
    plugin_id: "example.echo",
    publisher_id: "example",
    source_revision: "fixture",
    source_url: "https://example.com",
    summary: "Echo text",
    title: "Echo",
    version: "1.10.0",
  };
  const versions = ["1.10.0", "1.9.0"].map((version) => ({
    availability: "listed",
    version,
  }));
  await page.route("**/api/marketplace/v1/plugins**", (route) => {
    const path = new URL(route.request().url()).pathname.split("/");
    const exact = path.length === 7;
    return route.fulfill({
      contentType: "application/json",
      json: {
        cached: false,
        catalog_id: "version-fixture",
        ...(exact
          ? { release: { ...release, version: path[6] }, versions }
          : { releases: [release], total: 1 }),
      },
    });
  });
  await page.goto(base);
  await page.getByText("1 plugin", { exact: true }).waitFor();
  await page.getByRole("link", { exact: true, name: "Echo 1.10.0" }).click();
  const picker = page.getByRole("combobox", { exact: true, name: "Version" });
  await picker.click();
  await page.getByRole("option", { exact: true, name: "1.9.0" }).click();
  await page.waitForFunction(
    () => document.title === "Echo 1.9.0 · Lenso Marketplace"
  );
  assert.equal(new URL(page.url()).searchParams.get("version"), "1.9.0");
  await page.reload();
  await page.getByRole("combobox", { name: "Version" }).waitFor();
  assert.equal(await page.title(), "Echo 1.9.0 · Lenso Marketplace");
  await page.goBack();
  await page.waitForFunction(
    () => document.title === "Echo 1.10.0 · Lenso Marketplace"
  );
  await picker.focus();
  await picker.press("ArrowDown");
  await page.getByRole("option", { exact: true, name: "1.9.0" }).waitFor();
  await page.keyboard.press("Escape");
  assert.deepEqual(errors, []);
  console.log(
    "Version selection, direct reload, history and keyboard navigation passed"
  );
} finally {
  await browser.close();
}
