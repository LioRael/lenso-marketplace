import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import stylex from "@stylexjs/unplugin/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

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
          "index.html",
          "vite.config.ts",
          "tsconfig.json",
          "../../pnpm-lock.yaml",
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
