import { chmod, copyFile, mkdir, readFile, readdir, symlink, writeFile } from "node:fs/promises";
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
  expect(await readFile(join(home, ".qc", "config.toml"), "utf8")).toEqual(
    await readFile(join(packageRoot, "share/settings", "config.toml"), "utf8"),
  );
  expect(await readFile(join(home, ".qc", ".gitignore"), "utf8")).toBe(".default-settings/\nsessions/\n");
  for (const name of ["git-commit-push.md", "joke.md", "system-status.md", "web-surf.md"]) {
    expect(await readFile(join(home, ".qc", "prompts", "samples", name), "utf8")).toEqual(
      await readFile(join(packageRoot, "share/settings", "prompts", "samples", name), "utf8"),
    );
  }
  await expect(readFile(marker, "utf8")).rejects.toMatchObject({ code: "ENOENT" });
}

const fixtureBin = join(packageRoot, "tests/fixtures/bin");

// Parent `setup()` only copies the pi fixture. Child tests need every agent fixture.
async function installFixtures(bin: string): Promise<void> {
  for (const name of await readdir(fixtureBin)) {
    await copyFile(join(fixtureBin, name), join(bin, name));
    await chmod(join(bin, name), 0o755);
  }
}

async function setupAgents() {
  const ctx = await setup();
  await installFixtures(ctx.bin);
  return ctx;
}

it.each([
  { long: ["--help"], short: ["-h"], output: "help" },
  { long: ["--version"], short: ["-v"], output: "version" },
])("bootstraps fresh homes and gives $short the same output as $long without invoking agents", async ({
  long,
  short,
  output,
}) => {
  const first = await setupAgents();
  const firstMarker = join(first.root, "pi-marker");
  const firstResult = await run(first.cwd, first.home, first.bin, long, { QC_PI_MARKER: firstMarker });
  await assertInformationalBootstrap(first.home, firstMarker);
  const second = await setupAgents();
  const secondMarker = join(second.root, "pi-marker");
  const secondResult = await run(second.cwd, second.home, second.bin, short, { QC_PI_MARKER: secondMarker });
  await assertInformationalBootstrap(second.home, secondMarker);
  expect(firstResult).toMatchObject({ code: 0, err: "" });
  expect(secondResult).toEqual(firstResult);
  if (output === "version") {
    expect(firstResult.out).toBe(`${JSON.parse(await readFile(join(packageRoot, "package.json"), "utf8")).version}\n`);
  }
});

it.each([["--help"], ["-h"], ["--version"], ["-v"]].map((args) => [args]))(
  "skips config loading when the global sentinel exists: %j",
  async (args) => {
    const { root, cwd, home, bin } = await setupAgents();
    const active = join(home, ".qc", "prompts", "user.md");
    const sample = join(home, ".qc", "prompts", "samples", "joke.md");
    const marker = join(root, "pi-marker");
    await mkdir(dirname(active), { recursive: true });
    await mkdir(dirname(sample), { recursive: true });
    await writeFile(join(home, ".qc", "config.toml"), "not = [");
    await writeFile(active, "user prompt");
    await writeFile(sample, "stale sample");
    const result = await run(cwd, home, bin, args, { QC_PI_MARKER: marker });
    expect(result).toMatchObject({ code: 0, err: "" });
    expect(await readFile(join(home, ".qc", "config.toml"), "utf8")).toBe("not = [");
    expect(await readFile(active, "utf8")).toBe("user prompt");
    expect(await readFile(sample, "utf8")).toBe("stale sample");
    await expect(readFile(marker, "utf8")).rejects.toMatchObject({ code: "ENOENT" });
  },
);

it("wires -s and -a through the built prompt pipeline", async () => {
  const { root, cwd, home, bin } = await setupAgents();
  const record = join(root, "record.json");
  await writeFile(join(cwd, ".qc", "prompts", "short.md"), "Prompt !`printf shell`");
  const result = await run(cwd, home, bin, ["short", "-s", "/bin/sh", "-a", "tail"], {
    SHELL: join(root, "missing-shell"),
    QC_RECORD: record,
    QC_AGENT_TEXT: "ok",
  });
  expect(result.code).toBe(0);
  expect(result.out).toContain("ok");
  expect(result.out).toMatch(/\[QC-SESSION\]/);
  expect(JSON.parse(await readFile(record, "utf8")).stdin).toBe(
    "Prompt shell\n\n---\n\nAdditional Message from the user:\n\ntail",
  );
});

