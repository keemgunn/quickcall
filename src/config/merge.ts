import type { Config, ToolConfig, ToolDefaults } from "./parse.js";
import type { ConfigPaths } from "./paths.js";
import { readConfig } from "./parse.js";
import type { ToolName } from "../tools/types.js";

function mergeToolDefaults(global: ToolDefaults | undefined, project: ToolDefaults | undefined): ToolDefaults | undefined {
  if (!global && !project) return undefined;
  return {
    defaultModel: project?.defaultModel ?? global?.defaultModel,
    defaultThinking: project?.defaultThinking ?? global?.defaultThinking,
    command: project?.command ?? global?.command,
  };
}

function mergeToolConfig(global: ToolConfig, project: ToolConfig): ToolConfig {
  const tools: ToolConfig["tools"] = {};
  for (const name of ["pi", "cursor", "claude", "opencode", "antigravity"] as ToolName[]) {
    const merged = mergeToolDefaults(global.tools[name], project.tools[name]);
    if (merged) tools[name] = merged;
  }
  return {
    default: project.default ?? global.default,
    tools,
  };
}

export async function loadConfig(paths: ConfigPaths): Promise<Config> {
  const [global, project] = await Promise.all([readConfig(paths.global), readConfig(paths.project)]);
  return {
    shell: project.shell ?? global.shell,
    tool: mergeToolConfig(global.tool, project.tool),
  };
}

export const effectiveShell = (cliShell: string | undefined, config: Config, environment: NodeJS.ProcessEnv): string =>
  cliShell ?? config.shell ?? environment.SHELL ?? "/bin/sh";
