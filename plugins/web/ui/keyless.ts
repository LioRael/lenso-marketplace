import type { Catalog, Release } from "./model";

const limit = 4 * 1024 * 1024;
const currentPath = "/api/marketplace/v3/current";
const sha256 = /^[a-f0-9]{64}$/u;
const digestPattern = /^sha256:[a-f0-9]{64}$/u;
const sourcePattern = /^[a-f0-9]{40}$/u;
type ObjectRecord = Record<string, unknown>;
interface Descriptor {
  path: string;
  sha256: string;
  size: number;
}
interface Head {
  schema: string;
  catalog_id: string;
  revision: number;
  catalog: Descriptor;
  bundle: Descriptor;
  provenance: {
    repository: string;
    workflow: string;
    ref: string;
    source_sha: string;
  };
}

const requireValue = (condition: unknown): void => {
  if (!condition) {
    throw new Error("Invalid current catalog");
  }
};
const object = (value: unknown): ObjectRecord => {
  requireValue(value && typeof value === "object" && !Array.isArray(value));
  return value as ObjectRecord;
};
const text = (value: unknown): string => {
  requireValue(
    typeof value === "string" && value.length > 0 && value.length <= 131072
  );
  return value as string;
};
const integer = (value: unknown, maximum = Number.MAX_SAFE_INTEGER): number => {
  requireValue(
    Number.isSafeInteger(value) && Number(value) > 0 && Number(value) <= maximum
  );
  return value as number;
};
const array = (value: unknown): unknown[] => {
  requireValue(Array.isArray(value) && value.length > 0 && value.length <= 512);
  return value as unknown[];
};
const https = (value: unknown): string => {
  const url = new URL(text(value));
  requireValue(url.protocol === "https:" && !url.username && !url.password);
  return url.href;
};
const digestText = (value: unknown): string => {
  const result = text(value);
  requireValue(digestPattern.test(result));
  return result;
};
const exactKeys = (value: ObjectRecord, keys: string[]) => {
  requireValue(
    Object.keys(value).length === keys.length &&
      keys.every((key) => Object.hasOwn(value, key))
  );
};
const descriptor = (value: unknown): Descriptor => {
  const data = object(value);
  exactKeys(data, ["path", "sha256", "size"]);
  const hash = text(data.sha256);
  requireValue(
    sha256.test(hash) &&
      data.path === `/api/marketplace/v3/objects/${hash}.json`
  );
  return {
    path: text(data.path),
    sha256: hash,
    size: integer(data.size, limit),
  };
};
export const parseHead = (value: unknown): Head => {
  const data = object(value);
  exactKeys(data, [
    "schema",
    "catalog_id",
    "revision",
    "catalog",
    "bundle",
    "provenance",
  ]);
  requireValue(
    data.schema === "lenso.marketplace.keyless-current.v1" &&
      data.catalog_id === "lenso-official-v2"
  );
  const provenance = object(data.provenance);
  exactKeys(provenance, ["repository", "workflow", "ref", "source_sha"]);
  requireValue(
    provenance.repository === "LioRael/lenso-marketplace" &&
      provenance.workflow === ".github/workflows/publish-keyless-catalog.yml" &&
      provenance.ref === "refs/heads/main" &&
      sourcePattern.test(text(provenance.source_sha))
  );
  return {
    bundle: descriptor(data.bundle),
    catalog: descriptor(data.catalog),
    catalog_id: text(data.catalog_id),
    provenance: {
      ref: text(provenance.ref),
      repository: text(provenance.repository),
      source_sha: text(provenance.source_sha),
      workflow: text(provenance.workflow),
    },
    revision: integer(data.revision),
    schema: text(data.schema),
  };
};

