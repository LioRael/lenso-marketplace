import { createEventScope } from "@lenso/workers-runtime";

// Marketplace owns storage operations; the Runtime owns fencing and cleanup.
export const createStorageScope = (storageFactory, configuration) =>
  createEventScope((scope) => {
    const controller = new AbortController();
    const storage = storageFactory(controller.signal, scope);
    return {
      configuration,
      storage(operation, input) {
        return scope.operation(() => ({
          abort: () => controller.abort(),
          promise: storage(operation, input),
        })).promise;
      },
    };
  });
