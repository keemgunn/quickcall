import { expect, it } from "vitest";
import { buildPiArgs, parsePiOutput } from "../../src/tools/pi.js";
import { buildCursorArgs, parseCursorOutput } from "../../src/tools/cursor.js";
import { buildClaudeArgs, parseClaudeOutput } from "../../src/tools/claude.js";
import { buildOpenCodeArgs, parseOpenCodeOutput } from "../../src/tools/opencode.js";
import { buildAntigravityArgs, parseAntigravityOutput } from "../../src/tools/antigravity.js";
import type { AgentRequest } from "../../src/tools/types.js";

const base = (over: Partial<AgentRequest> = {}): AgentRequest => ({
  prompt: "hello",
  workdir: "/work",
  sessionId: "260901-1200--pi--abcdef",
  command: "pi",
  env: {},
  ...over,
});

it("builds pi argv with force-allow and session id", () => {
  expect(buildPiArgs(base({ model: "m", thinking: "high", noSkills: true, skillPath: "/s" }))).toEqual([
    "--mode",
    "json",
    "-a",
    "--session-id",
    "260901-1200--pi--abcdef",
    "--model",
    "m",
    "--thinking",
    "high",
    "--no-skills",
    "--skill",
    "/s",
  ]);
});

it("parses pi JSONL session id and assistant text", () => {
  const stdout = [
    JSON.stringify({ type: "session", id: "native-pi", version: 3 }),
    JSON.stringify({ type: "message", role: "assistant", content: [{ type: "text", text: "answer" }] }),
  ].join("\n");
  expect(parsePiOutput(stdout, "fallback")).toMatchObject({ text: "answer", nativeId: "native-pi" });
});

it("extracts pi assistant text from live message_end content", () => {
  const joke = "Why do programmers prefer dark mode? Because light attracts bugs.";
  const stdout = [
    JSON.stringify({ type: "session", id: "native-pi", version: 3 }),
    JSON.stringify({
      type: "message_end",
      message: { role: "assistant", content: [{ type: "text", text: joke }] },
    }),
  ].join("\n");
  expect(parsePiOutput(stdout, "fallback")).toMatchObject({ text: joke, nativeId: "native-pi" });
});

it("extracts only assistant message_end text when the live stream also echoes the user prompt", () => {
  const prompt = "# Tell a Short Joke\n- Prefer a classic setup and punchline, or a single one-liner.";
  const joke = "Why did the scarecrow win an award? Because he was outstanding in his field!";
  const stdout = [
    JSON.stringify({ type: "session", id: "native-pi", version: 3 }),
    JSON.stringify({
      type: "message_end",
      message: { role: "user", content: [{ type: "text", text: prompt }] },
    }),
    JSON.stringify({
      type: "message_end",
      message: {
        role: "assistant",
        content: [
          { type: "thinking", thinking: "Pick a classic one-liner." },
          { type: "text", text: joke },
        ],
      },
    }),
    JSON.stringify({ type: "message", role: "user", content: [{ type: "text", text: prompt }] }),
  ].join("\n");
  const parsed = parsePiOutput(stdout, "fallback");
  expect(parsed).toMatchObject({ text: joke, nativeId: "native-pi" });
  expect(parsed.text).not.toContain("Tell a Short Joke");
});

it("builds cursor argv with force-allow and workspace", () => {
  expect(buildCursorArgs(base({ command: "agent", model: "composer-2.5", nativeId: "uuid" }))).toEqual([
    "-p",
    "--force",
    "--yolo",
    "--approve-mcps",
    "--trust",
    "--output-format",
    "json",
    "--workspace",
    "/work",
    "--model",
    "composer-2.5",
    "--resume",
    "uuid",
    "hello",
  ]);
  expect(parseCursorOutput(JSON.stringify({ result: "ok", session_id: "sid" }))).toEqual({
    text: "ok",
    nativeId: "sid",
    result: { result: "ok", session_id: "sid" },
  });
});

it("returns a cursor empty-JSON error instead of throwing so callers can attach stderr", () => {
  expect(parseCursorOutput("")).toMatchObject({
    text: "",
    nativeId: "",
    error: "cursor agent produced empty JSON output",
  });
  expect(parseCursorOutput("not-json")).toMatchObject({
    text: "",
    nativeId: "",
    error: "cursor agent produced invalid JSON output",
  });
});

