#!/usr/bin/env node
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { parseArgs } from "./args.js";
import {
  bootstrapSettings,
  configPaths,
  effectiveShell,
  loadConfig,
  installSamplePrompts,
  type Config,
} from "./config/index.js";
import { QcError } from "./errors.js";
import { installAgentHarness } from "./harness.js";
import { emptyAssistantText, error, failOutput, help, interrupted, inspectCommand, openSessionCommand, sessionLine, textEnvelope, usageFields, version } from "./messages.js";
import { createProgress } from "./progress.js";
import { appendPrompt, readPrompt, type PromptMeta } from "./prompt.js";
import { expandShell } from "./shell-output.js";
import { findLatestSession, loadSession, mintSessionId, saveSession, type SessionMapping } from "./session.js";
import { resolveSettings } from "./settings.js";
import { runAgent } from "./tools/dispatch.js";
import { spawnInteractive } from "./tools/spawn.js";
import { buildTuiArgs } from "./tools/tui.js";
import { DEFAULT_COMMANDS, isToolName, type AgentResult, type ToolName } from "./tools/types.js";

function starterPath(importMetaUrl: string): string {
  return join(dirname(fileURLToPath(importMetaUrl)), "..", "share", "settings", "config.toml");
}

async function resolveOpenMapping(
  open: true | string,
  toolFlag: string | undefined,
  home: string,
  cwd: string,
): Promise<{ id: string; mapping: SessionMapping }> {
  if (toolFlag !== undefined) {
    if (!isToolName(toolFlag)) throw new QcError(`unknown tool '${toolFlag}'`);
  }
  const tool = toolFlag;
  if (open === true) {
    return findLatestSession(home, cwd, tool);
  }
  const mapping = await loadSession(home, open);
  if (toolFlag !== undefined && toolFlag !== mapping.tool) {
    throw new QcError(`tool mismatch: session is '${mapping.tool}' but resolved tool is '${toolFlag}'`);
  }
  return { id: open, mapping };
}

async function openSession(
  open: true | string,
  toolFlag: string | undefined,
  home: string,
  cwd: string,
  config: Config,
  environment: NodeJS.ProcessEnv,
): Promise<number> {
  const { id, mapping } = await resolveOpenMapping(open, toolFlag, home, cwd);
  const command = config.tool.tools[mapping.tool]?.command ?? DEFAULT_COMMANDS[mapping.tool];
  return spawnInteractive(
    command,
    buildTuiArgs(mapping.tool, mapping.native_id),
    mapping.cwd,
    environment,
    mapping.tool,
    async () => {
      await saveSession(home, id, { ...mapping, updated: new Date().toISOString() });
    },
  );
}

/** Replace the prompt token so `--debug` argv never dumps the user turn. */
function redactArgv(argv: string[], prompt: string): string[] {
  return argv.map((token) => (token === prompt ? "<prompt>" : token));
}