const projectRelease = (
  input: unknown,
  statuses: Map<string, string>
): Release => {
  const entry = object(input);
  const record = object(entry.record);
  const plugin = text(entry.plugin_id);
  const version = text(entry.version);
  requireValue(record.plugin_id === plugin && record.version === version);
  const channel = text(entry.channel);
  requireValue(
    ["portable", "linked_cargo", "package", "release_content"].includes(channel)
  );
  const metadata =
    channel === "release_content" ? object(record.metadata) : record;
  const release: Release = {
    availability: statuses.get(`${plugin}@${version}`) ?? "",
    channel: channel as Release["channel"],
    integrity: [],
    license: text(metadata.license),
    plugin_id: plugin,
    publisher_id: text(metadata.publisher_id),
    record,
    source_revision: text(metadata.source_revision),
    source_url: https(metadata.source_url),
    summary: text(metadata.summary),
    title: text(metadata.title),
    version,
  };
  requireValue(release.availability);
  if (typeof metadata.description === "string") {
    release.description = metadata.description;
  }
  if (channel === "portable") {
    const artifact = object(record.artifact);
    https(artifact.url);
    release.artifact = {
      digest: digestText(artifact.digest),
      manifest_digest: digestText(artifact.manifest_digest),
      size: integer(artifact.size),
    };
    release.package_size = release.artifact.size;
    release.integrity = [
      { label: "Archive SHA-256", value: release.artifact.digest },
      { label: "Manifest SHA-256", value: release.artifact.manifest_digest },
    ];
  } else if (channel === "linked_cargo") {
    text(record.package);
    https(record.registry_url);
    for (const target of array(record.targets)) {
      text(target);
    }
    requireValue(
      ["linked_plugin", "host_provided"].includes(text(record.integration))
    );
    release.integrity = [
      { label: "Crate SHA-256", value: digestText(record.crate_digest) },
    ];
  } else if (channel === "package") {
    release.integrity = array(record.distributions).map((value) => {
      const distribution = object(value);
      requireValue(
        distribution.kind === "npm_package" ||
          distribution.kind === "cargo_crate"
      );
      text(distribution.package);
      requireValue(distribution.version === version);
      https(distribution.registry_url);
      for (const target of array(distribution.targets)) {
        text(target);
      }
      return {
        label: `${text(distribution.id)} archive SHA-256`,
        value: digestText(distribution.integrity),
      };
    });
  } else {
    requireValue(
      ["content_only", "portable", "linked_cargo", "package"].includes(
        text(record.base_kind)
      )
    );
    digestText(record.base_release_identity);
    const content = array(record.content).map((value) => {
      const item = object(value);
      requireValue(
        ["editable_template", "development_extension"].includes(text(item.kind))
      );
      https(item.url);
      return {
        label: `${text(item.id)} archive SHA-256`,
        size: integer(item.size),
        value: digestText(item.digest),
      };
    });
    release.integrity = content.map(({ label, value }) => ({ label, value }));
    if (content.length === 1) {
      release.package_size = content[0].size;
    }
  }
  release.integrity?.push({
    label: "Source revision",
    value: release.source_revision,
  });
  return release;
};

export const projectKeylessCatalog = (input: unknown, head: Head): Catalog => {
  const data = object(input);
  requireValue(
    data.schema === "lenso.marketplace.keyless-catalog.v1" &&
      data.catalog_id === head.catalog_id &&
      data.revision === head.revision
  );
  integer(data.issued_at);
  const statuses = new Map<string, string>();
  for (const value of array(data.statuses)) {
    const status = object(value);
    const key = `${text(status.plugin_id)}@${text(status.version)}`;
    requireValue(
      !statuses.has(key) &&
        ["listed", "yanked", "revoked"].includes(text(status.state))
    );
    statuses.set(key, text(status.state));
  }
  const releases = array(data.releases).map((entry) =>
    projectRelease(entry, statuses)
  );
  const identities = new Set(
    releases.map((release) => `${release.plugin_id}@${release.version}`)
  );
  requireValue(
    identities.size === releases.length &&
      identities.size === statuses.size &&
      [...statuses.keys()].every((key) => identities.has(key))
  );
  return {
    cached: false,
    catalog_id: head.catalog_id,
    licenses: [...new Set(releases.map((release) => release.license))],
    publishers: [...new Set(releases.map((release) => release.publisher_id))],
    releases,
    total: releases.length,
  };
};

