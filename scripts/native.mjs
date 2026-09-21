#!/usr/bin/env node
// Platform/libc selector only. No qc argument parser.
// Diagnostics on stderr; one absolute native path on stdout.

import { accessSync, constants, existsSync, realpathSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const thisFile = fileURLToPath(import.meta.url);
const defaultPackageRoot = resolve(dirname(thisFile), "..");
const BINARIES = new Set(["qc", "qc-bootstrap"]);

function fail(message) {
  const error = new Error(message);
  error.packaging = true;
  return error;
}

/**
 * Map OS/arch/libc to a Rust target triple.
 * libcFamily is required on linux (glibc | musl); ignored on darwin.
 */
export function resolveTarget({ platform, arch, libcFamily } = {}) {
  if (platform === "darwin") {
    if (arch === "arm64") {
      return "aarch64-apple-darwin";
    }
    if (arch === "x64") {
      return "x86_64-apple-darwin";
    }
    throw fail(`qc: unsupported platform darwin/${arch || "?"}`);
  }

  if (platform === "linux") {
    if (libcFamily !== "glibc" && libcFamily !== "musl") {
      const got = libcFamily == null || libcFamily === "" ? "none" : String(libcFamily);
      throw fail(
        `qc: unsupported or ambiguous libc (${got}); expected glibc or musl`,
      );
    }
    const abi = libcFamily === "musl" ? "musl" : "gnu";
    if (arch === "x64") {
      return `x86_64-unknown-linux-${abi}`;
    }
    if (arch === "arm64") {
      return `aarch64-unknown-linux-${abi}`;
    }
    throw fail(`qc: unsupported platform linux/${arch || "?"}/${libcFamily}`);
  }

  throw fail(`qc: unsupported platform ${platform || "?"}/${arch || "?"}`);
}

export function resolveNativePath({
  packageRoot,
  binary = "qc",
  platform,
  arch,
  libcFamily,
} = {}) {
  if (!BINARIES.has(binary)) {
    throw fail(`qc: unknown native binary '${binary ?? ""}'`);
  }
  const root = resolve(packageRoot ?? defaultPackageRoot);
  const triple = resolveTarget({ platform, arch, libcFamily });
  const selected = join(root, "dist", triple, binary);
  if (!existsSync(selected)) {
    throw fail(`qc: native ${binary} missing for ${triple} at ${selected}`);
  }
  try {
    accessSync(selected, constants.X_OK);
  } catch {
    throw fail(`qc: native ${binary} is not executable: ${selected}`);
  }
  return selected;
}

async function detectLibcFamily() {
  let detect;
  try {
    detect = await import("detect-libc");
  } catch {
    throw fail("qc: detect-libc is not installed; cannot select a Linux binary");
  }
  return detect.familySync();
}

function usage() {
  process.stderr.write(
    "qc: usage: native.mjs --select <qc|qc-bootstrap> [--package-root <dir>]\n",
  );
}

async function main(argv) {
  let binary;
  let packageRoot = defaultPackageRoot;
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--select") {
      binary = argv[index + 1];
      index += 1;
      continue;
    }
    if (arg === "--package-root") {
      packageRoot = argv[index + 1];
      index += 1;
      continue;
    }
    usage();
    process.exit(2);
  }
  if (!binary || !BINARIES.has(binary)) {
    usage();
    process.exit(2);
  }

  const platform = process.platform;
  const arch = process.arch;
  // libc family only affects Linux ELFs; darwin has no glibc/musl choice.
  const libcFamily = platform === "linux" ? await detectLibcFamily() : null;

  try {
    const selected = resolveNativePath({
      packageRoot,
      binary,
      platform,
      arch,
      libcFamily,
    });
    process.stdout.write(`${selected}\n`);
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exit(1);
  }
}

function invokedAsCli() {
  if (process.argv.slice(2).includes("--select")) {
    return true;
  }
  if (!process.argv[1]) {
    return false;
  }
  try {
    return realpathSync(process.argv[1]) === realpathSync(thisFile);
  } catch {
    return false;
  }
}

if (invokedAsCli()) {
  await main(process.argv.slice(2));
}
