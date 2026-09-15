import { clearTimers } from "./clock.mjs";
import {
  initSync,
  __wbg_reset_state,
  handle_http,
} from "./pkg/lenso_marketplace_workers_host.js";
import module from "./pkg/lenso_marketplace_workers_host_bg.wasm";
import { createHttpHandler } from "./runtime/http.mjs";
import { createEventRunner } from "./runtime/runner.mjs";
import { createStorageScope } from "./storage-scope.mjs";
import { createStorage } from "./storage.mjs";

let proofBoot;
const runner = createEventRunner({
  clearTimers,
  eventLimitMs: 5000,
  instantiate: () => initSync({ module }),
  maxConcurrent: 1,
  resetState: __wbg_reset_state,
  retirementAdmissionLimit: 16,
});
export default {
  fetch(request, env) {
    const handler = createHttpHandler({
      bodyReadTimeoutMs: 5000,
      createScope() {
        return createStorageScope(
          (signal) => {
            const storage = createStorage(
              env.MARKETPLACE_DB,
              env.MARKETPLACE_OBJECTS,
              signal
            );
            return async (...args) => {
              try {
                return await storage(...args);
              } catch (error) {
                if (env.PROOF_DIAGNOSTICS === "1") {
                  console.error(
                    "Marketplace proof storage:",
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
            diagnostics: env.PROOF_DIAGNOSTICS === "1",
            key_id: env.CATALOG_KEY_ID,
            public_key_hex: env.CATALOG_PUBLIC_KEY,
          })
        );
      },
      handleHttp: handle_http,
      maxRequestBodyBytes: 65536,
      maxRequestHeadBytes: 16384,
      maxResponseBodyBytes: 4 * 1024 * 1024,
      onReceipt(receipt, response) {
        if (env.PROOF_DIAGNOSTICS === "1") {
          proofBoot ??= crypto.randomUUID();
          response.headers.set(
            "x-proof-generation",
            String(receipt.generation)
          );
          response.headers.set(
            "x-proof-wasm-memory",
            String(receipt.wasm_memory_bytes)
          );
          response.headers.set("x-proof-boot", proofBoot);
        }
      },
      run: async (...args) => {
        try {
          return await runner.run(...args);
        } catch (error) {
          if (env.PROOF_DIAGNOSTICS === "1") {
            console.error("Marketplace proof host:", String(error));
          }
          throw error;
        }
      },
    });
    return handler(request);
  },
};
