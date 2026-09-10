import { chmod, copyFile, mkdir, readFile, readdir, symlink, unlink, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, expect, it } from "vitest";
import { removeTemp, tempDir } from "../helpers/temp.js";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const fixtureBin = join(packageRoot, "tests/fixtures/bin");
const fixtures = join(packageRoot, "tests/fixtures");
const loader = join(packageRoot, "node_modules/tsx/dist/loader.mjs");
const clean: string[] = [];
afterEach(async () => Promise.all(clean.splice(0).map(removeTemp)));

async function installFixtures(bin: string): Promise<void> {
  await mkdir(bin, { recursive: true });
  for (const name of await readdir(fixtureBin)) {
    await copyFile(join(fixtureBin, name), join(bin, name));
    await chmod(join(bin, name), 0o755);
  }
  await symlink(process.execPath, join(bin, "node")).catch(() => undefined);
}

async function invoke(root: string, args: string[], extra: NodeJS.ProcessEnv = {}) {
  const { spawn } = await import("node:child_process");
  const cwd = join(root, "work");
  const home = join(root, "home");
  const bin = join(root, "bin");
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await installFixtures(bin);
  return new Promise<{ code: number | null; out: string; err: string }>((resolveResult, reject) => {
    const child = spawn(process.execPath, ["--import", loader, join(packageRoot, "src/cli.ts"), ...args], {
      cwd,
      env: { ...process.env, ...extra, HOME: home, PATH: bin, SHELL: "/bin/sh" },
    });
    let out = "";
    let err = "";
    child.stdout.on("data", (x) => {
      out += x;
    });
    child.stderr.on("data", (x) => {
      err += x;
    });
    child.on("error", reject);
    child.on("close", (code) => resolveResult({ code, out, err }));
  });
}

it("wires real config, prompt, shell expansion, and fixture Pi JSON", async () => {
  const root = await tempDir("integration");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await mkdir(join(home, ".qc"), { recursive: true });
  await copyFile(join(fixtures, "config/global.toml"), join(home, ".qc/config.toml"));
  await copyFile(join(fixtures, "config/project.toml"), join(cwd, ".qc/config.toml"));
  await writeFile(
    join(cwd, ".qc/prompts/daily.md"),
    (await readFile(join(fixtures, "prompts/frontmatter.md"), "utf8"))
      .replace("Body text", (await readFile(join(fixtures, "prompts/shell-output.md"), "utf8")).replace("First", "Body")),
  );
  const record = join(root, "record.json");
  const result = await invoke(root, ["daily", "--append", "More !`printf append`"], {
    QC_RECORD: record,
    QC_AGENT_TEXT: "done",
  });
  expect(result.code).toBe(0);
  expect(result.out).toContain("done");
  expect(result.out).toMatch(/\[QC-SESSION\] \d{6}-\d{4}--pi--[a-z0-9]{6}/);
  const recorded = JSON.parse(await readFile(record, "utf8"));
  expect(recorded.argv).toEqual([
    "--mode",
    "json",
    "-a",
    "--session-id",
    expect.stringMatching(/^\d{6}-\d{4}--pi--[a-z0-9]{6}$/),
    "--model",
    "prompt/model",
    "--thinking",
    "high",
    "--no-skills",
    "--skill",
    join(home, ".config/custom-skills"),
  ]);
  expect(recorded.stdin).toBe("Body one then two\n\n\n\n---\n\nAdditional Message from the user:\n\nMore append");
});

it("ignores leftover [command-permissions] deny rules and still expands then spawns", async () => {
  const root = await tempDir("ignored-permissions");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  const marker = join(root, "marker");
  const record = join(root, "record.json");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  // Stale deny table must not block expansion or agent spawn.
  await writeFile(join(cwd, ".qc", "config.toml"), '[command-permissions]\n"*" = "allow"\n"touch *" = "deny"\n');
  // Fixture PATH has no touch; printf is a shell builtin and can write the side-effect marker.
  await writeFile(join(cwd, ".qc", "prompts", "allowed.md"), `!\`printf start && printf ran > ${marker}\``);
  const result = await invoke(root, ["allowed"], { QC_RECORD: record, QC_AGENT_TEXT: "ok" });
  expect(result.code).toBe(0);
  expect(result.out).toContain("ok");
  expect(await readFile(marker, "utf8")).toBe("ran");
  expect(JSON.parse(await readFile(record, "utf8")).stdin).toContain("start");
});

it("expands every expression even when a stale permission table would have unmatched whoami", async () => {
  const root = await tempDir("cross-expression");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  const commandMarker = join(root, "command-marker");
  const record = join(root, "record.json");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "config.toml"), '[command-permissions]\n"printf *" = "allow"\n');
  await writeFile(join(cwd, ".qc", "prompts", "both.md"), `!\`printf side > ${commandMarker}\` !\`printf done\``);
  const result = await invoke(root, ["both"], { QC_RECORD: record, QC_AGENT_TEXT: "ok" });
  expect(result.code).toBe(0);
  expect(await readFile(commandMarker, "utf8")).toBe("side");
  expect(JSON.parse(await readFile(record, "utf8")).stdin).toContain("done");
});

