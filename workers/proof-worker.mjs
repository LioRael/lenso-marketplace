import { createStorageScope } from "./storage-scope.mjs";
import { createStorage } from "./storage.mjs";
// Disposable G3 storage proof only. Production configuration uses worker.mjs.
import publicWorker from "./worker.mjs";

const CATALOG = "workers-g3-proof";
const DATABASE = "2b913921-3f0a-43b4-ae8f-cb219c21db81";
const BUCKET = "lenso-marketplace-g3-proof";
const MAX_INPUT = 128 * 1024;
const encoder = new TextEncoder();

const sha256 = async (bytes) =>
  new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
const hex = (bytes) =>
  Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
const equalHashes = (left, right) => {
  let difference = 0;
  for (let index = 0; index < 32; index += 1) {
    // eslint-disable-next-line no-bitwise -- Compare all 32 digest bytes without a data-dependent early exit.
    difference |= left[index] ^ right[index];
  }
  return difference === 0;
};
const canonical = (value) => {
  if (Array.isArray(value)) {
    return value.map(canonical);
  }
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.keys(value)
        .toSorted()
        .map((key) => [key, canonical(value[key])])
    );
  }
  return value;
};
const equalState = (left, right) =>
  JSON.stringify(canonical(left)) === JSON.stringify(canonical(right));
const response = (status, value) =>
  Response.json(value, {
    headers: {
      "cache-control": "no-store",
      "x-content-type-options": "nosniff",
    },
    status,
  });

const readInput = async (request) => {
  if (!request.body) {
    throw new Error("missing body");
  }
  const reader = request.body.getReader();
  let timer;
  let onAbort;
  // eslint-disable-next-line promise/avoid-new -- Bridge request abort and timer callbacks into the body-read race.
  const interrupted = new Promise((_resolve, reject) => {
    onAbort = () => reject(new Error("request aborted"));
    request.signal.addEventListener("abort", onAbort, { once: true });
    timer = setTimeout(() => reject(new Error("body deadline")), 5000);
  });
  let size = 0;
  const chunks = [];
  try {
    for (;;) {
      request.signal.throwIfAborted();
      const { value, done } = await Promise.race([reader.read(), interrupted]);
      if (done) {
        break;
      }
      size += value.byteLength;
      if (size > MAX_INPUT) {
        throw new Error("input bound");
      }
      chunks.push(value);
    }
    const bytes = new Uint8Array(size);
    let offset = 0;
    for (const chunk of chunks) {
      bytes.set(chunk, offset);
      offset += chunk.byteLength;
    }
    return bytes;
  } finally {
    clearTimeout(timer);
    request.signal.removeEventListener("abort", onAbort);
    try {
      await reader.cancel();
    } catch {
      /* Reader release is required even after cancellation failure. */
    }
    reader.releaseLock();
  }
};

const durableState = async (env, storage) => {
  const pointer = await env.MARKETPLACE_DB.prepare(
    "SELECT catalog_id,token,object_key FROM marketplace_accepted WHERE catalog_id=?"
  )
    .bind(CATALOG)
    .first();
  if (!pointer) {
    throw new Error("accepted pointer missing");
  }
  // The adapter checks whole-object content addressing and envelope/token identity.
  const state = JSON.parse(
    await storage("accepted", JSON.stringify({ catalog: CATALOG }))
  );
  const object = await env.MARKETPLACE_OBJECTS.head(pointer.object_key);
  if (!state || !object || state.token !== pointer.token) {
    throw new Error("state changed while inspecting");
  }
  return {
    evidence: {
      checkpoint_revision: state.checkpoint.revision,
      history_sha256: hex(
        await sha256(
          encoder.encode(
            JSON.stringify(canonical(state.checkpoint.release_identities))
          )
        )
      ),
      history_size: Object.keys(state.checkpoint.release_identities).length,
      object: { etag: object.etag, key: pointer.object_key, size: object.size },
      pointer,
    },
    state,
  };
};

const invalidFixture = (fixture, now) =>
  fixture.version !== 1 ||
  fixture.catalog !== CATALOG ||
  now < fixture.verified_at ||
  now >= fixture.valid_until ||
  fixture.expected.checkpoint.revision !== 5 ||
  fixture.candidates.length !== 2 ||
  fixture.candidates[0].checkpoint.revision !== 6 ||
  fixture.candidates[1].checkpoint.revision !== 7 ||
  fixture.candidates[0].token === fixture.candidates[1].token;

const outsideProofResources = (env) =>
  env.CATALOG_ID !== CATALOG ||
  env.G3_PROOF_DATABASE_ID !== DATABASE ||
  env.G3_PROOF_BUCKET_NAME !== BUCKET ||
  env.PROOF_DIAGNOSTICS !== "1";

