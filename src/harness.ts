import { access, cp, mkdir, readdir } from "node:fs/promises";
import { constants } from "node:fs";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { QcError } from "./errors.js";

export type HarnessFramework = "opencode" | "cursor" | "pi" | "claude" | "codex" | "gemini";

export const HARNESS_FRAMEWORKS: readonly HarnessFramework[] = [
  "opencode",
  "cursor",
  "pi",
  "claude",
  "codex",
  "gemini",
];

export const HARNESS_COMMAND_BASENAME = "qc-create-prompt.md";

const NESTED_COMMAND_DIRS: Readonly<Record<"opencode" | "cursor" | "claude" | "gemini", string>> = {
  opencode: ".config/opencode/commands/qc",
  cursor: ".cursor/commands/qc",
  claude: ".claude/commands/qc",
  gemini: ".gemini/commands/qc",
};

export function isHarnessFramework(name: string): name is HarnessFramework {
  return (HARNESS_FRAMEWORKS as readonly string[]).includes(name);
}

export function unknownFrameworkError(framework: string): string {
  return `unknown agent-harness framework '${framework}'; expected one of: ${HARNESS_FRAMEWORKS.join(", ")}`;
}

export function resolveBundledHarnessRoot(importMetaUrl = import.meta.url): string {
  return join(dirname(fileURLToPath(importMetaUrl)), "..", "share", "agent-harness");
}

export function commandDestination(framework: HarnessFramework, home: string): string {
  if (framework === "pi") return join(home, ".pi", "prompts", HARNESS_COMMAND_BASENAME);
  if (framework === "codex") return join(home, ".codex", "prompts", HARNESS_COMMAND_BASENAME);
  return join(home, NESTED_COMMAND_DIRS[framework], HARNESS_COMMAND_BASENAME);
}

export function skillsDestination(home: string): string {
  return join(home, ".agents", "skills", "qc");
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

async function copyFileEnsuringParent(source: string, destination: string): Promise<void> {
  await mkdir(dirname(destination), { recursive: true });
  await cp(source, destination, { force: true });
}

async function copySkillTree(sourceRoot: string, destinationRoot: string): Promise<void> {
  for (const entry of await readdir(sourceRoot, { withFileTypes: true })) {
    const source = join(sourceRoot, entry.name);
    const destination = join(destinationRoot, entry.name);
    if (entry.isDirectory()) {
      await copySkillTree(source, destination);
    } else if (entry.isFile()) {
      await copyFileEnsuringParent(source, destination);
    }
  }
}

export interface HarnessInstallInput {
  frameworks: readonly HarnessFramework[];
  installSkills: boolean;
  home: string;
  bundledRoot?: string;
}

export interface HarnessInstallResult {
  copied: string[];
  skipped: string[];
}

export async function installAgentHarness(input: HarnessInstallInput): Promise<HarnessInstallResult> {
  const bundledRoot = input.bundledRoot ?? resolveBundledHarnessRoot();
  const commandSource = join(bundledRoot, "commands", HARNESS_COMMAND_BASENAME);
  const skillSource = join(bundledRoot, "skills", "qc");
  const copied: string[] = [];
  const skipped: string[] = [];

  for (const framework of input.frameworks) {
    const destination = commandDestination(framework, input.home);
    const existed = await pathExists(destination);
    await copyFileEnsuringParent(commandSource, destination);
    (existed ? skipped : copied).push(destination);
  }

  if (input.installSkills) {
    const destinationRoot = skillsDestination(input.home);
    const skillFile = join(destinationRoot, "SKILL.md");
    const existed = await pathExists(skillFile);
    await copySkillTree(skillSource, destinationRoot);
    (existed ? skipped : copied).push(skillFile);
  }

  return { copied, skipped };
}

export interface RefreshKnownInput {
  home: string;
  bundledRoot?: string;
}

export interface RefreshKnownResult extends HarnessInstallResult {
  known: boolean;
}

/** Refresh only destinations that already contain packaged harness assets. */
export async function refreshKnown(input: RefreshKnownInput): Promise<RefreshKnownResult> {
  const frameworks: HarnessFramework[] = [];
  for (const framework of HARNESS_FRAMEWORKS) {
    if (await pathExists(commandDestination(framework, input.home))) {
      frameworks.push(framework);
    }
  }
  const installSkills = await pathExists(join(skillsDestination(input.home), "SKILL.md"));
  if (frameworks.length === 0 && !installSkills) {
    return { copied: [], skipped: [], known: false };
  }
  const result = await installAgentHarness({
    frameworks,
    installSkills,
    home: input.home,
    bundledRoot: input.bundledRoot,
  });
  return { ...result, known: true };
}

export function parseHarnessFrameworks(argv: readonly string[]): HarnessFramework[] {
  const seen = new Set<string>();
  const frameworks: HarnessFramework[] = [];
  for (const name of argv) {
    if (seen.has(name)) {
      throw new QcError(`duplicate agent-harness framework '${name}'; expected one of: ${HARNESS_FRAMEWORKS.join(", ")}`);
    }
    seen.add(name);
    if (!isHarnessFramework(name)) throw new QcError(unknownFrameworkError(name));
    frameworks.push(name);
  }
  return frameworks;
}