it("uses field-level real TOML precedence and ignores leftover permission tables", async () => {
  const root = await tempDir("scalar-override");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  const record = join(root, "record.json");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(
    join(home, ".qc/config.toml"),
    '[tool.pi]\ndefault_model = "global/model"\ndefault_thinking = "medium"\n[command-permissions]\n"*" = "deny"\n',
  );
  await writeFile(
    join(cwd, ".qc/config.toml"),
    '[tool.pi]\ndefault_thinking = "high"\n[command-permissions]\n"printf *" = "allow"\n',
  );
  await writeFile(join(cwd, ".qc/prompts/plain.md"), "!`printf ok`");
  const result = await invoke(root, ["plain"], { QC_RECORD: record, QC_AGENT_TEXT: "ok-out" });
  expect(result.code).toBe(0);
  const recorded = JSON.parse(await readFile(record, "utf8"));
  expect(recorded.argv).toEqual([
    "--mode",
    "json",
    "-a",
    "--session-id",
    expect.stringMatching(/^\d{6}-\d{4}--pi--[a-z0-9]{6}$/),
    "--model",
    "global/model",
    "--thinking",
    "high",
  ]);
  expect(recorded.stdin).toBe("ok");
});

it("loads HOME fixtures with stale [command-permissions] and still expands whoami", async () => {
  const root = await tempDir("permission-modes");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  const record = join(root, "record.json");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc/config.toml"), '[command-permissions]\n"printf *" = "allow"\n"*" = "ask"\n');
  await writeFile(join(cwd, ".qc/prompts/plain.md"), "!`printf whoami-ok`");
  const withTable = await invoke(root, ["plain"], { QC_RECORD: record, QC_AGENT_TEXT: "ok" });
  expect(withTable.code).toBe(0);
  expect(JSON.parse(await readFile(record, "utf8")).stdin).toBe("whoami-ok");
  await writeFile(join(home, ".qc/config.toml"), '[tool.pi]\ndefault_model = "global/model"\n');
  const withoutTable = await invoke(root, ["plain"], { QC_RECORD: record, QC_AGENT_TEXT: "ok" });
  expect(withoutTable.code).toBe(0);
  expect(JSON.parse(await readFile(record, "utf8")).stdin).not.toContain("!`");
});

it("keeps shell stdout from nonzero commands and discards stderr", async () => {
  const root = await tempDir("shell-failure");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  const record = join(root, "record.json");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc/config.toml"), "");
  await writeFile(
    join(cwd, ".qc/prompts/plain.md"),
    "!`printf kept; printf ignored >&2; false` !`printf ignored >&2; false`",
  );
  const result = await invoke(root, ["plain"], { QC_RECORD: record });
  expect(result.code).toBe(0);
  expect(result.out).toMatch(/\[QC-SESSION\]/);
  expect(JSON.parse(await readFile(record, "utf8")).stdin).toBe("kept ");
});

it("rejects removed qc_cli before shell expansion or agent", async () => {
  const root = await tempDir("renamed-cli");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  const marker = join(root, "marker");
  const record = join(root, "record.json");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "cursor.md"), `---\nqc_cli: cursor\n---\n!\`touch ${marker}\``);
  const result = await invoke(root, ["cursor"], { QC_PI_MARKER: marker, QC_RECORD: record });
  expect(result.code).toBe(1);
  expect(result.err).toContain("qc_cli");
  await expect(readFile(marker, "utf8")).rejects.toMatchObject({ code: "ENOENT" });
});

it("runs cursor adapter with force-allow flags and workdir", async () => {
  const root = await tempDir("cursor-tool");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  const work = join(root, "agent-work");
  const record = join(root, "record.json");
  await mkdir(work, { recursive: true });
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "sum.md"), "Summarize");
  const result = await invoke(root, ["sum", "--tool", "cursor", "--model", "composer-2.5", "--workdir", work], {
    QC_RECORD: record,
    QC_AGENT_TEXT: "cursor-ok",
  });
  expect(result.code).toBe(0);
  expect(result.out).toContain("cursor-ok");
  const recorded = JSON.parse(await readFile(record, "utf8"));
  expect(recorded.cwd).toBe(work);
  expect(recorded.argv).toEqual(
    expect.arrayContaining(["-p", "--force", "--yolo", "--approve-mcps", "--trust", "--output-format", "json", "--workspace", work, "--model", "composer-2.5", "Summarize"]),
  );
});

it("continues a session and supports append-only turns", async () => {
  const root = await tempDir("continue");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  const record = join(root, "record.json");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "follow.md"), "Follow up");
  const first = await invoke(root, ["follow", "--tool", "cursor"], {
    QC_RECORD: record,
    QC_AGENT_TEXT: "one",
    QC_CURSOR_SESSION: "resume-uuid-1",
  });
  expect(first.code).toBe(0);
  const sessionMatch = first.out.match(/\[QC-SESSION\] (\S+)/);
  expect(sessionMatch).toBeTruthy();
  const id = sessionMatch![1]!;
  const mapping = JSON.parse(await readFile(join(home, ".qc", "sessions", `${id}.json`), "utf8"));
  expect(mapping).toMatchObject({ tool: "cursor", native_id: "resume-uuid-1" });

  const second = await invoke(root, ["follow", "-c", id], { QC_RECORD: record, QC_AGENT_TEXT: "two" });
  expect(second.code).toBe(0);
  expect(JSON.parse(await readFile(record, "utf8")).argv).toEqual(
    expect.arrayContaining(["--resume", "resume-uuid-1"]),
  );

  const third = await invoke(root, ["-c", id, "-a", "also add tests"], {
    QC_RECORD: record,
    QC_AGENT_TEXT: "three",
  });
  expect(third.code).toBe(0);
  expect(JSON.parse(await readFile(record, "utf8")).argv.at(-1)).toBe("also add tests");
});

