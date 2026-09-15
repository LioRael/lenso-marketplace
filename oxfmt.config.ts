import { defineConfig } from "oxfmt";
import ultracite from "ultracite/oxfmt";

export default defineConfig({
  ...ultracite,
  // Markdown is maintained as authored product documentation; do not let a
  // formatter upgrade rewrite the full documentation history in this PR.
  ignorePatterns: [
    ...(ultracite.ignorePatterns ?? []),
    "**/*.md",
    "**/generated/**",
    "plugins/directory/config.schema.json",
    "plugins/web/config.schema.json",
    "docs/archive/**",
    "tests/workers/proof/cohort.json",
  ],
});
