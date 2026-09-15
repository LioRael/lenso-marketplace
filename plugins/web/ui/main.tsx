import { createRoot } from "react-dom/client";

import { App } from "./app";

import "@lenso/tokens/styles.css";
import "@lenso/ui/preflight.css";
import "@lenso/ui/styles.css";
import "./marketplace.css";

const root = document.querySelector("#root");
if (root) {
  createRoot(root).render(<App />);
}