it("sends append-only text raw to default pi stdin", async () => {
  const root = await tempDir("append-only-pi");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  const record = join(root, "record.json");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  const result = await invoke(root, ["--append", "inspect this repo"], {
    QC_RECORD: record,
    QC_AGENT_TEXT: "ok",
  });
  expect(result.code).toBe(0);
  expect(result.out).toMatch(/\[QC-SESSION\]/);
  expect(JSON.parse(await readFile(record, "utf8")).stdin).toBe("inspect this repo");
});

it("sends append-only text raw as cursor argv last token with model", async () => {
  const root = await tempDir("append-only-cursor");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  const record = join(root, "record.json");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  const result = await invoke(
    root,
    ["--tool", "cursor", "--model", "composer-2.5", "--append", "inspect this repo"],
    { QC_RECORD: record, QC_AGENT_TEXT: "cursor-ok" },
  );
  expect(result.code).toBe(0);
  const recorded = JSON.parse(await readFile(record, "utf8"));
  expect(recorded.argv).toEqual(expect.arrayContaining(["--model", "composer-2.5"]));
  expect(recorded.argv.at(-1)).toBe("inspect this repo");
});

it("wraps append when a prompt file is present for cursor", async () => {
  const root = await tempDir("review-append-cursor");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  const record = join(root, "record.json");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "review.md"), "Review the changes");
  const result = await invoke(
    root,
    ["review", "--tool", "cursor", "--model", "composer-2.5", "--append", "inspect this repo"],
    { QC_RECORD: record, QC_AGENT_TEXT: "wrapped-ok" },
  );
  expect(result.code).toBe(0);
  const recorded = JSON.parse(await readFile(record, "utf8"));
  expect(recorded.argv).toEqual(expect.arrayContaining(["--model", "composer-2.5"]));
  expect(recorded.argv.at(-1)).toBe(
    "Review the changes\n\n---\n\nAdditional Message from the user:\n\ninspect this repo",
  );
});

it("runs opencode adapter with force-allow flags and workdir", async () => {
  const root = await tempDir("opencode-tool");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  const work = join(root, "agent-work");
  const record = join(root, "record.json");
  await mkdir(work, { recursive: true });
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "sum.md"), "Summarize");
  const result = await invoke(root, ["sum", "--tool", "opencode", "--model", "opencode-go/x", "--workdir", work], {
    QC_RECORD: record,
    QC_AGENT_TEXT: "opencode-ok",
    QC_OPENCODE_SESSION: "ses_fixture_create",
  });
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
      "--title",
      expect.stringMatching(/^\d{6}-\d{4}--opencode--[a-z0-9]{6}$/),
      "-m",
      "opencode-go/x",
      "Summarize",
    ]),
  );
});

it("continues an opencode session with native -s mapping", async () => {
  const root = await tempDir("opencode-continue");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  const record = join(root, "record.json");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "follow.md"), "Follow up");
  const first = await invoke(root, ["follow", "--tool", "opencode"], {
    QC_RECORD: record,
    QC_AGENT_TEXT: "one",
    QC_OPENCODE_SESSION: "ses_resume_opencode",
  });
  expect(first.code).toBe(0);
  const sessionMatch = first.out.match(/\[QC-SESSION\] (\S+)/);
  expect(sessionMatch).toBeTruthy();
  const id = sessionMatch![1]!;
  const mapping = JSON.parse(await readFile(join(home, ".qc", "sessions", `${id}.json`), "utf8"));
  expect(mapping).toMatchObject({ tool: "opencode", native_id: "ses_resume_opencode" });

  const second = await invoke(root, ["follow", "-c", id], {
    QC_RECORD: record,
    QC_AGENT_TEXT: "two",
  });
  expect(second.code).toBe(0);
  expect(second.out).toContain("two");
  expect(JSON.parse(await readFile(record, "utf8")).argv).toEqual(
    expect.arrayContaining(["-s", "ses_resume_opencode"]),
  );
});

it("runs antigravity (agy) adapter with force-allow flags and --add-dir workdir", async () => {
  const root = await tempDir("agy-tool");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  const work = join(root, "agent-work");
  const record = join(root, "record.json");
  await mkdir(work, { recursive: true });
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "sum.md"), "Summarize");
  const result = await invoke(root, ["sum", "--tool", "antigravity", "--model", "m", "--thinking", "high", "--workdir", work], {
    QC_RECORD: record,
    QC_AGENT_TEXT: "agy-ok",
    QC_AGY_SESSION: "conv_fixture_create",
  });
  expect(result.code).toBe(0);
  expect(result.out).toContain("agy-ok");
  expect(result.out).toMatch(/\[QC-SESSION\] \d{6}-\d{4}--antigravity--[a-z0-9]{6}/);
  const recorded = JSON.parse(await readFile(record, "utf8"));
  expect(recorded.cwd).toBe(work);
  expect(recorded.argv).toEqual(
    expect.arrayContaining([
      "--dangerously-skip-permissions",
      "--output-format",
      "json",
      "--model",
      "m",
      "--effort",
      "high",
      "--add-dir",
      work,
      "-p",
      "Summarize",
    ]),
  );
});

