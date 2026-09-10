import { QcError } from "../errors.js";
import type { ToolName } from "./types.js";

/** Interactive resume only: no JSON, force-allow, model, thinking, or workspace flags. */
export function buildPiTuiArgs(nativeId: string): string[] {
  return ["--session-id", nativeId];
}

export function buildCursorTuiArgs(nativeId: string): string[] {
  return ["--resume", nativeId];
}

export function buildClaudeTuiArgs(nativeId: string): string[] {
  return ["--resume", nativeId];
}

export function buildOpenCodeTuiArgs(nativeId: string): string[] {
  return ["-s", nativeId];
}

export function buildAntigravityTuiArgs(nativeId: string): string[] {
  return ["--conversation", nativeId];
}

export function buildTuiArgs(tool: ToolName, nativeId: string): string[] {
  switch (tool) {
    case "pi":
      return buildPiTuiArgs(nativeId);
    case "cursor":
      return buildCursorTuiArgs(nativeId);
    case "claude":
      return buildClaudeTuiArgs(nativeId);
    case "opencode":
      return buildOpenCodeTuiArgs(nativeId);
    case "antigravity":
      return buildAntigravityTuiArgs(nativeId);
    default: {
      const _exhaustive: never = tool;
      throw new QcError(`unknown tool '${String(_exhaustive)}'`);
    }
  }
}
