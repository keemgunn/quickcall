import { Writable } from "node:stream";
import { expect, it } from "vitest";
import { createProgress } from "../../src/progress.js";

function capturingStream(isTTY = false): { stream: Writable & { isTTY: boolean }; chunks: string[] } {
  const chunks: string[] = [];
  const stream = new Writable({
    write(chunk, _encoding, callback) {
      chunks.push(String(chunk));
      callback();
    },
  }) as Writable & {
    isTTY: boolean;
    cursorTo: (x: number, y?: number) => boolean;
    moveCursor: (dx: number, dy: number) => boolean;
    clearLine: (dir: number) => boolean;
    columns: number;
    rows: number;
  };
  stream.isTTY = isTTY;
  stream.cursorTo = () => true;
  stream.moveCursor = () => true;
  stream.clearLine = () => true;
  stream.columns = 80;
  stream.rows = 24;
  return { stream, chunks };
}

it("writes live warnings immediately and quiet writes nothing", () => {
  const live = capturingStream(false);
  const liveProgress = createProgress({ live: true, stream: live.stream });
  liveProgress.warn("qc_thinking is ignored for tool 'cursor' (encode effort in the model slug)");
  expect(live.chunks.join("")).toBe(
    "[WARNING] qc_thinking is ignored for tool 'cursor' (encode effort in the model slug)\n",
  );

  const quiet = capturingStream(true);
  const quietProgress = createProgress({ live: false, stream: quiet.stream });
  quietProgress.start();
  quietProgress.warn("should not appear");
  quietProgress.fail("should not appear either");
  quietProgress.stop();
  expect(quiet.chunks.join("")).toBe("");
});

it("shows tool and elapsed on the live wait line", () => {
  const { stream, chunks } = capturingStream(true);
  let current = 0;
  let tick: () => void = () => undefined;
  const progress = createProgress({
    live: true,
    tool: "pi",
    stream,
    now: () => current,
    setInterval: (fn) => {
      tick = () => {
        fn();
      };
      return 0 as unknown as NodeJS.Timeout;
    },
    clearInterval: () => undefined,
  });
  progress.start();
  try {
    expect(chunks.join("")).toContain(" ⋅ pi ⋅ 000s");
    current = 12_000;
    tick();
    expect(chunks.join("")).toContain(" ⋅ pi ⋅ 012s");
  } finally {
    progress.stop();
  }
});

it("shows tool, model, and elapsed on the live wait line", () => {
  const { stream, chunks } = capturingStream(true);
  let current = 0;
  let tick: () => void = () => undefined;
  const progress = createProgress({
    live: true,
    tool: "cursor",
    model: "composer-2.5",
    stream,
    now: () => current,
    setInterval: (fn) => {
      tick = () => {
        fn();
      };
      return 0 as unknown as NodeJS.Timeout;
    },
    clearInterval: () => undefined,
  });
  progress.start();
  try {
    expect(chunks.join("")).toContain(" ⋅ cursor ⋅ composer-2.5 ⋅ 000s");
    current = 12_000;
    tick();
    expect(chunks.join("")).toContain(" ⋅ cursor ⋅ composer-2.5 ⋅ 012s");
  } finally {
    progress.stop();
  }
});