it("prints json envelope when --output json", async () => {
  const root = await tempDir("output-json");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const result = await invoke(root, ["plain", "--output", "json"], { QC_AGENT_TEXT: "text" });
  expect(result.code).toBe(0);
  const envelope = JSON.parse(result.out);
  expect(envelope).toMatchObject({ tool: "pi", warnings: [], exit: 0 });
  expect(envelope.session_id).toMatch(/^\d{6}-\d{4}--pi--[a-z0-9]{6}$/);
  expect(envelope.native_id).toBeTruthy();
  expect(typeof envelope.duration_s).toBe("number");
  expect(envelope.model).toBeNull();
  expect(envelope.usage).toBeUndefined();
  expect(envelope.debug).toBeUndefined();
  expect(result.err).not.toMatch(/\[QC-SESSION\]/);
  expect(result.err).not.toMatch(/qc: session:/);

  const debug = await invoke(root, ["plain", "--output", "json", "--debug"], { QC_AGENT_TEXT: "text" });
  expect(debug.code).toBe(0);
  expect(debug.err).toBe("");
  const debugEnvelope = JSON.parse(debug.out);
  expect(Array.isArray(debugEnvelope.debug)).toBe(true);
  expect(debugEnvelope.debug.some((line: string) => line.startsWith("tool="))).toBe(true);
  expect(debugEnvelope.debug.join("\n")).not.toContain("Plain");
});

it("does not stream fixture child stderr live; debug envelope includes it", async () => {
  const root = await tempDir("stderr-buffer");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const chatter = "No project session found for this directory";
  const quiet = await invoke(root, ["plain"], { QC_AGENT_TEXT: "joke", QC_PI_STDERR: chatter });
  expect(quiet.code).toBe(0);
  expect(quiet.err).not.toContain(chatter);
  expect(quiet.out).not.toContain(chatter);
  expect(quiet.out).not.toContain("[WARNING]");
  expect(quiet.out).not.toContain("[DEBUG]");

  const debug = await invoke(root, ["plain", "--debug"], { QC_AGENT_TEXT: "joke", QC_PI_STDERR: chatter });
  expect(debug.code).toBe(0);
  expect(debug.err).not.toContain(chatter);
  expect(debug.out).toContain("[DEBUG]");
  expect(debug.out).toContain(chatter);
});

it("prints a quiet success envelope matching today's one-shot blob", async () => {
  const root = await tempDir("quiet-envelope");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const result = await invoke(root, ["-q", "plain"], { QC_AGENT_TEXT: "joke-text" });
  expect(result.code).toBe(0);
  expect(result.err).toBe("");
  expect(result.out).toMatch(/^"""\njoke-text\n"""\n\[SUCCESS\] pi ⋅ \d+s\n\[QC-SESSION\] \d{6}-\d{4}--pi--[a-z0-9]{6}\n$/);
});

it("prints one success stdout envelope and saves the session mapping", async () => {
  const root = await tempDir("text-envelope");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const result = await invoke(root, ["plain"], { QC_AGENT_TEXT: "joke-text" });
  expect(result.code).toBe(0);
  expect(result.err).toBe("");
  expect(result.err).not.toContain("qc -o");
  expect(result.out).toMatch(/^"""\njoke-text\n"""\n\[SUCCESS\] pi ⋅ \d+s\n\[QC-SESSION\] \d{6}-\d{4}--pi--[a-z0-9]{6}\n$/);
  const sessionMatch = result.out.match(/\[QC-SESSION\] (\S+)/);
  expect(sessionMatch).toBeTruthy();
  const mapping = JSON.parse(await readFile(join(home, ".qc", "sessions", `${sessionMatch![1]}.json`), "utf8"));
  expect(mapping).toMatchObject({ tool: "pi" });
  expect(mapping.native_id).toBeTruthy();
  expect(mapping.usage).toBeUndefined();
});

it("prints [USAGE] and JSON usage from env-gated fixture payloads", async () => {
  const root = await tempDir("usage-envelope");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const text = await invoke(root, ["plain"], { QC_AGENT_TEXT: "joke-text", QC_PI_USAGE: "1" });
  expect(text.code).toBe(0);
  expect(text.out).toMatch(
    /^"""\njoke-text\n"""\n\[SUCCESS\] pi ⋅ \d+s\n\[USAGE\] 1\.2k in ⋅ 400 out ⋅ 50 cache ⋅ 10 think ⋅ 1\.7k total ⋅ 0\.012\n\[QC-SESSION\] \d{6}-\d{4}--pi--[a-z0-9]{6}\n$/,
  );
  const sessionId = text.out.match(/\[QC-SESSION\] (\S+)/)?.[1];
  const mapping = JSON.parse(await readFile(join(home, ".qc", "sessions", `${sessionId}.json`), "utf8"));
  expect(mapping.usage).toBeUndefined();

  const json = await invoke(root, ["plain", "--output", "json"], { QC_AGENT_TEXT: "text", QC_PI_USAGE: "1" });
  expect(json.code).toBe(0);
  const envelope = JSON.parse(json.out);
  expect(envelope.usage).toEqual({
    input_tokens: 1200,
    output_tokens: 400,
    cache_read_tokens: 50,
    thinking_tokens: 10,
    total_tokens: 1660,
    cost: 0.012,
  });

  const agy = await invoke(root, ["plain", "--tool", "antigravity"], {
    QC_AGENT_TEXT: "agy-ok",
    QC_AGY_USAGE: "1",
  });
  expect(agy.code).toBe(0);
  expect(agy.out).toContain("[USAGE] 1.2k in ⋅ 400 out ⋅ 50 cache ⋅ 20 think ⋅ 1.7k total");
  expect(agy.out).not.toContain("$");
});

