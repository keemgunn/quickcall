import { emptyAssistantText } from "../messages.js";
import { usageFromRecord, type AgentRequest, type AgentResult, type AgentUsage } from "./types.js";
import { spawnAgent } from "./spawn.js";

export function buildOpenCodeArgs(request: AgentRequest): string[] {
  const args = ["run", "--auto", "--format", "json", "--dir", request.workdir];
  if (!request.nativeId) args.push("--title", request.sessionId);
  else args.push("-s", request.nativeId);
  if (request.model) args.push("-m", request.model);
  if (request.thinking) args.push("--variant", request.thinking);
  // Message as last argv token (OpenCode run accepts the prompt on argv).
  args.push(request.prompt);
  return args;
}

function openCodeErrorDetail(error: unknown, stderr: string): string {
  if (error && typeof error === "object") {
    const record = error as Record<string, unknown>;
    const data = record.data;
    if (data && typeof data === "object") {
      const message = (data as { message?: unknown }).message;
      if (typeof message === "string" && message.trim()) return message.trim();
    }
    if (typeof record.message === "string" && record.message.trim()) return record.message.trim();
    if (typeof record.name === "string" && record.name.trim()) return record.name.trim();
  }
  if (typeof error === "string" && error.trim()) return error.trim();
  if (stderr.trim()) return stderr.trim();
  return "opencode reported a run error";
}

function eventReportedError(event: Record<string, unknown>): unknown | undefined {
  if (event.type === "error" && event.error !== undefined) return event.error;
  if (event.type === "session.error") {
    const properties = event.properties;
    if (properties && typeof properties === "object" && (properties as { error?: unknown }).error !== undefined) {
      return (properties as { error: unknown }).error;
    }
    if (event.error !== undefined) return event.error;
  }
  return undefined;
}

function eventUsageCandidate(event: Record<string, unknown>): unknown {
  if (event.usage !== undefined) return event.usage;
  const properties = event.properties;
  if (properties && typeof properties === "object") {
    const record = properties as Record<string, unknown>;
    if (record.usage !== undefined) return record.usage;
  }
  const part = event.part;
  if (part && typeof part === "object" && (part as { usage?: unknown }).usage !== undefined) {
    return (part as { usage: unknown }).usage;
  }
  return undefined;
}

function eventCostCandidate(event: Record<string, unknown>): number | undefined {
  if (typeof event.cost === "number") return event.cost;
  const properties = event.properties;
  if (properties && typeof properties === "object" && typeof (properties as { cost?: unknown }).cost === "number") {
    return (properties as { cost: number }).cost;
  }
  return undefined;
}

function openCodeNativeCandidate(event: Record<string, unknown>): string {
  if (typeof event.sessionID === "string" && event.sessionID) return event.sessionID;
  if (typeof event.session_id === "string" && event.session_id) return event.session_id;
  if (event.type === "session" && typeof event.id === "string" && event.id.startsWith("ses_")) return event.id;
  return "";
}

const PRETTY_ID = /^\d{6}-\d{4}--[a-z]+--[a-z0-9]{6}$/;

function seedOpenCodeNativeId(fallbackNativeId: string): string {
  if (!fallbackNativeId || PRETTY_ID.test(fallbackNativeId)) return "";
  return fallbackNativeId;
}

/**
 * OpenCode JSONL: capture text parts and observed session ids (`ses_*` / sessionID).
 * Do not use the create `--title` pretty id as a resumable native id.
 * `run --format json` emits `{type:"error", error}` for model/run failures.
 */
export function parseOpenCodeOutput(
  stdout: string,
  fallbackNativeId: string,
  stderr = "",
): { text: string; nativeId: string; result: unknown; error?: string; usage?: AgentUsage } {
  const lines = stdout
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
  const events: unknown[] = [];
  const texts: string[] = [];
  let nativeId = seedOpenCodeNativeId(fallbackNativeId);
  let reportedError: unknown;
  let usage: AgentUsage | undefined;

  for (const line of lines) {
    let event: Record<string, unknown>;
    try {
      event = JSON.parse(line) as Record<string, unknown>;
    } catch {
      continue;
    }
    events.push(event);

    const sessionCandidate = openCodeNativeCandidate(event);
    if (sessionCandidate) nativeId = sessionCandidate;

    const error = eventReportedError(event);
    if (error !== undefined) reportedError = error;

    const cost = eventCostCandidate(event);
    const lifted = usageFromRecord(eventUsageCandidate(event), cost !== undefined ? { cost } : undefined);
    if (lifted) usage = lifted;

    if (event.type === "text") {
      const part = event.part;
      if (part && typeof part === "object" && typeof (part as { text?: string }).text === "string") {
        texts.push((part as { text: string }).text);
      } else if (typeof event.text === "string") {
        texts.push(event.text);
      }
    }
    if (event.type !== "error" && event.type !== "session.error" && typeof event.result === "string") {
      texts.push(event.result);
    }
  }

  const text = texts.join("");
  const result = events.length ? events : stdout;
  if (reportedError !== undefined) {
    return { text, nativeId, result, error: openCodeErrorDetail(reportedError, stderr) };
  }
  if (!text.trim()) {
    return { text, nativeId, result, error: stderr.trim() || emptyAssistantText("opencode") };
  }
  return { text, nativeId, result, ...(usage ? { usage } : {}) };
}

export async function runOpenCode(request: AgentRequest): Promise<AgentResult> {
  const args = buildOpenCodeArgs(request);
  const capture = await spawnAgent(request.command, args, undefined, request.workdir, request.env, "opencode");
  const parsed = parseOpenCodeOutput(capture.stdout, request.nativeId ?? "", capture.stderr);
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
