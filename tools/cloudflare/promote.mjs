// Private publication operation. The operator supplies its trusted Rust verifier
// and primary D1/R2 bindings; no public route or JS signature policy lives here.
const encoder = new TextEncoder();
const limit = 4 * 1024 * 1024;
const same = (a, b) =>
  a === null
    ? b === null
    : b !== null &&
      a.revision === b.revision &&
      a.object_key === b.object_key &&
      a.digest === b.digest;
const sha256 = async (bytes) =>
  `sha256:${Array.from(
    new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
    (byte) => byte.toString(16).padStart(2, "0")
  ).join("")}`;

const objectBytes = async (bucket, key) => {
  const object = await bucket.get(key);
  if (!object) {
    return null;
  }
  const reader = object.body.getReader();
  const chunks = [];
  let size = 0;
  try {
    if (object.size > limit) {
      throw new Error("publication object exceeds limit");
    }
    for (;;) {
      const { done, value } = await reader.read();
      if (done) {
        break;
      }
      size += value.byteLength;
      if (size > limit) {
        throw new Error("publication object exceeds limit");
      }
      chunks.push(value);
    }
  } finally {
    try {
      await reader.cancel();
    } finally {
      reader.releaseLock();
    }
  }
  const bytes = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return bytes;
};
const equalBytes = (a, b) =>
  a !== null && a.length === b.length && a.every((v, i) => v === b[i]);

const validatePublication = async ({
  catalogId,
  envelope,
  expected,
  now,
  verify,
}) => {
  if (
    typeof envelope !== "string" ||
    !catalogId ||
    typeof verify !== "function"
  ) {
    throw new Error("explicit publication and verifier required");
  }
  const bytes = encoder.encode(envelope);
  if (bytes.byteLength > limit) {
    throw new Error("publication exceeds limit");
  }
  // `verify` is owned by the protected operator and invokes the shared Rust
  // verifier with configured trust. Never accept it from request input.
  const candidate = await verify(envelope);
  if (
    candidate.catalog_id !== catalogId ||
    !Number.isSafeInteger(candidate.revision) ||
    candidate.revision < 1 ||
    !Number.isSafeInteger(candidate.expires_at)
  ) {
    throw new Error("invalid verified publication identity");
  }
  const fresh = () => {
    if (now() >= candidate.expires_at) {
      throw new Error("publication expired before promotion");
    }
  };
  fresh();
  if (
    expected !== null &&
    (!expected ||
      !Number.isSafeInteger(expected.revision) ||
      expected.revision < 1 ||
      typeof expected.object_key !== "string" ||
      typeof expected.digest !== "string")
  ) {
    throw new Error("explicit expected pointer required");
  }
  if (expected && candidate.revision <= expected.revision) {
    throw new Error("publication must advance revision");
  }
  return { bytes, candidate, fresh };
};

const uploadPublication = async (bucket, target, bytes, fresh) => {
  const existing = await objectBytes(bucket, target.object_key);
  if (existing !== null && !equalBytes(existing, bytes)) {
    throw new Error("immutable publication object conflict");
  }
  if (existing === null) {
    fresh();
    try {
      await bucket.put(target.object_key, bytes, {
        onlyIf: { etagDoesNotMatch: "*" },
      });
    } catch (error) {
      // Confirm a possibly committed write. Never retry a mutation blindly.
      if (!equalBytes(await objectBytes(bucket, target.object_key), bytes)) {
        throw new Error("publication object write unconfirmed", {
          cause: error,
        });
      }
    }
  }
  if (!equalBytes(await objectBytes(bucket, target.object_key), bytes)) {
    throw new Error("immutable publication object conflict");
  }
  fresh();
};

export const promotePublication = async ({
  database,
  bucket,
  verify,
  catalogId,
  envelope,
  expected,
  now = () => Math.floor(Date.now() / 1000),
}) => {
  const { bytes, candidate, fresh } = await validatePublication({
    catalogId,
    envelope,
    expected,
    now,
    verify,
  });
  const digest = await sha256(bytes);
  const target = {
    digest,
    object_key: `publications/${encodeURIComponent(catalogId)}/${digest.slice(7)}.json`,
    revision: candidate.revision,
  };
  const read = async () =>
    (await database
      .prepare(
        "SELECT revision,object_key,digest FROM marketplace_publications WHERE catalog_id=?"
      )
      .bind(catalogId)
      .first()) ?? null;
  const current = await read();
  if (same(current, target)) {
    if (!equalBytes(await objectBytes(bucket, target.object_key), bytes)) {
      throw new Error("published object integrity failure");
    }
    fresh();
    return { status: "already_published", ...target };
  }
  if (!same(current, expected)) {
    throw new Error("publication pointer conflict");
  }
  await uploadPublication(bucket, target, bytes, fresh);
  const statement =
    expected === null
      ? database
          .prepare(
            "INSERT INTO marketplace_publications(catalog_id,revision,object_key,digest) VALUES(?,?,?,?) ON CONFLICT(catalog_id) DO NOTHING"
          )
          .bind(catalogId, target.revision, target.object_key, target.digest)
      : database
          .prepare(
            "UPDATE marketplace_publications SET revision=?,object_key=?,digest=? WHERE catalog_id=? AND revision=? AND object_key=? AND digest=?"
          )
          .bind(
            target.revision,
            target.object_key,
            target.digest,
            catalogId,
            expected.revision,
            expected.object_key,
            expected.digest
          );
  let result;
  let uncertain;
  try {
    result = await statement.run();
  } catch (error) {
    uncertain = error;
  }
  const observed = await read();
  if (same(observed, target)) {
    fresh();
    return {
      status:
        uncertain || !result?.success || result.meta?.changes !== 1
          ? "reconciled"
          : "published",
      ...target,
    };
  }
  if (uncertain || !result?.success) {
    throw new Error(
      "publication pointer write unconfirmed; read before retrying",
      { cause: uncertain }
    );
  }
  throw new Error("publication pointer conflict or superseded publication");
};
