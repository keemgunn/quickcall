<h1 align="center">QuickCall</h1>

<p align="center">
  <strong>Write the prompt once. Call the agent.</strong><br>
  QuickCall is a CLI for developers who run reusable Markdown prompts through headless coding agents with one command.
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/@keemgunn/quickcall"><img src="https://img.shields.io/npm/v/@keemgunn/quickcall" alt="Current QuickCall version on npm"></a>
</p>

```console
$ qc samples/joke
[=== ] ⋅ pi ⋅ <model> ⋅ 004s
"""
<one short joke from your configured agent>
"""
[SUCCESS] pi ⋅ <model> ⋅ 4s
[QC-SESSION] 260902-1430--pi--a1b2c3
```

<p align="center">
  <a href="#install"><strong>Install</strong></a> &middot;
  <a href="#quick-start"><strong>Quick start</strong></a> &middot;
  <a href="#basics"><strong>Basics</strong></a> &middot;
  <a href="USAGE.md"><strong>Usage</strong></a> &middot;
  <a href="PROVIDERS.md"><strong>Providers</strong></a> &middot;
  <a href="#support"><strong>Support</strong></a> &middot;
  <a href="#security-and-safety"><strong>Safety</strong></a>
</p>

---

## What this is

QuickCall (`qc`) turns Markdown prompt files into repeatable agent runs. It resolves a prompt, applies its settings, expands shell-output expressions, starts the selected agent in headless mode, and prints the agent's answer.

Use it when you want named prompts such as `review`, `release-notes`, or `system-status` without rebuilding a long command for each agent CLI.

QuickCall supports Pi, Cursor Agent, Claude Code, OpenCode, Antigravity, and Codex. Pi is the default. The project is pre-1.0, so commands and configuration may change between releases.

Command examples, expected output, and prompt authoring are in [USAGE.md](USAGE.md). Tool names, models, and thinking maps are in [PROVIDERS.md](PROVIDERS.md). Both files are public GitHub manuals and ship in the npm package next to this README.

QuickCall does not install agent CLIs, provide a prompt editor, or fall back to another agent when a binary is missing. Windows is not supported.

## Install

### Prerequisites

| Requirement | Why |
| --- | --- |
| Node.js 24 or newer | Selects the native `qc` binary (launcher only) |
| Unix-like environment | macOS Intel/Apple Silicon and Linux x64/ARM64 (glibc and musl). Windows is not supported |
| One supported agent CLI on `PATH` | Runs the resolved prompt |

No Rust toolchain is required to install or run QuickCall. The npm package ships prebuilt `qc` and `qc-bootstrap` binaries.

Supported agent binaries are `pi`, `agent`, `claude`, `opencode`, `agy`, and `codex`. Install and authenticate at least one before your first run.

```bash
npm install -g @keemgunn/quickcall --allow-scripts=@keemgunn/quickcall
```

```bash
qc --version
```

`--allow-scripts` lets postinstall run. Postinstall invokes the packaged `qc-bootstrap` helper: it refreshes package-owned defaults under `~/.qc/` (it does not overwrite an existing `~/.qc/config.toml` or your custom prompts) and, if `~/.agents/skills/` or `~/.claude/skills/` already contains a `qc` or `qc-*` skill directory, deletes those product directories in that destination and copies the packaged skill set there. A machine that never installed the harness stays empty until you run `qc --install-agent-harness`. If postinstall is skipped, the first valid `qc` run still repairs missing `~/.qc/` files.

The first setup creates starter settings and sample prompts under `~/.qc/`. Run `qc --install-sample-prompts` if an existing setup is missing the packaged samples.

<details>
<summary>Install with a coding agent</summary>

Give your agent this prompt:

```text
Install QuickCall from https://github.com/keemgunn/quickcall.

1. Confirm Node.js 24+ and one supported agent CLI are installed and authenticated.
2. Run: npm install -g @keemgunn/quickcall --allow-scripts=@keemgunn/quickcall
3. Verify: qc --help and qc --version
4. Run: qc --install-agent-harness
5. Do not run a prompt from an untrusted directory.
6. Report each check, destination, failure, and next command.
```

</details>

## Quick start

Install fresh copies of the packaged samples, then send one to your default agent:

```bash
qc --install-sample-prompts
qc samples/joke
```

On a TTY, stderr shows a bouncing wait bar and elapsed seconds while the agent runs. Stdout is the quoted answer, then `[SUCCESS]` and `[QC-SESSION]`. Pass `-q` for that same stdout blob with empty stderr. Packaged agent skills always pass `-q`. Hard fails print `[ERROR]` on stderr and leave stdout empty. Extra provider stderr that explains the fail is printed next. If no session was saved, the last live line is the native command (`agent`, `pi`, …) to copy.

