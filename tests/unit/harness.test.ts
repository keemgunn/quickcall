import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { afterEach, expect, it } from "vitest";
import {
  installAgentHarness,
  refreshKnown,
  resolveBundledSkillsRoot,
  skillDestinations,
} from "../../src/harness.js";
import { removeTemp, tempDir } from "../helpers/temp.js";

const clean: string[] = [];
afterEach(async () => Promise.all(clean.splice(0).map(removeTemp)));

async function writeTree(root: string, files: Record<string, string>): Promise<void> {
  for (const [relative, body] of Object.entries(files)) {
    const path = join(root, relative);
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, body);
  }
}

function skillFile(home: string, destIndex: number, skill: string, relative: string): string {
  return join(skillDestinations(home)[destIndex]!, skill, relative);
}

it("replace-installs packaged skills into both skill destinations", async () => {
  const root = await tempDir("harness-install");
  clean.push(root);
  const home = join(root, "home");
  const result = await installAgentHarness({ home, bundledRoot: resolveBundledSkillsRoot() });
  expect(result.copied).toContain(join(home, ".agents", "skills", "qc"));
  expect(result.copied).toContain(join(home, ".claude", "skills", "qc-create-prompt"));
  expect(await readFile(skillFile(home, 0, "qc", "SKILL.md"), "utf8")).toContain("name: qc");
  expect(await readFile(skillFile(home, 1, "qc", "SKILL.md"), "utf8")).toContain("name: qc");
  expect(await readFile(skillFile(home, 0, "qc-create-prompt", "SKILL.md"), "utf8")).toContain(
    "disable-model-invocation: true",
  );
  expect(await readFile(skillFile(home, 1, "qc-create-prompt", "openai.yaml"), "utf8")).toContain(
    "allow_implicit_invocation: false",
  );
  await expect(readFile(join(home, ".cursor", "commands", "qc", "qc-create-prompt.md"), "utf8")).rejects.toMatchObject({
    code: "ENOENT",
  });
});

it("deletes retired qc-* dirs and leaves unrelated skills", async () => {
  const root = await tempDir("harness-replace");
  clean.push(root);
  const home = join(root, "home");
  const agents = join(home, ".agents", "skills");
  await writeTree(agents, {
    "qc/SKILL.md": "stale",
    "qc-retired/SKILL.md": "retired",
    "other/SKILL.md": "keep",
  });
  const bundledRoot = join(root, "bundled");
  await writeTree(bundledRoot, {
    "qc/SKILL.md": "fresh qc",
    "qc-create-prompt/SKILL.md": "fresh command",
  });
  await installAgentHarness({ home, bundledRoot });
  expect(await readFile(join(agents, "qc", "SKILL.md"), "utf8")).toBe("fresh qc");
  expect(await readFile(join(agents, "qc-create-prompt", "SKILL.md"), "utf8")).toBe("fresh command");
  expect(await readFile(join(agents, "other", "SKILL.md"), "utf8")).toBe("keep");
  await expect(readFile(join(agents, "qc-retired", "SKILL.md"), "utf8")).rejects.toMatchObject({ code: "ENOENT" });
});

it("refreshKnown updates only dests that already have product skills", async () => {
  const root = await tempDir("harness-refresh-known");
  clean.push(root);
  const home = join(root, "home");
  const bundledRoot = join(root, "bundled");
  await writeTree(bundledRoot, { "qc/SKILL.md": "fresh skill" });
  await writeTree(join(home, ".agents", "skills"), { "qc/SKILL.md": "stale" });
  const result = await refreshKnown({ home, bundledRoot });
  expect(result.known).toBe(true);
  expect(await readFile(join(home, ".agents", "skills", "qc", "SKILL.md"), "utf8")).toBe("fresh skill");
  await expect(readFile(join(home, ".claude", "skills", "qc", "SKILL.md"), "utf8")).rejects.toMatchObject({
    code: "ENOENT",
  });
});

it("refreshKnown wipe-reinstalls every packaged skill on a dest that already has qc or qc-*", async () => {
  const root = await tempDir("harness-refresh-clean-slate");
  clean.push(root);
  const home = join(root, "home");
  const agents = join(home, ".agents", "skills");
  const claude = join(home, ".claude", "skills");
  const bundledRoot = join(root, "bundled");
  await writeTree(bundledRoot, {
    "qc/SKILL.md": "fresh qc",
    "qc-create-prompt/SKILL.md": "fresh command",
  });
  await writeTree(agents, {
    "qc/SKILL.md": "stale",
    "qc-retired/SKILL.md": "retired",
    "other/SKILL.md": "keep-agents",
    "qcode/SKILL.md": "not-a-product-prefix",
  });
  await writeTree(claude, { "unrelated/SKILL.md": "keep-claude" });
  const result = await refreshKnown({ home, bundledRoot });
  expect(result.known).toBe(true);
  expect(await readFile(join(agents, "qc", "SKILL.md"), "utf8")).toBe("fresh qc");
  expect(await readFile(join(agents, "qc-create-prompt", "SKILL.md"), "utf8")).toBe("fresh command");
  expect(await readFile(join(agents, "other", "SKILL.md"), "utf8")).toBe("keep-agents");
  expect(await readFile(join(agents, "qcode", "SKILL.md"), "utf8")).toBe("not-a-product-prefix");
  await expect(readFile(join(agents, "qc-retired", "SKILL.md"), "utf8")).rejects.toMatchObject({ code: "ENOENT" });
  await expect(readFile(join(claude, "qc", "SKILL.md"), "utf8")).rejects.toMatchObject({ code: "ENOENT" });
  expect(await readFile(join(claude, "unrelated", "SKILL.md"), "utf8")).toBe("keep-claude");
});

it("refreshKnown is a no-op when nothing was installed before", async () => {
  const root = await tempDir("harness-refresh-empty");
  clean.push(root);
  const home = join(root, "home");
  const result = await refreshKnown({ home, bundledRoot: resolveBundledSkillsRoot() });
  expect(result).toEqual({ copied: [], skipped: [], known: false });
});
