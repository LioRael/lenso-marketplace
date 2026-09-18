#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import {
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  writeFile,
} from "node:fs/promises";
import { cpus, platform, release, tmpdir, totalmem } from "node:os";
import { dirname, join, resolve } from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
process.chdir(root);
const output = resolve(
  process.argv[2] ?? `docs/archive/workers-d01/local-${Date.now()}.json`
);
if (process.argv.length > 3) {
  throw new Error(
    "Usage: node tools/workers/profile-matrix.mjs [evidence.json]"
  );
}
if (existsSync(output)) {
  throw new Error(
    "Evidence already exists; choose a new output path to preserve the receipt"
  );
}
const temporary = await mkdtemp(join(tmpdir(), "lenso-d01-"));
const taskOutputs = [
  "node_modules",
  "plugins/web/ui/dist",
  "apps/workers/pkg",
  "apps/workers/.wrangler",
  "tests/workers/profile/.wrangler",
].filter((path) => !existsSync(join(root, path)));
const frameworkCargo = resolve(root, "../../../.lenso-tools/bin/lenso-cargo");
const cargo =
  process.env.LENSO_CARGO ??
  (existsSync(frameworkCargo)
    ? frameworkCargo
    : (process.env.CARGO ?? "cargo"));
const env = {
  ...process.env,
  CARGO: cargo,
  WRANGLER_LOG_PATH: join(temporary, "wrangler.log"),
  WRANGLER_SEND_METRICS: "false",
};
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const identity = (command, args = []) => {
  const result = spawnSync(command, args, {
    cwd: root,
    encoding: "utf-8",
    env,
  });
  if (result.status !== 0) {
    throw new Error(`Identity command failed: ${command} ${args.join(" ")}`);
  }
  return result.stdout.trim();
};
const evidence = {
  checks: [],
  failureCount: 0,
  limits: {
    cold: "New Miniflare/workerd process and loaded artifact; OS file/JIT caches are not flushed",
    counters:
      "Read prefix and full event reported separately; fixture management is excluded",
    d1Rows:
      "Local D1 meta rows_read/rows_written for batch/run; first() exposes only returned rows",
    deploymentClaim: false,
    eventLimitMs: 5000,
    fixtureSetup:
      "Unmeasured local pointer restoration before each sample; changed candidate object absent",
    freshApp:
      "Every HTTP event starts and cleanly shuts down a new Rust Kernel App",
    instrumentation:
      "Local binding wrappers and diagnostic headers add overhead to every cell",
    latency:
      "Node performance.now around loopback HTTP fetch through full response body; not CPU",
    localOnly: true,
    maxConcurrent: 8,
    maxRequestBodyBytes: 65536,
    maxResponseBodyBytes: 4194304,
    memory:
      "Wasm linear memory at event receipt; not total isolate memory or sampled peak",
    percentiles:
      "Nearest rank; p99 of small samples is descriptive, not a production tail estimate",
    r2Bytes:
      "Returned R2 object sizes, including acceptance reads; not network framing or billed bytes",
    remoteResources: false,
    retirementAdmissionLimit: 16,
    signatures:
      "Pinned public synthetic bytes; expired display catalogs, full Rust browse verification",
    startup:
      "Miniflare construction through ready; excludes fixture seeding and first HTTP event",
    warm: "Same process/isolate; generation changes after the existing 16-admission retirement",
  },
  machine: {
    arch: process.arch,
    cpuModel: cpus()[0]?.model,
    logicalCpus: cpus().length,
    platform: platform(),
    release: release(),
    totalMemoryBytes: totalmem(),
  },
  passed: false,
  schema: "lenso.marketplace.workers.d01.v1",
  startedAt: new Date().toISOString(),
  unresolvedIssues: [],
};

const saveProgress = () => {
  evidence.failureCount = evidence.unresolvedIssues.length;
  mkdirSync(dirname(output), { recursive: true });
  writeFileSync(output, `${JSON.stringify(evidence, null, 2)}\n`);
};

const check = (command, args, requiredToRun = false) => {
  process.stderr.write(`D01 check: ${command} ${args.join(" ")}\n`);
  const start = performance.now();
  const result = spawnSync(command, args, {
    cwd: root,
    encoding: "utf-8",
    env,
    maxBuffer: 32 * 1024 * 1024,
    timeout: 600_000,
  });
  const log = `${result.stdout ?? ""}${result.stderr ?? ""}`;
  const record = {
    argv: [command, ...args],
    elapsedMs: performance.now() - start,
    exitCode: result.status,
    outputSha256: hash(log),
    passed: result.status === 0,
    ...(result.status === 0
      ? {}
      : {
          diagnostic: log.slice(-12_000),
          error: result.error?.message ?? null,
        }),
  };
  evidence.checks.push(record);
  if (!record.passed) {
    evidence.unresolvedIssues.push(`Check failed: ${record.argv.join(" ")}`);
    saveProgress();
    if (requiredToRun) {
      throw new Error(evidence.unresolvedIssues.at(-1));
    }
  }
  saveProgress();
};

const identities = async (paths) =>
  Object.fromEntries(
    await Promise.all(
      paths.map(async (path) => {
        const bytes = await readFile(join(root, path));
        return [path, { bytes: bytes.length, sha256: hash(bytes) }];
      })
    )
  );

const artifacts = async (directory) => {
  const records = {};
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      for (const [path, value] of Object.entries(
        await artifacts(join(directory, entry.name))
      )) {
        records[`${entry.name}/${path}`] = value;
      }
    } else {
      const bytes = await readFile(join(directory, entry.name));
      records[entry.name] = { bytes: bytes.length, sha256: hash(bytes) };
    }
  }
  return records;
};