it("hides inapplicable-field warnings on quiet unless --debug", async () => {
  const root = await tempDir("debug-warning");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const quiet = await invoke(root, ["-q", "plain", "--tool", "opencode", "--no-skills"], {
    QC_AGENT_TEXT: "ok",
    QC_OPENCODE_SESSION: "ses_warn",
  });
  expect(quiet.code).toBe(0);
  expect(quiet.out).not.toContain("[WARNING]");
  expect(quiet.err).not.toContain("[WARNING]");
  expect(quiet.err).not.toContain("qc: warning:");

  const debug = await invoke(root, ["-q", "plain", "--tool", "opencode", "--no-skills", "--debug"], {
    QC_AGENT_TEXT: "ok",
    QC_OPENCODE_SESSION: "ses_warn",
  });
  expect(debug.out).toContain("[WARNING] qc_no_skills is ignored for tool 'opencode'");
  expect(debug.err).toBe("");
});

it("prints live inapplicable-field warnings on stderr without --debug", async () => {
  const root = await tempDir("live-warning");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const result = await invoke(root, ["plain", "--tool", "cursor", "--thinking", "high"], {
    QC_AGENT_TEXT: "ok",
  });
  expect(result.code).toBe(0);
  expect(result.err).toContain("[WARNING] qc_thinking is ignored for tool 'cursor' (encode effort in the model slug)");
  expect(result.out).not.toContain("[WARNING]");
  expect(result.out).toMatch(/^"""\nok\n"""\n\[SUCCESS\]/);
});

it("keeps live warnings on stderr and the debug blob on stdout", async () => {
  const root = await tempDir("live-debug");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const result = await invoke(root, ["plain", "--tool", "cursor", "--thinking", "high", "--debug"], {
    QC_AGENT_TEXT: "ok",
  });
  expect(result.code).toBe(0);
  expect(result.err).toContain("[WARNING] qc_thinking is ignored for tool 'cursor' (encode effort in the model slug)");
  expect(result.out).toContain("[WARNING] qc_thinking is ignored for tool 'cursor' (encode effort in the model slug)");
  expect(result.out).toContain("[DEBUG]");
  expect(result.out).toMatch(/^"""\nok\n"""\n\[SUCCESS\]/);
});

it("prints one JSON object with or without -q and leaves stderr empty on success", async () => {
  const root = await tempDir("json-quiet");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const json = await invoke(root, ["plain", "--output", "json"], { QC_AGENT_TEXT: "text" });
  expect(json.code).toBe(0);
  expect(json.err).toBe("");
  const envelope = JSON.parse(json.out);
  expect(envelope).toMatchObject({ tool: "pi", warnings: [], exit: 0 });

  const quietJson = await invoke(root, ["-q", "plain", "--output", "json", "--tool", "cursor", "--thinking", "high"], {
    QC_AGENT_TEXT: "text",
  });
  expect(quietJson.code).toBe(0);
  expect(quietJson.err).toBe("");
  const quietEnvelope = JSON.parse(quietJson.out);
  expect(quietEnvelope.warnings).toEqual([
    "qc_thinking is ignored for tool 'cursor' (encode effort in the model slug)",
  ]);
});

it("prints the fail envelope with or without -q", async () => {
  const root = await tempDir("fail-quiet");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const liveFail = await invoke(root, ["plain"], { QC_AGENT_TEXT: "" });
  expect(liveFail.code).toBe(1);
  expect(liveFail.out).toBe("");
  expect(liveFail.err).toMatch(
    /^\[ERROR\] pi produced empty assistant text\n\[QC-SESSION\] (\d{6}-\d{4}--pi--[a-z0-9]{6})\nqc -o \1\n$/,
  );

  const quietFail = await invoke(root, ["-q", "plain"], { QC_AGENT_TEXT: "" });
  expect(quietFail.code).toBe(1);
  expect(quietFail.out).toBe("");
  expect(quietFail.err).toMatch(/^\[ERROR\] pi produced empty assistant text\n\[QC-SESSION\] \d{6}-\d{4}--pi--[a-z0-9]{6}\n$/);
  expect(quietFail.err).not.toContain("qc -o");
});

it("rejects -o combined with -q at parse", async () => {
  const root = await tempDir("open-quiet");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  const result = await invoke(root, ["-o", "-q"]);
  expect(result.code).toBe(1);
  expect(result.out).toBe("");
  expect(result.err).toBe("[ERROR] --open cannot be combined with --quiet\n");
});