it("sends append-only text raw through the built CLI to fixture pi", async () => {
  const { root, cwd, home, bin } = await setupAgents();
  const record = join(root, "record.json");
  const result = await run(cwd, home, bin, ["--append", "inspect this repo"], {
    QC_RECORD: record,
    QC_AGENT_TEXT: "ok",
  });
  expect(result.code).toBe(0);
  expect(result.out).toContain("ok");
  expect(result.out).toMatch(/\[QC-SESSION\]/);
  expect(JSON.parse(await readFile(record, "utf8")).stdin).toBe("inspect this repo");
});

it("prints one quoted success envelope on the built CLI", async () => {
  const { cwd, home, bin } = await setupAgents();
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const result = await run(cwd, home, bin, ["plain"], { QC_AGENT_TEXT: "joke" });
  expect(result.code).toBe(0);
  expect(result.err).toBe("");
  expect(result.out).toMatch(
    /^"""\njoke\n"""\n\[SUCCESS\] pi ⋅ openai-codex\/gpt-5\.6-luna ⋅ \d+s\n\[QC-SESSION\] \d{6}-\d{4}--pi--[a-z0-9]{6}\n$/,
  );
});

it("prints only the final envelope when built qc is passed -q", async () => {
  const { cwd, home, bin } = await setupAgents();
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const result = await run(cwd, home, bin, ["-q", "plain"], { QC_AGENT_TEXT: "joke" });
  expect(result.code).toBe(0);
  expect(result.err).toBe("");
  expect(result.out).toMatch(
    /^"""\njoke\n"""\n\[SUCCESS\] pi ⋅ openai-codex\/gpt-5\.6-luna ⋅ \d+s\n\[QC-SESSION\] \d{6}-\d{4}--pi--[a-z0-9]{6}\n$/,
  );
});

it("prints one JSON object on the built CLI with or without -q", async () => {
  const { cwd, home, bin } = await setupAgents();
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const json = await run(cwd, home, bin, ["plain", "--output", "json"], { QC_AGENT_TEXT: "joke" });
  expect(json.code).toBe(0);
  expect(json.err).toBe("");
  expect(JSON.parse(json.out)).toMatchObject({ tool: "pi", warnings: [], exit: 0 });
  const quiet = await run(cwd, home, bin, ["-q", "plain", "--output", "json"], { QC_AGENT_TEXT: "joke" });
  expect(quiet.code).toBe(0);
  expect(quiet.err).toBe("");
  expect(JSON.parse(quiet.out)).toMatchObject({ tool: "pi", warnings: [], exit: 0 });
});

it("bootstraps config, resolves nested aliases, and extracts assistant text", async () => {
  const { root, cwd, home, bin } = await setupAgents();
  const record = join(root, "record.json");
  await writeFile(
    join(cwd, ".qc", "prompts", "nested", "report.md"),
    "---\nqc_model: smoke/model\nqc_thinking: high\nqc_no_skills: true\nqc_skill_path: ~/skills\n---\nPrompt !`printf body`",
  );
  const first = await run(cwd, home, bin, ["nested/report", "--append", "Add !`printf tail`"], {
    QC_RECORD: record,
    QC_AGENT_TEXT: "answer",
  });
  expect(first.code).toBe(0);
  expect(first.out).toContain("answer");
  expect(first.out).toMatch(/\[QC-SESSION\]/);
  expect(await readFile(join(home, ".qc", "config.toml"))).toEqual(
    await readFile(join(packageRoot, "share/settings", "config.toml")),
  );
  const recorded = JSON.parse(await readFile(record, "utf8"));
  expect(recorded.argv).toEqual([
    "--mode",
    "json",
    "-a",
    "--session-id",
    expect.stringMatching(/^\d{6}-\d{4}--pi--[a-z0-9]{6}$/),
    "--model",
    "smoke/model",
    "--thinking",
    "high",
    "--no-skills",
    "--skill",
    join(home, "skills"),
  ]);
  expect(recorded.stdin).toBe("Prompt body\n\n---\n\nAdditional Message from the user:\n\nAdd tail");

  await writeFile(join(home, ".qc", "config.toml"), '[tool.pi]\ndefault_model = "saved/model"\n');
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const second = await run(cwd, home, bin, ["plain"], { QC_RECORD: record, QC_AGENT_TEXT: "x" });
  expect(second.code).toBe(0);
  expect(JSON.parse(await readFile(record, "utf8")).argv).toEqual([
    "--mode",
    "json",
    "-a",
    "--session-id",
    expect.stringMatching(/^\d{6}-\d{4}--pi--[a-z0-9]{6}$/),
    "--model",
    "saved/model",
  ]);
});

