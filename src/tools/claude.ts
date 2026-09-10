import { randomUUID } from "node:crypto";
import { QcError } from "../errors.js";
import { usageFromRecord, type AgentRequest, type AgentResult, type AgentUsage } from "./types.js";
import { spawnAgent } from "./spawn.js";

/** Claude `--effort` accepts these qc_thinking values; others warn+ignore upstream. */
export const CLAUDE_EFFORT = new Set(["low", "medium", "high", "xhigh", "max"]);

export function buildClaudeArgs(request: AgentRequest): { args: string[]; createUuid?: string } {
  const args = ["-p", "--dangerously-skip-permissions", "--output-format", "json"];
  if (request.model) args.push("--model", request.model);
  if (request.thinking && CLAUDE_EFFORT.has(request.thinking)) args.push("--effort", request.thinking);
  if (request.nativeId) {
    args.push("--resume", request.nativeId);
  } else {
    const createUuid = randomUUID();
    args.push("--session-id", createUuid, "--name", request.sessionId);
    return { args: [...args, request.prompt], createUuid };
  }
  args.push(request.prompt);
  return { args };
}

const CLAUDE_UNRECOGNIZED_MODEL = "[claude-code:unrecognized_model]";

function claudeRunErrorDetail(payload: Record<string, unknown>, stderr: string): string {
  if (typeof payload.error === "string" && payload.error.trim()) return payload.error.trim();
  if (typeof payload.result === "string" && payload.result.trim()) return payload.result.trim();
  if (stderr.trim()) return stderr.trim();
  return "claude reported a run error";
}

function claudeReportedRunError(payload: Record<string, unknown>, stderr: string): boolean {
  if (payload.is_error === true) return true;
  if (typeof payload.subtype === "string" && payload.subtype.startsWith("error")) return true;
  if (Array.isArray(payload.errors) && payload.errors.length > 0) return true;
  return stderr.includes(CLAUDE_UNRECOGNIZED_MODEL);
}

export function parseClaudeOutput(
  stdout: string,
  fallbackNativeId: string,
  stderr = "",
): { text: string; nativeId: string; result: unknown; error?: string; usage?: AgentUsage } {
  const trimmed = stdout.trim();
  if (!trimmed) {
    return { text: "", nativeId: fallbackNativeId, result: stdout, error: "claude produced empty JSON output" };
  }
  let payload: Record<string, unknown>;
  try {
    payload = JSON.parse(trimmed) as Record<string, unknown>;
  } catch {
    return { text: "", nativeId: fallbackNativeId, result: stdout, error: "claude produced invalid JSON output" };
  }
  const nativeId =
    typeof payload.session_id === "string" && payload.session_id ? payload.session_id : fallbackNativeId;
  const text = typeof payload.result === "string" ? payload.result : "";
  const extras: Pick<AgentUsage, "cost" | "cost_currency"> = {};
  if (typeof payload.total_cost_usd === "number") {
    extras.cost = payload.total_cost_usd;
    extras.cost_currency = "USD";
  }
  const usage = usageFromRecord(payload.usage, extras);
  if (claudeReportedRunError(payload, stderr)) {
    return { text, nativeId, result: payload, error: claudeRunErrorDetail(payload, stderr), ...(usage ? { usage } : {}) };
  }
  if (!nativeId) throw new QcError("claude JSON missing session_id");
  return { text, nativeId, result: payload, ...(usage ? { usage } : {}) };
}

export async function runClaude(request: AgentRequest): Promise<AgentResult> {
  const built = buildClaudeArgs(request);
  const capture = await spawnAgent(request.command, built.args, undefined, request.workdir, request.env, "claude");
  const fallback = request.nativeId ?? built.createUuid ?? "";
  const parsed = parseClaudeOutput(capture.stdout, fallback, capture.stderr);
  return {
    text: parsed.text,
    nativeId: parsed.nativeId,
    exit: capture.exit,
    result: parsed.result,
    stderr: capture.stderr,
    argv: built.args,
    stdoutBytes: capture.stdout.length,
    ...(parsed.error ? { error: parsed.error } : {}),
    ...(parsed.usage ? { usage: parsed.usage } : {}),
  };
}
