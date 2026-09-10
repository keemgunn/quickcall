import { describe, expect, it } from "vitest";
import { parseArgs } from "../../src/args.js";

describe("arguments", () => {
  it.each(
    [
      ["daily/report"],
      ["daily/report", "--shell", "/bin/sh"],
      ["daily/report", "--append", "hello"],
      ["daily/report", "--tool", "cursor", "--model", "composer-2.5"],
      ["daily/report", "--thinking", "high", "--workdir", "/tmp/w"],
      ["daily/report", "--output", "json"],
      ["daily/report", "--debug"],
      ["daily/report", "-q"],
      ["daily/report", "--quiet"],
      ["daily/report", "-c", "260901-1200--pi--a1b2c3"],
      ["daily/report", "--skill", "/skills", "--no-skills"],
    ].map((argv) => [argv]),
  )("accepts prompt forms: %j", (argv) => {
    expect(parseArgs(argv)).toMatchObject({ prompt: "daily/report" });
  });

  it("accepts continue+append without a prompt", () => {
    const parsed = parseArgs(["-c", "260901-1200--pi--a1b2c3", "-a", "also add tests"]);
    expect(parsed.continue).toBe("260901-1200--pi--a1b2c3");
    expect(parsed.append).toBe("also add tests");
    expect(parsed.prompt).toBeUndefined();
  });

  it("accepts append-only without a prompt", () => {
    const parsed = parseArgs(["-a", "only"]);
    expect(parsed.append).toBe("only");
    expect(parsed.prompt).toBeUndefined();
  });

  it("accepts append-only with tool and model", () => {
    const parsed = parseArgs(["--tool", "cursor", "--model", "composer-2.5", "--append", "inspect this repo"]);
    expect(parsed).toMatchObject({
      tool: "cursor",
      model: "composer-2.5",
      append: "inspect this repo",
    });
    expect(parsed.prompt).toBeUndefined();
  });

  it("accepts bare -o/--open without a prompt", () => {
    const parsed = parseArgs(["-o"]);
    expect(parsed.open).toBe(true);
    expect(parsed.prompt).toBeUndefined();
    expect(parseArgs(["--open"])).toEqual(parsed);
  });

  it("consumes the next token as an open id when it is not an option", () => {
    const id = "260901-1200--pi--a1b2c3";
    const parsed = parseArgs(["-o", id]);
    expect(parsed.open).toBe(id);
    expect(parsed.prompt).toBeUndefined();
    expect(parseArgs(["--open", id])).toEqual(parsed);
    expect(parseArgs(["-o", id, "--tool", "pi"])).toMatchObject({ open: id, tool: "pi" });
    expect(parseArgs(["-o", "--tool", "cursor"])).toMatchObject({ open: true, tool: "cursor" });
  });

  it("rejects attached --open=id and -o<id> as unknown options", () => {
    expect(() => parseArgs(["--open=260901-1200--pi--a1b2c3"])).toThrow("unknown option '--open=260901-1200--pi--a1b2c3'");
    expect(() => parseArgs(["-o260901-1200--pi--a1b2c3"])).toThrow("unknown option '-o260901-1200--pi--a1b2c3'");
  });

  it("parses -q/--quiet as one identity", () => {
    expect(parseArgs(["daily/report", "-q"])).toMatchObject({ prompt: "daily/report", quiet: true });
    expect(parseArgs(["daily/report", "--quiet"])).toEqual(parseArgs(["daily/report", "-q"]));
    expect(parseArgs(["-q", "--append", "inspect this repo"])).toMatchObject({ quiet: true, append: "inspect this repo" });
    expect(
      parseArgs(["review", "-q", "--tool", "cursor", "--model", "composer-2.5", "--debug", "--output", "json"]),
    ).toMatchObject({
      prompt: "review",
      quiet: true,
      tool: "cursor",
      model: "composer-2.5",
      debug: true,
      output: "json",
    });
  });

  it("parses new option values", () => {
    expect(
      parseArgs([
        "p",
        "--tool",
        "claude",
        "--model",
        "sonnet",
        "--thinking",
        "high",
        "--workdir",
        "/w",
        "--output",
        "json",
        "--debug",
        "--continue",
        "260901-1200--claude--abcdef",
        "--skill",
        "/s",
        "--no-skills",
      ]),
    ).toMatchObject({
      prompt: "p",
      tool: "claude",
      model: "sonnet",
      thinking: "high",
      workdir: "/w",
      output: "json",
      debug: true,
      continue: "260901-1200--claude--abcdef",
      skill: "/s",
      noSkills: true,
    });
  });

  it.each(["constructor", "toString", "__proto__"].map((prompt) => [prompt]))(
    "preserves prototype-key prompt references: %s",
    (prompt) => expect(parseArgs([prompt])).toMatchObject({ prompt }),
  );

  it("accepts unrecognized single-dash values", () => {
    expect(parseArgs(["daily/report", "--shell", "-custom-shell"])).toMatchObject({ shell: "-custom-shell" });
    expect(parseArgs(["daily/report", "--append", "-message"])).toMatchObject({ append: "-message" });
  });

  it.each([["--help"], ["-h"], ["--version"], ["-v"], ["--install-sample-prompts"]].map((argv) => [argv]))(
    "accepts standalone %j",
    (argv) => {
      if (argv[0] === "--help" || argv[0] === "-h") expect(parseArgs(argv)).toMatchObject({ help: true });
      else if (argv[0] === "--version" || argv[0] === "-v") expect(parseArgs(argv)).toMatchObject({ version: true });
      else expect(parseArgs(argv)).toMatchObject({ installSamplePrompts: true });
    },
  );

  it("accepts standalone harness install", () =>
    expect(parseArgs(["--install-agent-harness"])).toEqual({
      help: false,
      version: false,
      installSamplePrompts: false,
      installAgentHarness: true,
    }));

  it.each(
    [
      [
        ["daily/report", "-s", "/bin/sh"],
        ["daily/report", "--shell", "/bin/sh"],
      ],
      [
        ["daily/report", "-a", "hello"],
        ["daily/report", "--append", "hello"],
      ],
      [
        ["daily/report", "-c", "id"],
        ["daily/report", "--continue", "id"],
      ],
      [
        ["-o", "260901-1200--pi--a1b2c3"],
        ["--open", "260901-1200--pi--a1b2c3"],
      ],
      [["-o"], ["--open"]],
      [["-h"], ["--help"]],
      [["-v"], ["--version"]],
      [
        ["daily/report", "-q"],
        ["daily/report", "--quiet"],
      ],
    ].map((aliases) => [aliases]),
  )("normalizes short aliases: %j", ([short, long]) => expect(parseArgs(short)).toEqual(parseArgs(long)));

  it.each<[string[], string]>([
    [["daily/report", "-s"], "--shell requires a value"],
    [["daily/report", "-a"], "--append requires a value"],
    [["daily/report", "-c"], "--continue requires a value"],
    [["daily/report", "--output", "xml"], "--output must be 'text' or 'json'"],
    [["daily/report", "-s", "-a", "hello"], "--shell requires a value"],
    [["daily/report", "--tool", "--wat"], "--tool requires a value"],
    [["daily/report", "-s", "/bin/sh", "--shell", "/bin/zsh"], "--shell was provided more than once"],
    [["-h", "--help"], "--help was provided more than once"],
    [["--install-agent-harness", "cursor"], "--install-agent-harness must be used alone"],
    [["review", "-o"], "--open cannot be combined with a prompt reference"],
    [["-o", "-c", "260901-1200--pi--a1b2c3"], "--open cannot be combined with --continue"],
    [["-o", "-a", "x"], "--open cannot be combined with --append"],
    [["-o", "--output", "json"], "--open cannot be combined with --output"],
    [["-o", "--debug"], "--open cannot be combined with --debug"],
    [["-o", "-q"], "--open cannot be combined with --quiet"],
    [["--quiet", "-o", "260901-1200--pi--a1b2c3"], "--open cannot be combined with --quiet"],
    [["review", "-q", "--quiet"], "--quiet was provided more than once"],
    [["review", "-q", "-q"], "--quiet was provided more than once"],
    [["-q", "-h"], "--help and --version must be used alone"],
    [["--quiet", "--version"], "--help and --version must be used alone"],
    [["-q", "--install-agent-harness"], "--install-agent-harness must be used alone"],
    [["review", "-qv"], "unknown option '-qv'"],
    [["-qc", "review"], "unknown option '-qc'"],
    [["-o", "--model", "m"], "--open cannot be combined with --model"],
    [["-o", "--thinking", "high"], "--open cannot be combined with --thinking"],
    [["-o", "--workdir", "/w"], "--open cannot be combined with --workdir"],
    [["-o", "--skill", "/s"], "--open cannot be combined with --skill"],
    [["-o", "--no-skills"], "--open cannot be combined with --no-skills"],
    [["-o", "-s", "/bin/sh"], "--open cannot be combined with --shell"],
    [["-o", "--help"], "--help and --version must be used alone"],
  ])("uses canonical diagnostics: %j", (argv, message) => expect(() => parseArgs(argv)).toThrow(message));

  it.each(
    [
      [],
      ["a", "b"],
      ["a", "--wat"],
      ["--shell"],
      ["--append"],
      ["-c", "id"],
      ["--tool", "cursor"],
      ["review", "-o"],
      ["-o", "-c", "id"],
      ["-o", "-a", "x"],
      ["-o", "--debug"],
      ["-o", "-q"],
      ["review", "-q", "--quiet"],
      ["review", "-qv"],
      ["-qc", "review"],
      ["-q", "-h"],
      ["--open=id"],
      ["-o260901"],
      ["--help", "--version"],
      ["--help", "a"],
      ["--version", "a"],
      ["-hv"],
      ["a", "-s/bin/sh"],
      ["--install-sample-prompts", "a"],
      ["--install-agent-harness", "cursor"],
      ["daily/report", "--install-sample-prompts"],
    ].map((argv) => [argv]),
  )("rejects invalid grammar: %j", (argv) => expect(() => parseArgs(argv)).toThrow());

  it.each<[string[], string]>([
    [[], "a prompt reference or --append is required"],
    [["--tool", "cursor"], "a prompt reference or --append is required"],
    [["-c", "id"], "a prompt reference or --append is required"],
  ])("requires prompt or append: %j", (argv, message) => expect(() => parseArgs(argv)).toThrow(message));
});
