import { usageFromRecord, type AgentRequest, type AgentResult, type AgentUsage } from "./types.js";
import { spawnAgent } from "./spawn.js";

export function buildCursorArgs(request: AgentRequest): string[] {
  const args = ["-p", "--force", "--yolo", "--approve-mcps", "--trust", "--output-format", "json"];
  args.push("--workspace", request.workdir);
  if (request.model) args.push("--model", request.model);
  if (request.nativeId) args.push("--resume", request.nativeId);
  // Prompt as final argv token; Cursor headless accepts the message on argv.
  args.push(request.prompt);
  return args;
}

export function parseCursorOutput(stdout: string): { text: string; nativeId: string; result: unknown; error?: string; usage?: AgentUsage } {
  const trimmed = stdout.trim();
  if (!trimmed) {
    return { text: "", nativeId: "", result: stdout, error: "cursor agent produced empty JSON output" };
  }
  let payload: Record<string, unknown>;
  try {
    payload = JSON.parse(trimmed) as Record<string, unknown>;
  } catch {
    return { text: "", nativeId: "", result: stdout, error: "cursor agent produced invalid JSON output" };
  }
  const text = typeof payload.result === "string" ? payload.result : "";
  const nativeId = typeof payload.session_id === "string" ? payload.session_id : "";
  const usage = usageFromRecord(payload.usage);
  if (!nativeId) return { text, nativeId: "", result: payload, error: "cursor agent JSON missing session_id" };
  return { text, nativeId, result: payload, ...(usage ? { usage } : {}) };
}

export async function runCursor(request: AgentRequest): Promise<AgentResult> {
  const args = buildCursorArgs(request);
  const capture = await spawnAgent(request.command, args, undefined, request.workdir, request.env, "cursor");
  const parsed = parseCursorOutput(capture.stdout);
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
