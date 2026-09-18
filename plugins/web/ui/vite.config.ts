import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import stylex from "@stylexjs/unplugin/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const sampleAssets = [
  "projects.png",
  "observe.png",
  "git.png",
  "files.png",
  "notes.png",
  "echo.png",
  "projects-preview.png",
];

const contentSecurityPolicy =
  "default-src 'self'; img-src 'self' https:; script-src 'self'; style-src 'self'; style-src-elem 'self' 'sha256-kLmvWqfziFavKtqHqRsb90f006UAK2Dmd0It5Iz2KFA='; object-src 'none'; base-uri 'none'; frame-ancestors 'none'";

export default defineConfig({
  build: {
    cssCodeSplit: false,
    emptyOutDir: true,
    outDir: "dist",
    rolldownOptions: {
      output: {
        assetFileNames: "marketplace.[ext]",
        entryFileNames: "marketplace.js",
      },
    },
  },
  plugins: [
    // Catalog styles must not reuse the independently numbered package CSS layers.
    stylex({ devMode: "off", useCSSLayers: false }),
    react(),
    {
      closeBundle() {
        const output = join(import.meta.dirname, "dist");
        const sampleOutput = join(output, "sample-assets");
        mkdirSync(sampleOutput, { recursive: true });
        for (const assetFile of sampleAssets) {
          copyFileSync(
            join(import.meta.dirname, "assets", assetFile),
            join(sampleOutput, assetFile)
          );
        }
        writeFileSync(
          join(output, "_headers"),
          `/*\n  Cache-Control: no-cache\n  Content-Security-Policy: ${contentSecurityPolicy}\n  X-Content-Type-Options: nosniff\n`
        );
        const hash = createHash("sha256");
        for (const file of [
          "main.tsx",
          "app.tsx",
          "components.tsx",
          "model.ts",
          "navigation.ts",
          "catalog.ts",
          "saved.ts",
          "sample.ts",
          "controls.ts",
          "catalog-layout.stylex.ts",
          "marketplace.css",
          ...sampleAssets.map((assetFile) => `assets/${assetFile}`),
          "index.html",
          "vite.config.ts",
          "tsconfig.json",
          "../../../pnpm-lock.yaml",
        ]) {
          hash
            .update(file)
            .update("\0")
            .update(readFileSync(join(import.meta.dirname, file)))
            .update("\0");
        }
        writeFileSync(
          join(import.meta.dirname, "dist/source.sha256"),
          hash.digest("hex")
        );
      },
      name: "marketplace-source-fingerprint",
    },
  ],
  root: import.meta.dirname,
});
