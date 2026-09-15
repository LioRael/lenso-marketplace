/* eslint-disable promise/avoid-new, promise/prefer-await-to-then, promise/prefer-await-to-callbacks, promise/prefer-catch -- Explicit detachable resolvers fence late native completions after Wasm reset. Awaiting them would retain the abandoned continuation; paired then handlers also own native rejection immediately. */
import { createCancellationScope } from "./runtime/http.mjs";

export const createStorageScope = (storageFactory, configuration) => {
  const controller = new AbortController();
  const storage = storageFactory(controller.signal);
  const pending = new Set();
  let live = true;
  const forwards = new Set();
  const scope = createCancellationScope({
    configuration,
    storage(operation, input) {
      const promise = storage(operation, input);
      pending.add(promise);
      // Register cleanup in the request that owns this storage operation.
      void promise
        .finally(() => pending.delete(promise))
        .catch(() => {
          /* Native rejection is forwarded separately while cleanup is observed. */
        });
      // Native D1/R2 operations cannot always be aborted. An abandoned
      // generation must never receive their late Promise continuations.
      return new Promise((resolve, reject) => {
        const forward = { reject, resolve };
        forwards.add(forward);
        const finish = (method, value) => {
          const callback = forward[method];
          forward.resolve = undefined;
          forward.reject = undefined;
          forwards.delete(forward);
          if (live) {
            return callback?.(value);
          }
        };
        promise.then(
          (value) => finish("resolve", value),
          (error) => finish("reject", error)
        );
      });
    },
  });
  const cancel = scope.abort;
  scope.abort = () => {
    cancel();
    controller.abort();
  };
  const { invalidate } = scope;
  scope.invalidate = () => {
    live = false;
    for (const forward of forwards) {
      forward.resolve = undefined;
      forward.reject = undefined;
    }
    forwards.clear();
    invalidate();
  };
  let settlement;
  scope.settled = () =>
    (settlement ??= (async () => {
      let timer;
      try {
        return await Promise.race([
          Promise.allSettled(pending).then(() => true),
          new Promise((resolve) => {
            timer = setTimeout(() => resolve(false), 250);
          }),
        ]);
      } finally {
        clearTimeout(timer);
      }
    })());
  return scope;
};