it("preserves sibling prompt files and seeds samples only when config is missing", async () => {
  const { cwd, home, bin } = await setupAgents();
  const sibling = join(home, ".qc", "prompts", "mine.md");
  const active = join(home, ".qc", "prompts", "samples", "git-commit-push.md");
  await mkdir(dirname(sibling), { recursive: true });
  await mkdir(dirname(active), { recursive: true });
  await writeFile(sibling, "keep me");
  await writeFile(active, "user sample");
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const result = await run(cwd, home, bin, ["plain"], { QC_AGENT_TEXT: "ok" });
  expect(result.code).toBe(0);
  expect(await readFile(sibling, "utf8")).toBe("keep me");
  expect(await readFile(active, "utf8")).toEqual(
    await readFile(join(packageRoot, "share/settings", "prompts", "samples", "git-commit-push.md"), "utf8"),
  );
});

it("leaves prompt directories unchanged when global config exists", async () => {
  const { cwd, home, bin } = await setupAgents();
  const active = join(home, ".qc", "prompts", "user.md");
  const sample = join(home, ".qc", "prompts", "samples", "joke.md");
  await mkdir(dirname(active), { recursive: true });
  await mkdir(dirname(sample), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(active, "user prompt");
  await writeFile(sample, "stale sample");
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const result = await run(cwd, home, bin, ["plain"], { QC_AGENT_TEXT: "ok" });
  expect(result.code).toBe(0);
  expect(await readFile(active, "utf8")).toBe("user prompt");
  expect(await readFile(sample, "utf8")).toBe("stale sample");
});

it("refreshes packaged samples through the built install flag", async () => {
  const { home, bin } = await setupAgents();
  const sample = join(home, ".qc", "prompts", "samples", "joke.md");
  await mkdir(dirname(sample), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "existing");
  await writeFile(sample, "stale");
  const result = await run(process.cwd(), home, bin, ["--install-sample-prompts"]);
  expect(result.code).toBe(0);
  expect(await readFile(sample, "utf8")).toEqual(
    await readFile(join(packageRoot, "share/settings", "prompts", "samples", "joke.md"), "utf8"),
  );
});

it("installs packaged skills through the built CLI", async () => {
  const { home, bin } = await setupAgents();
  const result = await run(process.cwd(), home, bin, ["--install-agent-harness"]);
  expect(result.code).toBe(0);
  expect(await readFile(join(home, ".agents", "skills", "qc", "SKILL.md"), "utf8")).toContain("name: qc");
  expect(await readFile(join(home, ".claude", "skills", "qc-create-prompt", "SKILL.md"), "utf8")).toContain(
    "disable-model-invocation: true",
  );
  await expect(readFile(join(home, ".cursor", "commands", "qc", "qc-create-prompt.md"), "utf8")).rejects.toMatchObject({
    code: "ENOENT",
  });
});

it("runs compiled bootstrap entries for settings refresh and harness refresh-known", async () => {
  const { home } = await setupAgents();
  const settingsBootstrap = join(packageRoot, "dist", "bootstrap-settings.js");
  const harnessBootstrap = join(packageRoot, "dist", "bootstrap-harness.js");
  await mkdir(join(home, ".agents", "skills", "qc"), { recursive: true });
  await writeFile(join(home, ".agents", "skills", "qc", "SKILL.md"), "stale");
  await new Promise<void>((resolveRun, reject) => {
    spawn(process.execPath, [settingsBootstrap, "refresh"], { env: { ...process.env, HOME: home }, stdio: "ignore" })
      .on("error", reject)
      .on("close", (code) => (code === 0 ? resolveRun() : reject(new Error(String(code)))));
  });
  expect(await readFile(join(home, ".qc", ".gitignore"), "utf8")).toBe(".default-settings/\nsessions/\n");
  await new Promise<void>((resolveRun, reject) => {
    spawn(process.execPath, [harnessBootstrap, "refresh-known"], {
      env: { ...process.env, HOME: home },
      stdio: "ignore",
    })
      .on("error", reject)
      .on("close", (code) => (code === 0 ? resolveRun() : reject(new Error(String(code)))));
  });
  expect(await readFile(join(home, ".agents", "skills", "qc", "SKILL.md"), "utf8")).toContain("name: qc");
  expect(await readFile(join(home, ".agents", "skills", "qc-create-prompt", "SKILL.md"), "utf8")).toContain(
    "disable-model-invocation: true",
  );
  await expect(readFile(join(home, ".claude", "skills", "qc", "SKILL.md"), "utf8")).rejects.toMatchObject({
    code: "ENOENT",
  });
});

it("propagates agent exit and reports representative errors", async () => {
  const { cwd, home, bin } = await setupAgents();
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const piFailure = await run(cwd, home, bin, ["plain"], {
    QC_AGENT_TEXT: "out",
    QC_PI_STDERR: "err",
    QC_PI_EXIT: "7",
  });
  expect(piFailure.code).toBe(7);
  expect(piFailure.out).toContain("out");
  expect(piFailure.out).toMatch(/\[QC-SESSION\]/);
  expect(piFailure.err).not.toContain("err");
  const missing = await run(cwd, home, join(cwd, "empty"), ["absent"]);
  expect(missing.code).toBe(1);
  expect(missing.err).toContain("prompt alias 'absent' was not found");
});

it("preserves bootstrap edits and applies only supplied project tool fields", async () => {
  const { cwd, home, bin } = await setupAgents();
  const record = join(home, "record.json");
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  await run(cwd, home, bin, ["plain"], { QC_AGENT_TEXT: "a" });
  await writeFile(
    join(home, ".qc", "config.toml"),
    '[tool.pi]\ndefault_model = "saved/model"\ndefault_thinking = "medium"\n',
  );
  await mkdir(join(cwd, ".qc"), { recursive: true });
  await writeFile(join(cwd, ".qc", "config.toml"), '[tool.pi]\ndefault_thinking = "high"\n');
  const result = await run(cwd, home, bin, ["plain"], { QC_RECORD: record, QC_AGENT_TEXT: "b" });
  expect(result.code).toBe(0);
  expect(JSON.parse(await readFile(record, "utf8")).argv).toEqual([
    "--mode",
    "json",
    "-a",
    "--session-id",
    expect.stringMatching(/^\d{6}-\d{4}--pi--[a-z0-9]{6}$/),
    "--model",
    "saved/model",
    "--thinking",
    "high",
  ]);
});

it("resolves every canonical direct path and alias lookup in contract order", async () => {
  const { root, cwd, home, bin } = await setupAgents();
  const record = join(root, "record.json");
  const global = join(home, ".qc", "prompts");
  await mkdir(join(cwd, "folder"), { recursive: true });
  await mkdir(join(global, "nested"), { recursive: true });
  await writeFile(join(cwd, "absolute.md"), "absolute");
  await writeFile(join(cwd, "relative.md"), "relative");
  await writeFile(join(root, "parent.md"), "parent");
  await writeFile(join(cwd, "folder", "prompt.md"), "suffix");
  await writeFile(join(cwd, ".qc", "prompts", "shared.md"), "project wins");
  await writeFile(join(global, "shared.md"), "global loses");
  await writeFile(join(global, "global.md"), "global");
  await writeFile(join(global, "nested", "report.md"), "nested");
  for (const [reference, expected] of [
    [join(cwd, "absolute.md"), "absolute"],
    ["./relative.md", "relative"],
    ["../parent.md", "parent"],
    ["folder/prompt.md", "suffix"],
    ["shared", "project wins"],
    ["global", "global"],
    ["nested/report", "nested"],
  ] as const) {
    const result = await run(cwd, home, bin, [reference], { QC_RECORD: record, QC_AGENT_TEXT: "x" });
    expect(result.code).toBe(0);
    expect(JSON.parse(await readFile(record, "utf8")).stdin).toBe(expected);
  }
  const missing = await run(cwd, home, bin, ["missing/candidate"]);
  const projectCandidate = join(cwd, ".qc", "prompts", "missing/candidate.md");
  const globalCandidate = join(home, ".qc", "prompts", "missing/candidate.md");
  expect(missing.code).toBe(1);
  const projectIndex = missing.err.indexOf(projectCandidate);
  const globalIndex = missing.err.indexOf(globalCandidate);
  expect(projectIndex).toBeGreaterThanOrEqual(0);
  expect(globalIndex).toBeGreaterThan(projectIndex);
});

it.each([
  { signal: "SIGINT" as const, code: 130 },
  { signal: "SIGTERM" as const, code: 143 },
])("forwards $signal from built qc to active fixture Pi and exits $code", async ({ signal, code }) => {
  const { root, cwd, home, bin } = await setupAgents();
  const ready = join(root, `${signal}.ready`);
  const signalRecord = join(root, `${signal}.record`);
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const active = start(cwd, home, bin, ["plain"], {
    QC_PI_WAIT: "1",
    QC_PI_READY: ready,
    QC_PI_SIGNAL_RECORD: signalRecord,
  });
  await waitForReady(ready);
  active.child.kill(signal);
  const result = await active.result;
  expect(result.code).toBe(code);
  expect(result.out).toBe("");
  expect(result.err).toMatch(
    new RegExp(`^\\[ERROR\\] interrupted \\(signal ${signal === "SIGINT" ? 2 : 15}\\)\\n\\[QC-SESSION\\] (\\d{6}-\\d{4}--pi--[a-z0-9]{6})\\nqc -o \\1\\n$`),
  );
  expect(await readFile(signalRecord, "utf8")).toBe(signal);
  const sessionId = result.err.match(/\[QC-SESSION\] (\S+)/)?.[1];
  expect(sessionId).toBeTruthy();
  await expect(readFile(join(home, ".qc", "sessions", `${sessionId}.json`), "utf8")).resolves.toContain(`"tool": "pi"`);
});

it("uses centralized diagnostics for malformed input, invalid shell, and missing binary", async () => {
  const { root, cwd, home, bin } = await setupAgents();
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  await mkdir(join(home, ".qc"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "default-model = [");
  const malformed = await run(cwd, home, bin, ["plain"]);
  expect(malformed.err).toContain("[ERROR]");
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "bad.md"), "---\nqc_approve: yes\n---\nbody");
  const frontmatter = await run(cwd, home, bin, ["bad"]);
  expect(frontmatter.err).toContain("[ERROR]");
  const shell = await run(cwd, home, bin, ["plain", "--shell", join(root, "missing-shell")]);
  expect(shell.err).toContain("shell executable could not be started");
  const noPi = join(root, "no-pi");
  await mkdir(noPi);
  await symlink(process.execPath, join(noPi, "node"));
  const pi = await run(cwd, home, noPi, ["plain"]);
  expect(pi.err).toMatch(/pi executable 'pi' was not found/i);
});

