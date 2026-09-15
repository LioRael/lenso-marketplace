// Local-only entry. Never deploy: the runner supplies disposable D1/R2 bindings.
import { createMarketplaceWorker } from "../../../apps/workers/http-host.mjs";

const worker = createMarketplaceWorker({
  onReceipt(receipt, response) {
    response.headers.set(
      "x-d01-receipt",
      JSON.stringify({
        generation: receipt.generation,
        shutdown: receipt.shutdown,
        wasmMemoryBytes: receipt.wasm_memory_bytes,
      })
    );
  },
});

const instrument = (database, bucket, trace, competingWinner) => {
  let winner = competingWinner;
  const statements = new WeakMap();
  const statement = (raw, sql, args = []) => {
    const wrapped = {
      bind(...values) {
        return statement(raw.bind(...values), sql, values);
      },
      async first() {
        const value = await raw.first();
        trace.push({ kind: "d1.first", rowsReturned: value ? 1 : 0, sql });
        return value;
      },
      async run() {
        // Schedule a real competing primary D1 write immediately before CAS.
        // Rust must observe the lost CAS and revalidate a fresh durable winner.
        if (winner) {
          const value = winner;
          winner = undefined;
          const injected = await database
            .prepare(
              "UPDATE marketplace_accepted SET token=?,object_key=? WHERE catalog_id=?"
            )
            .bind(value.token, value.object_key, value.catalog_id)
            .run();
          trace.push({ kind: "fixture.race", meta: injected.meta });
        }
        const result = await raw.run();
        trace.push({ args, kind: "d1.run", meta: result.meta, sql });
        return result;
      },
    };
    statements.set(wrapped, raw);
    return wrapped;
  };
  return {
    bucket: {
      async get(key) {
        const object = await bucket.get(key);
        trace.push({ bytes: object?.size ?? 0, key, kind: "r2.get" });
        return object;
      },
      async put(key, bytes, options) {
        const object = await bucket.put(key, bytes, options);
        trace.push({
          bytes: new TextEncoder().encode(bytes).byteLength,
          created: object !== null,
          key,
          kind: "r2.put",
        });
        return object;
      },
    },
    database: {
      async batch(queries) {
        const results = await database.batch(
          queries.map((query) => statements.get(query))
        );
        trace.push({
          kind: "d1.batch",
          meta: results.map((row) => row.meta),
          rowsReturned: results.reduce(
            (sum, row) => sum + row.results.length,
            0
          ),
          statements: queries.length,
        });
        return results;
      },
      prepare(sql) {
        return statement(database.prepare(sql), sql);
      },
    },
  };
};

export default {
  async fetch(request, env, context) {
    const trace = [];
    const winner = request.headers.get("x-d01-winner");
    const measured = instrument(
      env.MARKETPLACE_DB,
      env.MARKETPLACE_OBJECTS,
      trace,
      winner ? JSON.parse(winner) : undefined
    );
    const response = await worker.fetch(
      request,
      {
        ...env,
        MARKETPLACE_DB: measured.database,
        MARKETPLACE_OBJECTS: measured.bucket,
      },
      context
    );
    response.headers.set("x-d01-storage", JSON.stringify(trace));
    return response;
  },
};
