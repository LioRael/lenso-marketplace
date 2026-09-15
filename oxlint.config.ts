import { defineConfig } from "oxlint";
import core from "ultracite/oxlint/core";
import react from "ultracite/oxlint/react";

export default defineConfig({
  extends: [core, react],
  ignorePatterns: [
    "**/generated/**",
    "**/pkg/**",
    "**/evidence/**",
    "docs/archive/**",
  ],
  // Ultracite 7.10/Oxlint 1.77 introduced stricter React Compiler and
  // migration rules. Keep the existing Console code contract stable while
  // the dedicated lint-migration work is staged separately.
  rules: {
    "no-await-in-loop": "off",
    "no-unreachable-loop": "off",
    "no-void": "off",
    "prefer-named-capture-group": "off",
    "react/button-has-type": "off",
    "react/function-component-definition": "off",
    "react/jsx-no-constructed-context-values": "off",
    "react/jsx-no-useless-fragment": "off",
    "react/react-compiler": "off",
    "typescript/method-signature-style": "off",
    "unicorn/import-style": "off",
    "unicorn/numeric-separators-style": "off",
    "unicorn/prefer-export-from": "off",
  },
});