function debugFacts(input: {
  tool: string;
  model?: string;
  command: string;
  argv: string[];
  prompt: string;
  cwd: string;
  exit: number;
  stdoutBytes: number;
  stderr: string;
  nativeId: string;
  durationS: number;
}): string[] {
  const lines = [
    `tool=${input.tool}`,
    `model=${input.model ?? ""}`,
    `command=${input.command}`,
    `argv=${redactArgv(input.argv, input.prompt).join(" ")}`,
    `cwd=${input.cwd}`,
    `child_exit=${input.exit}`,
    `stdout_bytes=${input.stdoutBytes} stderr_bytes=${input.stderr.length}`,
    `native_id=${input.nativeId}`,
    `elapsed_s=${input.durationS}`,
  ];
  if (input.stderr) lines.push(`child_stderr:\n${input.stderr.replace(/\n$/, "")}`);
  return lines;
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

  if (args.installAgentHarness) {
    await installAgentHarness({ home });
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

  // Open path: mapping + configured binary + silent TUI spawn. No prompt/expand/envelope.
  if (args.open !== undefined) {
    return openSession(args.open, args.tool, home, cwd, config, environment);
  }

  let meta: PromptMeta = {};
  let body = "";
  if (args.prompt) {
    const prompt = await readPrompt(args.prompt, cwd, home);
    meta = prompt.meta;
    body = prompt.body;
  }

  let lockedTool: ToolName | undefined;
  let nativeId: string | undefined;
  let sessionId: string | undefined;
  let sessionCwd: string | undefined;
  let sessionCreated: string | undefined;
  let priorWarnings: string[] = [];

  if (args.continue) {
    const mapping = await loadSession(home, args.continue);
    lockedTool = mapping.tool;
    nativeId = mapping.native_id;
    sessionId = args.continue;
    sessionCwd = mapping.cwd;
    sessionCreated = mapping.created;
    priorWarnings = mapping.warnings;
  }

  const settings = resolveSettings(
    {
      tool: args.tool,
      model: args.model,
      thinking: args.thinking,
      workdir: args.workdir,
      noSkills: args.noSkills,
      skillPath: args.skill,
      output: args.output,
    },
    meta,
    config,
    lockedTool,
  );

  // Workdir: flag/frontmatter override session cwd; else session cwd on continue; else invocation cwd.
  const workdir = settings.workdir ?? sessionCwd ?? cwd;

  // No prompt file → --append is the whole user turn (raw). Prompt present → wrap.
  const rawAppend = args.prompt === undefined;
  const complete = appendPrompt(body, args.append, rawAppend);
  const expanded = await expandShell(
    complete,
    effectiveShell(args.shell, config, environment),
    config,
    workdir,
    environment,
  );

  const prettyId = sessionId ?? mintSessionId(settings.tool);
  const created = new Date().toISOString();

  const live = settings.output === "text" && !args.quiet;
  const progress = createProgress({ live, tool: settings.tool, model: settings.model });
  if (live) {
    for (const warning of settings.warnings) progress.warn(warning);
  }

  const started = Date.now();
  let agent: AgentResult;
  progress.start();
  try {
    agent = await runAgent(settings.tool, {
      prompt: expanded,
      model: settings.model,
      thinking: settings.thinking,
      workdir,
      sessionId: prettyId,
      nativeId,
      noSkills: settings.noSkills,
      skillPath: settings.skillPath,
      command: settings.command,
      env: environment,
    });
  } finally {
    progress.stop();
  }
  const durationS = Math.round((Date.now() - started) / 1000);
  const failed = Boolean(agent.error) || !agent.text.trim();
  const warnings = [...priorWarnings, ...settings.warnings];

  if (agent.nativeId) {
    await saveSession(home, prettyId, {
      tool: settings.tool,
      native_id: agent.nativeId,
      cwd: workdir,
      created: sessionCreated ?? created,
      updated: new Date().toISOString(),
      warnings,
    });
  }

  if (failed) {
    const primary =
      agent.error ||
      (agent.exit >= 128 ? interrupted(agent.exit - 128) : "") ||
      emptyAssistantText(settings.tool);
    process.stderr.write(failOutput(primary, agent.stderr));
    if (agent.nativeId) {
      process.stderr.write(`${sessionLine(prettyId)}\n`);
      if (live) process.stderr.write(`${openSessionCommand(prettyId)}\n`);
    } else if (live) {
      process.stderr.write(`${inspectCommand(settings.command)}\n`);
    }
    return agent.exit >= 128 ? agent.exit : agent.exit || 1;
  }

  const debugLines = args.debug
    ? debugFacts({
        tool: settings.tool,
        model: settings.model,
        command: settings.command,
        argv: agent.argv,
        prompt: expanded,
        cwd: workdir,
        exit: agent.exit,
        stdoutBytes: agent.stdoutBytes,
        stderr: agent.stderr,
        nativeId: agent.nativeId,
        durationS,
      })
    : undefined;
  if (debugLines && !usageFields(agent.usage)) debugLines.push("usage_absent");

  // One stdout write on success. Hard fails never reach here (QcError → catch → stderr [ERROR]).
  if (settings.output === "json") {
    const envelope: Record<string, unknown> = {
      tool: settings.tool,
      session_id: prettyId,
      native_id: agent.nativeId,
      result: agent.result,
      warnings,
      exit: agent.exit,
      model: settings.model ?? null,
      duration_s: durationS,
    };
    const usage = usageFields(agent.usage);
    if (usage) envelope.usage = usage;
    if (debugLines) envelope.debug = debugLines;
    process.stdout.write(`${JSON.stringify(envelope, null, 2)}\n`);
  } else {
    process.stdout.write(
      textEnvelope({
        text: agent.text,
        tool: settings.tool,
        durationS,
        sessionId: prettyId,
        model: settings.model,
        warnings: settings.warnings,
        debugLines,
        usage: agent.usage,
      }),
    );
  }

  return agent.exit;
}

main()
  .then((code) => {
    process.exitCode = code;
  })
  .catch((cause: unknown) => {
    process.stderr.write(`${error(cause instanceof Error ? cause.message : String(cause))}\n`);
    process.exitCode = cause instanceof QcError ? cause.exitCode : 1;
  });
