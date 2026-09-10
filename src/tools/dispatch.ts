import type { AgentRequest, AgentResult, ToolName } from "./types.js";
import { runPi } from "./pi.js";
import { runCursor } from "./cursor.js";
import { runClaude } from "./claude.js";
import { runOpenCode } from "./opencode.js";
import { runAntigravity } from "./antigravity.js";
import { QcError } from "../errors.js";

/** Deep seam: one call site for every headless JSON adapter. */
export async function runAgent(tool: ToolName, request: AgentRequest): Promise<AgentResult> {
  switch (tool) {
    case "pi":
      return runPi(request);
    case "cursor":
      return runCursor(request);
    case "claude":
      return runClaude(request);
    case "opencode":
      return runOpenCode(request);
    case "antigravity":
      return runAntigravity(request);
    default: {
      const _exhaustive: never = tool;
      throw new QcError(`unknown tool '${String(_exhaustive)}'`);
    }
  }
}
