/** Shared agent adapter types. Callers only pass resolved settings + prompt; adapters own argv/JSON. */

export const TOOL_NAMES = ["pi", "cursor", "claude", "opencode", "antigravity"] as const;
export type ToolName = (typeof TOOL_NAMES)[number];

/** Default binary names when config omits `[tool.<name>] command`. */
export const DEFAULT_COMMANDS: Record<ToolName, string> = {
  pi: "pi",
  cursor: "agent",
  claude: "claude",
  opencode: "opencode",
  antigravity: "agy",
};

export function isToolName(value: string): value is ToolName {
  return (TOOL_NAMES as readonly string[]).includes(value);
}

export interface AgentRequest {
  prompt: string;
  model?: string;
  /** Shared thinking knob; adapters map or ignore per tool. */
  thinking?: string;
  workdir: string;
  /** qc pretty session id (also used as Pi session id / Claude --name / OpenCode --title). */
  sessionId: string;
  /** Native tool session id when continuing; undefined on create. */
  nativeId?: string;
  noSkills?: boolean;
  skillPath?: string;
  command: string;
  env: NodeJS.ProcessEnv;
}

export interface AgentResult {
  text: string;
  nativeId: string;
  exit: number;
  /** Parsed tool payload retained for `--output json` envelopes. */
  result: unknown;
  /** Captured child stderr. Appended to hard-fail `[ERROR]` when it adds information. Also in `--debug`. Never streamed live. */
  stderr: string;
  /** Spawned argv (may include the prompt token; CLI redacts it for debug). */
  argv: string[];
  stdoutBytes: number;
  /** Provider/adapter failure after a native id may already be known. */
  error?: string;
  /** Provider-reported usage for this turn only. Omit when nothing was reported. */
  usage?: AgentUsage;
}

export interface AgentUsage {
  input_tokens?: number;
  output_tokens?: number;
  cache_read_tokens?: number;
  thinking_tokens?: number;
  total_tokens?: number;
  cost?: number;
  cost_currency?: string;
}

/** Lift standard token/cost fields from a provider JSON object. */
export function usageFromRecord(raw: unknown, extras?: Pick<AgentUsage, "cost" | "cost_currency">): AgentUsage | undefined {
  const out: AgentUsage = { ...extras };
  if (raw && typeof raw === "object") {
    const u = raw as Record<string, unknown>;
    if (typeof u.input_tokens === "number") out.input_tokens = u.input_tokens;
    else if (typeof u.input === "number") out.input_tokens = u.input;
    if (typeof u.output_tokens === "number") out.output_tokens = u.output_tokens;
    else if (typeof u.output === "number") out.output_tokens = u.output;
    if (typeof u.cache_read_tokens === "number") out.cache_read_tokens = u.cache_read_tokens;
    else if (typeof u.cache_read_input_tokens === "number") out.cache_read_tokens = u.cache_read_input_tokens;
    else if (typeof u.cacheRead === "number") out.cache_read_tokens = u.cacheRead;
    if (typeof u.thinking_tokens === "number") out.thinking_tokens = u.thinking_tokens;
    else if (typeof u.reasoning === "number") out.thinking_tokens = u.reasoning;
    if (typeof u.total_tokens === "number") out.total_tokens = u.total_tokens;
    else if (typeof u.totalTokens === "number") out.total_tokens = u.totalTokens;
    if (typeof u.cost === "number") out.cost = u.cost;
    else if (u.cost && typeof u.cost === "object" && typeof (u.cost as { total?: unknown }).total === "number") {
      out.cost = (u.cost as { total: number }).total;
    }
    if (typeof u.cost_currency === "string" && u.cost_currency) out.cost_currency = u.cost_currency;
  }
  if (Object.keys(out).length === 0) return undefined;
  return out;
}
