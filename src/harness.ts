import { access, cp, mkdir, readdir, rm } from "node:fs/promises";
import { constants } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const PRODUCT_SKILL_PREFIX = "qc";

const SKILL_DEST_RELATIVE = [".agents/skills", ".claude/skills"] as const;

export function resolveBundledSkillsRoot(importMetaUrl = import.meta.url): string {
  return join(dirname(fileURLToPath(importMetaUrl)), "..", "share", "skills");
}

export function skillDestinations(home: string): string[] {
  return SKILL_DEST_RELATIVE.map((relative) => join(home, relative));
}

export function isProductSkillName(name: string): boolean {
  return name === PRODUCT_SKILL_PREFIX || name.startsWith(`${PRODUCT_SKILL_PREFIX}-`);
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

async function listDirNames(path: string): Promise<string[]> {
  if (!(await pathExists(path))) return [];
  const entries = await readdir(path, { withFileTypes: true });
  return entries.filter((entry) => entry.isDirectory()).map((entry) => entry.name);
}

/** True when dest already has a product skill dir (`qc` or `qc-*`, not a raw `qc*` glob). */
async function destHasProductSkills(dest: string): Promise<boolean> {
  for (const name of await listDirNames(dest)) {
    if (isProductSkillName(name)) return true;
  }
  return false;
}

/** Clean-slate: delete dest dirs named `qc` or `qc-*`, then copy every packaged skill directory. */
async function replaceInstallDest(bundledRoot: string, dest: string): Promise<string[]> {
  await mkdir(dest, { recursive: true });
  for (const name of await listDirNames(dest)) {
    if (!isProductSkillName(name)) continue;
    await rm(join(dest, name), { recursive: true, force: true });
  }
  const copied: string[] = [];
  for (const name of await listDirNames(bundledRoot)) {
    const destination = join(dest, name);
    await cp(join(bundledRoot, name), destination, { recursive: true, force: true });
    copied.push(destination);
  }
  return copied;
}

export interface HarnessInstallInput {
  home: string;
  bundledRoot?: string;
}

export interface HarnessInstallResult {
  copied: string[];
  skipped: string[];
}

export async function installAgentHarness(input: HarnessInstallInput): Promise<HarnessInstallResult> {
  const bundledRoot = input.bundledRoot ?? resolveBundledSkillsRoot();
  const copied: string[] = [];
  for (const dest of skillDestinations(input.home)) {
    copied.push(...(await replaceInstallDest(bundledRoot, dest)));
  }
  return { copied, skipped: [] };
}

export interface RefreshKnownInput {
  home: string;
  bundledRoot?: string;
}

export interface RefreshKnownResult extends HarnessInstallResult {
  known: boolean;
}

/**
 * Per dest: if it already contains `qc` or `qc-*`, wipe those dirs and copy the
 * full packaged set. Skip dests with no product skills. Package postinstall
 * runs this. First-time install is `qc --install-agent-harness`.
 */
export async function refreshKnown(input: RefreshKnownInput): Promise<RefreshKnownResult> {
  const bundledRoot = input.bundledRoot ?? resolveBundledSkillsRoot();
  const copied: string[] = [];
  const skipped: string[] = [];
  for (const dest of skillDestinations(input.home)) {
    if (await destHasProductSkills(dest)) {
      copied.push(...(await replaceInstallDest(bundledRoot, dest)));
    } else {
      skipped.push(dest);
    }
  }
  if (copied.length === 0) {
    return { copied: [], skipped: [], known: false };
  }
  return { copied, skipped, known: true };
}
