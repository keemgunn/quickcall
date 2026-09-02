<h1 align="center">QuickCall</h1>

<p align="center">
  <strong>Reusable Markdown prompts, one command to Pi.</strong><br>
  Resolve a prompt, apply settings, expand shell output, and send the result to Pi print mode.
</p>

<p align="center">
  <a href="#what-this-is"><strong>What this is</strong></a> ·
  <a href="#install"><strong>Install</strong></a> ·
  <a href="#quick-start"><strong>Quick start</strong></a> ·
  <a href="#basics"><strong>Basics</strong></a> ·
  <a href="#settings"><strong>Settings</strong></a> ·
  <a href="#agent-harness"><strong>Agent harness</strong></a> ·
  <a href="#workflows"><strong>Workflows</strong></a> ·
  <a href="#all-commands"><strong>Commands</strong></a> ·
  <a href="#safety-notes"><strong>Safety</strong></a>
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/@keemgunn/quickcall"><img src="https://img.shields.io/npm/v/@keemgunn/quickcall" alt="npm version"></a>
  <img src="https://img.shields.io/badge/node-%3E%3D24-339933" alt="Node.js 24+">
</p>

---

## What this is

QuickCall (`qc`) is a small CLI wrapper around an existing [Pi](https://pi.dev) installation. It turns Markdown prompt files into repeatable Pi invocations:

- Resolve prompts by alias or direct path.
- Apply YAML frontmatter and layered TOML config for Pi model, thinking, skills, and approval flags.
- Optionally append one-time user instructions.
- Expand approved shell-output expressions (`!\`command\``) into the prompt body.
- Send the final text to Pi through stdin (never through a shell command).

QuickCall does not install Pi, host a prompt editor, or walk parent directories for config. Windows is outside v1.

## Install

### Prerequisites

| Requirement | Purpose |
| --- | --- |
| Node.js 24 or newer | Runs the `qc` CLI |
| Unix-like environment | v1 runtime target |
| `pi` on `PATH` | Pi print mode target ([Pi quickstart](https://pi.dev/docs/latest/quickstart)) |

### For humans

```bash
npm install -g @keemgunn/quickcall --allow-scripts=@keemgunn/quickcall
qc --version
```

Postinstall refreshes package-owned `~/.qc/.default-settings/` and `.gitignore`, seeds missing `~/.qc/config.toml`, hard-refreshes missing `~/.qc/prompts/samples/`, and runs harness `refresh-known` for previously installed destinations only. It does not auto-install harness files on first npm install.

The package may not be on npm yet. Once `@keemgunn/quickcall` is published, global install works as above.

### For AI agents

Give your coding agent this prompt:

```text
Read the QuickCall README at https://github.com/keemgunn/quickcall before acting.

1. Check Node.js 24+ and a working `pi` on PATH.
2. When published: npm install -g @keemgunn/quickcall --allow-scripts=@keemgunn/quickcall
3. Verify `qc --help` and `qc --version`.
4. Run `qc --install-agent-harness cursor` (or your agent host framework) to install the packaged skill and command.
5. Do not run prompts from untrusted directories yet.
6. Report every check, copy destination, failure, and the exact next command I should run.
```

## Quick start

### Recommended: use an agent

Install the packaged skill and command:

```bash
qc --install-agent-harness cursor
```

Then ask your agent to create or run a prompt:

```text
/qc-create-prompt
```

Or invoke an existing alias:

```bash
qc review
```

### By yourself

Create a project prompt at `.qc/prompts/review.md`:

```markdown
# Review This Project

Inspect the current project and report concrete correctness, security, and maintainability issues.
```

Run it from that project directory:

```bash
qc review
```

Add one-time instructions without editing the file:

```bash
qc review --append "Focus on the authentication changes."
```

## Basics

### Command grammar

```text
qc <prompt-reference> [(-s | --shell) <path>] [(-a | --append) <text>]
qc (-h | --help)
qc (-v | --version)
qc --install-sample-prompts
qc --install-agent-harness [<framework> ...]
```

Help, version, and install flags must be used alone (`--install-agent-harness` may include framework names). Short options are exact separate tokens (`-s /bin/sh`, not `-s/bin/sh`). Mixing short and long forms of the same option is an error.

### Prompt lookup

| Reference | Behavior |
| --- | --- |
| `/work/prompts/review.md` | Absolute direct path |
| `./notes/review.md` | Relative direct path from cwd |
| `notes/review.md` | Direct path (ends in `.md`) |
| `review` | Alias: project then global `prompts/` |
| `team/review` | Nested alias with same order |

Alias order:

```text
1. <cwd>/.qc/prompts/<alias>.md
2. ~/.qc/prompts/<alias>.md
```

Direct paths never fall back to aliases. Alias paths cannot escape their `prompts/` root with `..` or symlinks.

### Prompt frontmatter

Optional YAML at the top of the Markdown file. qc removes it before Pi sees the body.

| Field | Effect |
| --- | --- |
| `description` | Documentation only |
| `qc_cli` | Runtime backend; v1 allows only `pi` |
| `qc_model` | `--model <value>` |
| `qc_thinking` | `--thinking <value>` |
| `qc_no_skills` | `true` → `--no-skills` |
| `qc_skill_path` | One `--skill <path>` (`~/` expands from `HOME`) |
| `qc_approve` | `--approve` or `--no-approve` |

Use `qc_*` keys only. Legacy `qpi_*`, `pqi_*`, or `pi_*` keys are errors.

### Shell output

Exact form only:

```markdown
!`git status --short`
```

qc authorizes every parsed command segment, runs allowed expressions concurrently through the selected shell, and replaces each expression with raw stdout in source order. stderr and non-zero exit status are ignored; empty stdout inserts an empty string. Inserted output is not rescanned.

### Pi execution

qc spawns `pi` directly:

```text
pi -p [--model <value>] [--thinking <value>] [--no-skills] [--skill <path>] [--approve|--no-approve]
```

The final multiline prompt goes to Pi stdin. Pi stdout, stderr, and normal non-zero exits pass through unchanged.

## Settings

qc reads two TOML files:

| Scope | Path |
| --- | --- |
| Global | `~/.qc/config.toml` |
| Project | `<cwd>/.qc/config.toml` |

Parent directories are not searched.

### First run

When `~/.qc/config.toml` is missing, the first valid invocation (including `--help` and `--version`) repairs `~/.qc/`:

- Creates `.default-settings/` and `.gitignore` when absent.
- Copies starter `config.toml`.
- Hard-refreshes `prompts/samples/` from packaged defaults.
- Preserves other files already under `prompts/`.

Postinstall and upgrades refresh `.default-settings/` and `.gitignore` without overwriting live config or samples when the sentinel exists.

Only `~/.qc/prompts/` participates in global alias lookup. Delete `~/.qc/config.toml` to repeat setup while keeping other prompt files.

| Alias | Purpose |
| --- | --- |
| `samples/joke` | Tell one short joke |
| `samples/system-status` | Summarize live host facts from shell output |
| `samples/git-commit-push` | Review, commit, and push pending Git work |

```bash
qc samples/joke
qc samples/system-status
```

`samples/git-commit-push` commits and pushes. Read it before running.

### Config keys

```toml
default-cli = "pi"
shell = "/bin/sh"
default-model = "provider/model"
default-thinking = "medium"

[command-permissions]
"*" = "allow"
"rm -rf *" = "deny"
```

| Key | Purpose |
| --- | --- |
| `shell` | Executable for shell-output expressions |
| `default-cli` | Runtime when frontmatter omits `qc_cli`; v1 allows only `pi` |
| `default-model` | Model when frontmatter omits `qc_model` |
| `default-thinking` | Thinking when frontmatter omits `qc_thinking` |
| `[command-permissions]` | Ordered `allow` / `deny` rules on full command segments |

Runtime: `qc_cli` → project `default-cli` → global `default-cli` → `pi`. Other scalars: CLI flag → frontmatter → project config → global config → Pi default. Permission rules merge global declarations first, then project; last match wins. With no permission table in either file, substitutions are allowed; once either file defines the table, unmatched segments are denied.

`--shell` overrides only the substitution shell, not how Pi is launched.

## Agent Harness

Install packaged assets with qc:

```bash
qc --install-agent-harness              # skills only -> ~/.agents/skills/qc/
qc --install-agent-harness cursor       # cursor command + skill
qc --install-agent-harness cursor pi    # multiple frameworks + skill
```

Supported frameworks: `opencode`, `cursor`, `pi`, `claude`, `codex`, `gemini`.

| Asset | Installed path |
| --- | --- |
| Skill | `~/.agents/skills/qc/` |
| Cursor command | `~/.cursor/commands/qc/qc-create-prompt.md` |
| OpenCode command | `~/.config/opencode/commands/qc/qc-create-prompt.md` |
| Claude command | `~/.claude/commands/qc/qc-create-prompt.md` |
| Gemini command | `~/.gemini/commands/qc/qc-create-prompt.md` |
| Pi command | `~/.pi/prompts/qc-create-prompt.md` |
| Codex command | `~/.codex/prompts/qc-create-prompt.md` |

Upgrades refresh previously installed harness files only; first install does not push harness assets automatically.

## Workflows

### Create a new prompt

**With an agent:** run `/qc-create-prompt` after `qc --install-agent-harness`.

**By yourself:** write a Markdown file under `.qc/prompts/<alias>.md` or `~/.qc/prompts/<alias>.md` with optional frontmatter, then `qc <alias>` from the intended cwd.

### Run with runtime context

**With an agent:** ask it to run `qc <alias> --append "…"` with trusted cwd and prompt sources.

**By yourself:**

```bash
qc summarize --append "Limit output to changed files since yesterday."
```

### Constrain shell commands

**With an agent:** review `<cwd>/.qc/config.toml` `[command-permissions]` before prompts that embed shell-output expressions.

**By yourself:** add rules to global or project config; put broad `allow` before narrow `deny`. Denied or unmatched segments stop the run before any command executes.

## All commands

| Command | Purpose |
| --- | --- |
| `qc <prompt-reference>` | Resolve prompt, expand shell output, run Pi |
| `qc -s <path>` / `qc --shell <path>` | Override substitution shell only |
| `qc -a <text>` / `qc --append <text>` | Append user message block before expansion |
| `qc --install-sample-prompts` | Replace `~/.qc/prompts/samples/` from packaged defaults |
| `qc --install-agent-harness [<framework> ...]` | Install packaged harness skill and/or framework commands |
| `qc -h` / `qc --help` | Print help (repairs global files if needed) |
| `qc -v` / `qc --version` | Print package version |

Runtime errors use `qc: error:` on stderr. Help and version use stdout. Successful stdout is Pi output only.

## Safety notes

**The prompt, `--append` text, and `<cwd>/.qc/config.toml` are trusted executable input.**

- Project config can choose the substitution shell and override global permission rules.
- Shell-output expressions execute through that shell with the invocation cwd and inherited environment.
- Permission tables reduce accidental execution; they do not make untrusted project configuration safe.
- Review cwd config, prompt files, and append text before invoking `qc`.
- qc never interpolates the final prompt or Pi argv into a shell command.
