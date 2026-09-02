---
type: release_note
project: "quickcall"
description: "First public release of QuickCall — run reusable Markdown prompts through Pi print mode."
date: "2026-09-02T08:24:06.558Z"
release_channel: "public"
version_code: "v0.0.1"
---

## Highlights

- First public release. `qc` turns a Markdown file into a finished prompt and pipes it straight to Pi print mode.
- Install from npm: `npm install -g @keemgunn/quickcall` (or `pnpm add -g @keemgunn/quickcall`).
- MIT licensed.

## New

- **Prompt lookup** — `qc <prompt-ref>` resolves a prompt by alias, relative path, absolute path, or `folder/name` suffix. A project's `.qc/prompts/` wins over your global `~/.qc/prompts/`, so a repo can override a shared prompt without renaming it.
- **Layered configuration** — packaged defaults, then `~/.qc/config.toml`, then the current directory's `.qc/config.toml`. Later layers override earlier ones field by field.
- **Shell-output expressions** — write `` !`command` `` in a prompt body or in `--append` text and the command's output replaces it before the prompt reaches Pi.
- **Command permissions** — declare `[command-permissions]` with glob rules. Once the table exists, a command with no matching rule is denied. Every expression is authorized before any of them runs, so a denied command produces no partial effects.
- **Prompt frontmatter** — per-prompt YAML controls model, thinking level, and CLI target without touching config.
- **Agent harness** — `qc --install-agent-harness` installs the `qc` skill and the `qc-create-prompt` command into your agent setup.
- **First-run bootstrap** — creates `~/.qc/` with starter config and sample prompts, and preserves your edits across upgrades.

## Requirements

- Node.js **24 or newer**
- **Pi** already installed and on your `PATH`. QuickCall does not install Pi.

## Security

Read a prompt before you run it. Running `qc` in a directory trusts that directory's `.qc/config.toml` as executable configuration, and prompt bodies may contain shell-output expressions that execute commands. Project config can select the shell and override global command permissions.

The permission table is a guardrail, not a sandbox. The final prompt is passed to Pi through stdin and is never interpolated into a shell command.

## Notes

`0.0.0` on npm was a placeholder published only so trusted publishing could be configured. `0.0.1` is the first real release.
