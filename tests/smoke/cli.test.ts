import { chmod, copyFile, mkdir, readFile, symlink, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";
import { afterEach, expect, it } from "vitest";
import { removeTemp, tempDir } from "../helpers/temp.js";
const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../.."); const fixture = join(packageRoot, "tests/fixtures/bin/pi"); const cli = join(packageRoot, "dist/cli.js");
const clean: string[] = []; afterEach(async () => Promise.all(clean.splice(0).map(removeTemp)));
async function setup() { const root = await tempDir("smoke"); clean.push(root); const cwd = join(root, "work"); const home = join(root, "home"); const bin = join(root, "bin"); await mkdir(join(cwd, ".qc", "prompts", "nested"), { recursive: true }); await mkdir(bin, { recursive: true }); await copyFile(fixture, join(bin, "pi")); await chmod(join(bin, "pi"), 0o755); await symlink(process.execPath, join(bin, "node")); return { root, cwd, home, bin }; }
function start(cwd: string, home: string, bin: string, args: string[], extra: NodeJS.ProcessEnv = {}) { const child = spawn(process.execPath, [cli, ...args], { cwd, env: { ...process.env, ...extra, HOME: home, PATH: bin, SHELL: extra.SHELL ?? "/bin/sh" } }); let out = ""; let err = ""; const result = new Promise<{ code: number | null; out: string; err: string }>((resolveResult, reject) => { child.stdout.on("data", (x) => { out += x; }); child.stderr.on("data", (x) => { err += x; }); child.on("error", reject); child.on("close", (code) => resolveResult({ code, out, err })); }); return { child, result }; }
async function run(cwd: string, home: string, bin: string, args: string[], extra: NodeJS.ProcessEnv = {}) { return start(cwd, home, bin, args, extra).result; }
// Budget must cover two cold Node starts plus a stdin round-trip while vitest
// runs test files in parallel; the project testTimeout still guards a real hang.
async function waitForReady(path: string) { for (let attempt = 0; attempt < 500; attempt += 1) { try { await readFile(path); return; } catch (cause: unknown) { if ((cause as NodeJS.ErrnoException).code !== "ENOENT") throw cause; await new Promise<void>((resolveWait) => setTimeout(resolveWait, 10)); } } throw new Error(`timed out waiting for fixture Pi readiness: ${path}`); }
async function assertInformationalBootstrap(home: string, marker: string) {
  expect(await readFile(join(home, ".qc", "config.toml"), "utf8")).toEqual(await readFile(join(packageRoot, "share/settings", "config.toml"), "utf8"));
  expect(await readFile(join(home, ".qc", ".gitignore"), "utf8")).toBe(".default-settings/\n");
  expect(await readFile(join(home, ".qc", ".default-settings", "config.toml"), "utf8")).toEqual(await readFile(join(packageRoot, "share/settings", "config.toml"), "utf8"));
  for (const name of ["git-commit-push.md", "joke.md", "system-status.md"]) expect(await readFile(join(home, ".qc", "prompts", "samples", name), "utf8")).toEqual(await readFile(join(packageRoot, "share/settings", "prompts", "samples", name), "utf8"));
  await expect(readFile(marker, "utf8")).rejects.toMatchObject({ code: "ENOENT" });
}
it.each([
  { long: ["--help"], short: ["-h"], output: "help" }, { long: ["--version"], short: ["-v"], output: "version" }
])("bootstraps fresh homes and gives $short the same output as $long without invoking Pi", async ({ long, short, output }) => {
  const first = await setup(); const firstMarker = join(first.root, "pi-marker"); const firstResult = await run(first.cwd, first.home, first.bin, long, { QC_PI_MARKER: firstMarker }); await assertInformationalBootstrap(first.home, firstMarker);
  const second = await setup(); const secondMarker = join(second.root, "pi-marker"); const secondResult = await run(second.cwd, second.home, second.bin, short, { QC_PI_MARKER: secondMarker }); await assertInformationalBootstrap(second.home, secondMarker);
  expect(firstResult).toMatchObject({ code: 0, err: "" }); expect(secondResult).toEqual(firstResult); if (output === "version") expect(firstResult.out).toBe(`${JSON.parse(await readFile(join(packageRoot, "package.json"), "utf8")).version}\n`);
});
it.each([["--help"], ["-h"], ["--version"], ["-v"]].map((args) => [args]))("skips config loading and leaves bootstrap directories unchanged when the global sentinel exists: %j", async (args) => {
  const { root, cwd, home, bin } = await setup(); const active = join(home, ".qc", "prompts", "user.md"); const sample = join(home, ".qc", "prompts", "samples", "joke.md"); const marker = join(root, "pi-marker"); await mkdir(dirname(active), { recursive: true }); await mkdir(dirname(sample), { recursive: true }); await writeFile(join(home, ".qc", "config.toml"), "not = ["); await writeFile(active, "user prompt"); await writeFile(sample, "stale sample");
  const result = await run(cwd, home, bin, args, { QC_PI_MARKER: marker }); expect(result).toMatchObject({ code: 0, err: "" }); expect(await readFile(join(home, ".qc", "config.toml"), "utf8")).toBe("not = ["); expect(await readFile(active, "utf8")).toBe("user prompt"); expect(await readFile(sample, "utf8")).toBe("stale sample"); await expect(readFile(marker, "utf8")).rejects.toMatchObject({ code: "ENOENT" });
});
it.each([["-h", "prompt"], ["-v", "--version"]].map((args) => [args]))("rejects invalid informational forms before bootstrap: %j", async (args) => {
  const { root, cwd, home, bin } = await setup(); const marker = join(root, "pi-marker"); const result = await run(cwd, home, bin, args, { QC_PI_MARKER: marker }); expect(result.code).toBe(1); await expect(readFile(join(home, ".qc", "config.toml"), "utf8")).rejects.toMatchObject({ code: "ENOENT" }); await expect(readFile(marker, "utf8")).rejects.toMatchObject({ code: "ENOENT" });
});
it("wires -s and -a through the built prompt pipeline", async () => {
  const { root, cwd, home, bin } = await setup(); const record = join(root, "record.json"); await writeFile(join(cwd, ".qc", "prompts", "short.md"), "Prompt !`printf shell`"); const result = await run(cwd, home, bin, ["short", "-s", "/bin/sh", "-a", "tail"], { SHELL: join(root, "missing-shell"), QC_RECORD: record }); expect(result).toMatchObject({ code: 0, err: "" }); expect(JSON.parse(await readFile(record, "utf8"))).toEqual({ argv: ["-p"], stdin: "Prompt shell\n\n---\n\nAdditional Message from the user:\n\ntail" });
});
it("bootstraps config, resolves nested aliases, and transports exact Pi input", async () => {
  const { root, cwd, home, bin } = await setup(); const record = join(root, "record.json"); await writeFile(join(cwd, ".qc", "prompts", "nested", "report.md"), "---\nqc_model: smoke/model\nqc_thinking: high\nqc_no_skills: true\nqc_skill_path: ~/skills\nqc_approve: false\n---\nPrompt !`printf body`");
  const first = await run(cwd, home, bin, ["nested/report", "--append", "Add !`printf tail`"], { QC_RECORD: record, QC_PI_STDOUT: "answer\n" }); expect(first).toMatchObject({ code: 0, out: "answer\n", err: "" }); expect(await readFile(join(home, ".qc", "config.toml"))).toEqual(await readFile(join(packageRoot, "share/settings", "config.toml"))); for (const name of ["git-commit-push.md", "joke.md", "system-status.md"]) expect(await readFile(join(home, ".qc", "prompts", "samples", name), "utf8")).toEqual(await readFile(join(packageRoot, "share/settings", "prompts", "samples", name), "utf8")); await expect(readFile(join(home, ".qc", "prompts-samples", "samples", "git-commit-push.md"), "utf8")).rejects.toMatchObject({ code: "ENOENT" }); expect(JSON.parse(await readFile(record, "utf8"))).toEqual({ argv: ["-p", "--model", "smoke/model", "--thinking", "high", "--no-skills", "--skill", join(home, "skills"), "--no-approve"], stdin: "Prompt body\n\n---\n\nAdditional Message from the user:\n\nAdd tail" });
  await writeFile(join(home, ".qc", "config.toml"), 'default-model = "saved/model"\n'); await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain"); const second = await run(cwd, home, bin, ["plain"], { QC_RECORD: record }); expect(second.code).toBe(0); expect(JSON.parse(await readFile(record, "utf8")).argv).toEqual(["-p", "--model", "saved/model"]);
});
it("preserves sibling prompt files and seeds samples only when config is missing", async () => {
  const { cwd, home, bin } = await setup(); const sibling = join(home, ".qc", "prompts", "mine.md"); const active = join(home, ".qc", "prompts", "samples", "git-commit-push.md");
  await mkdir(dirname(sibling), { recursive: true }); await mkdir(dirname(active), { recursive: true }); await writeFile(sibling, "keep me"); await writeFile(active, "user sample"); await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const result = await run(cwd, home, bin, ["plain"]); expect(result.code).toBe(0); expect(await readFile(sibling, "utf8")).toBe("keep me"); expect(await readFile(active, "utf8")).toEqual(await readFile(join(packageRoot, "share/settings", "prompts", "samples", "git-commit-push.md"), "utf8"));
});
it("leaves prompt directories unchanged when global config exists", async () => {
  const { cwd, home, bin } = await setup(); const active = join(home, ".qc", "prompts", "user.md"); const sample = join(home, ".qc", "prompts", "samples", "joke.md"); await mkdir(dirname(active), { recursive: true }); await mkdir(dirname(sample), { recursive: true }); await writeFile(join(home, ".qc", "config.toml"), ""); await writeFile(active, "user prompt"); await writeFile(sample, "stale sample"); await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const result = await run(cwd, home, bin, ["plain"]); expect(result.code).toBe(0); expect(await readFile(active, "utf8")).toBe("user prompt"); expect(await readFile(sample, "utf8")).toBe("stale sample");
});
it("refreshes packaged samples through the built install flag", async () => {
  const { home, bin } = await setup(); const sample = join(home, ".qc", "prompts", "samples", "joke.md"); await mkdir(dirname(sample), { recursive: true }); await writeFile(join(home, ".qc", "config.toml"), "existing"); await writeFile(sample, "stale");
  const result = await run(process.cwd(), home, bin, ["--install-sample-prompts"]); expect(result.code).toBe(0);
  expect(await readFile(sample, "utf8")).toEqual(await readFile(join(packageRoot, "share/settings", "prompts", "samples", "joke.md"), "utf8"));
});
it("installs harness assets through the built CLI", async () => {
  const { home, bin } = await setup(); const result = await run(process.cwd(), home, bin, ["--install-agent-harness", "cursor"]); expect(result.code).toBe(0);
  expect(await readFile(join(home, ".cursor", "commands", "qc", "qc-create-prompt.md"), "utf8")).toContain("# create-prompt");
  expect(await readFile(join(home, ".agents", "skills", "qc", "SKILL.md"), "utf8")).toContain("name: qc");
});
it("runs compiled bootstrap entries for settings refresh and harness refresh-known", async () => {
  const { home } = await setup(); const settingsBootstrap = join(packageRoot, "dist", "bootstrap-settings.js"); const harnessBootstrap = join(packageRoot, "dist", "bootstrap-harness.js");
  await mkdir(join(home, ".cursor", "commands", "qc"), { recursive: true });
  await writeFile(join(home, ".cursor", "commands", "qc", "qc-create-prompt.md"), "stale"); await mkdir(join(home, ".agents", "skills", "qc"), { recursive: true }); await writeFile(join(home, ".agents", "skills", "qc", "SKILL.md"), "stale");
  await new Promise<void>((resolveRun, reject) => { spawn(process.execPath, [settingsBootstrap, "refresh"], { env: { ...process.env, HOME: home }, stdio: "ignore" }).on("error", reject).on("close", (code) => (code === 0 ? resolveRun() : reject(new Error(String(code))))); });
  expect(await readFile(join(home, ".qc", ".gitignore"), "utf8")).toBe(".default-settings/\n");
  await new Promise<void>((resolveRun, reject) => { spawn(process.execPath, [harnessBootstrap, "refresh-known"], { env: { ...process.env, HOME: home }, stdio: "ignore" }).on("error", reject).on("close", (code) => (code === 0 ? resolveRun() : reject(new Error(String(code))))); });
  expect(await readFile(join(home, ".cursor", "commands", "qc", "qc-create-prompt.md"), "utf8")).toContain("# create-prompt");
  expect(await readFile(join(home, ".agents", "skills", "qc", "SKILL.md"), "utf8")).toContain("name: qc");
});
it("propagates Pi streams and status, and reports representative errors", async () => {
  const { cwd, home, bin } = await setup(); await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain"); const piFailure = await run(cwd, home, bin, ["plain"], { QC_PI_STDOUT: "out", QC_PI_STDERR: "err", QC_PI_EXIT: "7" }); expect(piFailure).toEqual({ code: 7, out: "out", err: "err" }); const missing = await run(cwd, home, join(cwd, "empty"), ["absent"]); expect(missing.code).toBe(1); expect(missing.err).toContain("prompt alias 'absent' was not found");
});
it("preserves bootstrap edits and applies only supplied project scalar fields", async () => {
  const { cwd, home, bin } = await setup(); const record = join(home, "record.json"); await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain"); await run(cwd, home, bin, ["plain"]); await writeFile(join(home, ".qc", "config.toml"), 'default-model = "saved/model"\ndefault-thinking = "medium"\n'); await mkdir(join(cwd, ".qc"), { recursive: true }); await writeFile(join(cwd, ".qc", "config.toml"), 'default-thinking = "high"\n');
  const result = await run(cwd, home, bin, ["plain"], { QC_RECORD: record }); expect(result.code).toBe(0); expect(await readFile(join(home, ".qc", "config.toml"), "utf8")).toContain("saved/model"); expect(JSON.parse(await readFile(record, "utf8")).argv).toEqual(["-p", "--model", "saved/model", "--thinking", "high"]);
});
it("resolves every canonical direct path and alias lookup in contract order", async () => {
  const { root, cwd, home, bin } = await setup(); const record = join(root, "record.json"); const global = join(home, ".qc", "prompts"); await mkdir(join(cwd, "folder"), { recursive: true }); await mkdir(join(global, "nested"), { recursive: true }); await writeFile(join(cwd, "absolute.md"), "absolute"); await writeFile(join(cwd, "relative.md"), "relative"); await writeFile(join(root, "parent.md"), "parent"); await writeFile(join(cwd, "folder", "prompt.md"), "suffix"); await writeFile(join(cwd, ".qc", "prompts", "shared.md"), "project wins"); await writeFile(join(global, "shared.md"), "global loses"); await writeFile(join(global, "global.md"), "global"); await writeFile(join(global, "nested", "report.md"), "nested");
  for (const [reference, expected] of [[join(cwd, "absolute.md"), "absolute"], ["./relative.md", "relative"], ["../parent.md", "parent"], ["folder/prompt.md", "suffix"], ["shared", "project wins"], ["global", "global"], ["nested/report", "nested"]]) { const result = await run(cwd, home, bin, [reference], { QC_RECORD: record }); expect(result.code).toBe(0); expect(JSON.parse(await readFile(record, "utf8")).stdin).toBe(expected); }
  const missing = await run(cwd, home, bin, ["missing/candidate"]); const projectCandidate = join(cwd, ".qc", "prompts", "missing/candidate.md"); const globalCandidate = join(home, ".qc", "prompts", "missing/candidate.md"); expect(missing.code).toBe(1); const projectIndex = missing.err.indexOf(projectCandidate); const globalIndex = missing.err.indexOf(globalCandidate); expect(projectIndex).toBeGreaterThanOrEqual(0); expect(globalIndex).toBeGreaterThan(projectIndex);
});
it.each([{ signal: "SIGINT" as const, code: 130 }, { signal: "SIGTERM" as const, code: 143 }])("forwards $signal from built qc to active fixture Pi and exits $code", async ({ signal, code }) => {
  const { root, cwd, home, bin } = await setup(); const ready = join(root, `${signal}.ready`); const signalRecord = join(root, `${signal}.record`); await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain"); const active = start(cwd, home, bin, ["plain"], { QC_PI_WAIT: "1", QC_PI_READY: ready, QC_PI_SIGNAL_RECORD: signalRecord }); await waitForReady(ready); active.child.kill(signal); expect(await active.result).toEqual({ code, out: "", err: "" }); expect(await readFile(signalRecord, "utf8")).toBe(signal);
});
it("uses centralized diagnostics for malformed input, invalid shell, and missing Pi", async () => {
  const { root, cwd, home, bin } = await setup(); await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain"); await mkdir(join(home, ".qc"), { recursive: true }); await writeFile(join(home, ".qc", "config.toml"), "default-model = ["); const malformed = await run(cwd, home, bin, ["plain"]); expect(malformed.err).toContain("qc: error: invalid TOML"); await writeFile(join(home, ".qc", "config.toml"), ""); await writeFile(join(cwd, ".qc", "prompts", "bad.md"), "---\nqc_approve: yes\n---\nbody"); const frontmatter = await run(cwd, home, bin, ["bad"]); expect(frontmatter.err).toContain("qc: error:"); const shell = await run(cwd, home, bin, ["plain", "--shell", join(root, "missing-shell")]); expect(shell.err).toContain("shell executable could not be started"); const noPi = join(root, "no-pi"); await mkdir(noPi); await symlink(process.execPath, join(noPi, "node")); const pi = await run(cwd, home, noPi, ["plain"]); expect(pi.err).toContain("Pi executable 'pi' was not found");
});
it("preflights allowed, denied, unmatched, and absent-table substitutions without partial effects", async () => {
  const { root, cwd, home, bin } = await setup(); const marker = join(root, "marker"); const piMarker = join(root, "pi-marker"); const record = join(root, "record.json"); await mkdir(join(home, ".qc"), { recursive: true }); await writeFile(join(cwd, ".qc", "prompts", "shell.md"), `!\`printf allowed\``); await writeFile(join(home, ".qc", "config.toml"), '[command-permissions]\n"printf *" = "allow"\n'); const allowed = await run(cwd, home, bin, ["shell"], { QC_RECORD: record }); expect(allowed.code).toBe(0); expect(JSON.parse(await readFile(record, "utf8")).stdin).toBe("allowed"); await writeFile(join(cwd, ".qc", "prompts", "blocked.md"), `!\`printf early && touch ${marker}\``); await writeFile(join(home, ".qc", "config.toml"), '[command-permissions]\n"*" = "allow"\n"touch *" = "deny"\n'); const denied = await run(cwd, home, bin, ["blocked"], { QC_PI_MARKER: piMarker }); expect(denied.code).toBe(1); await expect(readFile(marker, "utf8")).rejects.toMatchObject({ code: "ENOENT" }); await expect(readFile(piMarker, "utf8")).rejects.toMatchObject({ code: "ENOENT" }); await writeFile(join(cwd, ".qc", "prompts", "unmatched.md"), "!`whoami`"); await writeFile(join(home, ".qc", "config.toml"), '[command-permissions]\n"printf *" = "allow"\n'); const unmatched = await run(cwd, home, bin, ["unmatched"]); expect(unmatched.err).toContain("no matching");   await writeFile(join(home, ".qc", "config.toml"), ""); const absent = await run(cwd, home, bin, ["unmatched"], { QC_RECORD: record }); expect(absent.code).toBe(0); expect(JSON.parse(await readFile(record, "utf8")).stdin).not.toContain("!`");
});
it("rejects unsupported qc_cli and default-cli before shell expansion or Pi", async () => {
  const { root, cwd, home, bin } = await setup(); const marker = join(root, "marker"); const record = join(root, "record.json");
  await mkdir(join(home, ".qc"), { recursive: true }); await writeFile(join(cwd, ".qc", "prompts", "cursor.md"), `---\nqc_cli: cursor\n---\n!\`touch ${marker}\``);
  const promptCli = await run(cwd, home, bin, ["cursor"], { QC_PI_MARKER: marker, QC_RECORD: record });
  expect(promptCli.code).toBe(1); expect(promptCli.err).toContain("unsupported cli runtime");
  await expect(readFile(marker, "utf8")).rejects.toMatchObject({ code: "ENOENT" });
  await expect(readFile(record, "utf8")).rejects.toMatchObject({ code: "ENOENT" });
  await writeFile(join(home, ".qc", "config.toml"), 'default-cli = "opencode"\n'); await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const configCli = await run(cwd, home, bin, ["plain"], { QC_RECORD: record });
  expect(configCli.code).toBe(1); expect(configCli.err).toContain("unsupported cli runtime");
  await expect(readFile(record, "utf8")).rejects.toMatchObject({ code: "ENOENT" });
});
