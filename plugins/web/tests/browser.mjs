// Real Host regression for catalog/detail navigation, selection races,
// saved collection, filtering and responsive document behavior. Multi-release data
// below is explicitly presentation-only; the first checks use the signed catalog.
import assert from "node:assert/strict";

import { chromium } from "playwright";

const base = process.env.MARKETPLACE_TEST_URL;
assert.ok(base, "Set MARKETPLACE_TEST_URL to a running fixture Host");
const browser = await chromium.launch();
try {
  const page = await browser.newPage({
    colorScheme: "light",
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
  const catalog = page.getByRole("region", { name: "Plugin catalog" });
  const detail = page.getByRole("region", { name: "Release details" });
  const echo = catalog.getByRole("link", { name: /Echo.*0.1.0/u });
  await page.addInitScript(() => {
    if (!sessionStorage.getItem("lenso-marketplace-theme")) {
      sessionStorage.setItem("lenso-marketplace-theme", "light");
    }
  });
  await page.goto(base);
  await echo.waitFor();
  const liveResponse = await page.request.get(
    `${base}/api/marketplace/v1/plugins`
  );
  const live = await liveResponse.json();
  const sampleArt = await page.request.get(
    `${base}/sample-assets/projects.png`
  );
  assert.equal(sampleArt.status(), 200);
  assert.ok(sampleArt.headers()["content-type"].startsWith("image/png"));
  const missingArt = await page.request.get(
    `${base}/sample-assets/missing.png`
  );
  assert.equal(missingArt.status(), 404);

  for (const query of [
    "publisher=missing",
    "license=missing",
    "ids=",
    "ids=missing%401.0.0",
    "q=missing",
  ]) {
    const response = await page.request.get(
      `${base}/api/marketplace/v1/plugins?${query}`
    );
    assert.equal(response.status(), 200);
    const value = await response.json();
    assert.equal(value.total, 0);
    assert.deepEqual(value.releases, []);
    assert.deepEqual(value.publishers, ["test-publisher"]);
  }
  // A marker proves navigation did not replace the document.
  await page.evaluate(() => {
    window.marketplaceNavigationProof = "same-document";
  });
  const resting = await echo.boundingBox();
  await echo.hover();
  assert.deepEqual(await echo.boundingBox(), resting);
  await echo.focus();
  await echo.press("Tab");
  await page.keyboard.press("Shift+Tab");
  assert.ok(await echo.evaluate((el) => el === document.activeElement));
  assert.notEqual(
    await echo.evaluate((el) => getComputedStyle(el, "::after").outlineStyle),
    "none"
  );
  assert.ok(
    await echo.evaluate((element) => {
      const ring = getComputedStyle(element, "::after");
      const outset =
        Number(ring.outlineWidth.replace("px", "")) +
        Number(ring.outlineOffset.replace("px", ""));
      const row = element.closest("article").getBoundingClientRect();
      const viewport = element
        .closest('[aria-label="Search results"]')
        .getBoundingClientRect();
      return (
        row.top - outset >= viewport.top &&
        row.bottom + outset <= viewport.bottom
      );
    }),
    "The first release focus ring must fit inside the scrolling viewport"
  );
  // The whole highlighted row opens the release, including its icon. Publisher
  // links and Save remain separate targets, covered below by their own actions.
  const iconBounds = await catalog
    .locator('[data-slot="catalog-media"]')
    .first()
    .boundingBox();
  await page.mouse.click(
    iconBounds.x + iconBounds.width / 2,
    iconBounds.y + iconBounds.height / 2
  );
  await detail
    .getByRole("heading", { exact: true, level: 1, name: "Echo" })
    .waitFor();
  await page.getByRole("link", { exact: true, name: "← All plugins" }).click();
  await echo.waitFor();
  await echo.press("Enter");
  await detail
    .getByRole("heading", { exact: true, level: 1, name: "Echo" })
    .waitFor();
  assert.equal(await catalog.isVisible(), false);
  assert.equal(
    await page.evaluate(() => window.marketplaceNavigationProof),
    "same-document"
  );
  await detail
    .getByRole("link", { exact: true, name: "← All plugins" })
    .click();
  assert.ok(await echo.evaluate((el) => el === document.activeElement));
  await page.goBack();
  await detail
    .getByRole("heading", { exact: true, level: 1, name: "Echo" })
    .waitFor();
  await page.reload();
  await detail
    .getByRole("heading", { exact: true, level: 1, name: "Echo" })
    .waitFor();
  assert.equal(await page.title(), "Echo 0.1.0 · Lenso Marketplace");
  const integrity = detail.getByRole("button", { name: "Release integrity" });
  await integrity.focus();
  await integrity.press("Enter");
  assert.equal(await integrity.getAttribute("aria-expanded"), "true");
  await detail.getByText("Archive SHA-256", { exact: true }).waitFor();
  await integrity.press("Enter");
  await detail.getByRole("button", { exact: true, name: "Save Echo" }).click();
  await page.getByRole("link", { name: /^Saved plugins/u }).click();
  await echo.waitFor();
  await catalog
    .getByRole("button", { exact: true, name: "Unsave Echo" })
    .click();
  await catalog
    .getByRole("heading", { name: "No saved plugins yet" })
    .waitFor();
  // Consumer link resets must not override the library's action contrast.
  const actionContrast = await catalog
    .getByRole("link", { name: "Show all plugins" })
    .evaluate((element) => {
      const style = getComputedStyle(element);
      // Runs in the browser realm, so the helper must remain inside evaluate.
      // eslint-disable-next-line unicorn/consistent-function-scoping
      const luminance = (color) => {
        const values = color
          .match(/[\d.]+/gu)
          .slice(0, 3)
          .map(Number)
          .map((value) => {
            const channel = value / 255;
            return channel <= 0.04045
              ? channel / 12.92
              : ((channel + 0.055) / 1.055) ** 2.4;
          });
        return values[0] * 0.2126 + values[1] * 0.7152 + values[2] * 0.0722;
      };
      const foreground = luminance(style.color);
      const background = luminance(style.backgroundColor);
      return (
        (Math.max(foreground, background) + 0.05) /
        (Math.min(foreground, background) + 0.05)
      );
    });
  assert.ok(
    actionContrast >= 4.5,
    `Empty-state action contrast is ${actionContrast}`
  );
  const undo = page.getByRole("button", { exact: true, name: "Undo" });
  assert.ok(await undo.evaluate((el) => el === document.activeElement));
  await undo.click();
  await echo.waitFor();
  await page.getByRole("link", { exact: true, name: "Browse" }).click();
  assert.equal(await catalog.getByRole("combobox").count(), 0);
  await catalog
    .getByRole("link", { exact: true, name: "test-publisher" })
    .click();
  await catalog.getByRole("combobox", { name: "Publishers" }).waitFor();
  await page.goto(`${base}/?publisher=test-publisher&license=MIT`);
  await catalog.getByRole("combobox", { name: "Licenses" }).waitFor();
  await catalog
    .getByRole("link", { name: "Remove license filter: MIT" })
    .click();
  assert.equal(
    new URL(page.url()).searchParams.get("publisher"),
    "test-publisher"
  );
  const search = catalog.getByRole("textbox", { name: "Search plugins" });
  await search.fill("Echo");
  await search.press("Enter");
  await echo.waitFor();
  await catalog
    .getByRole("button", { exact: true, name: "Clear search" })
    .click();
  assert.equal(new URL(page.url()).searchParams.get("q"), null);
  assert.equal(
    new URL(page.url()).searchParams.get("publisher"),
    "test-publisher"
  );
  assert.ok(await search.evaluate((el) => el === document.activeElement));
  await search.fill("draft");
  await search.press("Escape");
  assert.equal(await search.inputValue(), "");
  await search.fill("中文");
  await search.dispatchEvent("keydown", {
    code: "Enter",
    isComposing: true,
    key: "Enter",
  });
  assert.equal(new URL(page.url()).searchParams.get("q"), null);
  await catalog
    .getByRole("textbox", { name: "Search plugins" })
    .fill("missing");
  await catalog.getByRole("button", { exact: true, name: "Search" }).click();
  await catalog.getByRole("heading", { name: "No matching plugins" }).waitFor();
  await catalog
    .getByRole("link", { exact: true, name: "Clear all filters" })
    .click();
  await echo.waitFor();
  await echo.click();
  await detail
    .getByRole("heading", { exact: true, level: 1, name: "Echo" })
    .waitFor();
  if (process.env.MARKETPLACE_SCREENSHOTS) {
    await page.screenshot({
      path: `${process.env.MARKETPLACE_SCREENSHOTS}/marketplace-desktop.png`,
    });
  }
  await page.getByRole("button", { name: "Switch to dark theme" }).click();
  await page.setViewportSize({ height: 844, width: 390 });
  assert.equal(await catalog.isVisible(), false);
  assert.ok(await detail.isVisible());
  const heading = detail.getByRole("heading", {
    exact: true,
    level: 1,
    name: "Echo",
  });
  const before = await heading.boundingBox();
  await detail
    .getByRole("button", { exact: true, name: "Unsave Echo" })
    .click();
  assert.deepEqual(await heading.boundingBox(), before);
  const dismiss = page.getByRole("button", { name: "Dismiss notification" });
  const bounds = await dismiss.boundingBox();
  assert.ok(
    bounds.x >= 0 &&
      bounds.x + bounds.width <= 390 &&
      bounds.y + bounds.height <= 844
  );
  await dismiss.click();
  assert.ok(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth
    )
  );
  if (process.env.MARKETPLACE_SCREENSHOTS) {
    await page.screenshot({
      fullPage: true,
      path: `${process.env.MARKETPLACE_SCREENSHOTS}/marketplace-detail-dark.png`,
    });
  }
  // Narrow layouts must keep the metadata and integrity action reachable below the body.
  await integrity.focus();
  await integrity.press("Enter");
  await detail.getByText("Archive SHA-256", { exact: true }).waitFor();
  await integrity.press("Enter");
  await detail
    .getByRole("link", { exact: true, name: "← All plugins" })
    .click();
  assert.ok(await catalog.isVisible());
  await page.route("**/api/marketplace/v1/plugins?*", (route) =>
    route.fulfill({ body: "{}", status: 503 })
  );
  await page.reload();
  await catalog.getByRole("heading", { name: "Catalog unavailable" }).waitFor();
  await page.unroute("**/api/marketplace/v1/plugins?*");
  await catalog.getByRole("button", { name: "Try again" }).click();
  await echo.waitFor();
  await page.goto(`${base}/?plugin=example.missing&version=1.0.0`);
  await detail.getByRole("heading", { name: "Release not found" }).waitFor();
  // Explicit multi-release UI fixture: no production inventory is added.
  const samples = Array.from({ length: 30 }, (_, index) => ({
    ...live.releases[0],
    description: "Publisher text: <img src=x onerror=alert(1)>\n\n第二段说明。",
    plugin_id: `fixture.plugin${index}`,
    summary:
      index === 1
        ? "长描述 / Multilingual plugin metadata remains readable across narrow layouts.".repeat(
            3
          )
        : `Presentation fixture ${index + 1}; not a published marketplace release.`,
    title: `Sample plugin ${index + 1}`,
  }));
  await page.route("**/api/marketplace/v1/plugins?*", (route) =>
    route.fulfill({
      body: JSON.stringify({ ...live, releases: samples, total: 30 }),
      contentType: "application/json",
    })
  );
  const waiting = Promise.withResolvers();
  await page.route(
    "**/api/marketplace/v1/plugins/fixture.plugin*/*",
    (route) => {
      const id = new URL(route.request().url()).pathname.split("/").at(-2);
      if (id === "fixture.plugin0") {
        waiting.resolve(route);
        return;
      }
      return route.fulfill({
        body: JSON.stringify({
          ...live,
          release: samples.find((item) => item.plugin_id === id),
        }),
        contentType: "application/json",
      });
    }
  );
  await page.setViewportSize({ height: 900, width: 1280 });
  await page.goto(base);
  await catalog
    .getByRole("heading", { exact: true, name: "Sample plugin 1" })
    .waitFor();
  assert.equal(
    await catalog.getByRole("button", { exact: true, name: "Next" }).count(),
    0
  );
  await catalog.getByRole("link", { name: /^Sample plugin 1 0/u }).click();
  await detail.getByRole("heading", { name: "Loading release" }).waitFor();
  const slow = await waiting.promise;
  await detail
    .getByRole("link", { exact: true, name: "← All plugins" })
    .click();
  await catalog.getByRole("link", { name: /^Sample plugin 2 0/u }).click();
  await detail
    .getByRole("heading", { exact: true, level: 1, name: "Sample plugin 2" })
    .waitFor();
  await detail
    .getByText("Publisher text: <img src=x onerror=alert(1)>", { exact: true })
    .waitFor();
  assert.equal(
    await detail
      .getByRole("region", { exact: true, name: "Overview" })
      .locator("img")
      .count(),
    0
  );
  if (slow) {
    await slow
      .fulfill({
        body: JSON.stringify({ ...live, release: samples[0] }),
        contentType: "application/json",
      })
      .catch(() => {
        /* The superseded request may already be cancelled by Chromium. */
      });
  }
  assert.equal(
    await detail.getByRole("heading", { level: 1 }).textContent(),
    "Sample plugin 2"
  );
  await detail
    .getByRole("link", { exact: true, name: "← All plugins" })
    .click();
  if (process.env.MARKETPLACE_SCREENSHOTS) {
    await page.screenshot({
      path: `${process.env.MARKETPLACE_SCREENSHOTS}/marketplace-sample-catalog.png`,
    });
  }
  const scroller = catalog.getByRole("region", { name: "Search results" });
  await catalog.getByRole("link", { name: /^Sample plugin 20 0/u }).click();
  await detail
    .getByRole("heading", { exact: true, level: 1, name: "Sample plugin 20" })
    .waitFor();
  // The catalog is hidden on detail routes; restoration is checked after return.
  await detail
    .getByRole("link", { exact: true, name: "← All plugins" })
    .click();
  const scrollBefore = await scroller.evaluate((element) => element.scrollTop);
  await catalog.getByRole("link", { name: /^Sample plugin 20 0/u }).click();
  await detail
    .getByRole("heading", { exact: true, level: 1, name: "Sample plugin 20" })
    .waitFor();
  assert.ok(scrollBefore > 0);
  await detail
    .getByRole("link", { exact: true, name: "← All plugins" })
    .click();
  assert.equal(
    await scroller.evaluate((element) => element.scrollTop),
    scrollBefore
  );
  await catalog.getByRole("link", { name: /^Sample plugin 2 0/u }).click();
  await detail
    .getByRole("heading", { exact: true, level: 1, name: "Sample plugin 2" })
    .waitFor();
  await page.setViewportSize({ height: 844, width: 390 });
  assert.ok(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth
    )
  );
  if (process.env.MARKETPLACE_SCREENSHOTS) {
    await page.screenshot({
      fullPage: true,
      path: `${process.env.MARKETPLACE_SCREENSHOTS}/marketplace-long-narrow.png`,
    });
  }
  // Selected design uses an explicitly separate sample catalog and saved collection.
  await page.unrouteAll({ behavior: "wait" });
  let sampleRequests = 0;
  page.on("request", (request) => {
    if (request.url().includes("/sample-api/")) {
      sampleRequests += 1;
    }
  });
  await page.setViewportSize({ height: 880, width: 803 });
  await page.goto(`${base}/?catalog=sample`);
  await catalog
    .getByRole("link", { exact: true, name: "Projects 0.1.0" })
    .waitFor();
  assert.equal(await catalog.locator('[data-slot="catalog-item"]').count(), 6);

  assert.ok(
    await page
      .getByRole("link", { exact: true, name: "Saved plugins 0" })
      .isVisible()
  );
  const sampleSearch = page.getByRole("textbox", { name: "Search plugins" });
  await sampleSearch.fill("Notes");
  await sampleSearch.press("Enter");
  assert.equal(await catalog.locator('[data-slot="catalog-item"]').count(), 1);
  await page.getByRole("button", { exact: true, name: "Clear search" }).click();
  assert.equal(await catalog.locator('[data-slot="catalog-item"]').count(), 6);
  await page.reload();

  if (process.env.MARKETPLACE_SCREENSHOTS) {
    await page.screenshot({
      path: `${process.env.MARKETPLACE_SCREENSHOTS}/lenso-catalog-catalog.png`,
    });
  }
  await catalog
    .getByRole("link", { exact: true, name: "Projects 0.1.0" })
    .click();
  await detail
    .getByRole("heading", { exact: true, name: "Projects" })
    .waitFor();
  const mainColumn = detail.getByRole("region", {
    exact: true,
    name: "Overview",
  });
  const asideColumn = detail.getByRole("complementary", {
    exact: true,
    name: "Release information",
  });
  const mainBounds = await mainColumn.boundingBox();
  const asideBounds = await asideColumn.boundingBox();
  assert.ok(
    asideBounds.y >= mainBounds.y + mainBounds.height,
    "Sample details must keep their small metadata section in the reading flow"
  );
  assert.equal(
    await asideColumn.evaluate(
      (element) => getComputedStyle(element).borderBlockStartWidth
    ),
    "1px",
    "Catalog detail must render its section boundary after StyleX compilation"
  );
  assert.equal(
    await detail.getByText("sample.projects", { exact: true }).count(),
    0
  );
  await page.reload();
  await detail
    .getByRole("heading", { exact: true, name: "Projects" })
    .waitFor();
  if (process.env.MARKETPLACE_SCREENSHOTS) {
    await page.screenshot({
      path: `${process.env.MARKETPLACE_SCREENSHOTS}/lenso-catalog-detail.png`,
    });
  }
  await detail
    .getByRole("button", { exact: true, name: "Save Projects" })
    .click();
  await page
    .getByRole("link", { exact: true, name: "Saved plugins 1" })
    .click();
  assert.ok(page.url().includes("catalog=sample"));
  assert.equal(await catalog.locator('[data-slot="catalog-item"]').count(), 1);
  await catalog
    .getByRole("link", { exact: true, name: "Projects 0.1.0" })
    .click();
  await page.setViewportSize({ height: 844, width: 390 });
  await page.getByRole("button", { name: "Dismiss notification" }).click();
  const mobileSave = await detail
    .getByRole("button", { exact: true, name: "Unsave Projects" })
    .boundingBox();
  assert.ok(
    mobileSave && mobileSave.x >= 0 && mobileSave.x + mobileSave.width <= 390
  );
  assert.ok(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth
    )
  );
  if (process.env.MARKETPLACE_SCREENSHOTS) {
    await page.screenshot({
      path: `${process.env.MARKETPLACE_SCREENSHOTS}/lenso-catalog-mobile.png`,
    });
  }
  assert.equal(
    sampleRequests,
    0,
    "Sample catalog must remain local presentation data"
  );
  await page
    .getByRole("link", { exact: true, name: "View real catalog" })
    .first()
    .click();
  await echo.waitFor();
  assert.ok(
    await page
      .getByRole("link", { exact: true, name: "Saved plugins 0" })
      .isVisible()
  );
  await page
    .getByRole("link", { exact: true, name: "Explore the sample catalog" })
    .click();
  assert.ok(
    await page
      .getByRole("link", { exact: true, name: "Saved plugins 1" })
      .isVisible()
  );
  // Explicit publisher-media fixture: remote URLs are intercepted, never downloaded.
  const mediaPage = await browser.newPage({
    viewport: { height: 900, width: 1280 },
  });
  mediaPage.on("pageerror", (error) => errors.push(error.message));
  const mediaRelease = {
    ...live.releases[0],
    presentation: {
      getting_started:
        "Open the workspace.\n\n<script>window.publisherExecuted = true</script>",
      icon_url: "https://publisher.example/icon.png",
      screenshots: [
        {
          caption: "Publisher workspace preview",
          url: "https://publisher.example/screen.png",
        },
        {
          caption: "Unavailable publisher preview",
          url: "https://publisher.example/missing.png",
        },
      ],
    },
  };
  const mediaBytes = await sampleArt.body();
  await mediaPage.route("https://publisher.example/**", (route) =>
    route.request().url().endsWith("missing.png")
      ? route.fulfill({ body: "Missing", status: 404 })
      : route.fulfill({ body: mediaBytes, contentType: "image/png" })
  );
  await mediaPage.route("**/api/marketplace/v1/plugins**", (route) =>
    route.fulfill({
      body: JSON.stringify({
        ...live,
        release: mediaRelease,
        releases: [mediaRelease],
        total: 1,
      }),
      contentType: "application/json",
    })
  );
  await mediaPage.goto(
    `${base}/?plugin=${mediaRelease.plugin_id}&version=${mediaRelease.version}`
  );
  const mediaDetail = mediaPage.getByRole("region", {
    name: "Release details",
  });
  await mediaDetail.getByRole("heading", { name: "Getting started" }).waitFor();
  const publisherPreview = mediaDetail.getByRole("img", {
    exact: true,
    name: "Publisher workspace preview",
  });
  await publisherPreview.scrollIntoViewIfNeeded();
  await mediaPage.waitForFunction(() => {
    const img = document.querySelector(".publisher-preview img");
    return img?.complete && img.naturalWidth > 0;
  });
  assert.ok(await publisherPreview.evaluate((img) => img.naturalWidth > 0));
  await mediaDetail
    .locator("figcaption")
    .filter({ hasText: "Unavailable publisher preview" })
    .scrollIntoViewIfNeeded();
  await mediaDetail.getByText("Preview unavailable", { exact: true }).waitFor();
  assert.equal(
    await publisherPreview.getAttribute("referrerpolicy"),
    "no-referrer"
  );
  assert.ok(
    await mediaDetail
      .getByText("<script>window.publisherExecuted = true</script>", {
        exact: true,
      })
      .isVisible()
  );
  assert.equal(
    await mediaPage.evaluate(() => window.publisherExecuted),
    undefined
  );
  mediaRelease.presentation.icon_url = "https://publisher.example/missing.png";
  await mediaPage.reload();
  await mediaDetail.getByRole("heading", { name: "Getting started" }).waitFor();
  await mediaDetail
    .getByRole("heading", { exact: true, name: mediaRelease.title })
    .scrollIntoViewIfNeeded();
  await mediaDetail
    .locator("img.publisher-icon")
    .waitFor({ state: "detached" });
  assert.ok((await mediaDetail.locator("svg.lucide-package").count()) > 0);
  await mediaPage.setViewportSize({ height: 844, width: 390 });
  assert.ok(
    await mediaPage.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth
    )
  );
  await mediaPage.close();
  // A catalog can expire after rendering; the notice must appear without
  // replacing the release list or breaking exact-release navigation.
  const stalePage = await browser.newPage({
    viewport: { height: 844, width: 390 },
  });
  const expiry = Math.floor(Date.now() / 1000) + 3;
  await stalePage.route("**/api/marketplace/v1/plugins**", async (route) => {
    const isDetail =
      new URL(route.request().url()).pathname !== "/api/marketplace/v1/plugins";
    await route.fulfill({
      json: {
        ...live,
        expires_at: expiry,
        stale: false,
        ...(isDetail ? { release: live.releases[0] } : {}),
      },
    });
  });
  await stalePage.goto(base);
  await stalePage
    .getByText("Catalog is out of date", { exact: true })
    .waitFor({ timeout: 6000 });
  await stalePage.getByRole("link", { name: /Echo.*0.1.0/u }).click();
  await stalePage.getByRole("region", { name: "Release details" }).waitFor();
  await stalePage
    .getByRole("region", { name: "Release details" })
    .getByText("Catalog is out of date", { exact: true })
    .waitFor();
  assert.ok(
    await stalePage.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth
    )
  );
  await stalePage.screenshot({
    fullPage: true,
    path: "/tmp/lenso-marketplace-stale-mobile.png",
  });
  await stalePage.close();
  assert.deepEqual(errors, []);
  console.log(
    "Real Host and explicit multi-release UI checks passed: persistent navigation, history, request race, save/undo, filters, narrow/dark and recovery"
  );
} finally {
  await browser.close();
}
