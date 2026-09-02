#!/usr/bin/env node
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { parseArgs } from "./args.js";
import {
  bootstrapSettings,
  configPaths,
  effectiveShell,
  loadConfig,
  resolveCli,
  installSamplePrompts,
} from "./config/index.js";
import { QcError } from "./errors.js";
import { installAgentHarness, parseHarnessFrameworks } from "./harness.js";
import { error, help, version } from "./messages.js";
import { appendPrompt, readPrompt } from "./prompt.js";
import { piArgs, runPi } from "./pi.js";
import { expandShell } from "./shell-output.js";

function starterPath(importMetaUrl: string): string {
  return join(dirname(fileURLToPath(importMetaUrl)), "..", "share", "settings", "config.toml");
}

export async function main(argv = process.argv.slice(2), environment = process.env): Promise<number> {
  const args = parseArgs(argv);
  const cwd = process.cwd();
  const home = environment.HOME;
  if (!home) throw new QcError("HOME is required to locate qc configuration");

  const paths = configPaths(home, cwd, starterPath(import.meta.url));

  if (args.installSamplePrompts) {
    await installSamplePrompts(paths);
    return 0;
  }

  if (args.installAgentHarness !== undefined) {
    const frameworks = parseHarnessFrameworks(args.installAgentHarness.frameworks);
    await installAgentHarness({
      frameworks,
      installSkills: true,
      home,
    });
    return 0;
  }

  await bootstrapSettings("repair", paths);

  if (args.help) {
    process.stdout.write(`${help}\n`);
    return 0;
  }

  if (args.version) {
    process.stdout.write(`${version}\n`);
    return 0;
  }

  const config = await loadConfig(paths);
  const prompt = await readPrompt(args.prompt!, cwd, home);
  resolveCli(prompt.meta.cli, config.cli);
  const complete = appendPrompt(prompt.body, args.append);
  const expanded = await expandShell(
    complete,
    effectiveShell(args.shell, config, environment),
    config,
    cwd,
    environment,
  );
  return runPi(expanded, piArgs(prompt.meta, config.model, config.thinking), cwd, environment);
}

main()
  .then((code) => {
    process.exitCode = code;
  })
  .catch((cause: unknown) => {
    process.stderr.write(`${error(cause instanceof Error ? cause.message : String(cause))}\n`);
    process.exitCode = cause instanceof QcError ? cause.exitCode : 1;
  });