it("prints hard-fail [ERROR] on stderr with empty stdout for agy invalid model", async () => {
  const root = await tempDir("agy-hard-fail");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const detail =
    'invalid model selection (--model "gemini-3.8-flash"): model gemini-3.8-flash is not recognized as a known model or custom model in settings';
  const result = await invoke(root, ["plain", "--tool", "antigravity", "--model", "gemini-3.8-flash"], {
    QC_AGY_ERROR: detail,
  });
  expect(result.code).toBe(1);
  expect(result.out).toBe("");
  expect(result.err).toBe(`[ERROR] ${detail}\nagy\n`);
  expect(result.err).not.toContain("qc -o");
  const sessions = join(home, ".qc", "sessions");
  await expect(readdir(sessions)).rejects.toMatchObject({ code: "ENOENT" });
});

it("prints cursor empty-JSON stderr and a copy-paste native command when no session was saved", async () => {
  const root = await tempDir("cursor-empty-json");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const provider = "Not logged in. Run `agent` to authenticate.\n";
  const live = await invoke(root, ["plain", "--tool", "cursor"], {
    QC_AGENT_EMPTY_JSON: "1",
    QC_AGENT_STDERR: provider,
  });
  expect(live.code).toBe(1);
  expect(live.out).toBe("");
  expect(live.err).toBe(
    "[ERROR] cursor agent produced empty JSON output\nNot logged in. Run `agent` to authenticate.\nagent\n",
  );
  expect(live.err).not.toContain("qc -o");
  await expect(readdir(join(home, ".qc", "sessions"))).rejects.toMatchObject({ code: "ENOENT" });

  const quiet = await invoke(root, ["-q", "plain", "--tool", "cursor"], {
    QC_AGENT_EMPTY_JSON: "1",
    QC_AGENT_STDERR: provider,
  });
  expect(quiet.code).toBe(1);
  expect(quiet.out).toBe("");
  expect(quiet.err).toBe(
    "[ERROR] cursor agent produced empty JSON output\nNot logged in. Run `agent` to authenticate.\n",
  );
  expect(quiet.err).not.toContain("\nagent\n");
  expect(quiet.err).not.toContain("qc -o");
});

it("prints hard-fail [ERROR] for empty assistant text and still saves a Pi session", async () => {
  const root = await tempDir("empty-assistant");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const result = await invoke(root, ["plain"], { QC_AGENT_TEXT: "" });
  expect(result.code).toBe(1);
  expect(result.out).toBe("");
  expect(result.err).toMatch(
    /^\[ERROR\] pi produced empty assistant text\n\[QC-SESSION\] (\d{6}-\d{4}--pi--[a-z0-9]{6})\nqc -o \1\n$/,
  );
  const sessionId = result.err.match(/\[QC-SESSION\] (\S+)/)?.[1];
  expect(sessionId).toBeTruthy();
  const mapping = JSON.parse(await readFile(join(home, ".qc", "sessions", `${sessionId}.json`), "utf8"));
  expect(mapping).toMatchObject({ tool: "pi", native_id: sessionId, cwd });
});

it("continues and opens a mapping saved from a failed Pi turn", async () => {
  const root = await tempDir("fail-resume");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  const record = join(root, "record.json");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const failed = await invoke(root, ["plain"], { QC_AGENT_TEXT: "" });
  const sessionId = failed.err.match(/\[QC-SESSION\] (\S+)/)?.[1];
  expect(sessionId).toBeTruthy();
  const mapping = JSON.parse(await readFile(join(home, ".qc", "sessions", `${sessionId}.json`), "utf8"));

  const continued = await invoke(root, ["-c", sessionId!, "--append", "retry now"], { QC_AGENT_TEXT: "ok" });
  expect(continued.code).toBe(0);
  expect(continued.out).toContain("ok");
  expect(continued.out).toContain(`[QC-SESSION] ${sessionId}`);

  const opened = await invoke(root, ["-o", sessionId!], { QC_RECORD: record });
  expect(opened.code).toBe(0);
  expect(JSON.parse(await readFile(record, "utf8")).argv).toEqual(["--session-id", mapping.native_id]);
});

it("prints hard-fail [ERROR] for claude unrecognized-model JSON and still saves a session", async () => {
  const root = await tempDir("claude-hard-fail");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const detail =
    "There's an issue with the selected model (some-wrong-fake-model). It may not exist or you may not have access to it.";
  const result = await invoke(root, ["plain", "--tool", "claude", "--model", "some-wrong-fake-model"], {
    QC_CLAUDE_ERROR: detail,
  });
  expect(result.code).toBe(1);
  expect(result.out).toBe("");
  expect(result.err).toMatch(
    new RegExp(
      `^\\[ERROR\\] ${detail.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\n\\[claude-code:unrecognized_model\\]\\n\\[QC-SESSION\\] (\\d{6}-\\d{4}--claude--[a-z0-9]{6})\\nqc -o \\1\\n$`,
    ),
  );
  const sessionId = result.err.match(/\[QC-SESSION\] (\S+)/)?.[1];
  expect(sessionId).toBeTruthy();
  const mapping = JSON.parse(await readFile(join(home, ".qc", "sessions", `${sessionId}.json`), "utf8"));
  expect(mapping.tool).toBe("claude");
  expect(mapping.native_id).toMatch(/^[0-9a-f-]{36}$/i);
});