To create your own prompt, save this as `.qc/prompts/review.md`:

```markdown
---
description: Review the current changes for correctness bugs.
---

Review the current changes. Report only correctness bugs, regressions, and missing tests.
Include file and line references for every finding.
```

Run it by filename-free alias:

```bash
qc review
qc review --append "Focus on the authentication changes."
qc review --tool cursor --model composer-2.5 --append "inspect this repo"
```

For a one-shot instruction with no prompt file, pass only `--append`:

```bash
qc --append "inspect this repo"
qc --tool cursor --model composer-2.5 --append "inspect this repo"
```

## Major features

- **Reusable prompt aliases.** Keep project prompts in `.qc/prompts/` and global prompts in `~/.qc/prompts/`, then call either with `qc <alias>`.
- **One command across six agents.** Shared flags select the tool, model, thinking level, workdir, output mode, and saved session. QuickCall translates them into each agent's native arguments. `-o` opens a stored session in that agent's own TUI.
- **Prompt-owned settings.** YAML frontmatter travels with a prompt. Global and project TOML supply defaults without copying metadata into every file.
- **Live shell context.** Exact `` !`command` `` expressions insert command stdout before the prompt reaches the agent.
- **Script-friendly output.** TTY default `text` shows a live wait on stderr, then the quoted answer on stdout. Agents and scripts pass `-q` for the one-shot blob. `--output json` prints one object.

## Basics

### Prompt lookup

| Reference | Resolution |
| --- | --- |
| `/work/prompts/review.md` | Absolute file path |
| `./notes/review.md` | File path relative to the current directory |
| `notes/review.md` | Relative file path because it ends in `.md` |
| `review` | Project alias, then global alias |
| `team/review` | Nested project alias, then global alias |

QuickCall checks only the current directory for `.qc/config.toml` and project prompt aliases. It does not search parent directories.

### Prompt frontmatter

YAML frontmatter is optional. QuickCall removes it before sending the body to the agent.

```markdown
---
description: Review the current changes.
qc_tool: claude
qc_model: sonnet
qc_thinking: high
qc_workdir: /path/to/app
---

Review the current changes for correctness bugs.
```

| Field | Effect |
| --- | --- |
| `description` | Documents the prompt |
| `qc_tool` | Selects `pi`, `cursor`, `claude`, `opencode`, `antigravity`, or `codex` |
| `qc_model` | Selects the tool's model or slug |
| `qc_thinking` | Maps a shared thinking level to the selected tool |
| `qc_workdir` | Sets the shell-expansion and agent working directory |
| `qc_no_skills` | Passes Pi's `--no-skills` when `true` |
| `qc_skill_path` | Passes Pi's `--skill <path>` |

Use an absolute `qc_workdir`; this field does not expand `~`. Use only `qc_*` metadata keys. Removed keys such as `qc_cli` and `qc_approve` fail with a rename error. See the [provider catalog](PROVIDERS.md) for valid tool names, model formats, thinking maps, and native flag behavior.

### Shell output

QuickCall recognizes only the exact form `` !`command` ``:

```markdown
Review this working tree:

!`git status --short`
```

Expressions run concurrently in the effective workdir. Their stdout replaces the expression.

## Settings

QuickCall reads global settings from `~/.qc/config.toml` and project overrides from `<cwd>/.qc/config.toml`.

```toml
shell = "/bin/sh"

[tool]
default = "pi"

[tool.pi]
default_model = "provider/model"
default_thinking = "medium"
```

First-run copies the packaged starter, which pins a lowest-cost `default_model` per tool. Confirm ids in [PROVIDERS.md](PROVIDERS.md).

