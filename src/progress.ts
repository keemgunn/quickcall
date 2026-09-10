import ora from "ora";
import { error, waitLine, warn } from "./messages.js";

export interface Progress {
  start(): void;
  stop(): void;
  warn(detail: string): void;
  fail(detail: string): void;
}

export type ProgressStream = NodeJS.WritableStream & { isTTY?: boolean };

export interface CreateProgressOptions {
  live: boolean;
  tool?: string;
  model?: string;
  stream?: ProgressStream;
  now?: () => number;
  setInterval?: (handler: () => void, timeout: number) => NodeJS.Timeout;
  clearInterval?: (id: NodeJS.Timeout) => void;
}

const noop: Progress = {
  start() {},
  stop() {},
  warn() {},
  fail() {},
};

/** Live stderr spinner + tagged lines, or a quiet no-op. Spinner wraps `runAgent` only. */
export function createProgress(options: CreateProgressOptions): Progress {
  if (!options.live) return noop;

  const stream = options.stream ?? process.stderr;
  const tty = Boolean(stream.isTTY);
  const now = options.now ?? Date.now;
  const schedule = options.setInterval ?? setInterval;
  const unschedule = options.clearInterval ?? clearInterval;
  const tool = options.tool ?? "";
  const model = options.model;
  const spinner = ora({
    spinner: "bouncingBar",
    text: waitLine(tool, 0, model),
    stream,
    discardStdin: false,
    isEnabled: tty,
  });

  let timer: NodeJS.Timeout | undefined;
  let startedAt = 0;

  const persist = (line: string): void => {
    if (spinner.isSpinning) {
      spinner.clear();
      stream.write(`${line}\n`);
      spinner.render();
    } else {
      stream.write(`${line}\n`);
    }
  };

  return {
    start() {
      startedAt = now();
      // isEnabled:false still prints a static line; only start ora on a TTY.
      if (tty) spinner.start();
      timer = schedule(() => {
        spinner.text = waitLine(tool, Math.floor((now() - startedAt) / 1000), model);
        if (spinner.isSpinning) spinner.render();
      }, 1000);
    },
    stop() {
      if (timer !== undefined) unschedule(timer);
      timer = undefined;
      if (spinner.isSpinning) spinner.stop();
    },
    warn(detail: string) {
      persist(warn(detail));
    },
    fail(detail: string) {
      persist(error(detail));
    },
  };
}
