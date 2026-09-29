import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { constants } from "node:fs";
import { mkdir, mkdtemp, open } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

export const publisher = Object.freeze({
  ref: "refs/heads/main",
  repository: "LioRael/lenso-marketplace",
  workflow: ".github/workflows/publish-keyless-catalog.yml",
});

export const verificationArguments = (artifact, bundle, sourceCommit) => {
  if (!/^[a-f0-9]{40}$/u.test(sourceCommit)) {
    throw new Error("An exact reviewed source commit is required");
  }
  if (
    !artifact ||
    !bundle ||
    artifact.startsWith("-") ||
    bundle.startsWith("-")
  ) {
    throw new Error("Local artifact and bundle paths are required");
  }
  return [
    "attestation",
    "verify",
    artifact,
    "--hostname",
    "github.com",
    "--bundle",
    bundle,
    "--repo",
    publisher.repository,
    "--cert-identity",
    `https://github.com/${publisher.repository}/${publisher.workflow}@${publisher.ref}`,
    "--cert-oidc-issuer",
    "https://token.actions.githubusercontent.com",
    "--source-ref",
    publisher.ref,
    "--source-digest",
    sourceCommit,
    "--signer-digest",
    sourceCommit,
    "--predicate-type",
    "https://slsa.dev/provenance/v1",
    "--deny-self-hosted-runners",
    "--format",
    "json",
  ];
};

export const digest = (bytes) =>
  createHash("sha256").update(bytes).digest("hex");

export const readBoundedFile = async (path, limit = 4 * 1024 * 1024) => {
  const handle = await open(
    path,
    constants.O_RDONLY + (constants.O_NOFOLLOW ?? 0)
  );
  try {
    const info = await handle.stat();
    if (!info.isFile() || info.size <= 0 || info.size > limit) {
      throw new Error("Input must be a bounded non-empty regular file");
    }
    const bytes = Buffer.alloc(limit + 1);
    let offset = 0;
    while (offset < bytes.length) {
      const result = await handle.read(
        bytes,
        offset,
        bytes.length - offset,
        offset
      );
      if (result.bytesRead === 0) {
        break;
      }
      offset += result.bytesRead;
    }
    if (offset !== info.size || offset > limit) {
      throw new Error("Input changed or exceeds byte limit");
    }
    return bytes.subarray(0, offset);
  } finally {
    await handle.close();
  }
};

export const verifierEnvironment = (
  environment = process.env,
  scratchDirectory = ""
) => {
  const scratch =
    scratchDirectory ||
    join(environment.RUNNER_TEMP || tmpdir(), "lenso-keyless-gh");
  return {
    GH_CONFIG_DIR: join(scratch, "config"),
    HOME: scratch,
    PATH: environment.PATH || "/usr/local/bin:/usr/bin:/bin",
    TMPDIR: environment.RUNNER_TEMP || tmpdir(),
    XDG_CACHE_HOME: join(scratch, "cache"),
    ...(environment.GH_TOKEN ? { GH_TOKEN: environment.GH_TOKEN } : {}),
  };
};

export const verifyArtifact = async (
  artifact,
  bundle,
  sourceCommit,
  expectedDigest
) => {
  if (!/^[a-f0-9]{64}$/u.test(expectedDigest)) {
    throw new Error("An exact reviewed catalog SHA-256 is required");
  }
  if (digest(await readBoundedFile(artifact)) !== expectedDigest) {
    throw new Error("Catalog bytes do not match the reviewed digest");
  }
  await readBoundedFile(bundle);
  const scratch = await mkdtemp(
    join(process.env.RUNNER_TEMP || tmpdir(), "lenso-keyless-gh-")
  );
  const environment = verifierEnvironment(process.env, scratch);
  await mkdir(environment.GH_CONFIG_DIR, { mode: 0o700 });
  await mkdir(environment.XDG_CACHE_HOME, { mode: 0o700 });
  const result = spawnSync(
    "gh",
    verificationArguments(artifact, bundle, sourceCommit),
    {
      encoding: "utf-8",
      env: environment,
      maxBuffer: 8 * 1024 * 1024,
      timeout: 60_000,
    }
  );
  if (result.error || result.status !== 0) {
    throw new Error("Publisher attestation verification failed");
  }
  let verified;
  try {
    verified = JSON.parse(result.stdout);
  } catch {
    throw new Error("Publisher verifier returned invalid output");
  }
  if (!Array.isArray(verified) || verified.length === 0) {
    throw new Error("Verifier returned no verified attestation");
  }
  return verified;
};

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  const [artifact, bundle, sourceCommit, expectedDigest, ...extra] =
    process.argv.slice(2);
  if (extra.length > 0) {
    throw new Error("Unexpected verification arguments");
  }
  await verifyArtifact(artifact, bundle, sourceCommit, expectedDigest);
  console.log("Catalog digest and publisher attestation verified");
}
