import { defineConfig } from "oxfmt";
import ultracite from "ultracite/oxfmt";

export default defineConfig({
  ...ultracite,
  // Markdown is maintained as authored product documentation; do not let a
  // formatter upgrade rewrite the full documentation history in this PR.
  ignorePatterns: [
    ...(ultracite.ignorePatterns ?? []),
    "**/*.md",
    "service/**/generated/**",
    "contracts/**/generated/**",
    "plugins/**/generated/**",
    // Rust build output and frozen proof records have byte-level integrity checks.
    "directory/config.schema.json",
    "web/config.schema.json",
    "workers/evidence/**",
    "workers/recovery/evidence/**",
    "workers/proof/cohort.json",
    // These exact deployed configs are hashed by the qualification receipts.
    "workers/wrangler.jsonc",
    "workers/recovery/wrangler.jsonc",
  ],
});
