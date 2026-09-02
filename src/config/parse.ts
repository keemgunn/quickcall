import { readFile, stat } from "node:fs/promises";
import { parse } from "@iarna/toml";
import { QcError } from "../errors.js";
import { unsupportedCli } from "../messages.js";

export interface PermissionRule {
  pattern: string;
  action: "allow" | "deny";
  source: string;
}

export interface Config {
  shell?: string;
  model?: string;
  thinking?: string;
  cli?: string;
  rules: PermissionRule[];
  hasPermissions: boolean;
}

const V1_CLI = "pi";

function validateCli(value: string, path: string, key: string): string {
  if (value !== V1_CLI) throw new QcError(`${key} in ${path}: ${unsupportedCli(value)}`);
  return value;
}

/** Re-parse each source declaration as TOML because parsed object key order loses integer-like keys. */
function orderedPermissions(source: string, path: string): Array<[string, unknown]> {
  const entries: Array<[string, unknown]> = [];
  let cursor = 0;
  let inPermissions = false;
  const line = (): string => {
    const start = cursor;
    while (cursor < source.length && source[cursor] !== "\n") cursor += 1;
    const result = source.slice(start, cursor);
    if (source[cursor] === "\n") cursor += 1;
    return result;
  };
  const permissionHeader = (declaration: string): boolean => {
    try {
      const parsed = parse(`${declaration}\nqc_order_probe = "probe"`) as Record<string, unknown>;
      const table = parsed["command-permissions"];
      return !!table && typeof table === "object" && !Array.isArray(table) && (table as Record<string, unknown>).qc_order_probe === "probe";
    } catch {
      return false;
    }
  };
  while (cursor < source.length) {
    const declaration = line();
    const trimmed = declaration.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    if (trimmed.startsWith("[")) {
      inPermissions = permissionHeader(declaration);
      continue;
    }
    if (!inPermissions) continue;
    try {
      const parsed = parse(`[command-permissions]\n${declaration}`) as Record<string, unknown>;
      const table = parsed["command-permissions"];
      if (!table || typeof table !== "object" || Array.isArray(table) || Object.keys(table as Record<string, unknown>).length !== 1) {
        throw new Error("permission rules must use direct keys");
      }
      const [pattern, action] = Object.entries(table as Record<string, unknown>)[0]!;
      if (action && typeof action === "object") throw new Error("permission rules must use direct keys");
      entries.push([pattern, action]);
    } catch (cause) {
      throw new QcError(`invalid TOML in ${path}: ${cause instanceof Error ? cause.message : String(cause)}`);
    }
  }
  return entries;
}

export async function readConfig(path: string): Promise<Config> {
  try {
    await stat(path);
  } catch (cause: unknown) {
    if ((cause as NodeJS.ErrnoException).code === "ENOENT") return { rules: [], hasPermissions: false };
    throw cause;
  }
  let raw: Record<string, unknown>;
  try {
    raw = parse(await readFile(path, "utf8")) as Record<string, unknown>;
  } catch (cause) {
    throw new QcError(`invalid TOML in ${path}: ${cause instanceof Error ? cause.message : String(cause)}`);
  }
  const allowed = new Set(["shell", "default-model", "default-thinking", "default-cli", "command-permissions"]);
  for (const key of Object.keys(raw)) if (!allowed.has(key)) throw new QcError(`unknown config key '${key}' in ${path}`);
  const text = (key: string): string | undefined => {
    const value = raw[key];
    if (value === undefined) return undefined;
    if (typeof value !== "string" || !value) throw new QcError(`${key} in ${path} must be a non-empty string`);
    return value;
  };
  const cli = raw["default-cli"] === undefined ? undefined : validateCli(text("default-cli")!, path, "default-cli");
  const permissions = raw["command-permissions"];
  const rules: PermissionRule[] = [];
  if (permissions !== undefined) {
    if (!permissions || typeof permissions !== "object" || Array.isArray(permissions)) {
      throw new QcError(`command-permissions in ${path} must be a table`);
    }
    if (Object.values(permissions as Record<string, unknown>).some((value) => typeof value !== "string")) {
      throw new QcError(`command-permissions in ${path} must contain direct string rules`);
    }
    for (const [pattern, action] of orderedPermissions(await readFile(path, "utf8"), path)) {
      if (typeof action !== "string" || (action !== "allow" && action !== "deny")) {
        throw new QcError(`permission '${pattern}' in ${path} must be allow or deny`);
      }
      rules.push({ pattern, action, source: path });
    }
  }
  return {
    shell: text("shell"),
    model: text("default-model"),
    thinking: text("default-thinking"),
    cli,
    rules,
    hasPermissions: permissions !== undefined,
  };
}
