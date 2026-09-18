import { createWorkersHttpHost } from "@lenso/workers-runtime/host";

import { artifactResponse } from "./artifacts.mjs";
import * as bindings from "./pkg/lenso_marketplace_workers_host.js";
import wasmModule from "./pkg/lenso_marketplace_workers_host_bg.wasm";
import { createStorageScope } from "./storage-scope.mjs";
import { createStorage } from "./storage.mjs";

// A browser navigation loads the document, module, stylesheet, images and
// catalog data concurrently. Keep a bounded Wasm admission window large
// enough for one navigation while retaining the runtime's hard upper bound.
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
      return artifact ?? host.fetch(request, env, ctx);
    },
  });
};
