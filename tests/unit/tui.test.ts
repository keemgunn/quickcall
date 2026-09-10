import { expect, it } from "vitest";
import {
  buildAntigravityTuiArgs,
  buildClaudeTuiArgs,
  buildCursorTuiArgs,
  buildOpenCodeTuiArgs,
  buildPiTuiArgs,
  buildTuiArgs,
} from "../../src/tools/tui.js";

const forbidden = [
  "--mode",
  "json",
  "-p",
  "--print",
  "-a",
  "--yolo",
  "--dangerously-skip-permissions",
  "--auto",
  "--trust",
  "--force",
  "--approve-mcps",
  "--model",
  "--thinking",
  "--workspace",
  "--dir",
  "--add-dir",
  "--output-format",
  "run",
];

it("builds per-tool TUI resume argv without json, force-allow, model, thinking, or workspace flags", () => {
  const nativeId = "native-session-1";
  expect(buildPiTuiArgs(nativeId)).toEqual(["--session-id", nativeId]);
  expect(buildCursorTuiArgs(nativeId)).toEqual(["--resume", nativeId]);
  expect(buildClaudeTuiArgs(nativeId)).toEqual(["--resume", nativeId]);
  expect(buildOpenCodeTuiArgs(nativeId)).toEqual(["-s", nativeId]);
  expect(buildAntigravityTuiArgs(nativeId)).toEqual(["--conversation", nativeId]);
  expect(buildTuiArgs("pi", nativeId)).toEqual(buildPiTuiArgs(nativeId));
  expect(buildTuiArgs("cursor", nativeId)).toEqual(buildCursorTuiArgs(nativeId));
  expect(buildTuiArgs("claude", nativeId)).toEqual(buildClaudeTuiArgs(nativeId));
  expect(buildTuiArgs("opencode", nativeId)).toEqual(buildOpenCodeTuiArgs(nativeId));
  expect(buildTuiArgs("antigravity", nativeId)).toEqual(buildAntigravityTuiArgs(nativeId));
  for (const args of [
    buildPiTuiArgs(nativeId),
    buildCursorTuiArgs(nativeId),
    buildClaudeTuiArgs(nativeId),
    buildOpenCodeTuiArgs(nativeId),
    buildAntigravityTuiArgs(nativeId),
  ]) {
    expect(args.some((token) => forbidden.includes(token))).toBe(false);
  }
});
