import { spawn } from "node:child_process";
import { access, stat } from "node:fs/promises";
import { constants } from "node:fs";
import { delimiter, isAbsolute, join } from "node:path";
import type { Config } from "./config/index.js";
import { QcError } from "./errors.js";
interface Expression { start: number; end: number; command: string; }
export function expressions(text: string): Expression[] {
  const found: Expression[] = [];
  for (let index = 0; index < text.length - 2; index += 1) {
    if (text[index] !== "!" || text[index + 1] !== "`") continue;
    let cursor = index + 2; let command = ""; let closed = false;
    while (cursor < text.length) { if (text[cursor] === "\\" && cursor + 1 < text.length) { command += text.slice(cursor, cursor + 2); cursor += 2; continue; } if (text[cursor] === "`") { closed = true; break; } command += text[cursor++]!; }
    if (!closed) continue; found.push({ start: index, end: cursor + 1, command }); index = cursor;
  }
  return found;
}
async function executable(shell: string, environment: NodeJS.ProcessEnv): Promise<void> {
  const candidates = isAbsolute(shell) || shell.includes("/") ? [shell] : (environment.PATH ?? "").split(delimiter).map((part) => join(part, shell));
  let selected: string | undefined;
  for (const candidate of candidates) try { if ((await stat(candidate)).isFile()) { await access(candidate, constants.X_OK); selected = candidate; break; } } catch { /* next PATH entry */ }
  if (!selected) throw new QcError(`shell executable could not be started: ${shell}`);
  await new Promise<void>((resolve, reject) => {
    const child = spawn(shell, ["-c", ":"], { env: environment, stdio: ["ignore", "ignore", "pipe"] });
    child.stderr.on("data", () => { /* discard probe diagnostics */ });
    child.once("error", () => reject(new QcError(`shell executable could not be started: ${shell}`)));
    child.once("close", (code) => code === 0 ? resolve() : reject(new QcError(`shell executable could not be started: ${shell}`)));
  });
}
function run(shell: string, command: string, cwd: string, environment: NodeJS.ProcessEnv): Promise<string> {
  return new Promise((resolve, reject) => { const child = spawn(shell, ["-c", command], { cwd, env: environment, stdio: ["ignore", "pipe", "pipe"] }); let stdout = ""; child.stdout.on("data", (chunk: Buffer) => { stdout += chunk.toString(); }); child.stderr.on("data", () => { /* discard command diagnostics without backpressure */ }); child.once("error", () => reject(new QcError(`shell executable could not be started: ${shell}`))); child.once("close", () => resolve(stdout)); });
}
export async function expandShell(text: string, shell: string, _config: Config, cwd: string, environment: NodeJS.ProcessEnv): Promise<string> {
  // _config kept for call-site compatibility; permission authorize/preflight was removed.
  const found = expressions(text);
  await executable(shell, environment); if (!found.length) return text;
  const outputs = await Promise.all(found.map((expression) => run(shell, expression.command, cwd, environment)));
  let output = ""; let cursor = 0; found.forEach((expression, index) => { output += text.slice(cursor, expression.start) + outputs[index]!; cursor = expression.end; }); return output + text.slice(cursor);
}
