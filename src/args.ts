import { QcError } from "./errors.js";
import { parseHarnessFrameworks } from "./harness.js";

export interface Args {
  prompt?: string;
  shell?: string;
  append?: string;
  help: boolean;
  version: boolean;
  installSamplePrompts: boolean;
  /** Undefined when flag absent; empty array means skills only. */
  installAgentHarness?: HarnessInstallArgs;
}

export type HarnessInstallArgs = { frameworks: string[] };

const aliases = new Map([
  ["-h", "--help"],
  ["-v", "--version"],
  ["-s", "--shell"],
  ["-a", "--append"],
]);

const optionTokens = new Set([...aliases.keys(), ...aliases.values()]);

function assertExclusive(result: Args, flag: string, argv: string[]): void {
  if (result.prompt || result.shell || result.append) {
    throw new QcError(`${flag} cannot be combined with a prompt reference`);
  }
  if (result.help || result.version || result.installSamplePrompts || result.installAgentHarness !== undefined) {
    throw new QcError(`${flag} must be used alone`);
  }
}

export function parseArgs(argv: string[]): Args {
  const result: Args = { help: false, version: false, installSamplePrompts: false };

  for (let index = 0; index < argv.length; index += 1) {
    const value = aliases.get(argv[index]!) ?? argv[index]!;

    if (value === "--help") {
      if (result.help) throw new QcError("--help was provided more than once");
      assertExclusive(result, value, argv);
      result.help = true;
      continue;
    }

    if (value === "--version") {
      if (result.version) throw new QcError("--version was provided more than once");
      assertExclusive(result, value, argv);
      result.version = true;
      continue;
    }

    if (value === "--install-sample-prompts") {
      if (result.installSamplePrompts) throw new QcError("--install-sample-prompts was provided more than once");
      assertExclusive(result, value, argv);
      result.installSamplePrompts = true;
      continue;
    }

    if (value === "--install-agent-harness") {
      if (result.installAgentHarness !== undefined) {
        throw new QcError("--install-agent-harness was provided more than once");
      }
      assertExclusive(result, value, argv);
      result.installAgentHarness = { frameworks: argv.slice(index + 1) };
      break;
    }

    if (value === "--shell" || value === "--append") {
      const next = argv[++index];
      if (next === undefined || next.startsWith("--") || optionTokens.has(next) || (value === "--shell" && !next)) {
        throw new QcError(`${value} requires a value`);
      }
      const key = value === "--shell" ? "shell" : "append";
      if (result[key] !== undefined) throw new QcError(`${value} was provided more than once`);
      result[key] = next;
      continue;
    }

    if (value.startsWith("-")) throw new QcError(`unknown option '${value}'`);
    if (result.prompt !== undefined) throw new QcError("only one prompt reference is allowed");
    result.prompt = value;
  }

  if (!result.help && !result.version && !result.installSamplePrompts && result.installAgentHarness === undefined && !result.prompt) {
    throw new QcError("a prompt reference is required");
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

  if (result.installAgentHarness !== undefined) {
    parseHarnessFrameworks(result.installAgentHarness.frameworks);
  }

  return result;
}