it("ignores leftover [command-permissions] and still expands substitutions", async () => {
  const { root, cwd, home, bin } = await setupAgents();
  const marker = join(root, "marker");
  const record = join(root, "record.json");
  await mkdir(join(home, ".qc"), { recursive: true });
  await writeFile(join(cwd, ".qc", "prompts", "shell.md"), `!\`printf allowed\``);
  await writeFile(join(home, ".qc", "config.toml"), '[command-permissions]\n"printf *" = "allow"\n');
  const allowed = await run(cwd, home, bin, ["shell"], { QC_RECORD: record, QC_AGENT_TEXT: "x" });
  expect(allowed.code).toBe(0);
  expect(JSON.parse(await readFile(record, "utf8")).stdin).toBe("allowed");
  // Stale deny/ask tables must not block expansion or agent spawn.
  // Fixture PATH has no touch; printf builtin writes the side-effect marker.
  await writeFile(join(cwd, ".qc", "prompts", "blocked.md"), `!\`printf early && printf ran > ${marker}\``);
  await writeFile(join(home, ".qc", "config.toml"), '[command-permissions]\n"*" = "allow"\n"printf *" = "deny"\n');
  const denied = await run(cwd, home, bin, ["blocked"], { QC_RECORD: record, QC_AGENT_TEXT: "x" });
  expect(denied.code).toBe(0);
  expect(await readFile(marker, "utf8")).toBe("ran");
  await writeFile(join(cwd, ".qc", "prompts", "unmatched.md"), "!`printf whoami-ok`");
  await writeFile(join(home, ".qc", "config.toml"), '[command-permissions]\n"printf *" = "allow"\n"*" = "ask"\n');
  const ask = await run(cwd, home, bin, ["unmatched"], { QC_RECORD: record, QC_AGENT_TEXT: "x" });
  expect(ask.code).toBe(0);
  expect(JSON.parse(await readFile(record, "utf8")).stdin).toBe("whoami-ok");
  await writeFile(join(home, ".qc", "config.toml"), "");
  const absent = await run(cwd, home, bin, ["unmatched"], { QC_RECORD: record, QC_AGENT_TEXT: "x" });
  expect(absent.code).toBe(0);
  expect(JSON.parse(await readFile(record, "utf8")).stdin).not.toContain("!`");
});

