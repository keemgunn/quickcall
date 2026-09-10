import { access, copyFile, cp, mkdir, rm, writeFile } from "node:fs/promises";
import { constants } from "node:fs";
import { dirname } from "node:path";
import type { ConfigPaths } from "./paths.js";

export type BootstrapMode = "refresh" | "repair";

/** Paths under ~/.qc/ that packaged bootstrap writes into .gitignore. */
export const REFERENCE_GITIGNORE_ENTRIES = [".default-settings/", "sessions/"] as const;

/** Repair path used by normal qc invocations; refresh is for postinstall/install-current. */
export async function bootstrapGlobal(paths: ConfigPaths): Promise<void> {
  await bootstrapSettings("repair", paths);
}

async function pathExists(path: string): Promise<boolean> {
  try {
    await access(path, constants.F_OK);
    return true;
  } catch (cause: unknown) {
    if ((cause as NodeJS.ErrnoException).code === "ENOENT") return false;
    throw cause;
  }
}

/** Create ~/.qc through an existing symlink without replacing that inode. */
async function ensureGlobalRoot(paths: ConfigPaths): Promise<void> {
  await mkdir(paths.globalRoot, { recursive: true });
}

/** Force-replace the packaged settings reference mirror under ~/.qc/.default-settings/. */
async function refreshDefaultSettings(paths: ConfigPaths): Promise<void> {
  await ensureGlobalRoot(paths);
  await rm(paths.defaultSettings, { recursive: true, force: true });
  await cp(paths.bundledSettings, paths.defaultSettings, { recursive: true, force: true });
}

/** Overwrite ~/.qc/.gitignore with package-owned ignore entries. */
async function refreshGitignore(paths: ConfigPaths): Promise<void> {
  await ensureGlobalRoot(paths);
  // Session mappings are runtime state; keep them out of a versioned ~/.qc/.
  await writeFile(paths.gitignore, `${REFERENCE_GITIGNORE_ENTRIES.join("\n")}\n`);
}

/** Create ~/.qc/.gitignore only when absent (repair mode). */
async function ensureGitignore(paths: ConfigPaths): Promise<void> {
  if (await pathExists(paths.gitignore)) return;
  await refreshGitignore(paths);
}

/** Copy packaged config.toml when the bootstrap sentinel is missing. Never overwrites. */
async function seedConfig(paths: ConfigPaths): Promise<void> {
  await ensureGlobalRoot(paths);
  await mkdir(dirname(paths.global), { recursive: true });
  try {
    await copyFile(paths.starter, paths.global, constants.COPYFILE_EXCL);
  } catch (cause: unknown) {
    if (!(cause instanceof Error) || (cause as NodeJS.ErrnoException).code !== "EEXIST") throw cause;
  }
}

/** Wipe and recopy packaged sample prompts into ~/.qc/prompts/samples/. */
export async function hardRefreshSamplePrompts(paths: ConfigPaths): Promise<void> {
  await ensureGlobalRoot(paths);
  await mkdir(dirname(paths.samplePrompts), { recursive: true });
  await rm(paths.samplePrompts, { recursive: true, force: true });
  await cp(paths.starterSamplePrompts, paths.samplePrompts, { recursive: true, force: true });
}

/** CLI flag: refresh samples only; does not create config.toml. */
export async function installSamplePrompts(paths: ConfigPaths): Promise<void> {
  await hardRefreshSamplePrompts(paths);
}

export async function bootstrapSettings(mode: BootstrapMode, paths: ConfigPaths): Promise<void> {
  if (mode === "refresh") {
    await refreshDefaultSettings(paths);
    await refreshGitignore(paths);
  } else if (!(await pathExists(paths.defaultSettings))) {
    await refreshDefaultSettings(paths);
    await ensureGitignore(paths);
  } else {
    await ensureGitignore(paths);
  }

  const configMissing = !(await pathExists(paths.global));
  if (configMissing) {
    await seedConfig(paths);
    await hardRefreshSamplePrompts(paths);
  }
}
