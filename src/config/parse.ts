import { readFile, stat } from "node:fs/promises";
import { parse } from "@iarna/toml";
import { QcError } from "../errors.js";
import { renamedConfigKey } from "../messages.js";
import { isToolName, type ToolName } from "../tools/types.js";

export interface ToolDefaults {
  defaultModel?: string;
  defaultThinking?: string;
  command?: string;
}

export interface ToolConfig {
  default?: ToolName;
  tools: Partial<Record<ToolName, ToolDefaults>>;
}

export interface Config {
  shell?: string;
  tool: ToolConfig;
}

const EMPTY_TOOL: ToolConfig = { tools: {} };

function parseToolTable(raw: Record<string, unknown>, path: string): ToolConfig {
  const result: ToolConfig = { tools: {} };
  const allowedTop = new Set(["default", "pi", "cursor", "claude", "opencode", "antigravity"]);
  for (const key of Object.keys(raw)) {
    if (!allowedTop.has(key)) throw new QcError(`unknown tool config key '${key}' in ${path}`);
  }
  if (raw.default !== undefined) {
    if (typeof raw.default !== "string" || !raw.default) throw new QcError(`tool.default in ${path} must be a non-empty string`);
    if (!isToolName(raw.default)) throw new QcError(`tool.default in ${path} must be one of: pi, cursor, claude, opencode, antigravity`);
    result.default = raw.default;
  }
  for (const name of ["pi", "cursor", "claude", "opencode", "antigravity"] as const) {
    const table = raw[name];
    if (table === undefined) continue;
    if (!table || typeof table !== "object" || Array.isArray(table)) {
      throw new QcError(`tool.${name} in ${path} must be a table`);
    }
    const entry = table as Record<string, unknown>;
    const allowed = new Set(["default_model", "default_thinking", "command"]);
    for (const key of Object.keys(entry)) {
      if (!allowed.has(key)) throw new QcError(`unknown tool.${name} key '${key}' in ${path}`);
    }
    const text = (key: string): string | undefined => {
      const value = entry[key];
      if (value === undefined) return undefined;
      if (typeof value !== "string" || !value) throw new QcError(`tool.${name}.${key} in ${path} must be a non-empty string`);
      return value;
    };
    result.tools[name] = {
      defaultModel: text("default_model"),
      defaultThinking: text("default_thinking"),
      command: text("command"),
    };
  }
  return result;
}

export async function readConfig(path: string): Promise<Config> {
  try {
    await stat(path);
  } catch (cause: unknown) {
    if ((cause as NodeJS.ErrnoException).code === "ENOENT") {
      return { tool: { ...EMPTY_TOOL } };
    }
    throw cause;
  }
  let raw: Record<string, unknown>;
  const source = await readFile(path, "utf8");
  try {
    raw = parse(source) as Record<string, unknown>;
  } catch (cause) {
    throw new QcError(`invalid TOML in ${path}: ${cause instanceof Error ? cause.message : String(cause)}`);
  }

  if (Object.prototype.hasOwnProperty.call(raw, "default-cli")) {
    throw new QcError(renamedConfigKey("default-cli", '[tool] default = "…"'));
  }
  if (Object.prototype.hasOwnProperty.call(raw, "default-model")) {
    throw new QcError(renamedConfigKey("default-model", "[tool.<name>] default_model"));
  }
  if (Object.prototype.hasOwnProperty.call(raw, "default-thinking")) {
    throw new QcError(renamedConfigKey("default-thinking", "[tool.<name>] default_thinking"));
  }

  // Keep command-permissions in the allow-list so leftover tables do not hard-fail.
  const allowed = new Set(["shell", "tool", "command-permissions"]);
  for (const key of Object.keys(raw)) if (!allowed.has(key)) throw new QcError(`unknown config key '${key}' in ${path}`);

  const text = (key: string): string | undefined => {
    const value = raw[key];
    if (value === undefined) return undefined;
    if (typeof value !== "string" || !value) throw new QcError(`${key} in ${path} must be a non-empty string`);
    return value;
  };

  let tool: ToolConfig = { tools: {} };
  if (raw.tool !== undefined) {
    if (!raw.tool || typeof raw.tool !== "object" || Array.isArray(raw.tool)) {
      throw new QcError(`tool in ${path} must be a table`);
    }
    tool = parseToolTable(raw.tool as Record<string, unknown>, path);
  }

  // Leftover [command-permissions] is ignored (no allow/deny validation).
  return {
    shell: text("shell"),
    tool,
  };
}