const readBytes = async (
  response: Response,
  signal: AbortSignal,
  maximum: number
): Promise<Uint8Array<ArrayBuffer>> => {
  requireValue(response.ok && response.body);
  if (!response.body) {
    throw new Error("Current catalog response has no body");
  }
  const reader = response.body.getReader();
  const parts: Uint8Array[] = [];
  let size = 0;
  try {
    for (;;) {
      signal.throwIfAborted();
      const { done, value } = await reader.read();
      signal.throwIfAborted();
      if (done) {
        break;
      }
      size += value.byteLength;
      requireValue(size <= maximum);
      parts.push(value);
    }
    requireValue(size > 0);
    const bytes = new Uint8Array(size);
    let offset = 0;
    for (const part of parts) {
      bytes.set(part, offset);
      offset += part.byteLength;
    }
    return bytes;
  } finally {
    await reader.cancel();
    reader.releaseLock();
  }
};

export const loadKeylessCatalog = async (
  callerSignal: AbortSignal,
  fetcher: typeof fetch = fetch
): Promise<Catalog> => {
  const signal = AbortSignal.any([callerSignal, AbortSignal.timeout(15000)]);
  const options: RequestInit = {
    cache: "no-store",
    credentials: "omit",
    redirect: "error",
    signal,
  };
  const readHead = async () =>
    parseHead(
      JSON.parse(
        new TextDecoder("utf-8", { fatal: true }).decode(
          await readBytes(await fetcher(currentPath, options), signal, 16384)
        )
      )
    );
  const head = await readHead();
  const bytes = await readBytes(
    await fetcher(head.catalog.path, options),
    signal,
    head.catalog.size
  );
  requireValue(bytes.byteLength === head.catalog.size);
  const hash = [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
  requireValue(hash === head.catalog.sha256);
  const catalog = projectKeylessCatalog(
    JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)),
    head
  );
  const rechecked = await readHead();
  requireValue(JSON.stringify(head) === JSON.stringify(rechecked));
  return catalog;
};

export const selectCatalog = (catalog: Catalog, endpoint: string): Catalog => {
  const url = new URL(endpoint, "https://marketplace.lenso.dev");
  const exact = /^\/api\/marketplace\/v3\/display\/([^/]+)\/([^/]+)$/u.exec(
    url.pathname
  );
  const releases = catalog.releases ?? [];
  if (exact) {
    const id = decodeURIComponent(exact[1]);
    const version = decodeURIComponent(exact[2]);
    return {
      ...catalog,
      release: releases.find(
        (release) => release.plugin_id === id && release.version === version
      ),
      versions: releases
        .filter((release) => release.plugin_id === id)
        .map((release) => ({
          availability: release.availability,
          version: release.version,
        })),
    };
  }
  const q = (url.searchParams.get("q") ?? "").toLowerCase();
  const publisher = url.searchParams.get("publisher");
  const license = url.searchParams.get("license");
  const ids = url.searchParams.has("ids")
    ? new Set((url.searchParams.get("ids") ?? "").split(","))
    : null;
  const filtered = releases.filter(
    (release) =>
      (!q ||
        `${release.plugin_id} ${release.title} ${release.summary}`
          .toLowerCase()
          .includes(q)) &&
      (!publisher || release.publisher_id === publisher) &&
      (!license || release.license === license) &&
      (!ids || ids.has(`${release.plugin_id}@${release.version}`))
  );
  const offset = Math.max(
    0,
    Math.trunc(Number(url.searchParams.get("offset") ?? "0")) || 0
  );
  return {
    ...catalog,
    releases: filtered.slice(offset, offset + 30),
    total: filtered.length,
  };
};