it("rejects old default-cli rename keys before agent spawn", async () => {
  const { root, cwd, home, bin } = await setupAgents();
  const record = join(root, "record.json");
  await mkdir(join(home, ".qc"), { recursive: true });
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  await writeFile(join(home, ".qc", "config.toml"), 'default-cli = "opencode"\n');
  const configCli = await run(cwd, home, bin, ["plain"], { QC_RECORD: record });
  expect(configCli.code).toBe(1);
  expect(configCli.err).toContain("default-cli");
  await expect(readFile(record, "utf8")).rejects.toMatchObject({ code: "ENOENT" });
});

it("runs claude with thinking and workdir", async () => {
  const { root, cwd, home, bin } = await setupAgents();
  const work = join(root, "w");
  const record = join(root, "record.json");
  await mkdir(work, { recursive: true });
  await writeFile(join(cwd, ".qc", "prompts", "sum.md"), "Do work");
  const result = await run(cwd, home, bin, ["sum", "--tool", "claude", "--model", "sonnet", "--thinking", "high", "--workdir", work], {
    QC_RECORD: record,
    QC_AGENT_TEXT: "claude-ok",
  });
  expect(result.code).toBe(0);
  expect(result.out).toContain("claude-ok");
  const argv = JSON.parse(await readFile(record, "utf8")).argv as string[];
  expect(argv).toEqual(
    expect.arrayContaining(["-p", "--dangerously-skip-permissions", "--output-format", "json", "--model", "sonnet", "--effort", "high"]),
  );
  expect(JSON.parse(await readFile(record, "utf8")).cwd).toBe(work);
});

