---
name: qc
description: qc CLI. Resolve a Markdown prompt or --append text, send one headless turn to pi, cursor, claude, opencode, antigravity, or codex, and print the answer. MUST LOAD when invoking qc.
compatibility: agnostic
metadata:
  audience: agents
  domain: cli
---

# QuickCall

Use `qc -q` to send one turn to a headless agent tool (default `pi`). Always pass `-q` / `--quiet` so qc prints only the final envelope. A turn needs a prompt reference or `--append`. Open a stored session in the provider TUI with `-o/--open` (no `-q`). Fan-out is multiple processes: [Parallel Agent Call](#parallel-agent-call).

## Ground Truth

Package README (`README.md` after `npm install -g @keemgunn/quickcall`) owns installation and safety. `USAGE.md` owns command examples, prompt authoring, and expected output. Install packaged skills with `qc --install-agent-harness`. Package install/upgrade refreshes dests that already have `qc` or `qc-*` skills.

Models and thinking maps: [references/providers.md](references/providers.md). Do not invent model IDs from memory.

## Stored prompt vs one-shot `--append`

| Need | Form |
| --- | --- |
| Reusable Markdown prompt (alias or path) | `qc -q <prompt-reference>` |
| Reusable prompt plus extra user text | `qc -q <prompt-reference> --append "…"` |
| One-shot instruction, no prompt file | `qc -q --append "…"` |

Positional tokens are prompt aliases or paths only. Free text goes in `--append`.

## Recipes

```bash
# Stored prompt (default tool, usually pi)
qc -q review

# Prompt + wrapped append
qc -q review --append "Focus on auth."

# Prompt + tool + model + wrapped append
qc -q review --tool cursor --model composer-2.5 --append "inspect this repo"

# Prompt + native Codex (not Pi openai-codex/)
qc -q review --tool codex --model gpt-5.6-luna --thinking medium

# Append-only (raw turn, no wrapper)
qc -q --append "inspect this repo"
qc -q --tool cursor --model composer-2.5 --append "inspect this repo"

# Continue + raw append (no prompt file)
qc -q -c 260903-1540--pi--a1b2c3 -a "also add tests"

# Continue + prompt body as the new turn
qc -q followup.md -c 260903-1540--pi--a1b2c3

# Open newest session for this directory in the provider TUI
qc -o

# Open a printed pretty id
qc -o 260903-1540--pi--a1b2c3

# Open newest pi session for this directory
qc -o --tool pi
```

Still invalid: bare `qc`, `qc --tool cursor` alone, `qc -c <id>` alone. `-o` does not take a prompt, `-c`, `-a`, `--model`, `--output`, `--debug`, or `--quiet`.

## Parallel Agent Call

qc is one process, one turn. There is no `--parallel` flag. Independent turns are separate `qc` processes started in **one** host tool-call batch (several Shell/Bash calls in the same turn). Wait for every invocation, then read each envelope.

```bash
# Same prompt, different tools
qc -q samples/joke --tool pi
qc -q samples/joke --tool cursor
qc -q samples/joke --tool claude
qc -q samples/joke --tool opencode
qc -q samples/joke --tool antigravity
qc -q samples/joke --tool codex

# Independent tasks, same or mixed tools
qc -q --tool cursor --append "review auth only"
qc -q --tool claude --append "review payments only"
```

One `qc` per host tool call so stdout stays isolated. If the host cannot issue parallel calls, redirect each process to its own files, then `wait`:

```bash
qc -q samples/joke --tool pi > "$TMPDIR/qc-pi.out" 2>"$TMPDIR/qc-pi.err" &
qc -q samples/joke --tool cursor > "$TMPDIR/qc-cursor.out" 2>"$TMPDIR/qc-cursor.err" &
wait
```

Do not background several `qc` in one shell sharing stdout. `[SUCCESS]` and `[QC-SESSION]` lines interleave.

Match results by `[SUCCESS] <tool>` and `[QC-SESSION]`. Use `--output json` when a later step will parse.

Serialize these (one after another):

- `-c` / `--continue` of the same pretty id
- `-o` / `--open`
- Prompts that write the same files or run mutating `` !`command` ``

After a fan-out, `qc -o` without an id opens the newest mapping for that cwd (whichever finished last). Pass the printed pretty id or `--tool`.

A missing binary or provider `[ERROR]` fails only that process. Others still complete. No auto-fallback. Inspect side effects before retrying a failed turn.

## Flags

| Flag | Role |
| --- | --- |
| `--tool <name>` | Adapter: `pi`, `cursor`, `claude`, `opencode`, `antigravity`, `codex` |
| `--model <id>` | Model / slug for the selected tool |
| `--thinking <level>` | Shared thinking knob (tool-specific map in providers.md) |
| `--workdir <path>` | Shell-expansion and agent working directory |
| `--output <mode>` | `text` (default) or `json` |
| `--debug` | Add `[WARNING]` and `[DEBUG]` to the success stdout blob. Forbidden with `-o` |
| `-q`, `--quiet` | Print only the final envelope (no spinner). Always pass this. Forbidden with `-o` |
| `-c`, `--continue <id>` | Resume a qc session by pretty id; locks tool from the mapping |
| `-o`, `--open [id]` | Open a stored session in the provider TUI; `--tool` is the only companion |
| `-a`, `--append <text>` | Append user text, or the whole turn when no prompt is given |
| `--skill <path>` | Pi-only skill path |
| `--no-skills` | Pi-only: disable skills |
| `-s`, `--shell <path>` | Shell for `` !`command` `` only |

Cursor tool name is `cursor`; binary is `agent`. Cursor receives the prompt as the last argv token, not stdin. Codex tool name is `codex`; binary is `codex`. Prompt on stdin. Native Codex is not Pi's `openai-codex/` transport.

## Lookup

- Absolute paths, references beginning `./` or `../`, and every reference ending `.md` are direct paths. Resolve relative direct paths from the invocation cwd. No alias fallback.
- Other references are aliases: `<cwd>/.qc/prompts/<alias>.md`, then `~/.qc/prompts/<alias>.md`. Project wins. Nested aliases allowed; no `..` or symlink escape outside the selected `prompts/` root.

## Sessions

- After a headless run that observed a native id, stdout (success) or stderr (fail) includes `[QC-SESSION] <pretty-id>`. The mapping is written even for empty, provider-error, and interrupted turns.
- Resume headless with `-c <pretty-id>`. Continue locks the tool from the session mapping; an explicit mismatched `--tool` is an error.
- Open the provider TUI with `-o` (newest session for this directory) or `-o <pretty-id>`. Optional `--tool` filters bare `-o` or must match the mapping for `-o <id>`. qc prints no `[QC-SESSION]` line.
- Cwd restores from the mapping unless `--workdir` (or prompt `qc_workdir`) overrides it. `-o` always spawns in the mapping cwd.

## Output

Default headless `text` is a live wait on stderr (spinner on a TTY, live `[WARNING]` / `[ERROR]`), then the success envelope on stdout. Agents always pass `-q` / `--quiet` and parse the one-shot blob. Hard fail is empty stdout plus `[ERROR]` on stderr.

| Mode | Behavior |
| --- | --- |
| Default `text` | Live stderr wait; stdout `"""` assistant text, `[SUCCESS]`, optional `[USAGE]`, `[QC-SESSION]` |
| `-q` / `--quiet` | Today's one-shot blob on stdout; empty stderr on success |
| `-q --debug` | Today's debug blob on stdout; empty stderr on success |
| `--output json` | one JSON object: `tool`, `session_id`, `native_id`, `result`, `warnings`, `exit`, `model`, `duration_s`, optional `usage`. `debug` only with `--debug`. No spinner. `-q` is allowed and redundant |
| Hard fail | empty stdout; `[ERROR]` on stderr, then child stderr when it adds information, then `[QC-SESSION]` when a mapping was written (even if `--output json` was set). Live `text` also prints `qc -o <id>` or the native binary when unmapped. `-q` does not |
| `-o/--open` | inherited TUI stdio. Pre-spawn failures still `[ERROR]` |

`[SUCCESS]` is `tool ⋅ model ⋅ <N>s`. Skip the model segment when omitted: `[SUCCESS] pi ⋅ 12s`. `[USAGE]` is omitted when the provider reported nothing. Child stderr is buffered. It is not streamed live. `[DEBUG]` must not include the prompt.

## Trust

- Exact `` !`command` `` in the prompt body or `--append` text runs through the selected shell in the effective workdir before the agent starts.
- Invoking qc in a directory trusts that directory's `.qc/config.toml` (shell and tool defaults).
- Headless qc always force-allows agent tools and auto-trusts where the binary supports it. Residual native deny lists still bind.
- `-o/--open` starts the provider TUI without force-allow flags.
- Review cwd config, prompt, and `--append` text before running from a new or untrusted directory.

## Failures

- Preserve `[ERROR]` on stderr. Fix the named argument, path, config, frontmatter, shell, or missing binary.
- Do not silently retry after a turn that may have produced side effects until state is inspected.
- Missing agent binary is an error. No auto-fallback to another tool.
- Agent non-zero exit with extracted text is still a success envelope unless the provider reported a model or run error. Empty assistant text, empty/invalid provider JSON, Pi `stopReason: error`, Claude `is_error` / `[claude-code:unrecognized_model]`, OpenCode JSONL `{type:"error"}` or empty text, Agy `status: ERROR` (`payload.error`), Codex JSONL `turn.failed` / `error`, and signal exits are hard fails. Child stderr that adds information is printed after `[ERROR]`. Save a session when a native id is known; print `[QC-SESSION]` on stderr. Missing binary and Cursor/Agy empty native id write no mapping.

## Providers

Catalog only: [references/providers.md](references/providers.md). Use it for tool binaries, model formats, and thinking maps. Do not duplicate that catalog here.
