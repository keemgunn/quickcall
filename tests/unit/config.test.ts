import { mkdir, readFile, symlink, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { afterEach, expect, it } from "vitest";
import {
  REFERENCE_GITIGNORE_ENTRY,
  bootstrapSettings,
  configPaths,
  effectiveShell,
  hardRefreshSamplePrompts,
  installSamplePrompts,
  loadConfig,
  resolveCli,
} from "../../src/config/index.js";
import { removeTemp, tempDir } from "../helpers/temp.js";

const paths: string[] = [];
afterEach(async () => Promise.all(paths.splice(0).map(removeTemp)));

function starter(homeRoot: string): string {
  return join(homeRoot, "starter", "config.toml");
}

it("seeds config and sample prompts only when the sentinel is missing", async () => {
  const root = await tempDir("config"); paths.push(root); const home = join(root, "home"); const cwd = join(root, "cwd");
  await mkdir(join(cwd, ".qc"), { recursive: true });
  const bundled = join(root, "starter"); await mkdir(join(bundled, "prompts", "samples"), { recursive: true });
  await writeFile(join(bundled, "config.toml"), 'default-model = "starter/model"\n');
  await writeFile(join(bundled, "prompts", "samples", "joke.md"), "sample joke");
  await writeFile(join(bundled, "prompts", "user-owned.md"), "not copied");
  const config = configPaths(home, cwd, starter(root));
  await bootstrapSettings("repair", config);
  expect(await readFile(config.global, "utf8")).toBe('default-model = "starter/model"\n');
  expect(await readFile(join(config.samplePrompts, "joke.md"), "utf8")).toBe("sample joke");
  await expect(readFile(join(home, ".qc", "prompts", "user-owned.md"), "utf8")).rejects.toMatchObject({ code: "ENOENT" });
  await expect(readFile(join(home, ".qc", "prompts-samples", "samples", "joke.md"), "utf8")).rejects.toMatchObject({ code: "ENOENT" });
  expect(await readFile(config.gitignore, "utf8")).toBe(`${REFERENCE_GITIGNORE_ENTRY}\n`);
  expect(await readFile(join(config.defaultSettings, "config.toml"), "utf8")).toBe('default-model = "starter/model"\n');
  const loaded = await loadConfig(config); expect(loaded).toMatchObject({ model: "starter/model" }); expect(effectiveShell(undefined, loaded, {})).toBe("/bin/sh");
  await writeFile(config.global, "preserved"); await bootstrapSettings("repair", config); expect(await readFile(config.global, "utf8")).toBe("preserved");
});

it("preserves user prompts and skips sample refresh when config exists", async () => {
  const root = await tempDir("config-sentinel"); paths.push(root); const home = join(root, "home"); const cwd = join(root, "cwd");
  const bundled = join(root, "starter"); await mkdir(join(bundled, "prompts", "samples"), { recursive: true });
  await writeFile(join(bundled, "config.toml"), "starter"); await writeFile(join(bundled, "prompts", "samples", "joke.md"), "fresh sample");
  const config = configPaths(home, cwd, starter(root)); const userPrompt = join(home, ".qc", "prompts", "mine.md"); const staleSample = join(config.samplePrompts, "joke.md");
  await mkdir(dirname(userPrompt), { recursive: true }); await mkdir(dirname(staleSample), { recursive: true });
  await writeFile(config.global, "existing config"); await writeFile(userPrompt, "user prompt"); await writeFile(staleSample, "stale sample");
  await bootstrapSettings("repair", config);
  expect(await readFile(config.global, "utf8")).toBe("existing config"); expect(await readFile(userPrompt, "utf8")).toBe("user prompt"); expect(await readFile(staleSample, "utf8")).toBe("stale sample");
});

it("refresh replaces default settings and gitignore without touching live config or samples", async () => {
  const root = await tempDir("config-refresh"); paths.push(root); const home = join(root, "home"); const cwd = join(root, "cwd");
  const bundled = join(root, "starter"); await mkdir(join(bundled, "prompts", "samples"), { recursive: true });
  await writeFile(join(bundled, "config.toml"), "starter v2"); await writeFile(join(bundled, "prompts", "samples", "joke.md"), "starter v2 sample");
  const config = configPaths(home, cwd, starter(root));
  await mkdir(config.globalRoot, { recursive: true }); await writeFile(config.global, "live config"); await writeFile(config.gitignore, "stale\n");
  await mkdir(config.defaultSettings, { recursive: true }); await writeFile(join(config.defaultSettings, "stale.txt"), "old");
  await mkdir(config.samplePrompts, { recursive: true }); await writeFile(join(config.samplePrompts, "joke.md"), "live sample");
  await bootstrapSettings("refresh", config);
  expect(await readFile(config.global, "utf8")).toBe("live config"); expect(await readFile(join(config.samplePrompts, "joke.md"), "utf8")).toBe("live sample");
  expect(await readFile(config.gitignore, "utf8")).toBe(`${REFERENCE_GITIGNORE_ENTRY}\n`);
  expect(await readFile(join(config.defaultSettings, "config.toml"), "utf8")).toBe("starter v2");
});

it("installSamplePrompts hard-refreshes samples even when config exists", async () => {
  const root = await tempDir("config-install-samples"); paths.push(root); const home = join(root, "home"); const cwd = join(root, "cwd");
  const bundled = join(root, "starter"); await mkdir(join(bundled, "prompts", "samples"), { recursive: true });
  await writeFile(join(bundled, "config.toml"), "starter"); await writeFile(join(bundled, "prompts", "samples", "joke.md"), "packaged sample");
  const config = configPaths(home, cwd, starter(root));
  await mkdir(dirname(config.global), { recursive: true }); await writeFile(config.global, "existing config");
  await mkdir(config.samplePrompts, { recursive: true }); await writeFile(join(config.samplePrompts, "joke.md"), "stale sample");
  await installSamplePrompts(config);
  expect(await readFile(config.global, "utf8")).toBe("existing config"); expect(await readFile(join(config.samplePrompts, "joke.md"), "utf8")).toBe("packaged sample");
});

it("writes through a ~/.qc symlink without replacing the link inode", async () => {
  const root = await tempDir("config-symlink"); paths.push(root); const home = join(root, "home"); const cwd = join(root, "cwd"); const target = join(root, "target");
  const bundled = join(root, "starter"); await mkdir(join(bundled, "prompts", "samples"), { recursive: true });
  await writeFile(join(bundled, "config.toml"), "starter"); await writeFile(join(bundled, "prompts", "samples", "joke.md"), "sample");
  await mkdir(home, { recursive: true }); await mkdir(target, { recursive: true }); await symlink(target, join(home, ".qc"));
  const config = configPaths(home, cwd, starter(root));
  await bootstrapSettings("repair", config);
  expect(await readFile(join(target, "config.toml"), "utf8")).toBe("starter");
  expect(await readFile(join(target, "prompts", "samples", "joke.md"), "utf8")).toBe("sample");
});

it("hardRefreshSamplePrompts leaves sibling prompt files intact", async () => {
  const root = await tempDir("config-sample-refresh"); paths.push(root); const home = join(root, "home"); const cwd = join(root, "cwd");
  const bundled = join(root, "starter"); await mkdir(join(bundled, "prompts", "samples"), { recursive: true });
  await writeFile(join(bundled, "config.toml"), "starter"); await writeFile(join(bundled, "prompts", "samples", "joke.md"), "fresh");
  const config = configPaths(home, cwd, starter(root)); const sibling = join(home, ".qc", "prompts", "mine.md"); const stale = join(config.samplePrompts, "joke.md");
  await mkdir(dirname(sibling), { recursive: true }); await mkdir(dirname(stale), { recursive: true }); await writeFile(sibling, "keep me"); await writeFile(stale, "stale");
  await hardRefreshSamplePrompts(config);
  expect(await readFile(sibling, "utf8")).toBe("keep me"); expect(await readFile(stale, "utf8")).toBe("fresh");
});

it("validates TOML shape and preserves normal permission key declaration order", async () => {
  const root = await tempDir("config-order"); paths.push(root); const home = join(root, "home"); const cwd = join(root, "cwd"); await mkdir(join(cwd, ".qc"), { recursive: true });
  const config = configPaths(home, cwd, join(process.cwd(), "share/settings/config.toml"));
  await mkdir(join(home, ".qc"), { recursive: true });
  await writeFile(config.global, '[command-permissions]\npwd = "allow"\n"*" = "deny"\n"1" = "allow"\n"say \\"hi\\"" = "deny"\n');
  await writeFile(config.project, '[command-permissions]\n"echo *" = "allow"\n');
  const loaded = await loadConfig(config);
  expect(loaded.rules.map(({ pattern, action }) => [pattern, action])).toEqual([["pwd", "allow"], ["*", "deny"], ["1", "allow"], ['say "hi"', "deny"], ["echo *", "allow"]]);
  await writeFile(config.project, 'shell = ""\n'); await expect(loadConfig(config)).rejects.toThrow("non-empty");
  await writeFile(config.project, 'unknown = "x"\n'); await expect(loadConfig(config)).rejects.toThrow("unknown config");
  await writeFile(config.project, '[command-permissions]\n"*" = "ask"\n'); await expect(loadConfig(config)).rejects.toThrow("allow or deny");
  await writeFile(config.project, 'default-model = [\n'); await expect(loadConfig(config)).rejects.toThrow("invalid TOML");
  await writeFile(config.project, '[command-permissions]\nnested.value = "allow"\n'); await expect(loadConfig(config)).rejects.toThrow("direct string rules");
  await writeFile(config.project, '[command-permissions.nested]\npwd = "allow"\n'); await expect(loadConfig(config)).rejects.toThrow("direct string rules");
});

it("uses CLI, config, environment, then /bin/sh for the shell", async () => {
  const root = await tempDir("config-shell"); paths.push(root); const home = join(root, "home"); const cwd = join(root, "cwd"); await mkdir(join(home, ".qc"), { recursive: true }); await mkdir(join(cwd, ".qc"), { recursive: true });
  const config = configPaths(home, cwd, join(process.cwd(), "share/settings/config.toml")); await writeFile(config.global, 'shell = "/bin/sh"\n'); await writeFile(config.project, 'shell = "/bin/bash"\n');
  expect(effectiveShell("/bin/zsh", await loadConfig(config), { SHELL: "/bin/false" })).toBe("/bin/zsh"); expect(effectiveShell(undefined, await loadConfig(config), {})).toBe("/bin/bash");
  await writeFile(config.project, ""); expect(effectiveShell(undefined, await loadConfig(config), { SHELL: "/bin/zsh" })).toBe("/bin/sh");
});

it("loads default-cli = pi and rejects unsupported values", async () => {
  const root = await tempDir("config-cli"); paths.push(root); const home = join(root, "home"); const cwd = join(root, "cwd");
  await mkdir(join(home, ".qc"), { recursive: true }); await mkdir(join(cwd, ".qc"), { recursive: true });
  const config = configPaths(home, cwd, join(process.cwd(), "share/settings/config.toml"));
  await writeFile(config.global, 'default-cli = "pi"\n');
  expect(await loadConfig(config)).toMatchObject({ cli: "pi" });
  await writeFile(config.global, ""); expect(await loadConfig(config)).toMatchObject({ cli: undefined });
  await writeFile(config.global, 'default-cli = "cursor"\n'); await expect(loadConfig(config)).rejects.toThrow("unsupported cli runtime");
});

it("merges project default-cli over global", async () => {
  const root = await tempDir("config-cli-merge"); paths.push(root); const home = join(root, "home"); const cwd = join(root, "cwd");
  await mkdir(join(home, ".qc"), { recursive: true }); await mkdir(join(cwd, ".qc"), { recursive: true });
  const config = configPaths(home, cwd, join(process.cwd(), "share/settings/config.toml"));
  await writeFile(config.global, 'default-cli = "pi"\n');
  await writeFile(config.project, 'default-cli = "pi"\n');
  expect(resolveCli(undefined, await loadConfig(config).then((c) => c.cli))).toBe("pi");
  await writeFile(config.global, 'default-cli = "pi"\n'); await writeFile(config.project, "");
  expect(resolveCli(undefined, (await loadConfig(config)).cli)).toBe("pi");
});
