// npm postinstall: bootstrap global settings and refresh known harness assets.
//
// Delegates to compiled bootstrap entries so install-time behavior matches
// CLI repair paths. Soft-fails when dist/ is missing (e.g. dev checkout).

import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const settingsBootstrap = resolve(packageRoot, "dist", "bootstrap-settings.js");
const harnessBootstrap = resolve(packageRoot, "dist", "bootstrap-harness.js");

if (!existsSync(settingsBootstrap)) {
  process.stderr.write(
    "[@keemgunn/quickcall] postinstall: dist/bootstrap-settings.js missing; skip bootstrap (run build first).\n",
  );
  process.exit(0);
}

const settingsResult = spawnSync(process.execPath, [settingsBootstrap, "refresh"], {
  cwd: packageRoot,
  stdio: "inherit",
});

if (settingsResult.status !== 0) {
  process.stderr.write(
    "[@keemgunn/quickcall] postinstall: bootstrap refresh failed; global settings may need repair on next qc run.\n",
  );
}

if (!existsSync(harnessBootstrap)) {
  process.stderr.write(
    "[@keemgunn/quickcall] postinstall: dist/bootstrap-harness.js missing; skip harness refresh (run build first).\n",
  );
  process.exit(0);
}

const harnessResult = spawnSync(process.execPath, [harnessBootstrap, "refresh-known"], {
  cwd: packageRoot,
  stdio: "inherit",
});

if (harnessResult.status !== 0) {
  process.stderr.write(
    "[@keemgunn/quickcall] postinstall: harness refresh-known failed; run 'qc --install-agent-harness' after upgrading harness files.\n",
  );
}

process.exit(0);