const race = async (request, env) => {
  if (outsideProofResources(env)) {
    return response(404, { error: "not_found" });
  }
  const secret = env.G3_PROOF_KEY;
  const authorization = request.headers.get("authorization") ?? "";
  if (
    typeof secret !== "string" ||
    secret.length < 32 ||
    secret.length > 256 ||
    authorization.length > 300 ||
    !authorization.startsWith("Bearer ")
  ) {
    return response(403, { error: "forbidden" });
  }
  const supplied = authorization.slice(7);
  if (
    !equalHashes(
      await sha256(encoder.encode(supplied)),
      await sha256(encoder.encode(secret))
    )
  ) {
    return response(403, { error: "forbidden" });
  }
  if (request.method !== "POST" || new URL(request.url).search) {
    return response(405, { error: "method_not_allowed" });
  }
  if (!/^[0-9a-f]{64}$/u.test(env.G3_CAS_FIXTURE_SHA256 ?? "")) {
    return response(503, { error: "fixture_unconfigured" });
  }
  let fixture;
  try {
    const bytes = await readInput(request);
    // Pin exact bytes produced and verified by Rust; no JS signature implementation.
    if (hex(await sha256(bytes)) !== env.G3_CAS_FIXTURE_SHA256) {
      return response(400, { error: "fixture_mismatch" });
    }
    fixture = JSON.parse(
      new TextDecoder("utf-8", { fatal: true }).decode(bytes)
    );
    const now = Math.floor(Date.now() / 1000);
    if (invalidFixture(fixture, now)) {
      return response(400, { error: "fixture_invalid" });
    }
  } catch {
    return response(400, { error: "invalid_input" });
  }

  const scope = createStorageScope(
    (signal) =>
      createStorage(env.MARKETPLACE_DB, env.MARKETPLACE_OBJECTS, signal),
    "{}"
  );
  const abort = () => scope.abort();
  request.signal.addEventListener("abort", abort, { once: true });
  if (request.signal.aborted) {
    abort();
  }
  const storage = (...args) => scope.storage(...args);
  const operation = async () => {
    const before = await durableState(env, storage);
    if (!equalState(before.state, fixture.expected)) {
      return response(409, { error: "expected_state_changed" });
    }
    const attempt = (value) =>
      storage(
        "compare_exchange",
        JSON.stringify({
          catalog: CATALOG,
          expected: fixture.expected.token,
          value,
        })
      );
    // Both promises use the same observed token, without an intervening reread.
    // allSettled keeps ownership of the peer if either operation fails.
    const attempts = await Promise.allSettled(fixture.candidates.map(attempt));
    if (attempts.some((result) => result.status !== "fulfilled")) {
      throw new Error("write outcome uncertain");
    }
    const results = attempts.map((result) => JSON.parse(result.value));
    if (
      results.filter((value) => value === true).length !== 1 ||
      results.filter((value) => value === false).length !== 1
    ) {
      throw new Error("CAS winner mismatch");
    }
    const winner = results.indexOf(true);
    const after = await durableState(env, storage);
    if (!equalState(after.state, fixture.candidates[winner])) {
      throw new Error("durable winner mismatch");
    }
    const stale = JSON.parse(await attempt(fixture.candidates[1 - winner]));
    const final = await durableState(env, storage);
    if (stale !== false || !equalState(final.state, after.state)) {
      throw new Error("stale write changed winner");
    }
    return response(200, {
      after: after.evidence,
      before: before.evidence,
      candidates: fixture.candidates.map((value) => ({
        revision: value.checkpoint.revision,
        token: value.token,
      })),
      expected_token: fixture.expected.token,
      final: final.evidence,
      fixture_sha256: env.G3_CAS_FIXTURE_SHA256,
      passed: true,
      results,
      stale_result: stale,
      winner,
    });
  };
  let timer;
  let outcome;
  try {
    outcome = await Promise.race([
      operation(),
      // eslint-disable-next-line promise/avoid-new -- The timer must cancel request-owned storage before rejecting the race.
      new Promise((_resolve, reject) => {
        timer = setTimeout(() => {
          scope.abort();
          reject(new Error("proof deadline"));
        }, 10000);
      }),
    ]);
  } catch {
    // Do not replay or claim rollback after an ambiguous write.
    outcome = response(503, {
      error: "cas_proof_failed",
      write_outcome: "inspect_durable_state_before_retry",
    });
  } finally {
    clearTimeout(timer);
    scope.abort();
    request.signal.removeEventListener("abort", abort);
    const settled = await scope.settled();
    scope.invalidate();
    if (!settled) {
      outcome = response(503, {
        error: "cas_cleanup_unconfirmed",
        write_outcome: "inspect_durable_state_before_retry",
      });
    }
  }
  return outcome;
};

export default {
  fetch(request, env, context) {
    if (new URL(request.url).pathname === "/__proof/cas") {
      return race(request, env);
    }
    return publicWorker.fetch(request, env, context);
  },
};
