import { CLAUDE_EFFORT } from "./tools/claude.js";
import { AGY_EFFORT } from "./tools/antigravity.js";
import { DEFAULT_COMMANDS, isToolName, type ToolName } from "./tools/types.js";
import type { Config, ToolDefaults } from "./config/parse.js";
import type { PromptMeta } from "./prompt.js";
import { QcError } from "./errors.js";

export type OutputMode = "text" | "json";

export interface FlagOverrides {
  tool?: string;
  model?: string;
  thinking?: string;
  workdir?: string;
  noSkills?: boolean;
  skillPath?: string;
  output?: OutputMode;
}

export interface ResolvedSettings {
  tool: ToolName;
  model?: string;
  thinking?: string;
  workdir?: string;
  noSkills?: boolean;
  skillPath?: string;
  command: string;
  output: OutputMode;
  warnings: string[];
}

function toolDefaults(config: Config, tool: ToolName): ToolDefaults {
  return config.tool.tools[tool] ?? {};
}

function pickString(...values: Array<string | undefined>): string | undefined {
  for (const value of values) if (value !== undefined) return value;
  return undefined;
}

/**
 * Flags > frontmatter > project/global merged `[tool.*]` > omit.
 * Inapplicable fields warn + ignore; unsupported effort values warn + ignore.
 */
export function resolveSettings(
  flags: FlagOverrides,
  meta: PromptMeta,
  config: Config,
  lockedTool?: ToolName,
): ResolvedSettings {
  const warnings: string[] = [];
  const configuredDefault = config.tool.default ?? "pi";
  // Explicit tool only from flags/frontmatter — config default must not fight a continued session.
  const explicit = flags.tool ?? meta.tool;
  if (explicit !== undefined && !isToolName(explicit)) throw new QcError(`unknown tool '${explicit}'`);
  if (lockedTool !== undefined && explicit !== undefined && explicit !== lockedTool) {
    throw new QcError(`tool mismatch: session is '${lockedTool}' but resolved tool is '${explicit}'`);
  }
  const chosen = explicit ?? lockedTool ?? configuredDefault;
  if (!isToolName(chosen)) throw new QcError(`unknown tool '${chosen}'`);
  const tool = chosen;
  const defaults = toolDefaults(config, tool);
  const command = defaults.command ?? DEFAULT_COMMANDS[tool];

  let model = pickString(flags.model, meta.model, defaults.defaultModel);
  let thinking = pickString(flags.thinking, meta.thinking, defaults.defaultThinking);
  const workdir = pickString(flags.workdir, meta.workdir);
  let noSkills = flags.noSkills ?? meta.noSkills;
  let skillPath = pickString(flags.skillPath, meta.skillPath);
  const output = flags.output ?? "text";

  // Cursor ignores thinking entirely (effort lives in the exact model slug).
  if (tool === "cursor" && thinking !== undefined) {
    warnings.push(`qc_thinking is ignored for tool 'cursor' (encode effort in the model slug)`);
    thinking = undefined;
  }

  // Claude / antigravity map thinking → --effort; unsupported values warn+ignore.
  if (tool === "claude" && thinking !== undefined && !CLAUDE_EFFORT.has(thinking)) {
    warnings.push(`qc_thinking '${thinking}' is unsupported for claude --effort; ignoring`);
    thinking = undefined;
  }
  if (tool === "antigravity" && thinking !== undefined && !AGY_EFFORT.has(thinking)) {
    warnings.push(`qc_thinking '${thinking}' is unsupported for antigravity --effort; ignoring`);
    thinking = undefined;
  }

  // Pi-only skill flags.
  if (tool !== "pi") {
    if (noSkills) {
      warnings.push(`qc_no_skills is ignored for tool '${tool}'`);
      noSkills = undefined;
    }
    if (skillPath !== undefined) {
      warnings.push(`qc_skill_path is ignored for tool '${tool}'`);
      skillPath = undefined;
    }
  }

  return {
    tool,
    model,
    thinking,
    workdir,
    noSkills: noSkills || undefined,
    skillPath,
    command,
    output,
    warnings,
  };
}
