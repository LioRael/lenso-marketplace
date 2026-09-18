const ARTIFACT_PATH = /^\/artifacts\/([a-f0-9]{64})\.lenso-plugin$/u;
const MAX_ARTIFACT_BYTES = 256 * 1024 * 1024;

/**
 * Serve an immutable bundle directly from the Marketplace R2 bucket. Agent
 * acquisition deliberately does not follow redirects, so this route is the
 * stable 200-origin for every signed artifact URL.
 */
export const artifactResponse = async (request, env) => {
  const match = new URL(request.url).pathname.match(ARTIFACT_PATH);
  if (!match) {
    return null;
  }
  if (request.method !== "GET" && request.method !== "HEAD") {
    return new Response("Method Not Allowed", {
      headers: { allow: "GET, HEAD" },
      status: 405,
    });
  }
  if (!env?.MARKETPLACE_OBJECTS?.get) {
    return new Response("Artifact storage unavailable", { status: 503 });
  }
  const object = await env.MARKETPLACE_OBJECTS.get(
    `artifacts/${match[1]}.lenso-plugin`
  );
  if (!object) {
    return new Response("Artifact not found", { status: 404 });
  }
  if (
    !Number.isSafeInteger(object.size) ||
    object.size < 1 ||
    object.size > MAX_ARTIFACT_BYTES
  ) {
    return new Response("Artifact exceeds supported bounds", { status: 502 });
  }
  return new Response(request.method === "HEAD" ? null : object.body, {
    headers: {
      "cache-control": "public, max-age=31536000, immutable",
      "content-length": String(object.size),
      "content-type": "application/vnd.lenso.plugin",
    },
    status: 200,
  });
};
