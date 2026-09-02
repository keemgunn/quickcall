import { chmod, copyFile, mkdir, readFile, symlink, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, expect, it } from "vitest";
import { removeTemp, tempDir } from "../helpers/temp.js";
const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../.."); const fixture = join(packageRoot, "tests/fixtures/bin/pi"); const fixtures = join(packageRoot, "tests/fixtures"); const loader = join(packageRoot, "node_modules/tsx/dist/loader.mjs");
const clean: string[] = []; afterEach(async () => Promise.all(clean.splice(0).map(removeTemp)));
async function invoke(root: string, args: string[], extra: NodeJS.ProcessEnv = {}) {
  const { spawn } = await import("node:child_process"); const cwd = join(root, "work"); const home = join(root, "home"); const bin = join(root, "bin"); await mkdir(join(cwd, ".qc", "prompts"), { recursive: true }); await mkdir(bin, { recursive: true }); await copyFile(fixture, join(bin, "pi")); await chmod(join(bin, "pi"), 0o755); await symlink(process.execPath, join(bin, "node")).catch(() => undefined);
  return new Promise<{ code: number | null; out: string; err: string }>((resolveResult, reject) => { const child = spawn(process.execPath, ["--import", loader, join(packageRoot, "src/cli.ts"), ...args], { cwd, env: { ...process.env, ...extra, HOME: home, PATH: bin, SHELL: "/bin/sh" } }); let out = ""; let err = ""; child.stdout.on("data", (x) => { out += x; }); child.stderr.on("data", (x) => { err += x; }); child.on("error", reject); child.on("close", (code) => resolveResult({ code, out, err })); });
}
it("wires real config, prompt, shell expansion, and fixture Pi", async () => {
  const root = await tempDir("integration"); clean.push(root); const cwd = join(root, "work"); const home = join(root, "home"); await mkdir(join(cwd, ".qc", "prompts"), { recursive: true }); await mkdir(join(home, ".qc"), { recursive: true }); await copyFile(join(fixtures, "config/global.toml"), join(home, ".qc/config.toml")); await copyFile(join(fixtures, "config/project.toml"), join(cwd, ".qc/config.toml")); await writeFile(join(cwd, ".qc/prompts/daily.md"), (await readFile(join(fixtures, "prompts/frontmatter.md"), "utf8")).replace("Body text", (await readFile(join(fixtures, "prompts/shell-output.md"), "utf8")).replace("First", "Body"))); const record = join(root, "record.json");
  const result = await invoke(root, ["daily", "--append", "More !`printf append`"], { QC_RECORD: record }); expect(result.code).toBe(0); expect(JSON.parse(await readFile(record, "utf8"))).toEqual({ argv: ["-p", "--model", "prompt/model", "--thinking", "high", "--no-skills", "--skill", join(home, ".config/custom-skills"), "--no-approve"], stdin: "Body one then two\n\n\n\n---\n\nAdditional Message from the user:\n\nMore append" });
});
it("preflights a denied compound command before side effects or Pi", async () => {
  const root = await tempDir("denied"); clean.push(root); const cwd = join(root, "work"); const home = join(root, "home"); const marker = join(root, "marker"); await mkdir(join(home, ".qc"), { recursive: true }); await mkdir(join(cwd, ".qc", "prompts"), { recursive: true }); await writeFile(join(home, ".qc", "config.toml"), ""); await writeFile(join(cwd, ".qc", "config.toml"), '[command-permissions]\n"*" = "allow"\n"touch *" = "deny"\n'); await writeFile(join(cwd, ".qc", "prompts", "blocked.md"), `!\`printf start && touch ${marker}\``);
  const result = await invoke(root, ["blocked"], { QC_PI_MARKER: marker }); expect(result.code).toBe(1); expect(result.err).toContain("denied"); await expect(readFile(marker, "utf8")).rejects.toMatchObject({ code: "ENOENT" });
});
it("preflights every expression before any substitution side effect or Pi launch", async () => {
  const root = await tempDir("cross-expression"); clean.push(root); const cwd = join(root, "work"); const home = join(root, "home"); const commandMarker = join(root, "command-marker"); const piMarker = join(root, "pi-marker"); const record = join(root, "record.json");
  await mkdir(join(home, ".qc"), { recursive: true }); await mkdir(join(cwd, ".qc", "prompts"), { recursive: true }); await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "config.toml"), '[command-permissions]\n"touch *" = "allow"\n');
  await writeFile(join(cwd, ".qc", "prompts", "blocked.md"), `!\`touch ${commandMarker}\` !\`printf denied\``);
  const result = await invoke(root, ["blocked"], { QC_PI_MARKER: piMarker, QC_RECORD: record });
  expect(result.code).toBe(1); expect(result.err).toContain("no matching");
  await expect(readFile(commandMarker, "utf8")).rejects.toMatchObject({ code: "ENOENT" });
  await expect(readFile(piMarker, "utf8")).rejects.toMatchObject({ code: "ENOENT" });
  await expect(readFile(record, "utf8")).rejects.toMatchObject({ code: "ENOENT" });
});
it("denies expressions before starting the configured shell validation probe", async () => {
  const root = await tempDir("shell-preflight"); clean.push(root); const cwd = join(root, "work"); const home = join(root, "home"); const shell = join(root, "marker-shell"); const marker = join(root, "shell-marker");
  await mkdir(join(home, ".qc"), { recursive: true }); await mkdir(join(cwd, ".qc", "prompts"), { recursive: true }); await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(shell, '#!/bin/sh\n: > "$QC_SHELL_MARKER"\n'); await chmod(shell, 0o755);
  await writeFile(join(cwd, ".qc", "config.toml"), `shell = ${JSON.stringify(shell)}\n[command-permissions]\n"touch *" = "allow"\n`);
  await writeFile(join(cwd, ".qc", "prompts", "blocked.md"), "!`printf denied`");
  const result = await invoke(root, ["blocked"], { QC_SHELL_MARKER: marker });
  expect(result.code).toBe(1); expect(result.err).toContain("no matching");
  await expect(readFile(marker, "utf8")).rejects.toMatchObject({ code: "ENOENT" });
});
it("loads real global and project TOML scalar fields and later permission overrides", async () => {
  const root = await tempDir("precedence"); clean.push(root); const cwd = join(root, "work"); const home = join(root, "home"); const record = join(root, "record.json"); await mkdir(join(home, ".qc"), { recursive: true }); await mkdir(join(cwd, ".qc", "prompts"), { recursive: true }); await copyFile(join(fixtures, "config/global.toml"), join(home, ".qc/config.toml")); await writeFile(join(cwd, ".qc/config.toml"), 'default-thinking = "high"\n[command-permissions]\n"*" = "allow"\n"1" = "deny"\n'); await writeFile(join(home, ".qc/config.toml"), 'default-model = "global/model"\ndefault-thinking = "medium"\n[command-permissions]\n"*" = "allow"\n'); await writeFile(join(cwd, ".qc/prompts/plain.md"), "!`1`");
  const result = await invoke(root, ["plain"], { QC_RECORD: record }); expect(result.code).toBe(1); expect(result.err).toContain("'1'"); await expect(readFile(record, "utf8")).rejects.toMatchObject({ code: "ENOENT" });
});
it("uses field-level real TOML precedence and permits a later project override", async () => {
  const root = await tempDir("scalar-override"); clean.push(root); const cwd = join(root, "work"); const home = join(root, "home"); const record = join(root, "record.json"); await mkdir(join(home, ".qc"), { recursive: true }); await mkdir(join(cwd, ".qc", "prompts"), { recursive: true }); await writeFile(join(home, ".qc/config.toml"), 'default-model = "global/model"\ndefault-thinking = "medium"\n[command-permissions]\n"*" = "deny"\n'); await writeFile(join(cwd, ".qc/config.toml"), 'default-thinking = "high"\n[command-permissions]\n"printf *" = "allow"\n'); await writeFile(join(cwd, ".qc/prompts/plain.md"), "!`printf ok`");
  const result = await invoke(root, ["plain"], { QC_RECORD: record }); expect(result.code).toBe(0); expect(JSON.parse(await readFile(record, "utf8"))).toEqual({ argv: ["-p", "--model", "global/model", "--thinking", "high"], stdin: "ok" });
});
it("fails configured unmatched substitutions closed but permits an absent table", async () => {
  const root = await tempDir("permission-modes"); clean.push(root); const cwd = join(root, "work"); const home = join(root, "home"); const record = join(root, "record.json"); await mkdir(join(home, ".qc"), { recursive: true }); await mkdir(join(cwd, ".qc", "prompts"), { recursive: true }); await writeFile(join(home, ".qc/config.toml"), (await readFile(join(fixtures, "config/permissions.toml"), "utf8")).replace('"*" = "allow"', '"printf *" = "allow"')); await writeFile(join(cwd, ".qc/prompts/plain.md"), "!`whoami`");
  const denied = await invoke(root, ["plain"], { QC_RECORD: record }); expect(denied.code).toBe(1); expect(denied.err).toContain("no matching"); await writeFile(join(home, ".qc/config.toml"), 'default-model = "global/model"\n'); const allowed = await invoke(root, ["plain"], { QC_RECORD: record }); expect(allowed.code).toBe(0); expect(JSON.parse(await readFile(record, "utf8")).stdin).not.toContain("!`");
});
it("keeps shell stdout from nonzero commands and discards stderr", async () => {
  const root = await tempDir("shell-failure"); clean.push(root); const cwd = join(root, "work"); const home = join(root, "home"); const record = join(root, "record.json"); await mkdir(join(home, ".qc"), { recursive: true }); await mkdir(join(cwd, ".qc", "prompts"), { recursive: true }); await writeFile(join(home, ".qc/config.toml"), ""); await writeFile(join(cwd, ".qc/prompts/plain.md"), "!`printf kept; printf ignored >&2; false` !`printf ignored >&2; false`");
  const result = await invoke(root, ["plain"], { QC_RECORD: record }); expect(result).toMatchObject({ code: 0, err: "" }); expect(JSON.parse(await readFile(record, "utf8")).stdin).toBe("kept ");
});
it("does not resolve aliases from managed global prompt samples", async () => {
  const root = await tempDir("prompt-samples"); clean.push(root); const home = join(root, "home");
  await mkdir(join(home, ".qc", "prompts", "samples"), { recursive: true }); await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(home, ".qc", "prompts", "samples", "sample-only.md"), "sample prompt");
  const result = await invoke(root, ["sample-only"]); expect(result.code).toBe(1); expect(result.err).toContain("prompt alias 'sample-only' was not found");
});
it("rejects unsupported qc_cli before shell expansion or Pi", async () => {
  const root = await tempDir("unsupported-cli"); clean.push(root); const cwd = join(root, "work"); const home = join(root, "home"); const marker = join(root, "marker"); const record = join(root, "record.json");
  await mkdir(join(home, ".qc"), { recursive: true }); await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), ""); await writeFile(join(cwd, ".qc", "prompts", "cursor.md"), `---\nqc_cli: cursor\n---\n!\`touch ${marker}\``);
  const result = await invoke(root, ["cursor"], { QC_PI_MARKER: marker, QC_RECORD: record });
  expect(result.code).toBe(1); expect(result.err).toContain("unsupported cli runtime");
  await expect(readFile(marker, "utf8")).rejects.toMatchObject({ code: "ENOENT" });
  await expect(readFile(record, "utf8")).rejects.toMatchObject({ code: "ENOENT" });
});
it("installs harness assets through the CLI without bootstrapping settings", async () => {
  const root = await tempDir("harness-cli"); clean.push(root); const home = join(root, "home");
  const result = await invoke(root, ["--install-agent-harness", "cursor"]);
  expect(result.code).toBe(0);
  expect(await readFile(join(home, ".cursor", "commands", "qc", "qc-create-prompt.md"), "utf8")).toContain("# create-prompt");
  expect(await readFile(join(home, ".agents", "skills", "qc", "SKILL.md"), "utf8")).toContain("name: qc");
  await expect(readFile(join(home, ".qc", "config.toml"), "utf8")).rejects.toMatchObject({ code: "ENOENT" });
});
