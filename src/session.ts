import { randomBytes } from "node:crypto";
import { existsSync, realpathSync } from "node:fs";
import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { QcError } from "./errors.js";
import { isToolName, type ToolName } from "./tools/types.js";

export interface SessionMapping {
  tool: ToolName;
  native_id: string;
  cwd: string;
  created: string;
  updated: string;
  warnings: string[];
}

const SESSION_ID_RE = /^(\d{6}-\d{4})--([a-z]+)--([a-z0-9]{6})$/;

function pad(value: number, width: number): string {
  return String(value).padStart(width, "0");
}

/** Local-time pretty id: `yymmdd-hhmm--<tool>--<6 lowercase alnum>`. */
export function mintSessionId(tool: ToolName, now = new Date()): string {
  const yy = pad(now.getFullYear() % 100, 2);
  const mm = pad(now.getMonth() + 1, 2);
  const dd = pad(now.getDate(), 2);
  const hh = pad(now.getHours(), 2);
  const min = pad(now.getMinutes(), 2);
  const suffix = randomBytes(4).toString("base64url").toLowerCase().replace(/[^a-z0-9]/g, "").slice(0, 6);
  const filled = (suffix + "a1b2c3").slice(0, 6);
  return `${yy}${mm}${dd}-${hh}${min}--${tool}--${filled}`;
}

export function parseSessionId(id: string): { tool: ToolName; stamp: string; suffix: string } {
  const match = SESSION_ID_RE.exec(id);
  if (!match) throw new QcError(`invalid session id '${id}'`);
  const tool = match[2]!;
  if (!isToolName(tool)) throw new QcError(`invalid session id tool '${tool}'`);
  return { tool, stamp: match[1]!, suffix: match[3]! };
}

export function sessionsDir(home: string): string {
  return join(home, ".qc", "sessions");
}

export function sessionPath(home: string, id: string): string {
  return join(sessionsDir(home), `${id}.json`);
}

export async function loadSession(home: string, id: string): Promise<SessionMapping> {
  parseSessionId(id);
  let raw: string;
  try {
    raw = await readFile(sessionPath(home, id), "utf8");
  } catch (cause: unknown) {
    if ((cause as NodeJS.ErrnoException).code === "ENOENT") throw new QcError(`session not found: ${id}`);
    throw cause;
  }
  let parsed: SessionMapping;
  try {
    parsed = JSON.parse(raw) as SessionMapping;
  } catch {
    throw new QcError(`corrupt session mapping: ${id}`);
  }
  if (!isToolName(parsed.tool) || typeof parsed.native_id !== "string" || !parsed.native_id) {
    throw new QcError(`corrupt session mapping: ${id}`);
  }
  if (typeof parsed.cwd !== "string" || !parsed.cwd) throw new QcError(`corrupt session mapping: ${id}`);
  return {
    tool: parsed.tool,
    native_id: parsed.native_id,
    cwd: parsed.cwd,
    created: typeof parsed.created === "string" ? parsed.created : "",
    updated: typeof parsed.updated === "string" ? parsed.updated : "",
    warnings: Array.isArray(parsed.warnings) ? parsed.warnings.map(String) : [],
  };
}

export async function saveSession(home: string, id: string, mapping: SessionMapping): Promise<void> {
  parseSessionId(id);
  await mkdir(sessionsDir(home), { recursive: true });
  await writeFile(sessionPath(home, id), `${JSON.stringify(mapping, null, 2)}\n`, "utf8");
}

/**
 * Canonical cwd for session matching: path.resolve, then realpath when the path exists.
 * Relative stored cwd resolves against the current process cwd. No ancestor walk.
 */
export function cwdKey(cwd: string): string {
  const resolved = resolve(cwd);
  if (!existsSync(resolved)) return resolved;
  try {
    return realpathSync(resolved);
  } catch {
    return resolved;
  }
}

export interface FoundSession {
  id: string;
  mapping: SessionMapping;
}

function newerSession(a: FoundSession, b: FoundSession): FoundSession {
  if (a.mapping.updated !== b.mapping.updated) return a.mapping.updated > b.mapping.updated ? a : b;
  if (a.mapping.created !== b.mapping.created) return a.mapping.created > b.mapping.created ? a : b;
  return a.id >= b.id ? a : b;
}

/** Scan ~/.qc/sessions/*.json; skip unreadable/corrupt files; pick newest updated then created then id. */
export async function findLatestSession(home: string, cwd: string, tool?: ToolName): Promise<FoundSession> {
  const key = cwdKey(cwd);
  let names: string[];
  try {
    names = await readdir(sessionsDir(home));
  } catch (cause: unknown) {
    if ((cause as NodeJS.ErrnoException).code === "ENOENT") {
      throw new QcError(tool ? `no ${tool} session found for this directory` : "no session found for this directory");
    }
    throw cause;
  }

  let best: FoundSession | undefined;
  for (const name of names) {
    if (!name.endsWith(".json")) continue;
    const id = name.slice(0, -".json".length);
    try {
      const mapping = await loadSession(home, id);
      if (cwdKey(mapping.cwd) !== key) continue;
      if (tool !== undefined && mapping.tool !== tool) continue;
      const row = { id, mapping };
      best = best ? newerSession(best, row) : row;
    } catch {
      // Scan stays open: unreadable, corrupt, or invalid id files are skipped.
      continue;
    }
  }

  if (!best) {
    throw new QcError(tool ? `no ${tool} session found for this directory` : "no session found for this directory");
  }
  return best;
}
