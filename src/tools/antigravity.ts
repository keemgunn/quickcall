import { usageFromRecord, type AgentRequest, type AgentResult, type AgentUsage } from "./types.js";
import { spawnAgent } from "./spawn.js";

/** Antigravity `--effort` accepts only these values. */
export const AGY_EFFORT = new Set(["low", "medium", "high"]);

export function buildAntigravityArgs(request: AgentRequest): string[] {
  // agy -p takes the next argv token as the prompt value (unlike cursor's boolean -p).
  // Put every option first, then -p immediately followed by the prompt — never a flag after -p.
  // --add-dir tells print-mode which tree to trust; spawn cwd alone is not enough.
  const args = ["--dangerously-skip-permissions", "--output-format", "json"];
  if (request.model) args.push("--model", request.model);
  if (request.thinking && AGY_EFFORT.has(request.thinking)) args.push("--effort", request.thinking);
  if (request.nativeId) args.push("--conversation", request.nativeId);
  args.push("--add-dir", request.workdir);
  args.push("-p", request.prompt);
  return args;
}

export function parseAntigravityOutput(stdout: string, stderr = ""): { text: string; nativeId: string; result: unknown; error?: string; usage?: AgentUsage } {
  const trimmed = stdout.trim();
  if (!trimmed) {
    return { text: "", nativeId: "", result: stdout, error: stderr.trim() || "agy produced empty JSON output" };
  }
  let payload: Record<string, unknown>;
  try {
    payload = JSON.parse(trimmed) as Record<string, unknown>;
  } catch {
    return { text: "", nativeId: "", result: stdout, error: stderr.trim() || "agy produced invalid JSON output" };
  }
  const text = typeof payload.response === "string" ? payload.response : "";
  const nativeId = typeof payload.conversation_id === "string" ? payload.conversation_id : "";
  const payloadError = typeof payload.error === "string" ? payload.error.trim() : "";
  const status = typeof payload.status === "string" ? payload.status : "";
  const usage = usageFromRecord(payload.usage);
  // Invalid model and other provider failures return JSON with status ERROR and empty conversation_id.
  // Prefer payload.error; callers pass captured child stderr as fallback.
  if (status === "ERROR") {
    return { text, nativeId, result: payload, error: payloadError || stderr.trim() || "agy reported a run error" };
  }
  if (!nativeId) {
    return { text, nativeId: "", result: payload, error: payloadError || stderr.trim() || "agy JSON missing conversation_id" };
  }
  return { text, nativeId, result: payload, ...(usage ? { usage } : {}) };
}

export async function runAntigravity(request: AgentRequest): Promise<AgentResult> {
  const args = buildAntigravityArgs(request);
  const capture = await spawnAgent(request.command, args, undefined, request.workdir, request.env, "antigravity");
  const parsed = parseAntigravityOutput(capture.stdout, capture.stderr);
  return {
    text: parsed.text,
    nativeId: parsed.nativeId,
    exit: capture.exit,
    result: parsed.result,
    stderr: capture.stderr,
    argv: args,
    stdoutBytes: capture.stdout.length,
    ...(parsed.error ? { error: parsed.error } : {}),
    ...(parsed.usage ? { usage: parsed.usage } : {}),
  };
}
