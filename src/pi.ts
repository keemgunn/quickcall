import { spawn, type ChildProcess } from "node:child_process";
import { constants } from "node:os";
import { QcError } from "./errors.js";
import { missingPi } from "./messages.js";
import type { PromptMeta } from "./prompt.js";
export function piArgs(meta: PromptMeta, model?: string, thinking?: string): string[] {
  const args = ["-p"]; if (meta.model ?? model) args.push("--model", meta.model ?? model!); if (meta.thinking ?? thinking) args.push("--thinking", meta.thinking ?? thinking!); if (meta.noSkills) args.push("--no-skills"); if (meta.skillPath) args.push("--skill", meta.skillPath); if (meta.approve === true) args.push("--approve"); if (meta.approve === false) args.push("--no-approve"); return args;
}
export function runPi(prompt: string, args: string[], cwd: string, environment: NodeJS.ProcessEnv): Promise<number> {
  return new Promise((resolve, reject) => {
    let child: ChildProcess; try { child = spawn("pi", args, { cwd, env: environment, stdio: ["pipe", "pipe", "pipe"] }); } catch { reject(new QcError(missingPi)); return; }
    child.stdout!.pipe(process.stdout); child.stderr!.pipe(process.stderr);
    const forwardInt = () => { child.kill("SIGINT"); }; const forwardTerm = () => { child.kill("SIGTERM"); };
    const cleanup = () => { process.off("SIGINT", forwardInt); process.off("SIGTERM", forwardTerm); };
    let settled = false;
    const finish = (result: number | QcError): void => { if (settled) return; settled = true; cleanup(); result instanceof QcError ? reject(result) : resolve(result); };
    process.once("SIGINT", forwardInt); process.once("SIGTERM", forwardTerm);
    child.once("error", (cause: NodeJS.ErrnoException) => finish(new QcError(cause.code === "ENOENT" ? missingPi : `could not start Pi: ${cause.message}`)));
    child.once("close", (code, signal) => finish(code ?? (signal ? 128 + constants.signals[signal] : 1))); child.stdin!.end(prompt);
  });
}
