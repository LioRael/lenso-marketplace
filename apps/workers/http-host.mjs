import { createHttpHandler } from "@lenso/workers-runtime/http";
import { createEventRunner } from "@lenso/workers-runtime/runner";

import { clearTimers } from "./clock.mjs";
import {
  initSync,
  __wbg_reset_state,
  handle_http,
} from "./pkg/lenso_marketplace_workers_host.js";
import module from "./pkg/lenso_marketplace_workers_host_bg.wasm";
import { createStorageScope } from "./storage-scope.mjs";
import { createStorage } from "./storage.mjs";

export const createMarketplaceWorker = ({
  diagnostics = false,
  onReceipt,
} = {}) => {
  const runner = createEventRunner({
    clearTimers,
    eventLimitMs: 5000,
    instantiate: () => initSync({ module }),
    maxConcurrent: 1,
    resetState: __wbg_reset_state,
    retirementAdmissionLimit: 16,
  });
  return {
    fetch(request, env) {
      const handler = createHttpHandler({
        bodyReadTimeoutMs: 5000,
        createScope() {
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
                    console.error(
                      "Marketplace storage:",
                      args[0],
                      String(error)
                    );
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
        handleHttp: handle_http,
        maxRequestBodyBytes: 65536,
        maxRequestHeadBytes: 16384,
        maxResponseBodyBytes: 4 * 1024 * 1024,
        onReceipt,
        run: async (...args) => {
          try {
            return await runner.run(...args);
          } catch (error) {
            if (diagnostics) {
              console.error("Marketplace host:", String(error));
            }
            throw error;
          }
        },
      });
      return handler(request);
    },
  };
};
