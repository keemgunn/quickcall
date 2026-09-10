import { QcError } from "./errors.js";
import type { OutputMode } from "./settings.js";

export interface Args {
  prompt?: string;
  shell?: string;
  append?: string;
  tool?: string;
  model?: string;
  thinking?: string;
  workdir?: string;
  output?: OutputMode;
  debug?: boolean;
  quiet?: boolean;
  continue?: string;
  /** `true` = bare `-o`; string = pretty session id. */
  open?: true | string;
  skill?: string;
  noSkills?: boolean;
  help: boolean;
  version: boolean;
  installSamplePrompts: boolean;
  installAgentHarness: boolean;
}

const aliases = new Map([
  ["-h", "--help"],
  ["-v", "--version"],
  ["-s", "--shell"],
  ["-a", "--append"],
  ["-c", "--continue"],
  ["-o", "--open"],
  ["-q", "--quiet"],
]);

const optionTokens = new Set([
  ...aliases.keys(),
  ...aliases.values(),
  "--tool",
  "--model",
  "--thinking",
  "--workdir",
  "--output",
  "--debug",
  "--quiet",
  "--skill",
  "--no-skills",
  "--install-sample-prompts",
  "--install-agent-harness",
]);

function assertExclusive(result: Args, flag: string): void {
  if (
    result.prompt ||
    result.shell ||
    result.append ||
    result.tool ||
    result.model ||
    result.thinking ||
    result.workdir ||
    result.output ||
    result.debug ||
    result.continue ||
    result.skill ||
    result.noSkills
  ) {
    throw new QcError(`${flag} cannot be combined with a prompt reference`);
  }
  if (result.help || result.version || result.installSamplePrompts || result.installAgentHarness) {
    throw new QcError(`${flag} must be used alone`);
  }
}

function assertOpenExclusive(result: Args): void {
  const conflicts: Array<[unknown, string]> = [
    [result.prompt, "a prompt reference"],
    [result.continue, "--continue"],
    [result.append, "--append"],
    [result.output, "--output"],
    [result.debug, "--debug"],
    [result.quiet, "--quiet"],
    [result.model, "--model"],
    [result.thinking, "--thinking"],
    [result.workdir, "--workdir"],
    [result.skill, "--skill"],
    [result.noSkills, "--no-skills"],
    [result.shell, "--shell"],
  ];
  for (const [present, name] of conflicts) {
    if (present) throw new QcError(`--open cannot be combined with ${name}`);
  }
}

/** Next token is an open id only when present, not dash-prefixed, and not a known option. */
function takeOptionalOpenId(argv: string[], index: number): { value?: string; next: number } {
  const next = argv[index + 1];
  if (next === undefined || next.startsWith("-") || optionTokens.has(next)) {
    return { next: index };
  }
  return { value: next, next: index + 1 };
}

function takeValue(argv: string[], index: number, flag: string): { value: string; next: number } {
  const next = argv[index + 1];
  if (next === undefined || next.startsWith("--") || optionTokens.has(next) || (flag === "--shell" && !next)) {
    throw new QcError(`${flag} requires a value`);
  }
  return { value: next, next: index + 1 };
}

