import { spawn, type ChildProcess } from "node:child_process";
import { constants } from "node:os";
import { QcError } from "../errors.js";
import { missingBinary } from "../messages.js";

export interface SpawnCapture {
  stdout: string;
  stderr: string;
  exit: number;
}

const DRAIN_MS = 2_000;
const KILL_GRACE_MS = 1_000;

function signalGroup(child: ChildProcess, signal: NodeJS.Signals): void {
  if (child.pid === undefined) return;
  try {
    process.kill(-child.pid, signal);
  } catch {
    try {
      child.kill(signal);
    } catch {
      // Process group already gone.
    }
  }
}

function destroyPipes(child: ChildProcess): void {
  child.stdin?.destroy();
  child.stdout?.destroy();
  child.stderr?.destroy();
}

function reapGroup(child: ChildProcess): void {
  destroyPipes(child);
  signalGroup(child, "SIGTERM");
  setTimeout(() => {
    signalGroup(child, "SIGKILL");
  }, KILL_GRACE_MS).unref();
}

function exitFromChild(child: ChildProcess, code: number | null, signal: NodeJS.Signals | null): number {
  const resolvedCode = code ?? child.exitCode;
  const resolvedSignal = signal ?? child.signalCode;
  return resolvedCode ?? (resolvedSignal ? 128 + constants.signals[resolvedSignal] : 1);
}

/** Spawn a tool binary, feed stdin, capture stdout and stderr. Never shells the prompt. */
export function spawnAgent(
  command: string,
  args: string[],
  prompt: string | undefined,
  cwd: string,
  environment: NodeJS.ProcessEnv,
  toolLabel: string,
): Promise<SpawnCapture> {
  return new Promise((resolve, reject) => {
    let child: ChildProcess;
    try {
      child = spawn(command, args, {
        cwd,
        env: environment,
        stdio: [prompt === undefined ? "ignore" : "pipe", "pipe", "pipe"],
        detached: true,
      });
    } catch {
      reject(new QcError(missingBinary(toolLabel, command)));
      return;
    }

    let stdout = "";
    let stderr = "";
    child.stdout!.setEncoding("utf8");
    child.stdout!.on("data", (chunk: string) => {
      stdout += chunk;
    });
    // Buffer stderr until the child exits so qc can print one envelope (or one [ERROR]).
    // Piping live would leak provider chatter (Pi "No project session found…") before the result.
    child.stderr!.setEncoding("utf8");
    child.stderr!.on("data", (chunk: string) => {
      stderr += chunk;
    });

    const forwardInt = () => {
      signalGroup(child, "SIGINT");
    };
    const forwardTerm = () => {
      signalGroup(child, "SIGTERM");
    };
    const cleanup = () => {
      process.off("SIGINT", forwardInt);
      process.off("SIGTERM", forwardTerm);
    };

    let settled = false;
    const finish = (outcome: SpawnCapture | QcError): void => {
      if (settled) return;
      settled = true;
      cleanup();
      if (outcome instanceof QcError) reject(outcome);
      else resolve(outcome);
    };

    const capture = (code: number | null, signal: NodeJS.Signals | null): SpawnCapture => ({
      stdout,
      stderr,
      exit: exitFromChild(child, code, signal),
    });

    let drainTimer: NodeJS.Timeout | undefined;
    let exitCode: number | null = null;
    let exitSignal: NodeJS.Signals | null = null;

    process.once("SIGINT", forwardInt);
    process.once("SIGTERM", forwardTerm);

    child.once("error", (cause: NodeJS.ErrnoException) => {
      finish(new QcError(cause.code === "ENOENT" ? missingBinary(toolLabel, command) : `could not start ${command}: ${cause.message}`));
    });
    child.once("exit", (code, signal) => {
      exitCode = code;
      exitSignal = signal;
      drainTimer = setTimeout(() => {
        reapGroup(child);
        finish(capture(exitCode, exitSignal));
      }, DRAIN_MS);
    });
    child.once("close", () => {
      if (drainTimer) clearTimeout(drainTimer);
      reapGroup(child);
      finish(capture(exitCode, exitSignal));
    });

    if (prompt !== undefined) child.stdin!.end(prompt);
  });
}

/**
 * Silent interactive spawn: inherit stdio so qc prints nothing after the child starts.
 * SIGINT/SIGTERM are forwarded; the child's exit code is returned.
 */
export function spawnInteractive(
  command: string,
  args: string[],
  cwd: string,
  environment: NodeJS.ProcessEnv,
  toolLabel: string,
  onSpawn?: () => Promise<void>,
): Promise<number> {
  return new Promise((resolve, reject) => {
    let child: ChildProcess;
    try {
      child = spawn(command, args, {
        cwd,
        env: environment,
        stdio: "inherit",
      });
    } catch {
      reject(new QcError(missingBinary(toolLabel, command)));
      return;
    }

    const forwardInt = () => {
      child.kill("SIGINT");
    };
    const forwardTerm = () => {
      child.kill("SIGTERM");
    };
    const cleanup = () => {
      process.off("SIGINT", forwardInt);
      process.off("SIGTERM", forwardTerm);
    };

    let settled = false;
    const finish = (outcome: number | QcError): void => {
      if (settled) return;
      settled = true;
      cleanup();
      if (outcome instanceof QcError) reject(outcome);
      else resolve(outcome);
    };

    process.once("SIGINT", forwardInt);
    process.once("SIGTERM", forwardTerm);

    let started = Promise.resolve();
    child.once("spawn", () => {
      started = onSpawn ? onSpawn() : Promise.resolve();
    });
    child.once("error", (cause: NodeJS.ErrnoException) => {
      finish(new QcError(cause.code === "ENOENT" ? missingBinary(toolLabel, command) : `could not start ${command}: ${cause.message}`));
    });
    child.once("close", (code, signal) => {
      const exit = code ?? (signal ? 128 + constants.signals[signal] : 1);
      void started.then(
        () => finish(exit),
        (cause: unknown) => finish(cause instanceof QcError ? cause : new QcError(cause instanceof Error ? cause.message : String(cause))),
      );
    });
  });
}