it("prints hard-fail [ERROR] for opencode fake-model JSONL error and still saves a session", async () => {
  const root = await tempDir("opencode-hard-fail");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const detail = "Model not found: some-fake-wrong-model";
  const result = await invoke(root, ["plain", "--tool", "opencode", "--model", "some-fake-wrong-model"], {
    QC_OPENCODE_ERROR: detail,
  });
  expect(result.code).toBe(1);
  expect(result.out).toBe("");
  expect(result.err).toMatch(
    /^\[ERROR\] Model not found: some-fake-wrong-model\n\[QC-SESSION\] (\d{6}-\d{4}--opencode--[a-z0-9]{6})\nqc -o \1\n$/,
  );
  const sessionId = result.err.match(/\[QC-SESSION\] (\S+)/)?.[1];
  expect(sessionId).toBeTruthy();
  const mapping = JSON.parse(await readFile(join(home, ".qc", "sessions", `${sessionId}.json`), "utf8"));
  expect(mapping).toMatchObject({ tool: "opencode", native_id: "ses_fixture_opencode" });
});

it("installs packaged skills through the CLI without bootstrapping settings", async () => {
  const root = await tempDir("harness-cli");
  clean.push(root);
  const home = join(root, "home");
  const result = await invoke(root, ["--install-agent-harness"]);
  expect(result.code).toBe(0);
  expect(await readFile(join(home, ".agents", "skills", "qc", "SKILL.md"), "utf8")).toContain("name: qc");
  expect(await readFile(join(home, ".claude", "skills", "qc-create-prompt", "SKILL.md"), "utf8")).toContain(
    "disable-model-invocation: true",
  );
  expect(await readFile(join(home, ".claude", "skills", "qc-create-prompt", "openai.yaml"), "utf8")).toContain(
    "allow_implicit_invocation: false",
  );
  await expect(readFile(join(home, ".cursor", "commands", "qc", "qc-create-prompt.md"), "utf8")).rejects.toMatchObject({
    code: "ENOENT",
  });
  await expect(readFile(join(home, ".qc", "config.toml"), "utf8")).rejects.toMatchObject({ code: "ENOENT" });
});

async function writeMapping(
  home: string,
  id: string,
  mapping: {
    tool: string;
    native_id: string;
    cwd: string;
    created: string;
    updated: string;
    warnings: string[];
  },
): Promise<void> {
  await mkdir(join(home, ".qc", "sessions"), { recursive: true });
  await writeFile(join(home, ".qc", "sessions", `${id}.json`), `${JSON.stringify(mapping, null, 2)}\n`);
}

async function invokePath(
  root: string,
  args: string[],
  bin: string,
  extra: NodeJS.ProcessEnv = {},
) {
  const { spawn } = await import("node:child_process");
  const cwd = join(root, "work");
  const home = join(root, "home");
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  return new Promise<{ code: number | null; out: string; err: string }>((resolveResult, reject) => {
    const child = spawn(process.execPath, ["--import", loader, join(packageRoot, "src/cli.ts"), ...args], {
      cwd,
      env: { ...process.env, ...extra, HOME: home, PATH: bin, SHELL: "/bin/sh" },
    });
    let out = "";
    let err = "";
    child.stdout.on("data", (x) => {
      out += x;
    });
    child.stderr.on("data", (x) => {
      err += x;
    });
    child.on("error", reject);
    child.on("close", (code) => resolveResult({ code, out, err }));
  });
}

