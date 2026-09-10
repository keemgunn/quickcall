---
type: release_note
project: "quickcall"
description: "Five providers, live wait, session TUI, and tagged output. Command-permissions dropped."
date: "2026-09-10T01:23:38.518Z"
release_channel: "public"
version_code: "v0.1.0"
---

## Highlights

- `qc` now runs Pi, Cursor Agent, Claude Code, OpenCode, and Antigravity as one-shot JSON agents. Pi stays the default.
- Default TTY runs show a live wait bar with tool, model, and elapsed seconds. Pass `-q` for the one-shot stdout envelope. Agent skills guide agents to always pass `-q`.
- Failed turns that already have a native session still save an id. Resume with `qc -c` or open the provider TUI with `qc -o`.

## Breaking changes

- **Breaking:** `[command-permissions]` in config is ignored. Shell-output expressions (`!`command``) always run, subject to the selected shell. Leftover tables do nothing.



## Security

- Running `qc` in a directory trusts that directory's `.qc/config.toml` as executable configuration. Prompt bodies and `--append` text may contain shell-output expressions that execute commands.
- The permission table is gone. It was never a sandbox. Review the prompt, appended text, and cwd config before invocation.



## New

- Five adapters via `--tool pi|cursor|claude|opencode|antigravity` (binaries `pi`, `agent`, `claude`, `opencode`, `agy`). Model, thinking, and workdir flags override layered config and prompt frontmatter.
- `qc -o/--open [id]` resumes a stored session in that provider's own TUI, without force-allow. Bare `-o` picks the newest session for this directory. `--tool` is the only companion flag.
- `qc --append "…"` with no prompt file is a valid headless turn. The append text is the whole prompt. Prompt plus `--append` still wraps as an additional user message.
- Tagged stdout envelope: quoted assistant text, `[SUCCESS] tool ⋅ model ⋅ Ns`, optional `[USAGE]`, then `[QC-SESSION] id`. `--output json` prints one parseable object. `--debug` is the only way to see `[WARNING]` and captured child stderr on success.
- Live wait on TTY text runs. `-q/--quiet` prints the same envelope with empty stderr. Packaged agent skills always pass `-q`.
- Successful turns may print `[USAGE]` with provider-reported tokens and cost. Providers that send no usage omit the line.
- Failed live runs print `[ERROR]`, optional provider stderr, then `qc -o <id>` when a mapping exists, or the native binary name when it does not.
- `USAGE.md` ships next to README and PROVIDERS with command forms, prompt authoring, and expected output.
- Packaged sample `samples/web-surf` shows driving an external CLI. Those binaries are not part of qc.
- `qc --install-agent-harness` installs the `qc` skill and `/qc-create-prompt` as Command Skills into `~/.agents/skills/` and `~/.claude/skills/` only.



## Improved

- New installs pin the cheapest catalog model per tool. Confirm ids in PROVIDERS.md; they go stale. Existing `~/.qc/config.toml` is not overwritten.
- `[tool.<name>] command` is a PATH binary or executable path, not a shell prefix or env assignment.
- Pi model ids use prefixes `openai-codex/` and `opencode-go/`. Example: `--model openai-codex/gpt-5.6-sol`.
- PROVIDERS.md is a provider-selection guide with dated model snapshots and Latest URLs.
- npm install uses `--allow-scripts=@keemgunn/quickcall`. Postinstall refreshes already-installed `qc` / `qc-*` skills by wipe-reinstall.
- New `~/.qc/` settings installs ignore `sessions/` in `.gitignore`.
- Agents can run several `qc` processes in one turn. Serialize `-c` of the same id, `-o`, and mutating prompts.
- `qc --tool antigravity --workdir <dir>` passes `--add-dir` so print-mode can write in that directory.



## Fixed

- `agy -p` no longer swallows the next flag as the prompt.
- Local `install-current.sh` completes under pnpm 12 without `pnpm approve-builds` for esbuild, and staged prod install no longer fails when `CI=true`.



## Upgrade notes

- Requires Node.js 24 or newer and at least one supported agent CLI on `PATH`. QuickCall does not install those CLIs.
- **Read a prompt before you run it.** Leftover `[command-permissions]` does nothing.
- Re-run `qc --install-agent-harness` or reinstall with `--allow-scripts` so packaged skills pick up `-q` recipes and the Command Skill layout.
- Verify the install with `qc --version` after this tag lands. It should report `0.1.0`.

