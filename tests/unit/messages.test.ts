import { readFile } from "node:fs/promises";
import { expect, it } from "vitest";
import {
  compactTokens,
  debugLine,
  elapsedLabel,
  emptyAssistantText,
  error,
  failOutput,
  help,
  inspectCommand,
  interrupted,
  missingBinary,
  openSessionCommand,
  quotedText,
  sessionLine,
  successLine,
  waitLine,
  textEnvelope,
  usageFields,
  usageLine,
  version,
  warn,
} from "../../src/messages.js";

it("centralizes manifest-derived user-facing output", async () => {
  const manifest = JSON.parse(await readFile(new URL("../../package.json", import.meta.url), "utf8")) as {
    version: string;
  };
  expect(help).toContain("Usage: qc");
  expect(help).toContain("--tool");
  expect(help).toContain("--continue");
  expect(help).toContain("-o, --open");
  expect(help).toContain("-s, --shell");
  expect(help).toContain("-a, --append");
  expect(help).toContain("--debug");
  expect(help).toContain("-q, --quiet");
  expect(help).toContain("or the whole turn when no prompt is given");
  expect(help).toContain("Prompt reference is optional when -a/--append is set");
  expect(help).toContain("--install-sample-prompts");
  expect(help).toContain("--install-agent-harness");
  expect(error("bad")).toBe("[ERROR] bad");
  expect(warn("w")).toBe("[WARNING] w");
  expect(debugLine("spawn cwd=/tmp")).toBe("[DEBUG] spawn cwd=/tmp");
  expect(sessionLine("id")).toBe("[QC-SESSION] id");
  expect(missingBinary("pi", "pi")).toContain("pi");
  expect(emptyAssistantText("opencode")).toBe("opencode produced empty assistant text");
  expect(interrupted(2)).toBe("interrupted (signal 2)");
  expect(version).toBe(manifest.version);
});

it("pads elapsed seconds to at least three digits", () => {
  expect(elapsedLabel(0)).toBe("000s");
  expect(elapsedLabel(12)).toBe("012s");
  expect(elapsedLabel(999)).toBe("999s");
  expect(elapsedLabel(1000)).toBe("1000s");
});

it("quotes assistant text and formats success with optional model", () => {
  expect(quotedText("Why do programmers prefer dark mode?")).toBe(
    '"""\nWhy do programmers prefer dark mode?\n"""',
  );
  expect(successLine("antigravity", 50, "gemini-3.8-flash-high")).toBe(
    "[SUCCESS] antigravity ⋅ gemini-3.8-flash-high ⋅ 50s",
  );
  expect(successLine("pi", 12)).toBe("[SUCCESS] pi ⋅ 12s");
});

it("formats the live wait line with tool, optional model, and padded elapsed", () => {
  expect(waitLine("cursor", 4, "composer-2.5")).toBe(" ⋅ cursor ⋅ composer-2.5 ⋅ 004s");
  expect(waitLine("pi", 12)).toBe(" ⋅ pi ⋅ 012s");
  expect(waitLine("pi", 0)).toBe(" ⋅ pi ⋅ 000s");
});

it("formats a copy-paste open command for a saved pretty id", () => {
  expect(openSessionCommand("260905-1545--pi--a1b2c3")).toBe("qc -o 260905-1545--pi--a1b2c3");
});

it("appends child stderr to the fail body when it adds information", () => {
  expect(failOutput("cursor agent produced empty JSON output")).toBe(
    "[ERROR] cursor agent produced empty JSON output\n",
  );
  expect(failOutput("cursor agent produced empty JSON output", "Not logged in.\n")).toBe(
    "[ERROR] cursor agent produced empty JSON output\nNot logged in.\n",
  );
  expect(failOutput("agy refused the model", "agy refused the model\n")).toBe("[ERROR] agy refused the model\n");
});

it("formats a copy-paste native inspect command", () => {
  expect(inspectCommand("agent")).toBe("agent");
  expect(inspectCommand("/opt/bin/agent")).toBe("/opt/bin/agent");
  expect(inspectCommand("/opt/my agent")).toBe("'/opt/my agent'");
});

it("builds a text envelope with warnings and debug only when requested", () => {
  const base = textEnvelope({
    text: "joke",
    tool: "pi",
    durationS: 12,
    sessionId: "260906-1245--pi--abcdef",
  });
  expect(base).toBe(
    [
      '"""',
      "joke",
      '"""',
      "[SUCCESS] pi ⋅ 12s",
      "[QC-SESSION] 260906-1245--pi--abcdef",
      "",
    ].join("\n"),
  );
  expect(base).not.toContain("[WARNING]");
  expect(base).not.toContain("[DEBUG]");

  const debug = textEnvelope({
    text: "joke",
    tool: "pi",
    durationS: 12,
    sessionId: "260906-1245--pi--abcdef",
    model: "provider/model",
    warnings: ["qc_no_skills is ignored for tool 'opencode'"],
    debugLines: ["tool=pi", "child_stderr:\nNo project session found"],
  });
  expect(debug).toContain("[SUCCESS] pi ⋅ provider/model ⋅ 12s");
  expect(debug).toContain("[WARNING] qc_no_skills is ignored for tool 'opencode'");
  expect(debug).toContain("[DEBUG] tool=pi");
  expect(debug).toContain("[DEBUG] child_stderr:\nNo project session found");
});

it("formats [USAGE] between success and session and omits it when empty", () => {
  expect(compactTokens(400)).toBe("400");
  expect(compactTokens(1200)).toBe("1.2k");
  expect(compactTokens(1000)).toBe("1k");
  expect(usageLine()).toBeUndefined();
  expect(usageLine({})).toBeUndefined();
  expect(usageLine({ input_tokens: 1200, output_tokens: 400, cost: 0.012 })).toBe("[USAGE] 1.2k in ⋅ 400 out ⋅ 0.012");
  expect(usageLine({ input_tokens: 1200, output_tokens: 400, cost: 0.012, cost_currency: "USD" })).toBe(
    "[USAGE] 1.2k in ⋅ 400 out ⋅ $0.012",
  );
  expect(usageLine({ cost: 0.012, cost_currency: "EUR" })).toBe("[USAGE] 0.012 EUR");
  expect(usageLine({ cost: 0, cost_currency: "USD" })).toBeUndefined();
  expect(
    usageLine({
      input_tokens: 1200,
      output_tokens: 400,
      cache_read_tokens: 50,
      thinking_tokens: 10,
      total_tokens: 1660,
    }),
  ).toBe("[USAGE] 1.2k in ⋅ 400 out ⋅ 50 cache ⋅ 10 think ⋅ 1.7k total");
  expect(usageFields({ input_tokens: 10, output_tokens: 4, cost: 0.012 })).toEqual({
    input_tokens: 10,
    output_tokens: 4,
    cost: 0.012,
  });
  expect(usageFields({})).toBeUndefined();

  const withUsage = textEnvelope({
    text: "joke",
    tool: "pi",
    durationS: 12,
    sessionId: "260906-1245--pi--abcdef",
    model: "provider/model",
    usage: { input_tokens: 1200, output_tokens: 400, cost: 0.012 },
  });
  expect(withUsage).toBe(
    [
      '"""',
      "joke",
      '"""',
      "[SUCCESS] pi ⋅ provider/model ⋅ 12s",
      "[USAGE] 1.2k in ⋅ 400 out ⋅ 0.012",
      "[QC-SESSION] 260906-1245--pi--abcdef",
      "",
    ].join("\n"),
  );
});
