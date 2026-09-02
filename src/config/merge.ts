import { QcError } from "../errors.js";
import { unsupportedCli } from "../messages.js";
import type { Config } from "./parse.js";
import type { ConfigPaths } from "./paths.js";
import { readConfig } from "./parse.js";

const V1_CLI = "pi";

export async function loadConfig(paths: ConfigPaths): Promise<Config> {
  const [global, project] = await Promise.all([readConfig(paths.global), readConfig(paths.project)]);
  return {
    shell: project.shell ?? global.shell,
    model: project.model ?? global.model,
    thinking: project.thinking ?? global.thinking,
    cli: project.cli ?? global.cli,
    rules: [...global.rules, ...project.rules],
    hasPermissions: global.hasPermissions || project.hasPermissions,
  };
}

export const effectiveShell = (cliShell: string | undefined, config: Config, environment: NodeJS.ProcessEnv): string =>
  cliShell ?? config.shell ?? environment.SHELL ?? "/bin/sh";

/** Prompt qc_cli → project default-cli → global default-cli → pi. v1 rejects anything other than pi. */
export function resolveCli(promptCli: string | undefined, configCli: string | undefined): string {
  const cli = promptCli ?? configCli ?? V1_CLI;
  if (cli !== V1_CLI) throw new QcError(unsupportedCli(cli));
  return cli;
}
