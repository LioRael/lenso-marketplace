import { createWorkersHttpHost } from "@lenso/workers-runtime/host";

import { artifactResponse } from "./artifacts.mjs";
import * as bindings from "./pkg/lenso_marketplace_workers_host.js";
import wasmModule from "./pkg/lenso_marketplace_workers_host_bg.wasm";
import { createStorageScope } from "./storage-scope.mjs";
import { createStorage } from "./storage.mjs";

// Dynamic catalog requests share the Wasm admission window. Keep a bounded
// limit for API bursts while static UI assets are served by Cloudflare Assets.
const MAX_CONCURRENT_EVENTS = 8;

export const createMarketplaceWorker = ({
  diagnostics = false,
  onReceipt,
} = {}) => {
  const host = createWorkersHttpHost({
    bindings,
    createScope(_request, env) {
      return createStorageScope(
        (signal, scope) => {
          const storage = createStorage(
            env.MARKETPLACE_DB,
            env.MARKETPLACE_OBJECTS,
            signal,
            scope
          );
          return async (...args) => {
            try {
              return await storage(...args);
            } catch (error) {
              if (diagnostics) {
                console.error("Marketplace storage:", args[0], String(error));
              }
              throw error;
            }
          };
        },
        JSON.stringify({
          catalog_id: env.CATALOG_ID,
          diagnostics,
          key_id: env.CATALOG_KEY_ID,
          public_key_hex: env.CATALOG_PUBLIC_KEY,
        })
      );
    },
    limits: {
      bodyReadTimeoutMs: 5000,
      eventLimitMs: 5000,
      maxConcurrent: MAX_CONCURRENT_EVENTS,
      maxRequestBodyBytes: 65536,
      maxRequestHeadBytes: 16384,
      maxResponseBodyBytes: 4 * 1024 * 1024,
      retirementAdmissionLimit: 16,
    },
    onReceipt,
    wasmModule,
  });
  return Object.freeze({
    async fetch(request, env, ctx) {
      const artifact = await artifactResponse(request, env);
      if (artifact) {
        return artifact;
      }
      const { url } = request;
      const { pathname } = new URL(url);
      if (!pathname.startsWith("/api/")) {
        const assets = env?.ASSETS;
        // Assets are served before the Worker when they exist. This fallback
        // keeps missing static paths as 404s instead of sending them through
        // the Wasm host as if they were Marketplace API requests.
        return assets?.fetch
          ? assets.fetch(request)
          : new Response("Not Found", { status: 404 });
      }
      return host.fetch(request, env, ctx);
    },
  });
};