it("builds claude argv with skip-permissions and effort", () => {
  const created = buildClaudeArgs(base({ command: "claude", model: "sonnet", thinking: "high" }));
  expect(created.args).toContain("--dangerously-skip-permissions");
  expect(created.args.slice(0, 3)).toEqual(["-p", "--dangerously-skip-permissions", "--output-format"]);
  expect(created.args).toContain("--effort");
  expect(created.args).toContain("high");
  expect(created.args).toContain("--session-id");
  expect(created.args).toContain("--name");
  expect(created.args.at(-1)).toBe("hello");
  expect(created.createUuid).toMatch(/^[0-9a-f-]{36}$/i);

  const resumed = buildClaudeArgs(base({ command: "claude", nativeId: "native" }));
  expect(resumed.args).toContain("--resume");
  expect(resumed.args).toContain("native");
  expect(parseClaudeOutput(JSON.stringify({ result: "c", session_id: "s" }), "fb")).toMatchObject({
    text: "c",
    nativeId: "s",
  });
  expect(
    parseClaudeOutput(
      JSON.stringify({
        result: "c",
        session_id: "s",
        usage: { input_tokens: 100, output_tokens: 20 },
        total_cost_usd: 0.012,
      }),
      "fb",
    ),
  ).toMatchObject({
    usage: { input_tokens: 100, output_tokens: 20, cost: 0.012, cost_currency: "USD" },
  });
});

it("extracts pi usage from the last assistant message_end", () => {
  const stdout = [
    JSON.stringify({ type: "session", id: "native-pi", version: 3 }),
    JSON.stringify({
      type: "message_update",
      usage: { input: 9, output: 1, cost: { total: 0.001 } },
    }),
    JSON.stringify({
      type: "message_end",
      message: {
        role: "assistant",
        content: [{ type: "text", text: "answer" }],
        usage: {
          input: 1200,
          output: 400,
          cacheRead: 50,
          reasoning: 10,
          totalTokens: 1660,
          cost: { total: 0.012 },
        },
      },
    }),
  ].join("\n");
  expect(parsePiOutput(stdout, "fallback")).toMatchObject({
    text: "answer",
    nativeId: "native-pi",
    usage: {
      input_tokens: 1200,
      output_tokens: 400,
      cache_read_tokens: 50,
      thinking_tokens: 10,
      total_tokens: 1660,
      cost: 0.012,
    },
  });
});

it("returns pi stopReason error with session native id and no assistant text", () => {
  const detail = "OpenAI API error (429): rate_limit_exceeded";
  const stdout = [
    JSON.stringify({ type: "session", id: "native-pi", version: 3 }),
    JSON.stringify({
      type: "message_end",
      message: { role: "assistant", content: [], stopReason: "error", errorMessage: detail },
    }),
  ].join("\n");
  expect(parsePiOutput(stdout, "fallback")).toMatchObject({
    text: "",
    nativeId: "native-pi",
    error: detail,
  });
});

it("treats claude is_error JSON as a hard fail even when result has assistant text", () => {
  const detail =
    "There's an issue with the selected model (some-wrong-fake-model). It may not exist or you may not have access to it.";
  const stdout = JSON.stringify({
    type: "result",
    subtype: "success",
    is_error: true,
    result: detail,
    session_id: "sess-claude",
  });
  expect(parseClaudeOutput(stdout, "fb")).toMatchObject({
    text: detail,
    nativeId: "sess-claude",
    error: detail,
  });
});

it("treats claude unrecognized_model stderr as a hard fail when JSON still looks successful", () => {
  const detail =
    "There's an issue with the selected model (some-wrong-fake-model). It may not exist or you may not have access to it.";
  const stdout = JSON.stringify({
    type: "result",
    subtype: "success",
    is_error: false,
    result: detail,
    session_id: "sess-claude",
  });
  const stderr = "[claude-code:unrecognized_model] model not in catalog\n";
  expect(parseClaudeOutput(stdout, "fb", stderr)).toMatchObject({
    nativeId: "sess-claude",
    error: detail,
  });
});

