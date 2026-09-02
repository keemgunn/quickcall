import { describe, expect, it } from "vitest";
import { parseArgs } from "../../src/args.js";

describe("arguments", () => {
  it.each([["daily/report"], ["daily/report", "--shell", "/bin/sh"], ["daily/report", "--append", "hello"], ["daily/report", "--shell", "/bin/sh", "--append", "hello"], ["daily/report", "-s", "/bin/sh"], ["daily/report", "-a", "hello"], ["daily/report", "-s", "/bin/sh", "-a", "hello"]].map((argv) => [argv]))("accepts every prompt form: %j", (argv) => {
    expect(parseArgs(argv)).toMatchObject({ prompt: "daily/report" });
  });
  it.each(["constructor", "toString", "__proto__"].map((prompt) => [prompt]))("preserves prototype-key prompt references: %s", (prompt) => expect(parseArgs([prompt])).toMatchObject({ prompt }));
  it("accepts unrecognized single-dash values", () => {
    expect(parseArgs(["daily/report", "--shell", "-custom-shell"])).toMatchObject({ shell: "-custom-shell" }); expect(parseArgs(["daily/report", "--append", "-message"])).toMatchObject({ append: "-message" });
  });
  it.each([["--help"], ["-h"], ["--version"], ["-v"], ["--install-sample-prompts"]].map((argv) => [argv]))("accepts standalone %j", (argv) => {
    if (argv[0] === "--help" || argv[0] === "-h") expect(parseArgs(argv)).toMatchObject({ help: true });
    else if (argv[0] === "--version" || argv[0] === "-v") expect(parseArgs(argv)).toMatchObject({ version: true });
    else expect(parseArgs(argv)).toMatchObject({ installSamplePrompts: true });
  });
  it("accepts skills-only harness install", () => expect(parseArgs(["--install-agent-harness"])).toEqual({ help: false, version: false, installSamplePrompts: false, installAgentHarness: { frameworks: [] } }));
  it("accepts named harness frameworks", () => expect(parseArgs(["--install-agent-harness", "cursor", "opencode"])).toMatchObject({ installAgentHarness: { frameworks: ["cursor", "opencode"] } }));
  it.each([
    [["daily/report", "-s", "/bin/sh"], ["daily/report", "--shell", "/bin/sh"]], [["daily/report", "-a", "hello"], ["daily/report", "--append", "hello"]], [["-h"], ["--help"]], [["-v"], ["--version"]]
  ].map((aliases) => [aliases]))("normalizes short aliases: %j", ([short, long]) => expect(parseArgs(short)).toEqual(parseArgs(long)));
  it.each<[string[], string]>([
    [["daily/report", "-s"], "--shell requires a value"], [["daily/report", "-a"], "--append requires a value"], [["daily/report", "-s", "-a", "hello"], "--shell requires a value"], [["daily/report", "-a", "--help"], "--append requires a value"], [["daily/report", "--shell", "-v"], "--shell requires a value"],
    [["daily/report", "--shell", "--wat"], "--shell requires a value"], [["daily/report", "-s", "--wat"], "--shell requires a value"], [["daily/report", "--append", "--wat"], "--append requires a value"], [["daily/report", "-a", "--wat"], "--append requires a value"],
    [["daily/report", "-s", "/bin/sh", "--shell", "/bin/zsh"], "--shell was provided more than once"], [["daily/report", "-a", "one", "--append", "two"], "--append was provided more than once"], [["-h", "--help"], "--help was provided more than once"], [["-v", "--version"], "--version was provided more than once"],
    [["--install-agent-harness", "wat"], "unknown agent-harness framework 'wat'"], [["--install-agent-harness", "cursor", "cursor"], "duplicate agent-harness framework 'cursor'"]
  ])("uses canonical diagnostics for short forms: %j", (argv, message) => expect(() => parseArgs(argv)).toThrow(message));
  it.each([
    [], ["a", "b"], ["a", "--wat"], ["--shell"], ["--append"], ["--shell", "--append", "x"], ["a", "--shell", ""],
    ["a", "--shell", "/bin/sh", "--shell", "/bin/sh"], ["a", "--append", "x", "--append", "y"],
    ["--help", "--version"], ["--help", "a"], ["--version", "a"], ["--help", "--shell", "/bin/sh"], ["--version", "--append", "x"],
    ["-h", "-v"], ["-h", "a"], ["-v", "a"], ["-h", "-s", "/bin/sh"], ["-v", "-a", "x"], ["-hv"], ["a", "-s/bin/sh"], ["a", "-a=text"],
    ["--install-sample-prompts", "a"], ["--install-agent-harness", "--help"], ["daily/report", "--install-sample-prompts"]
  ].map((argv) => [argv]))("rejects invalid grammar: %j", (argv) => expect(() => parseArgs(argv)).toThrow());
});
