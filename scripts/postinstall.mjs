// npm postinstall: refresh package-owned settings, then known harness assets.
//
// Delegates to native qc-bootstrap so install-time behavior matches CLI repair.
// Soft-fails (warn, exit 0) when the native helper is missing or refresh fails.

import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { resolveNativePath } from "./native.mjs";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

async function hostLibcFamily() {
  if (process.platform !== "linux") {
    return null;
  }
  const detect = await import("detect-libc");
  return detect.familySync();
}

let bootstrapPath;
try {
  bootstrapPath = resolveNativePath({
    packageRoot,
    binary: "qc-bootstrap",
    platform: process.platform,
    arch: process.arch,
    libcFamily: await hostLibcFamily(),
  });
} catch (error) {
  process.stderr.write(
    `[@keemgunn/quickcall] postinstall: ${error.message}; skip bootstrap (run build first).\n`,
  );
  process.exit(0);
}

const settingsResult = spawnSync(bootstrapPath, ["settings", "refresh"], {
  cwd: packageRoot,
  stdio: "inherit",
});

if (settingsResult.status !== 0) {
  process.stderr.write(
    "[@keemgunn/quickcall] postinstall: bootstrap refresh failed; global settings may need repair on next qc run.\n",
  );
}

const harnessResult = spawnSync(bootstrapPath, ["harness", "refresh-known"], {
  cwd: packageRoot,
  stdio: "inherit",
});

if (harnessResult.status !== 0) {
  process.stderr.write(
    "[@keemgunn/quickcall] postinstall: harness refresh-known failed; run 'qc --install-agent-harness' after upgrading harness files.\n",
  );
}

process.exit(0);
