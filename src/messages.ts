import packageJson from "#package.json" with { type: "json" };

export const version = packageJson.version;

export const help = `Usage: qc <prompt-reference> [--shell <path>] [--append <text>]
       qc --install-sample-prompts
       qc --install-agent-harness [<framework> ...]

Run a Markdown prompt through Pi print mode.

Options:
  -s, --shell <path>   Shell used only for !\`command\` substitutions
  -a, --append <text>  Append a user message before shell expansion
      --install-sample-prompts
                       Replace ~/.qc/prompts/samples/ from packaged defaults
      --install-agent-harness [<framework> ...]
                       Install packaged agent harness commands and/or skill
  -h, --help           Show this help
  -v, --version        Show the qc version`;

export const error = (detail: string) => `qc: error: ${detail}`;
export const missingPi = "Pi executable 'pi' was not found on PATH. Install Pi and try again.";
export const unsupportedCli = (value: string) =>
  `unsupported cli runtime '${value}'; v1 only supports 'pi'`;