it("runs opencode with auto, format json, and workdir", async () => {
  const { root, cwd, home, bin } = await setupAgents();
  const work = join(root, "w");
  const record = join(root, "record.json");
  await mkdir(work, { recursive: true });
  await writeFile(join(cwd, ".qc", "prompts", "sum.md"), "Do work");
  const result = await run(
    cwd,
    home,
    bin,
    ["sum", "--tool", "opencode", "--model", "opencode-go/x", "--workdir", work],
    {
      QC_RECORD: record,
      QC_AGENT_TEXT: "opencode-ok",
      QC_OPENCODE_SESSION: "ses_smoke_opencode",
    },
  );
  expect(result.code).toBe(0);
  expect(result.out).toContain("opencode-ok");
  expect(result.out).toMatch(/\[QC-SESSION\] \d{6}-\d{4}--opencode--[a-z0-9]{6}/);
  const recorded = JSON.parse(await readFile(record, "utf8"));
  expect(recorded.cwd).toBe(work);
  expect(recorded.argv).toEqual(
    expect.arrayContaining([
      "run",
      "--auto",
      "--format",
      "json",
      "--dir",
      work,
      "-m",
      "opencode-go/x",
      "Do work",
    ]),
  );
});

it("runs antigravity (agy) with skip-permissions and extracts response", async () => {
  const { root, cwd, home, bin } = await setupAgents();
  const record = join(root, "record.json");
  await writeFile(join(cwd, ".qc", "prompts", "sum.md"), "Do work");
  const result = await run(cwd, home, bin, ["sum", "--tool", "antigravity", "--model", "m", "--thinking", "medium"], {
    QC_RECORD: record,
    QC_AGENT_TEXT: "agy-ok",
    QC_AGY_SESSION: "conv_smoke_agy",
  });
  expect(result.code).toBe(0);
  expect(result.out).toContain("agy-ok");
  expect(result.out).toMatch(/\[QC-SESSION\] \d{6}-\d{4}--antigravity--[a-z0-9]{6}/);
  expect(JSON.parse(await readFile(record, "utf8")).argv).toEqual(
    expect.arrayContaining([
      "-p",
      "--dangerously-skip-permissions",
      "--output-format",
      "json",
      "--model",
      "m",
      "--effort",
      "medium",
      "Do work",
    ]),
  );
});

