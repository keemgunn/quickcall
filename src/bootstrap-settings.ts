#!/usr/bin/env node
import { homedir } from "node:os";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { bootstrapSettings, configPaths, type BootstrapMode } from "./config/index.js";

const mode = process.argv[2];
if (mode !== "refresh" && mode !== "repair") {
  process.stderr.write("usage: qc-bootstrap-settings <refresh|repair>\n");
  process.exitCode = 1;
} else {
  const home = process.env.HOME ?? homedir();
  const starter = join(dirname(fileURLToPath(import.meta.url)), "..", "share", "settings", "config.toml");
  await bootstrapSettings(mode as BootstrapMode, configPaths(home, process.cwd(), starter));
}
