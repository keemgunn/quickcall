import packageJson from "#package.json" with { type: "json" };

import type { AgentUsage } from "./tools/types.js";

export const version = packageJson.version;

export const help = `Usage: qc [<prompt-reference>] [options]
       qc -o|--open [id] [--tool <name>]
       qc --install-sample-prompts
       qc --install-agent-harness

Run a Markdown prompt through a headless agent tool (default: pi).

Options:
  --tool <name>        Agent tool: pi, cursor, claude, opencode, antigravity
  --model <id>         Model / slug for the selected tool
  --thinking <level>   Shared thinking knob (tool-specific mapping)
  --workdir <path>     Working directory for shell expansion and agent spawn
  --output <mode>      text (default) or json
  --debug              Include [WARNING] and [DEBUG] lines on the stdout envelope
  -q, --quiet           Print only the final envelope (no spinner)
  -c, --continue <id>  Resume a qc session by pretty id
  -o, --open [id]      Open a stored session in the provider's native TUI
  --skill <path>       Pi-only skill path
  --no-skills          Pi-only: disable skills
  -s, --shell <path>   Shell used only for !\`command\` substitutions
  -a, --append <text>  Append user text, or the whole turn when no prompt is given
      --install-sample-prompts
                       Replace ~/.qc/prompts/samples/ from packaged defaults
      --install-agent-harness
                       Replace-install qc skills to ~/.agents/skills and ~/.claude/skills
  -h, --help           Show this help
  -v, --version        Show the qc version

Prompt reference is optional when -a/--append is set. -o/--open does not take a prompt.`;

export const error = (detail: string) => `[ERROR] ${detail}`;
export const warn = (detail: string) => `[WARNING] ${detail}`;
export const debugLine = (detail: string) => `[DEBUG] ${detail}`;
export const sessionLine = (id: string) => `[QC-SESSION] ${id}`;
export const openSessionCommand = (id: string) => `qc -o ${id}`;

const FAIL_STDERR_MAX = 8192;

/** Tagged fail reason, then child stderr when it is not already that reason. */
export function failOutput(primary: string, childStderr = ""): string {
  let extra = childStderr.trim();
  if (extra.length > FAIL_STDERR_MAX) extra = extra.slice(-FAIL_STDERR_MAX);
  if (!extra || extra === primary || primary.includes(extra)) return `${error(primary)}\n`;
  return `${error(primary)}\n${extra}\n`;
}

/** Copy-paste native binary for live fails with no saved mapping. */
export function inspectCommand(command: string): string {
  if (/[\s']/.test(command)) return `'${command.replace(/'/g, `'\\''`)}'`;
  return command;
}

/** Spinner timer suffix: 000s, 012s, 999s, 1000s (width grows after 999). */
export function elapsedLabel(seconds: number): string {
  return `${String(seconds).padStart(3, "0")}s`;
}

/** Tool, optional model, and elapsed joined with the same skip-model rule as `[SUCCESS]`. */
function statusSegments(tool: string, elapsed: string, model?: string): string {
  if (model) return `${tool} ⋅ ${model} ⋅ ${elapsed}`;
  return `${tool} ⋅ ${elapsed}`;
}

/** Live spinner text after the bouncing bar: ` ⋅ pi ⋅ 004s` or ` ⋅ cursor ⋅ composer-2.5 ⋅ 004s`. */
export function waitLine(tool: string, seconds: number, model?: string): string {
  return ` ⋅ ${statusSegments(tool, elapsedLabel(seconds), model)}`;
}

export function quotedText(text: string): string {
  const body = text.replace(/\n$/, "");
  return `"""\n${body}\n"""`;
}

export function successLine(tool: string, durationS: number, model?: string): string {
  return `[SUCCESS] ${statusSegments(tool, `${durationS}s`, model)}`;
}

export function compactTokens(count: number): string {
  if (Math.abs(count) < 1000) return String(count);
  return `${Number((count / 1000).toFixed(1))}k`;
}

function formatCost(cost: number): string {
  return String(cost);
}

/** Tagged usage line, or undefined when nothing reportable was present. */
export function usageLine(usage?: AgentUsage): string | undefined {
  if (!usage) return undefined;
  const parts: string[] = [];
  if (usage.input_tokens !== undefined) parts.push(`${compactTokens(usage.input_tokens)} in`);
  if (usage.output_tokens !== undefined) parts.push(`${compactTokens(usage.output_tokens)} out`);
  if (usage.cache_read_tokens !== undefined) parts.push(`${compactTokens(usage.cache_read_tokens)} cache`);
  if (usage.thinking_tokens !== undefined) parts.push(`${compactTokens(usage.thinking_tokens)} think`);
  if (usage.total_tokens !== undefined) parts.push(`${compactTokens(usage.total_tokens)} total`);
  if (usage.cost !== undefined && usage.cost !== 0) {
    if (usage.cost_currency === "USD") parts.push(`$${formatCost(usage.cost)}`);
    else if (usage.cost_currency) parts.push(`${formatCost(usage.cost)} ${usage.cost_currency}`);
    else parts.push(formatCost(usage.cost));
  }
  if (parts.length === 0) return undefined;
  return `[USAGE] ${parts.join(" ⋅ ")}`;
}

export function usageFields(usage?: AgentUsage): Record<string, number | string> | undefined {
  if (!usage) return undefined;
  const out: Record<string, number | string> = {};
  if (usage.input_tokens !== undefined) out.input_tokens = usage.input_tokens;
  if (usage.output_tokens !== undefined) out.output_tokens = usage.output_tokens;
  if (usage.cache_read_tokens !== undefined) out.cache_read_tokens = usage.cache_read_tokens;
  if (usage.thinking_tokens !== undefined) out.thinking_tokens = usage.thinking_tokens;
  if (usage.total_tokens !== undefined) out.total_tokens = usage.total_tokens;
  if (usage.cost !== undefined) out.cost = usage.cost;
  if (usage.cost_currency) out.cost_currency = usage.cost_currency;
  if (Object.keys(out).length === 0) return undefined;
  return out;
}

export interface TextEnvelopeInput {
  text: string;
  tool: string;
  durationS: number;
  sessionId: string;
  model?: string;
  warnings?: string[];
  debugLines?: string[];
  usage?: AgentUsage;
}

export function textEnvelope(input: TextEnvelopeInput): string {
  const parts = [quotedText(input.text), successLine(input.tool, input.durationS, input.model)];
  const usage = usageLine(input.usage);
  if (usage) parts.push(usage);
  parts.push(sessionLine(input.sessionId));
  if (input.debugLines) {
    for (const warning of input.warnings ?? []) parts.push(warn(warning));
    for (const line of input.debugLines) parts.push(debugLine(line));
  }
  return `${parts.join("\n")}\n`;
}

export const missingBinary = (tool: string, command: string) =>
  `${tool} executable '${command}' was not found on PATH. Install it and try again.`;

export const emptyAssistantText = (tool: string) => `${tool} produced empty assistant text`;

export const interrupted = (signal: number) => `interrupted (signal ${signal})`;

export const renamedConfigKey = (oldKey: string, replacement: string) =>
  `config key '${oldKey}' was removed; use ${replacement}`;

export const renamedFrontmatterKey = (oldKey: string, replacement: string) =>
  `frontmatter key '${oldKey}' was removed; use ${replacement}`;