it("opens a stored session with built qc -o, -o <id>, and -o --tool", async () => {
  const { root, cwd, home, bin } = await setupAgents();
  const record = join(root, "record.json");
  await mkdir(join(home, ".qc", "sessions"), { recursive: true });
  const older = "260901-1000--pi--aaaaaa";
  const newer = "260901-1000--pi--bbbbbb";
  await writeFile(
    join(home, ".qc", "sessions", `${older}.json`),
    `${JSON.stringify({
      tool: "pi",
      native_id: "old-native",
      cwd,
      created: "2026-09-01T00:00:00.000Z",
      updated: "2026-09-01T00:00:00.000Z",
      warnings: [],
    }, null, 2)}\n`,
  );
  await writeFile(
    join(home, ".qc", "sessions", `${newer}.json`),
    `${JSON.stringify({
      tool: "pi",
      native_id: "new-native",
      cwd,
      created: "2026-09-01T00:00:00.000Z",
      updated: "2026-09-01T02:00:00.000Z",
      warnings: [],
    }, null, 2)}\n`,
  );

  const bare = await run(cwd, home, bin, ["-o"], { QC_RECORD: record });
  expect(bare.code).toBe(0);
  expect(bare.err).not.toMatch(/qc: session:/);
  expect(JSON.parse(await readFile(record, "utf8")).argv).toEqual(["--session-id", "new-native"]);

  const filtered = await run(cwd, home, bin, ["-o", "--tool", "pi"], { QC_RECORD: record });
  expect(filtered.code).toBe(0);
  expect(JSON.parse(await readFile(record, "utf8")).argv).toEqual(["--session-id", "new-native"]);

  const byId = await run(cwd, home, bin, ["-o", older], { QC_RECORD: record });
  expect(byId.code).toBe(0);
  expect(byId.err).not.toMatch(/qc: session:/);
  expect(JSON.parse(await readFile(record, "utf8")).argv).toEqual(["--session-id", "old-native"]);

  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const continued = await run(cwd, home, bin, ["plain", "-c", newer], {
    QC_RECORD: record,
    QC_AGENT_TEXT: "headless-ok",
  });
  expect(continued.code).toBe(0);
  expect(continued.out).toContain("headless-ok");
  expect(continued.out).toMatch(/\[QC-SESSION\]/);
  expect(JSON.parse(await readFile(record, "utf8")).argv).toEqual(
    expect.arrayContaining(["--mode", "json", "-a", "--session-id", newer]),
  );
});

it("saves a mapping on empty-text fail and resumes it with built -c", async () => {
  const { cwd, home, bin } = await setupAgents();
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const failed = await run(cwd, home, bin, ["plain"], { QC_AGENT_TEXT: "" });
  expect(failed.code).toBe(1);
  expect(failed.out).toBe("");
  expect(failed.err).toMatch(/^\[ERROR\] pi produced empty assistant text\n\[QC-SESSION\] (\d{6}-\d{4}--pi--[a-z0-9]{6})\nqc -o \1\n$/);
  const sessionId = failed.err.match(/\[QC-SESSION\] (\S+)/)?.[1];
  expect(sessionId).toBeTruthy();
  const resumed = await run(cwd, home, bin, ["-c", sessionId!, "--append", "retry"], { QC_AGENT_TEXT: "resumed" });
  expect(resumed.code).toBe(0);
  expect(resumed.out).toContain("resumed");
  expect(resumed.out).toContain(`[QC-SESSION] ${sessionId}`);
});

it("prints [USAGE] on a built-CLI success when the fixture reports tokens", async () => {
  const { cwd, home, bin } = await setupAgents();
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const result = await run(cwd, home, bin, ["plain"], { QC_AGENT_TEXT: "joke", QC_PI_USAGE: "1" });
  expect(result.code).toBe(0);
  expect(result.out).toContain("[USAGE] 1.2k in ⋅ 400 out ⋅ 50 cache ⋅ 10 think ⋅ 1.7k total ⋅ 0.012");
  const json = await run(cwd, home, bin, ["plain", "--output", "json"], { QC_AGENT_TEXT: "joke", QC_PI_USAGE: "1" });
  expect(JSON.parse(json.out).usage).toMatchObject({ input_tokens: 1200, output_tokens: 400, cost: 0.012 });
});
