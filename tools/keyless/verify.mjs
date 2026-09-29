import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { pathToFileURL } from "node:url";

export const publisher = Object.freeze({
  repository: "LioRael/lenso-marketplace",
  workflow: ".github/workflows/publish-keyless-catalog.yml",
  ref: "refs/heads/main",
});

export function verificationArguments(artifact, bundle, sourceCommit) {
  if (!/^[a-f0-9]{40}$/u.test(sourceCommit)) {
    throw new Error("An exact reviewed source commit is required");
  }
  if (!artifact || !bundle || artifact.startsWith("-") || bundle.startsWith("-")) {
    throw new Error("Local artifact and bundle paths are required");
  }
  return [
    "attestation", "verify", artifact,
    "--hostname", "github.com",
    "--bundle", bundle,
    "--repo", publisher.repository,
    "--signer-repo", publisher.repository,
    "--signer-workflow", `${publisher.repository}/${publisher.workflow}`,
    "--cert-identity", `https://github.com/${publisher.repository}/${publisher.workflow}@${publisher.ref}`,
    "--cert-oidc-issuer", "https://token.actions.githubusercontent.com",
    "--source-ref", publisher.ref,
    "--source-digest", sourceCommit,
    "--signer-digest", sourceCommit,
    "--predicate-type", "https://slsa.dev/provenance/v1",
    "--deny-self-hosted-runners",
    "--format", "json",
  ];
}

export function digest(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

export async function verifyArtifact(artifact, bundle, sourceCommit, expectedDigest) {
  if (!/^[a-f0-9]{64}$/u.test(expectedDigest)) {
    throw new Error("An exact reviewed catalog SHA-256 is required");
  }
  if (digest(await readFile(artifact)) !== expectedDigest) {
    throw new Error("Catalog bytes do not match the reviewed digest");
  }
  const result = spawnSync("gh", verificationArguments(artifact, bundle, sourceCommit), {
    encoding: "utf8",
    timeout: 60_000,
    maxBuffer: 8 * 1024 * 1024,
  });
  if (result.error || result.status !== 0) {
    throw new Error(`Attestation verification failed: ${result.error?.message ?? result.stderr}`);
  }
  const verified = JSON.parse(result.stdout);
  if (!Array.isArray(verified) || verified.length === 0) {
    throw new Error("Verifier returned no verified attestation");
  }
  return verified;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [artifact, bundle, sourceCommit, expectedDigest, ...extra] = process.argv.slice(2);
  if (extra.length > 0) throw new Error("Unexpected verification arguments");
  await verifyArtifact(artifact, bundle, sourceCommit, expectedDigest);
  console.log("Catalog digest and publisher attestation verified");
}