it("builds opencode argv with auto, dir, title/resume", () => {
  expect(buildOpenCodeArgs(base({ command: "opencode", model: "opencode-go/x", thinking: "high" }))).toEqual([
    "run",
    "--auto",
    "--format",
    "json",
    "--dir",
    "/work",
    "--title",
    "260901-1200--pi--abcdef",
    "-m",
    "opencode-go/x",
    "--variant",
    "high",
    "hello",
  ]);
  expect(buildOpenCodeArgs(base({ nativeId: "ses_1" }))).toContain("-s");
  expect(parseOpenCodeOutput(
    `${JSON.stringify({ type: "session", sessionID: "ses_x" })}\n${JSON.stringify({ type: "text", part: { text: "t" } })}\n`,
    "fallback",
  )).toMatchObject({ text: "t", nativeId: "ses_x" });
  expect(parseOpenCodeOutput(`${JSON.stringify({ type: "text", part: { text: "t" } })}\n`, "260901-1200--opencode--abcdef")).toMatchObject({
    text: "t",
    nativeId: "",
  });
});

it("treats opencode JSONL error events as a hard fail", () => {
  const detail = "Model not found: some-fake-wrong-model";
  const stdout = [
    JSON.stringify({ type: "session", sessionID: "ses_x" }),
    JSON.stringify({
      type: "error",
      sessionID: "ses_x",
      error: { name: "UnknownError", data: { message: detail } },
    }),
  ].join("\n");
  expect(parseOpenCodeOutput(stdout, "pretty-fallback")).toMatchObject({
    text: "",
    nativeId: "ses_x",
    error: detail,
  });
});

it("treats opencode empty assistant text as a hard fail", () => {
  const stdout = `${JSON.stringify({ type: "session", sessionID: "ses_x" })}\n`;
  expect(parseOpenCodeOutput(stdout, "pretty-fallback")).toMatchObject({
    text: "",
    nativeId: "ses_x",
    error: "opencode produced empty assistant text",
  });
});

it("builds antigravity argv and parses response/conversation_id", () => {
  // agy -p takes the next token as the prompt value — flags must come first,
  // then -p immediately followed by the prompt (never a flag after -p).
  // --add-dir maps workdir so print-mode sees the project tree.
  const args = buildAntigravityArgs(base({ command: "agy", model: "m", thinking: "medium", nativeId: "c1" }));
  expect(args).toEqual([
    "--dangerously-skip-permissions",
    "--output-format",
    "json",
    "--model",
    "m",
    "--effort",
    "medium",
    "--conversation",
    "c1",
    "--add-dir",
    "/work",
    "-p",
    "hello",
  ]);
  const addDirIndex = args.indexOf("--add-dir");
  expect(addDirIndex).toBeGreaterThanOrEqual(0);
  expect(args[addDirIndex + 1]).toBe("/work");
  const pIndex = args.indexOf("-p");
  expect(pIndex).toBeGreaterThan(addDirIndex);
  expect(args[pIndex + 1]).toBe("hello");
  expect(args[pIndex + 1]?.startsWith("-")).toBe(false);
  expect(args.at(-1)).toBe("hello");
  expect(parseAntigravityOutput(JSON.stringify({ response: "r", conversation_id: "cid" }))).toMatchObject({
    text: "r",
    nativeId: "cid",
  });
  expect(
    parseAntigravityOutput(
      JSON.stringify({
        response: "r",
        conversation_id: "cid",
        usage: {
          input_tokens: 1200,
          output_tokens: 400,
          thinking_tokens: 20,
          cache_read_tokens: 50,
          total_tokens: 1660,
        },
      }),
    ),
  ).toMatchObject({
    usage: {
      input_tokens: 1200,
      output_tokens: 400,
      thinking_tokens: 20,
      cache_read_tokens: 50,
      total_tokens: 1660,
    },
  });
});

it("surfaces agy status ERROR from the payload error field", () => {
  const detail =
    'invalid model selection (--model "gemini-3.8-flash"): model gemini-3.8-flash is not recognized as a known model or custom model in settings';
  const stdout = JSON.stringify({
    status: "ERROR",
    conversation_id: "",
    error: detail,
    response: "",
  });
  expect(parseAntigravityOutput(stdout)).toMatchObject({
    text: "",
    nativeId: "",
    error: detail,
  });
});

it("falls back to captured stderr when agy JSON has no error field", () => {
  const stdout = JSON.stringify({ status: "ERROR", conversation_id: "", response: "" });
  expect(parseAntigravityOutput(stdout, "agy refused the model\n")).toMatchObject({
    nativeId: "",
    error: "agy refused the model",
  });
});
