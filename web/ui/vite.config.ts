import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import stylex from "@stylexjs/unplugin/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  root: import.meta.dirname,
  plugins: [
    // Catalog styles must not reuse the independently numbered package CSS layers.
    stylex({ devMode: "off", useCSSLayers: false }),
    react(),
    {
      name: "marketplace-source-fingerprint",
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
          "../../../../pnpm-lock.yaml",
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
    },
  ],
  build: {
    outDir: "dist",
    emptyOutDir: true,
    cssCodeSplit: false,
    rolldownOptions: {
      output: {
        entryFileNames: "marketplace.js",
        assetFileNames: "marketplace.[ext]",
      },
    },
  },
});