it("opens a stored session in the native TUI with recorded argv, spawn cwd, updated bump, and child exit", async () => {
  const root = await tempDir("open-id");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  const spawnCwd = join(root, "session-work");
  const record = join(root, "record.json");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await mkdir(spawnCwd, { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  const id = "260901-1200--pi--a1b2c3";
  const created = "2026-09-01T00:00:00.000Z";
  const updated = "2026-09-01T00:01:00.000Z";
  await writeMapping(home, id, {
    tool: "pi",
    native_id: "native-pi-1",
    cwd: spawnCwd,
    created,
    updated,
    warnings: ["kept"],
  });
  const result = await invoke(root, ["-o", id], { QC_RECORD: record, QC_PI_EXIT: "7" });
  expect(result.code).toBe(7);
  expect(result.err).not.toMatch(/qc: session:/);
  expect(result.err).not.toMatch(/qc: /);
  const recorded = JSON.parse(await readFile(record, "utf8"));
  expect(recorded.argv).toEqual(["--session-id", "native-pi-1"]);
  expect(recorded.cwd).toBe(spawnCwd);
  const mapping = JSON.parse(await readFile(join(home, ".qc", "sessions", `${id}.json`), "utf8"));
  expect(mapping).toMatchObject({
    tool: "pi",
    native_id: "native-pi-1",
    cwd: spawnCwd,
    created,
    warnings: ["kept"],
  });
  expect(mapping.updated > updated).toBe(true);
});

it("opens the newest cwd-matching session for bare -o and filters by --tool", async () => {
  const root = await tempDir("open-bare");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  const record = join(root, "record.json");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeMapping(home, "260901-1000--pi--aaaaaa", {
    tool: "pi",
    native_id: "old-pi",
    cwd,
    created: "2026-09-01T00:00:00.000Z",
    updated: "2026-09-01T00:00:00.000Z",
    warnings: [],
  });
  await writeMapping(home, "260901-1000--pi--bbbbbb", {
    tool: "pi",
    native_id: "new-pi",
    cwd,
    created: "2026-09-01T00:00:00.000Z",
    updated: "2026-09-01T02:00:00.000Z",
    warnings: [],
  });
  await writeMapping(home, "260901-1000--cursor--cccccc", {
    tool: "cursor",
    native_id: "cursor-native",
    cwd,
    created: "2026-09-01T03:00:00.000Z",
    updated: "2026-09-01T03:00:00.000Z",
    warnings: [],
  });
  const latest = await invoke(root, ["-o"], { QC_RECORD: record });
  expect(latest.code).toBe(0);
  expect(latest.err).not.toMatch(/qc: session:/);
  expect(JSON.parse(await readFile(record, "utf8")).argv).toEqual(["--resume", "cursor-native"]);

  const piOnly = await invoke(root, ["-o", "--tool", "pi"], { QC_RECORD: record });
  expect(piOnly.code).toBe(0);
  expect(JSON.parse(await readFile(record, "utf8")).argv).toEqual(["--session-id", "new-pi"]);
});

it("reports session not found, tool mismatch, missing binary, and empty tool filter for -o", async () => {
  const root = await tempDir("open-errors");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  const missing = await invoke(root, ["-o", "260901-1200--pi--zzzzzz"]);
  expect(missing.code).toBe(1);
  expect(missing.err).toMatch(/\[ERROR\] session not found: 260901-1200--pi--zzzzzz/);

  const id = "260901-1200--pi--a1b2c3";
  await writeMapping(home, id, {
    tool: "pi",
    native_id: "native-pi-1",
    cwd,
    created: "2026-09-01T00:00:00.000Z",
    updated: "2026-09-01T00:00:00.000Z",
    warnings: [],
  });
  const mismatch = await invoke(root, ["-o", id, "--tool", "cursor"]);
  expect(mismatch.code).toBe(1);
  expect(mismatch.err).toContain("tool mismatch: session is 'pi' but resolved tool is 'cursor'");

  const noClaude = await invoke(root, ["-o", "--tool", "claude"]);
  expect(noClaude.code).toBe(1);
  expect(noClaude.err).toContain("[ERROR] no claude session found for this directory");

  const emptyBin = join(root, "empty-bin");
  await mkdir(emptyBin, { recursive: true });
  await symlink(process.execPath, join(emptyBin, "node")).catch(() => undefined);
  const noBinary = await invokePath(root, ["-o", id], emptyBin);
  expect(noBinary.code).toBe(1);
  expect(noBinary.err).toMatch(/pi executable 'pi' was not found/i);
});

function pidAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

it("returns after the post-exit drain when a descendant still holds stdout", async () => {
  const root = await tempDir("hold-stdout");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  const sleeperPidPath = join(root, "sleeper.pid");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc", "prompts"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "prompts", "plain.md"), "Plain");
  const started = Date.now();
  const result = await invoke(root, ["plain"], {
    QC_AGENT_TEXT: "drained",
    QC_PI_HOLD_STDOUT: "1",
    QC_PI_SLEEPER_PID: sleeperPidPath,
  });
  const elapsed = Date.now() - started;
  expect(result.code).toBe(0);
  expect(result.out).toContain("drained");
  expect(elapsed).toBeGreaterThanOrEqual(1500);
  expect(elapsed).toBeLessThan(8000);
  const sleeperPid = Number(await readFile(sleeperPidPath, "utf8"));
  expect(Number.isInteger(sleeperPid) && sleeperPid > 0).toBe(true);
  for (let attempt = 0; attempt < 30 && pidAlive(sleeperPid); attempt += 1) {
    await new Promise<void>((resolveWait) => setTimeout(resolveWait, 50));
  }
  expect(pidAlive(sleeperPid)).toBe(false);
}, 15_000);

it("resolves the TUI binary from invocation-cwd [tool.<name>] command", async () => {
  const root = await tempDir("open-command");
  clean.push(root);
  const cwd = join(root, "work");
  const home = join(root, "home");
  const bin = join(root, "bin");
  const record = join(root, "record.json");
  await mkdir(join(home, ".qc"), { recursive: true });
  await mkdir(join(cwd, ".qc"), { recursive: true });
  await writeFile(join(home, ".qc", "config.toml"), "");
  await writeFile(join(cwd, ".qc", "config.toml"), '[tool.pi]\ncommand = "custom-pi"\n');
  const id = "260901-1200--pi--a1b2c3";
  await writeMapping(home, id, {
    tool: "pi",
    native_id: "native-custom",
    cwd,
    created: "2026-09-01T00:00:00.000Z",
    updated: "2026-09-01T00:00:00.000Z",
    warnings: [],
  });
  await installFixtures(bin);
  await copyFile(join(bin, "pi"), join(bin, "custom-pi"));
  await chmod(join(bin, "custom-pi"), 0o755);
  await unlink(join(bin, "pi"));
  const result = await invokePath(root, ["-o", id], bin, { QC_RECORD: record });
  expect(result.code).toBe(0);
  expect(JSON.parse(await readFile(record, "utf8")).argv).toEqual(["--session-id", "native-custom"]);
});
