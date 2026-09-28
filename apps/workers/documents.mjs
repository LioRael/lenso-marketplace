const DOCUMENT_PATH = /^\/documents\/sha256\/([a-f0-9]{64})\.md$/u;
const DIGEST = /^sha256:[a-f0-9]{64}$/u;
const MAX_DOCUMENT = 1024 * 1024;
const MAX_ENVELOPE = 4 * 1024 * 1024;
const decoder = new TextDecoder("utf-8", { fatal: true });
const sources = [
  [
    "release-details",
    "marketplace_release_details",
    "lenso.marketplace.release-details.v1",
  ],
  [
    "linked-cargo",
    "marketplace_linked_cargo",
    "lenso.marketplace.linked-cargo-snapshot.v1",
  ],
  ["packages", "marketplace_packages", "lenso.marketplace.package-snapshot.v1"],
  [
    "release-content",
    "marketplace_release_content",
    "lenso.marketplace.release-content.v2",
  ],
];

const sha256 = async (bytes) =>
  `sha256:${Array.from(
    new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
    (byte) => byte.toString(16).padStart(2, "0")
  ).join("")}`;

const readBounded = async (bucket, key, limit) => {
  const object = await bucket.get(key);
  if (!object) {
    return null;
  }
  if (
    !object.body ||
    !Number.isSafeInteger(object.size) ||
    object.size > limit
  ) {
    throw new Error("invalid document object size");
  }
  const reader = object.body.getReader();
  const chunks = [];
  let size = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) {
        break;
      }
      size += value.byteLength;
      if (size > limit) {
        throw new Error("document object exceeds bound");
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

const signedDocument = (bytes, catalogId, revision, schema, wanted) => {
  const envelope = JSON.parse(decoder.decode(bytes));
  const encoded = envelope?.payload_base64;
  if (typeof encoded !== "string" || encoded.length > MAX_ENVELOPE * 2) {
    throw new Error("invalid published envelope");
  }
  // The protected publisher already verified this exact envelope before its
  // digest became the current D1 pointer. This read only extracts an identity;
  // it does not add a second signature policy to the public Worker.
  const payload = JSON.parse(
    decoder.decode(
      Uint8Array.from(atob(encoded), (char) => char.codePointAt(0))
    )
  );
  if (
    payload?.schema !== schema ||
    payload.catalog_id !== catalogId ||
    payload.revision !== revision ||
    !Array.isArray(payload.releases)
  ) {
    throw new Error("published document snapshot mismatch");
  }
  for (const release of payload.releases) {
    const documents =
      release?.metadata?.documentation ?? release?.documentation;
    if (!Array.isArray(documents)) {
      continue;
    }
    for (const document of documents) {
      if (document?.digest !== wanted) {
        continue;
      }
      if (
        document.media_type !== "text/markdown" ||
        !Number.isSafeInteger(document.size) ||
        document.size < 1 ||
        document.size > MAX_DOCUMENT
      ) {
        throw new Error("invalid published document metadata");
      }
      return document;
    }
  }
  return null;
};

const sourceDocument = async (env, source, wanted, digest) => {
  const [channel, table, schema] = source;
  const pointer = await env.MARKETPLACE_DB.prepare(
    `SELECT revision,object_key,digest FROM ${table} WHERE catalog_id=?`
  )
    .bind(env.CATALOG_ID)
    .first();
  if (!pointer) {
    return null;
  }
  if (
    !Number.isSafeInteger(pointer.revision) ||
    pointer.revision < 1 ||
    !DIGEST.test(pointer.digest) ||
    pointer.object_key !==
      `${channel}/${encodeURIComponent(env.CATALOG_ID)}/${pointer.digest.slice(7)}.json`
  ) {
    throw new Error("invalid publication pointer");
  }
  const envelopeBytes = await readBounded(
    env.MARKETPLACE_OBJECTS,
    pointer.object_key,
    MAX_ENVELOPE
  );
  if (!envelopeBytes || (await sha256(envelopeBytes)) !== pointer.digest) {
    throw new Error("published envelope integrity failure");
  }
  const document = signedDocument(
    envelopeBytes,
    env.CATALOG_ID,
    pointer.revision,
    schema,
    wanted
  );
  if (!document) {
    return null;
  }
  const body = await readBounded(
    env.MARKETPLACE_OBJECTS,
    `documents/sha256/${digest}.md`,
    MAX_DOCUMENT
  );
  if (
    !body ||
    body.byteLength !== document.size ||
    (await sha256(body)) !== wanted
  ) {
    throw new Error("published document body integrity failure");
  }
  decoder.decode(body);
  return body;
};

export const documentResponse = async (request, env) => {
  const match = new URL(request.url).pathname.match(DOCUMENT_PATH);
  if (!match) {
    return null;
  }
  if (request.method !== "GET" && request.method !== "HEAD") {
    return new Response("Method Not Allowed", {
      headers: { allow: "GET, HEAD" },
      status: 405,
    });
  }
  if (
    !env?.MARKETPLACE_DB?.prepare ||
    !env?.MARKETPLACE_OBJECTS?.get ||
    typeof env.CATALOG_ID !== "string" ||
    !env.CATALOG_ID
  ) {
    return new Response("Document storage unavailable", { status: 503 });
  }
  const wanted = `sha256:${match[1]}`;
  try {
    for (const source of sources) {
      const body = await sourceDocument(env, source, wanted, match[1]);
      if (!body) {
        continue;
      }
      return new Response(request.method === "HEAD" ? null : body, {
        headers: {
          "cache-control": "no-store",
          "content-length": String(body.byteLength),
          "content-security-policy": "default-src 'none'; sandbox",
          "content-type": "text/markdown; charset=utf-8",
          "x-content-type-options": "nosniff",
          "x-lenso-document-digest": wanted,
        },
        status: 200,
      });
    }
    return new Response("Document not found", { status: 404 });
  } catch {
    return new Response("Verified document unavailable", { status: 503 });
  }
};
