import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { afterEach, expect, it } from "vitest";
import {
  HARNESS_COMMAND_BASENAME,
  commandDestination,
  installAgentHarness,
  refreshKnown,
  resolveBundledHarnessRoot,
  skillsDestination,
} from "../../src/harness.js";
import { removeTemp, tempDir } from "../helpers/temp.js";

const clean: string[] = [];
afterEach(async () => Promise.all(clean.splice(0).map(removeTemp)));

it("installs skills only when no frameworks are requested", async () => {
  const root = await tempDir("harness-skills"); clean.push(root); const home = join(root, "home");
  const result = await installAgentHarness({ frameworks: [], installSkills: true, home, bundledRoot: resolveBundledHarnessRoot() });
  expect(result.copied).toContain(join(skillsDestination(home), "SKILL.md"));
  expect(await readFile(join(skillsDestination(home), "SKILL.md"), "utf8")).toContain("name: qc");
  expect(result.copied.some((path) => path.endsWith(HARNESS_COMMAND_BASENAME))).toBe(false);
});

it("installs nested and flat command destinations plus skills", async () => {
  const root = await tempDir("harness-install"); clean.push(root); const home = join(root, "home");
  const result = await installAgentHarness({ frameworks: ["cursor", "pi", "codex"], installSkills: true, home, bundledRoot: resolveBundledHarnessRoot() });
  expect(await readFile(commandDestination("cursor", home), "utf8")).toContain("# create-prompt");
  expect(await readFile(commandDestination("pi", home), "utf8")).toContain("# create-prompt");
  expect(await readFile(commandDestination("codex", home), "utf8")).toContain("# create-prompt");
  expect(result.copied.length).toBeGreaterThan(0);
});

it("refreshKnown updates only previously installed destinations", async () => {
  const root = await tempDir("harness-refresh-known"); clean.push(root); const home = join(root, "home");
  const bundledRoot = join(root, "bundled"); await mkdir(join(bundledRoot, "commands"), { recursive: true }); await mkdir(join(bundledRoot, "skills", "qc"), { recursive: true });
  await writeFile(join(bundledRoot, "commands", HARNESS_COMMAND_BASENAME), "fresh command"); await writeFile(join(bundledRoot, "skills", "qc", "SKILL.md"), "fresh skill");
  await mkdir(join(home, ".cursor", "commands", "qc"), { recursive: true }); await writeFile(commandDestination("cursor", home), "stale");
  const result = await refreshKnown({ home, bundledRoot }); expect(result.known).toBe(true);
  expect(await readFile(commandDestination("cursor", home), "utf8")).toBe("fresh command");
  await expect(readFile(join(skillsDestination(home), "SKILL.md"), "utf8")).rejects.toMatchObject({ code: "ENOENT" });
});

it("refreshKnown is a no-op when nothing was installed before", async () => {
  const root = await tempDir("harness-refresh-empty"); clean.push(root); const home = join(root, "home");
  const result = await refreshKnown({ home, bundledRoot: resolveBundledHarnessRoot() });
  expect(result).toEqual({ copied: [], skipped: [], known: false });
});

it("refreshKnown refreshes skills when SKILL.md already exists", async () => {
  const root = await tempDir("harness-refresh-skill"); clean.push(root); const home = join(root, "home");
  const bundledRoot = join(root, "bundled"); await mkdir(join(bundledRoot, "skills", "qc"), { recursive: true });
  await writeFile(join(bundledRoot, "skills", "qc", "SKILL.md"), "fresh skill");
  await mkdir(skillsDestination(home), { recursive: true }); await writeFile(join(skillsDestination(home), "SKILL.md"), "stale");
  const result = await refreshKnown({ home, bundledRoot }); expect(result.known).toBe(true);
  expect(await readFile(join(skillsDestination(home), "SKILL.md"), "utf8")).toBe("fresh skill");
});