export function parseArgs(argv: string[]): Args {
  const result: Args = { help: false, version: false, installSamplePrompts: false, installAgentHarness: false };

  for (let index = 0; index < argv.length; index += 1) {
    const value = aliases.get(argv[index]!) ?? argv[index]!;

    if (value === "--help") {
      if (result.help) throw new QcError("--help was provided more than once");
      assertExclusive(result, value);
      result.help = true;
      continue;
    }

    if (value === "--version") {
      if (result.version) throw new QcError("--version was provided more than once");
      assertExclusive(result, value);
      result.version = true;
      continue;
    }

    if (value === "--install-sample-prompts") {
      if (result.installSamplePrompts) throw new QcError("--install-sample-prompts was provided more than once");
      assertExclusive(result, value);
      result.installSamplePrompts = true;
      continue;
    }

    if (value === "--install-agent-harness") {
      if (result.installAgentHarness) throw new QcError("--install-agent-harness was provided more than once");
      assertExclusive(result, value);
      result.installAgentHarness = true;
      continue;
    }

    if (value === "--no-skills") {
      if (result.noSkills !== undefined) throw new QcError(`${value} was provided more than once`);
      result.noSkills = true;
      continue;
    }

    if (value === "--debug") {
      if (result.debug !== undefined) throw new QcError(`${value} was provided more than once`);
      result.debug = true;
      continue;
    }

    if (value === "--quiet") {
      if (result.quiet !== undefined) throw new QcError(`${value} was provided more than once`);
      result.quiet = true;
      continue;
    }

    if (value === "--open") {
      if (result.open !== undefined) throw new QcError(`${value} was provided more than once`);
      const taken = takeOptionalOpenId(argv, index);
      index = taken.next;
      result.open = taken.value ?? true;
      continue;
    }

    if (
      value === "--shell" ||
      value === "--append" ||
      value === "--tool" ||
      value === "--model" ||
      value === "--thinking" ||
      value === "--workdir" ||
      value === "--output" ||
      value === "--continue" ||
      value === "--skill"
    ) {
      const taken = takeValue(argv, index, value);
      index = taken.next;
      if (value === "--shell") {
        if (result.shell !== undefined) throw new QcError(`${value} was provided more than once`);
        result.shell = taken.value;
      } else if (value === "--append") {
        if (result.append !== undefined) throw new QcError(`${value} was provided more than once`);
        result.append = taken.value;
      } else if (value === "--tool") {
        if (result.tool !== undefined) throw new QcError(`${value} was provided more than once`);
        result.tool = taken.value;
      } else if (value === "--model") {
        if (result.model !== undefined) throw new QcError(`${value} was provided more than once`);
        result.model = taken.value;
      } else if (value === "--thinking") {
        if (result.thinking !== undefined) throw new QcError(`${value} was provided more than once`);
        result.thinking = taken.value;
      } else if (value === "--workdir") {
        if (result.workdir !== undefined) throw new QcError(`${value} was provided more than once`);
        result.workdir = taken.value;
      } else if (value === "--output") {
        if (result.output !== undefined) throw new QcError(`${value} was provided more than once`);
        if (taken.value !== "text" && taken.value !== "json") {
          throw new QcError(`--output must be 'text' or 'json'`);
        }
        result.output = taken.value;
      } else if (value === "--continue") {
        if (result.continue !== undefined) throw new QcError(`${value} was provided more than once`);
        result.continue = taken.value;
      } else if (value === "--skill") {
        if (result.skill !== undefined) throw new QcError(`${value} was provided more than once`);
        result.skill = taken.value;
      }
      continue;
    }

    if (value.startsWith("-")) throw new QcError(`unknown option '${value}'`);
    if (result.prompt !== undefined) throw new QcError("only one prompt reference is allowed");
    result.prompt = value;
  }

  const informational =
    result.help || result.version || result.installSamplePrompts || result.installAgentHarness;

  if (result.open !== undefined) assertOpenExclusive(result);

  // A turn needs a prompt, --append, or -o/--open. Open skips the prompt-required check.
  if (!informational && result.open === undefined && result.prompt === undefined && result.append === undefined) {
    throw new QcError("a prompt reference or --append is required");
  }

  if ((result.help || result.version) && result.prompt) {
    throw new QcError("--help and --version cannot be combined with a prompt reference");
  }

  if (result.help && result.version) {
    throw new QcError("--help and --version cannot be combined");
  }

  if ((result.help || result.version) && argv.length !== 1) {
    throw new QcError("--help and --version must be used alone");
  }

  if (result.installSamplePrompts && argv.length !== 1) {
    throw new QcError("--install-sample-prompts must be used alone");
  }

  if (result.installAgentHarness && argv.length !== 1) {
    throw new QcError("--install-agent-harness must be used alone");
  }

  return result;
}