`[tool.<name>] command` is the executable for that adapter. Use a PATH name or a path to a binary. It is not a shell string and does not set env vars. The child inherits the `qc` process environment. Full rules are in [USAGE.md](USAGE.md#settings).

Setting precedence is command flags, prompt frontmatter, project tool settings, global tool settings, then the built-in default.

## Agent harness

```bash
qc --install-agent-harness
```

This replace-installs the packaged `qc` skill and `/qc-create-prompt` Command Skill under `~/.agents/skills/` and `~/.claude/skills/`. It deletes existing `qc` and `qc-*` directories in those destinations, then copies the packaged set.

npm install and upgrade do the same replace-install only for a destination that already has a `qc` or `qc-*` skill. They do not install skills the first time.

Use `/qc-create-prompt` to create a project or global prompt with valid frontmatter. The core `qc` skill is the agent usage map for invoking QuickCall (stored prompts, append-only turns, sessions, and provider settings). Ground truth for user-facing behavior is this README, [USAGE.md](USAGE.md), and [PROVIDERS.md](PROVIDERS.md).


## Workflows

### Choose another agent

```bash
qc review --tool cursor --model composer-2.5
qc review --tool claude --model sonnet --thinking high
qc review --tool opencode --model opencode-go/deepseek-v4-flash
qc review --tool codex --model gpt-5.6-luna --thinking medium
```

Provider model names and thinking support differ. Check the [provider catalog](PROVIDERS.md) before saving defaults. Native Codex is `--tool codex`. Pi `openai-codex/` ids stay on `--tool pi`.

### Continue a session

Use the id printed after a run:

```bash
qc followup.md --continue 260902-1430--pi--a1b2c3
qc --continue 260902-1430--cursor--k9m2x0 --append "Also add tests."
```

The saved session locks the agent type and restores its workdir unless `--workdir` or the supplied prompt's `qc_workdir` overrides it.

### Open a session in the native TUI

Use the pretty id printed after a headless run. qc starts that provider's interactive UI. It does not force-allow tools.

```bash
qc -o
qc -o 260902-1430--pi--a1b2c3
qc -o --tool pi
```

`--tool` is the only flag allowed with `-o`. Do not pass `-q` with `-o`.

### Return JSON

```bash
qc review --output json
```

The envelope contains `tool`, `session_id`, `native_id`, `result`, `warnings`, and `exit`.

## All commands

```text
qc [<prompt-reference>] [options]
qc -o|--open [id] [--tool <name>]
qc (-h | --help)
qc (-v | --version)
qc --install-sample-prompts
qc --install-agent-harness
```

| Command or option | Purpose |
| --- | --- |
| `qc <prompt-reference>` | Resolve a prompt and run the selected agent |
| `--tool <name>` | Select an agent adapter |
| `--model <id>` | Select a native model or slug |
| `--thinking <level>` | Set the shared thinking level |
| `--workdir <path>` | Set the expansion and agent working directory |
| `--output text\|json` | Select stdout format |
| `-q`, `--quiet` | Print only the final envelope (no spinner) |
| `-c`, `--continue <id>` | Resume a saved session |
| `-o`, `--open [id]` | Open a stored session in the provider's native TUI |
| `--skill <path>` | Add a Pi skill path |
| `--no-skills` | Disable Pi skills |
| `-s`, `--shell <path>` | Select the shell for `` !`command` `` only |
| `-a`, `--append <text>` | Append instructions, or provide the whole turn when no prompt is given |
| `--install-sample-prompts` | Replace packaged sample prompts |
| `--install-agent-harness` | Replace-install packaged agent skills on both dests |
| `-h`, `--help` | Print help |
| `-v`, `--version` | Print the installed version |

A prompt reference is optional when `--append` or `-o/--open` is set. Help, version, and install flags must be used alone. `-q` is forbidden with `-o`. Full examples and expected output: [USAGE.md](USAGE.md).

## Troubleshooting

### Agent executable not found

QuickCall does not fall back to another provider. Install the binary named in the error, authenticate it, and confirm it is on `PATH`, or set `[tool.<name>] command` to a PATH name or executable path. The [provider catalog](PROVIDERS.md) maps adapter names to default binaries.

### Prompt alias not found

Check `.qc/prompts/<alias>.md` in the current directory, then `~/.qc/prompts/<alias>.md`. Use a path ending in `.md` when you want direct-path lookup.

### Sample prompt missing

```bash
qc --install-sample-prompts
```

This replaces only `~/.qc/prompts/samples/`. It does not remove your other prompts.

## Support

Use [GitHub Issues](https://github.com/keemgunn/quickcall/issues) for reproducible bugs, questions, and feature requests. Include `qc --version`, the selected provider, the command you ran, and the complete error output. Remove secrets, private paths, and sensitive prompt content first.

## Security and safety

QuickCall treats the current directory as trusted. Headless runs send prompts to agents with force-allow or auto-trust enabled where the provider supports it. `qc -o/--open` starts the provider TUI without those flags. Review the prompt, `.qc/config.toml`, working directory, and `--append` text before every run from a new project.

- Project config can select the shell used for `` !`command` `` expressions. Those expressions execute commands before the agent starts.
- QuickCall sends the final prompt directly through stdin or an argument. It never interpolates the prompt into a shell command.
- Native provider deny rules may still block an operation.

Do not run QuickCall in an untrusted directory. Do not publish suspected vulnerabilities in a public issue. This repository does not yet provide a private vulnerability-reporting policy.

## License

MIT. See [LICENSE](LICENSE).
