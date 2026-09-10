import type { AgentRequest, AgentResult, AgentUsage } from "./types.js";
import { spawnAgent } from "./spawn.js";

/** Pi accepts every qc_thinking value as `--thinking`. */
export function buildPiArgs(request: AgentRequest): string[] {
  const args = ["--mode", "json", "-a", "--session-id", request.sessionId];
  if (request.model) args.push("--model", request.model);
  if (request.thinking) args.push("--thinking", request.thinking);
  if (request.noSkills) args.push("--no-skills");
  if (request.skillPath) args.push("--skill", request.skillPath);
  return args;
}

function textPartsFromContent(content: unknown): string[] {
  if (typeof content === "string") return [content];
  if (!Array.isArray(content)) return [];
  const texts: string[] = [];
  for (const part of content) {
    if (part && typeof part === "object" && (part as { type?: string }).type === "text" && typeof (part as { text?: string }).text === "string") {
      texts.push((part as { text: string }).text);
    }
  }
  return texts;
}

function usageFromPi(raw: unknown): AgentUsage | undefined {
  if (!raw || typeof raw !== "object") return undefined;
  const u = raw as Record<string, unknown>;
  const out: AgentUsage = {};
  if (typeof u.input === "number") out.input_tokens = u.input;
  if (typeof u.output === "number") out.output_tokens = u.output;
  if (typeof u.cacheRead === "number") out.cache_read_tokens = u.cacheRead;
  if (typeof u.reasoning === "number") out.thinking_tokens = u.reasoning;
  if (typeof u.totalTokens === "number") out.total_tokens = u.totalTokens;
  const cost = u.cost;
  if (cost && typeof cost === "object" && typeof (cost as { total?: unknown }).total === "number") {
    out.cost = (cost as { total: number }).total;
  }
  return Object.keys(out).length ? out : undefined;
}

function eventUsage(event: Record<string, unknown>): unknown {
  if (event.usage !== undefined) return event.usage;
  const message = event.message;
  if (message && typeof message === "object" && (message as { usage?: unknown }).usage !== undefined) {
    return (message as { usage: unknown }).usage;
  }
  return undefined;
}

/**
 * Extract assistant text and session id from Pi JSONL.
 * Live `--mode json` writes the reply on assistant `message_end.message.content[].text`.
 * User `message_end` echoes the prompt; skip it. Skip thinking parts (keep `type:text` only).
 * Session files (and older streams) use `{type:"message", role:"assistant"}` — keep as fallback.
 */
export function parsePiOutput(stdout: string, fallbackSessionId: string): { text: string; nativeId: string; result: unknown; error?: string; usage?: AgentUsage } {
  const lines = stdout
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
  let nativeId = fallbackSessionId;
  const liveTexts: string[] = [];
  const fallbackTexts: string[] = [];
  const events: unknown[] = [];
  let error: string | undefined;
  let endUsage: unknown;
  let updateUsage: unknown;

  for (const line of lines) {
    let event: Record<string, unknown>;
    try {
      event = JSON.parse(line) as Record<string, unknown>;
    } catch {
      continue;
    }
    events.push(event);
    if (event.type === "session" && typeof event.id === "string" && event.id) {
      nativeId = event.id;
    }
    if (event.type === "message_update") {
      const raw = eventUsage(event);
      if (raw !== undefined) updateUsage = raw;
    }
    if (event.type === "message_end") {
      const message = event.message;
      // Live JSONL echoes the user turn as message_end too; only assistant text is the reply.
      if (message && typeof message === "object") {
        const msg = message as { role?: string; content?: unknown; stopReason?: string; errorMessage?: string; usage?: unknown };
        if (msg.role === "assistant") {
          liveTexts.push(...textPartsFromContent(msg.content));
          if (msg.usage !== undefined) endUsage = msg.usage;
        }
        if (msg.stopReason === "error") {
          error = typeof msg.errorMessage === "string" && msg.errorMessage.trim() ? msg.errorMessage.trim() : "pi reported a run error";
        }
      }
    }
    if (event.type === "message" && event.role === "assistant") {
      fallbackTexts.push(...textPartsFromContent(event.content));
    }
    // Some Pi builds emit assistant text as a dedicated text event.
    if (event.type === "text" && typeof event.text === "string") fallbackTexts.push(event.text);
  }

  const usage = usageFromPi(endUsage ?? updateUsage);
  return {
    text: (liveTexts.length ? liveTexts : fallbackTexts).join(""),
    nativeId,
    result: events,
    ...(error ? { error } : {}),
    ...(usage ? { usage } : {}),
  };
}

export async function runPi(request: AgentRequest): Promise<AgentResult> {
  const args = buildPiArgs(request);
  const capture = await spawnAgent(request.command, args, request.prompt, request.workdir, request.env, "pi");
  const parsed = parsePiOutput(capture.stdout, request.sessionId);
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
