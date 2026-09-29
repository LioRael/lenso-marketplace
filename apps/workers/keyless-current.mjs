const PREFIX = "/api/marketplace/v3/";
const MAX_BYTES = 4 * 1024 * 1024;
const MAX_REVISION = Number.MAX_SAFE_INTEGER;
const DIGEST = /^[a-f0-9]{64}$/u;
const COMMIT = /^[a-f0-9]{40}$/u;
const HEAD_SQL =
  "SELECT catalog_id,revision,catalog_digest,catalog_size,bundle_digest,bundle_size,source_sha FROM marketplace_keyless_heads WHERE catalog_id=?";
const headers = (cache) => ({
  "access-control-allow-origin": "*",
  "cache-control": cache,
  "content-type": "application/json; charset=utf-8",
  "x-content-type-options": "nosniff",
});
const errorResponse = (status, error) =>
  Response.json({ error }, { headers: headers("no-store"), status });

const abortable = async (promise, signal) => {
  signal.throwIfAborted();
  const { promise: aborted, reject: rejectAbort } = Promise.withResolvers();
  const onAbort = () => rejectAbort(signal.reason);
  signal.addEventListener("abort", onAbort, { once: true });
  try {
    return await Promise.race([promise, aborted]);
  } finally {
    signal.removeEventListener("abort", onAbort);
  }
};

const validateHead = (row, catalogId) => {
  if (
    row.catalog_id !== catalogId ||
    !Number.isSafeInteger(row.revision) ||
    row.revision < 1 ||
    row.revision > MAX_REVISION ||
    !DIGEST.test(row.catalog_digest) ||
    !DIGEST.test(row.bundle_digest) ||
    !COMMIT.test(row.source_sha) ||
    ![row.catalog_size, row.bundle_size].every(
      (size) => Number.isSafeInteger(size) && size > 0 && size <= MAX_BYTES
    )
  ) {
    throw new Error("invalid keyless head");
  }
};

const boundedObject = async (object, signal) => {
  if (
    !object?.body ||
    !Number.isSafeInteger(object.size) ||
    object.size < 1 ||
    object.size > MAX_BYTES
  ) {
    throw new Error("invalid keyless object size");
  }
  const reader = object.body.getReader();
  const cancel = async () => {
    try {
      await reader.cancel();
    } catch {
      // A failed stream cancellation must not replace the original response error.
    }
  };
  signal.addEventListener("abort", cancel, { once: true });
  try {
    const parts = [];
    let total = 0;
    for (;;) {
      signal.throwIfAborted();
      const { done, value } = await reader.read();
      signal.throwIfAborted();
      if (done) {
        break;
      }
      total += value.byteLength;
      if (total > MAX_BYTES || total > object.size) {
        throw new Error("keyless object exceeds declared size");
      }
      parts.push(value);
    }
    if (total !== object.size) {
      throw new Error("keyless object size mismatch");
    }
    const bytes = new Uint8Array(total);
    let offset = 0;
    for (const part of parts) {
      bytes.set(part, offset);
      offset += part.byteLength;
    }
    return bytes;
  } finally {
    signal.removeEventListener("abort", cancel);
    await cancel();
    reader.releaseLock();
  }
};

const serveObject = async (bucket, wantedDigest, signal, method) => {
  const pending = bucket.get(`keyless/${wantedDigest}.json`);
  // Cancel a late body even when the binding cannot cancel its pending lookup.
  const releaseLateBody = async () => {
    try {
      const lateObject = await pending;
      if (signal.aborted && lateObject?.body) {
        await lateObject.body.cancel();
      }
    } catch {
      // The foreground request owns lookup errors; this task only releases late bodies.
    }
  };
  void releaseLateBody();
  const object = await abortable(pending, signal);
  signal.throwIfAborted();
  if (!object) {
    return errorResponse(404, "keyless_object_not_found");
  }
  const bytes = await boundedObject(object, signal);
  const actual = Array.from(
    new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
    (byte) => byte.toString(16).padStart(2, "0")
  ).join("");
  if (actual !== wantedDigest) {
    throw new Error("keyless object digest mismatch");
  }
  return new Response(method === "HEAD" ? null : bytes, {
    headers: {
      ...headers("public, max-age=31536000, immutable"),
      "content-length": String(bytes.byteLength),
      etag: `"sha256:${wantedDigest}"`,
    },
  });
};

// HTTPS authenticates current status; this pointer never replaces bundle verification.
export const keylessResponse = async (request, env) => {
  const url = new URL(request.url);
  if (!url.pathname.startsWith(PREFIX)) {
    return null;
  }
  if (url.search) {
    return errorResponse(400, "keyless_query_not_supported");
  }
  if (!["GET", "HEAD"].includes(request.method)) {
    const response = errorResponse(405, "method_not_allowed");
    response.headers.set("allow", "GET, HEAD");
    return response;
  }
  const objectMatch =
    /^\/api\/marketplace\/v3\/objects\/([a-f0-9]{64})\.json$/u.exec(
      url.pathname
    );
  if (url.pathname !== `${PREFIX}current` && !objectMatch) {
    return errorResponse(404, "keyless_route_not_found");
  }
  const signal = AbortSignal.any([request.signal, AbortSignal.timeout(5000)]);
  try {
    if (objectMatch) {
      return await serveObject(
        env.MARKETPLACE_OBJECTS,
        objectMatch[1],
        signal,
        request.method
      );
    }
    if (typeof env.CATALOG_ID !== "string" || !env.CATALOG_ID) {
      throw new Error("catalog identity missing");
    }
    const database = env.MARKETPLACE_DB.withSession("first-primary");
    const row = await abortable(
      database.prepare(HEAD_SQL).bind(env.CATALOG_ID).first(),
      signal
    );
    signal.throwIfAborted();
    if (!row) {
      return errorResponse(503, "keyless_catalog_not_promoted");
    }
    validateHead(row, env.CATALOG_ID);
    const descriptor = (sha256, size) => ({
      path: `${PREFIX}objects/${sha256}.json`,
      sha256,
      size,
    });
    const current = {
      bundle: descriptor(row.bundle_digest, row.bundle_size),
      catalog: descriptor(row.catalog_digest, row.catalog_size),
      catalog_id: row.catalog_id,
      provenance: {
        ref: "refs/heads/main",
        repository: "LioRael/lenso-marketplace",
        source_sha: row.source_sha,
        workflow: ".github/workflows/publish-keyless-catalog.yml",
      },
      revision: row.revision,
      schema: "lenso.marketplace.keyless-current.v1",
    };
    return new Response(
      request.method === "HEAD" ? null : JSON.stringify(current),
      {
        headers: headers("no-store"),
      }
    );
  } catch {
    return errorResponse(503, "keyless_status_unavailable");
  }
};