let matrix;
try {
  evidence.git = {
    branch: identity("git", ["branch", "--show-current"]),
    head: identity("git", ["rev-parse", "HEAD"]),
    status: identity("git", ["status", "--short"]),
  };
  const sourcePaths = identity("git", [
    "ls-files",
    "--cached",
    "--others",
    "--exclude-standard",
    "--",
    "apps/workers",
    "plugins/web",
    "plugins/directory",
    "contracts",
    "tests/workers/profile",
    "tools/workers",
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "pnpm-lock.yaml",
  ])
    .split("\n")
    .filter(Boolean)
    .toSorted();
  evidence.sources = await identities(sourcePaths);
  evidence.tools = {
    cargo: identity(cargo, ["+1.94.0", "--version"]),
    node: process.version,
    pnpm: identity("pnpm", ["--version"]),
    rustc: identity("rustc", ["+1.94.0", "--version", "--verbose"]),
    wasmBindgen: identity("wasm-bindgen", ["--version"]),
  };
  check("pnpm", ["install", "--frozen-lockfile"], true);
  for (const name of ["miniflare", "wrangler", "@lenso/workers-runtime"]) {
    const bytes = await readFile(
      join(root, "node_modules", name, "package.json")
    );
    evidence.tools[name] = {
      packageJsonSha256: hash(bytes),
      version: JSON.parse(bytes).version,
    };
  }
  check("pnpm", ["build"], true);
  check("pnpm", ["test:workers"]);
  check(cargo, [
    "+1.94.0",
    "clippy",
    "--locked",
    "-p",
    "lenso-marketplace-workers-host",
    "--target",
    "wasm32-unknown-unknown",
    "--",
    "-D",
    "warnings",
  ]);
  check(cargo, [
    "+1.94.0",
    "test",
    "--locked",
    "-p",
    "lenso-marketplace-web-plugin",
  ]);
  check(cargo, ["+1.94.0", "fmt", "--all", "--", "--check"]);
  check("pnpm", ["build:workers"], true);
  check("pnpm", [
    "exec",
    "wrangler",
    "deploy",
    "--dry-run",
    "--config",
    "apps/workers/wrangler.jsonc",
  ]);
  const artifact = join(temporary, "artifact");
  check(
    "pnpm",
    [
      "exec",
      "wrangler",
      "deploy",
      "--dry-run",
      "--config",
      "tests/workers/profile/wrangler.jsonc",
      "--outdir",
      artifact,
    ],
    true
  );
  // The dry-run source map names repository sources relative to its output
  // directory. When the artifact is placed in an isolated temporary directory,
  // those paths escape workerd's module root. Profiles do not need source maps,
  // so execute and identify the exact portable production bundle instead.
  const bundledWorker = join(artifact, "worker.js");
  const workerSource = await readFile(bundledWorker, "utf-8");
  await writeFile(
    bundledWorker,
    workerSource.replace(/\n\/\/# sourceMappingURL=worker\.js\.map\s*$/u, "\n")
  );
  await rm(join(artifact, "worker.js.map"), { force: true });
  evidence.artifacts = await artifacts(artifact);
  evidence.wasmArtifacts = await artifacts(join(root, "apps/workers/pkg"));
  // Resolve the exact workerd installed with the pinned Miniflare, not a global binary.
  const { createRequire } = await import("node:module");
  const require = createRequire(import.meta.url);
  const miniflareRequire = createRequire(require.resolve("miniflare"));
  const workerd = miniflareRequire("workerd");
  const binary = await readFile(workerd.default);
  evidence.tools.workerd = {
    binarySha256: hash(binary),
    bytes: binary.length,
    version: identity(workerd.default, ["--version"]),
  };
  check("node", ["--test", "tests/workers/profile/matrix.test.mjs"]);
  check("pnpm", ["lint"]);
  check("pnpm", ["format:check"]);
  check("git", ["diff", "--check"]);
  matrix = await import("../../tests/workers/profile/matrix.mjs");
  await matrix.runMatrix({ artifact, evidence, temporary });
  evidence.passed = evidence.checks.every((record) => record.passed);
} catch (error) {
  evidence.unresolvedIssues.push(String(error.stack ?? error));
} finally {
  matrix?.summarizeMatrix(evidence);
  evidence.finishedAt = new Date().toISOString();
  evidence.failureCount = evidence.unresolvedIssues.length;
  evidence.completedSamples =
    evidence.cells?.reduce((sum, cell) => sum + cell.records.length, 0) ?? 0;
  evidence.cleanup = { generatedPathsRemoved: [], temporaryRemoved: false };
  try {
    await rm(temporary, { force: true, recursive: true });
    evidence.cleanup.temporaryRemoved = true;
    for (const path of taskOutputs) {
      await rm(join(root, path), { force: true, recursive: true });
      evidence.cleanup.generatedPathsRemoved.push(path);
    }
  } catch (error) {
    evidence.passed = false;
    evidence.unresolvedIssues.push(`Cleanup failed: ${error}`);
    evidence.failureCount = evidence.unresolvedIssues.length;
  }
  await mkdir(dirname(output), { recursive: true });
  await writeFile(output, `${JSON.stringify(evidence, null, 2)}\n`);
  process.stderr.write(
    `D01 ${evidence.passed ? "passed" : "incomplete"}: ${output}\n`
  );
  process.exitCode = evidence.passed ? 0 : 1;
}
