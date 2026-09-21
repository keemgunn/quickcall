<h1 align="center">QuickCall usage</h1>

<p align="center">
  <strong>Every qc command, with the output it prints.</strong><br>
  This is the usage manual for QuickCall. Install and safety live in the README. Tool names, models, and thinking maps live in the provider catalog.
</p>

<p align="center">
  <a href="README.md"><strong>README</strong></a> &middot;
  <a href="#write-a-prompt"><strong>Write a prompt</strong></a> &middot;
  <a href="#run-a-turn"><strong>Run a turn</strong></a> &middot;
  <a href="#sessions"><strong>Sessions</strong></a> &middot;
  <a href="PROVIDERS.md"><strong>Providers</strong></a>
</p>

---

This page is a public GitHub manual, same class as [README.md](README.md) and [PROVIDERS.md](PROVIDERS.md). All three ship in the npm package.

| Manual | Job |
| --- | --- |
| [README.md](README.md) | What qc is, install, quick start, safety |
| This file | Every command form, prompt files, config, expected stdout and stderr |
| [PROVIDERS.md](PROVIDERS.md) | Tool names, binaries, models, thinking maps, native flags |

Do not pass native provider flags through `qc`. Use qc flags and prompt frontmatter. Model ids belong in the [provider catalog](PROVIDERS.md).

## Command map

```text
qc [<prompt-reference>] [options]
qc -o|--open [id] [--tool <name>]
qc (-h | --help)
qc (-v | --version)
qc --install-sample-prompts
qc --install-agent-harness
```

A turn needs a prompt reference, `--append`, or `-o/--open`. Help, version, and install flags must be used alone.

| Flag | Role |
| --- | --- |
| `--tool <name>` | Adapter: `pi`, `cursor`, `claude`, `opencode`, `antigravity`, `codex` |
| `--model <id>` | Native model or slug for that tool |
| `--thinking <level>` | Shared thinking knob (`off`, `minimal`, `low`, `medium`, `high`, `xhigh`, `max`) |
| `--workdir <path>` | Shell-expansion and agent working directory |
| `--output text\|json` | stdout format. Default `text` |
| `--debug` | Add `[WARNING]` and `[DEBUG]` to the success stdout blob. Forbidden with `-o` |
| `-q`, `--quiet` | Print only the final envelope (no spinner). Forbidden with `-o`. Agents and scripts use this |
| `-c`, `--continue <id>` | Resume a saved session in headless JSON |
| `-o`, `--open [id]` | Open a stored session in the provider TUI. `--tool` is the only companion |
| `--skill <path>` | Pi-only extra skill path |
| `--no-skills` | Pi-only: disable skills |
| `-s`, `--shell <path>` | Shell used only for `` !`command` `` |
| `-a`, `--append <text>` | Extra user text, or the whole turn when no prompt file is given |

Short options are exact separate tokens. `-hv`, `-s/bin/sh`, `-a=text`, `--open=id`, and `-o2609…` are unknown options.

```console
$ qc --help
Usage: qc [<prompt-reference>] [options]
       qc -o|--open [id] [--tool <name>]
       qc --install-sample-prompts
       qc --install-agent-harness
...
```

```console
$ qc --version
0.0.0
```

`--version` prints the installed package version only. This repository keeps placeholder `0.0.0` until a release stamps a real semver.

## Files qc uses

First valid `qc` run (including `--help` and `--version`) repairs missing global files. It does not overwrite an existing `~/.qc/config.toml` or your non-sample prompts.

| Path | Role |
| --- | --- |
| `~/.qc/config.toml` | Global defaults. Bootstrap sentinel |
| `~/.qc/prompts/` | Global aliases. Nested dirs allowed |
| `~/.qc/prompts/samples/` | Packaged samples. Replaced by `--install-sample-prompts` |
| `~/.qc/sessions/<id>.json` | Session mapping after a headless run when a native id is known, including failed turns |
| `<cwd>/.qc/config.toml` | Project defaults. Current directory only. No parent walk |
| `<cwd>/.qc/prompts/` | Project aliases. Win over global aliases |

```console
$ qc --install-sample-prompts
$ qc --install-agent-harness
```

Both commands print nothing on success and exit 0. Samples land in `~/.qc/prompts/samples/`. `--install-agent-harness` deletes every `qc` and `qc-*` directory under `~/.agents/skills/` and `~/.claude/skills/`, then copies the packaged skill set into both destinations.

npm install and upgrade (when postinstall runs) do that replace-install only for a destination that already has a `qc` or `qc-*` directory. They do not create skills on a machine that never installed the harness.

## Write a prompt

A prompt is a Markdown file. YAML frontmatter is optional. qc strips the whole frontmatter block before the agent sees the body.

### Where to put it

| Intent | File | Run |
| --- | --- | --- |
| This project only | `.qc/prompts/<alias>.md` | From that project: `qc <alias>` |
| Every project on this machine | `~/.qc/prompts/<alias>.md` | `qc <alias>` when no project file shadows it |
| Explicit file | any path ending `.md` | `qc ./notes/review.md` or `qc /abs/review.md` |

Aliases may nest: `.qc/prompts/team/review.md` is `qc team/review`.

A reference that ends in `.md`, is absolute, or starts with `./` or `../` is a **direct path**. Direct paths never fall back to alias lookup.

```console
$ qc review.md
[ERROR] prompt file not found: /Users/you/app/review.md
```

That looks for `./review.md` in the current directory, not `~/.qc/prompts/review.md`. Use `qc review` for an alias.

### Frontmatter

Open with `---` on the first line. Close with a line that is exactly `---`. The YAML inside must be a mapping.

```markdown
---
description: Review the current changes for correctness bugs.
qc_tool: claude
qc_model: sonnet
qc_thinking: high
qc_workdir: /path/to/app
---

Review the current changes. Report only correctness bugs, regressions, and missing tests.
Include file and line references for every finding.
```

| Field | Type | Effect |
| --- | --- | --- |
| `description` | string | Documentation only. Discarded with the frontmatter |
| `qc_tool` | non-empty string | `pi`, `cursor`, `claude`, `opencode`, `antigravity`, or `codex` |
| `qc_model` | non-empty string | That tool's model or slug |
| `qc_thinking` | non-empty string | Shared thinking level |
| `qc_workdir` | non-empty string | Absolute workdir. Does not expand `~` |
| `qc_no_skills` | `true` or `false` | Pi-only `--no-skills` when `true` |
| `qc_skill_path` | non-empty string | Pi-only `--skill`. A leading `~/` expands to home |

Omit every field the prompt does not need. Empty strings are invalid. Booleans must be YAML `true` / `false`, not `yes` / `no`.

Other keys such as `comment`, `owner`, or `tags` are allowed and stripped. Unknown `qc_*` keys fail. Removed keys `qc_cli` and `qc_approve` fail with a rename error. Prefixes `pi_`, `qpi_`, and `pqi_` fail.

```console
$ qc review
[ERROR] frontmatter key 'qc_cli' was removed; use qc_tool
```

### Body

Write the job the agent should do. Prefer a title, outcome, steps, constraints, and a required report shape. Do not put `$1` or `$ARGUMENTS` in the file. qc has no positional prompt arguments. Extra user text goes in `--append`.

Minimal project prompt:

```markdown
---
description: Review the current changes for correctness bugs.
---

Review the current changes. Report only correctness bugs, regressions, and missing tests.
Include file and line references for every finding.
```

Save as `.qc/prompts/review.md`, then:

```console
$ qc review
"""
<review text from the agent>
"""
[SUCCESS] pi ⋅ <model> ⋅ 12s
[USAGE] 1.2k in ⋅ 400 out ⋅ 0.012
[QC-SESSION] 260905-1545--pi--a1b2c3
```

That whole blob is stdout. On a TTY, stderr shows a bouncing wait bar, the resolved tool, optional model, and elapsed seconds until the agent exits. `[USAGE]` is omitted when the provider reports no tokens or cost. Pass `-q` for the same stdout blob with empty stderr. Hard fails print `[ERROR]` on stderr and leave stdout empty. Provider stderr that adds information is printed next. When a session mapping was written, fail stderr also has `[QC-SESSION]`. Live `text` then prints `qc -o <id>`, or the native binary when nothing was saved. `-q` does not print those copy-paste lines.

### Live shell output

qc recognizes only the exact form `` !`command` `` in the prompt body or `--append` text. Expressions run concurrently in the effective workdir, inherit the environment, and are replaced by stdout. Stderr and non-zero status are ignored. Inserted output is not scanned again.

```markdown
---
description: Summarize current host facts from captured shell output.
qc_no_skills: true
---

Report from these captured facts. Do not invent missing values.

- Host: !`uname -srm`
- Directory: !`pwd`
```

The agent receives the command output, not the backticks. Review every `` !`command` `` before you save the file. Project `.qc/config.toml` can select the shell that runs them.

### Create with an agent

```bash
qc --install-agent-harness
```

Then invoke `/qc-create-prompt`. That Command Skill writes a real file with valid frontmatter and reports the invocation. For a one-off instruction, use `qc -q --append` instead of creating a file.

## Settings

Global `~/.qc/config.toml` and optional `<cwd>/.qc/config.toml` use the same keys.

```toml
shell = "/bin/sh"

[tool]
default = "pi"

[tool.pi]
default_model = "provider/model"
default_thinking = "medium"
command = "pi"

[tool.cursor]
default_model = "composer-2.5"
command = "agent"
```

First-run copies the packaged starter, which pins a lowest-cost `default_model` per tool. Confirm ids in [PROVIDERS.md](PROVIDERS.md).

| Key | Effect |
| --- | --- |
| `shell` | Executable for `` !`command` `` only |
| `[tool] default` | Adapter when flags and frontmatter omit `qc_tool` |
| `[tool.<name>] default_model` | Per-tool model default |
| `[tool.<name>] default_thinking` | Per-tool thinking default |
| `[tool.<name>] command` | Executable for that adapter. PATH name or path to a binary |

`command` is the file qc spawns. Defaults: `pi`, `agent` for cursor, `claude`, `opencode`, `agy` for antigravity, `codex`. Project `[tool.<name>] command` wins over global, then the built-in name.

qc does not run `command` through a shell. These are invalid:

```toml
command = "FOO=bar opencode"
command = "env FOO=bar opencode"
```

The child inherits the environment of the `qc` process. Export variables in the shell that runs qc:

```bash
FOO=bar qc review --tool opencode
```

There is no per-tool env table. To pin env for one tool, wrap the binary and set `command` to that wrapper.

Removed keys `default-cli`, `default-model`, and `default-thinking` fail. A leftover `[command-permissions]` table is ignored.

Precedence: command flags, prompt frontmatter, project `[tool.*]`, global `[tool.*]`, then the built-in default (`pi` for tool).

```console
$ qc review --tool cursor --model composer-2.5 --thinking high
[WARNING] qc_thinking is ignored for tool 'cursor' (encode effort in the model slug)
"""
<review text>
"""
[SUCCESS] cursor ⋅ composer-2.5 ⋅ 12s
[QC-SESSION] 260905-1545--cursor--k9m2x0
```

The `[WARNING]` line is stderr. Stdout has no warning unless `--debug`. Quiet (`-q`) omits the warning unless `--debug`. Cursor ignores `--thinking`. Encode effort in the model slug. See [PROVIDERS.md](PROVIDERS.md).

## Run a turn

Each invocation is one turn in one process. There is no `--parallel` flag. Independent turns are separate `qc` processes; keep each process's stdout envelope intact (separate terminals, or redirect each to its own files).

### Packaged samples

```console
$ qc --install-sample-prompts
$ qc samples/joke
"""
Why don't scientists trust atoms? Because they make up everything.
"""
[SUCCESS] pi ⋅ <model> ⋅ 4s
[USAGE] 1.2k in ⋅ 400 out ⋅ 0.012
[QC-SESSION] 260905-1545--pi--a1b2c3
```

Joke text varies. `[USAGE]` appears only when the provider reported tokens or cost. `[QC-SESSION]` is stable in form: `yymmdd-hhmm--<tool>--<6 lowercase alnum>` in local time.

```console
$ qc samples/system-status
"""
<host summary built from uname, date, whoami, pwd, uptime>
"""
[SUCCESS] pi ⋅ <model> ⋅ 8s
[QC-SESSION] 260905-1546--pi--b2c3d4
```

`samples/git-commit-push` reviews the worktree, commits, and pushes. Run it only in a repository you intend to change.

`samples/web-surf` shows how a prompt can drive an external CLI such as `obscura`. Those binaries are not shipped with qc.

### Alias and path

```console
$ qc review
$ qc team/review
$ qc ./notes/review.md
$ qc /Users/you/prompts/review.md
```

Project alias wins over the same global alias.

```console
$ qc missing-alias
[ERROR] prompt alias 'missing-alias' was not found. Checked:
  /Users/you/app/.qc/prompts/missing-alias.md
  /Users/you/.qc/prompts/missing-alias.md
```

### Append

With a prompt file, `--append` is wrapped onto the body:

```text


---

Additional Message from the user:

<text>
```

```console
$ qc review --append "Focus on the authentication changes."
"""
<review text>
"""
[SUCCESS] pi ⋅ <model> ⋅ 12s
[QC-SESSION] 260905-1547--pi--c3d4e5
```

With no prompt file, `--append` is the whole user turn. No wrapper.

```console
$ qc --append "inspect this repo"
"""
<inspection text>
"""
[SUCCESS] pi ⋅ <model> ⋅ 12s
[QC-SESSION] 260905-1548--pi--d4e5f6
```

```console
$ qc --tool cursor --model composer-2.5 --append "inspect this repo"
"""
<inspection text>
"""
[SUCCESS] cursor ⋅ composer-2.5 ⋅ 12s
[QC-SESSION] 260905-1548--cursor--e5f6g7
```

Shell-output expressions inside `--append` still expand.

### Tool, model, thinking, workdir

```console
$ qc review --tool claude --model sonnet --thinking high
$ qc review --tool opencode --model opencode-go/deepseek-v4-flash --thinking high
$ qc review --tool antigravity --thinking medium
$ qc review --tool codex --model gpt-5.6-luna --thinking medium
$ qc review --workdir /path/to/app
```

`--tool cursor`, not `--tool agent`. Cursor's binary is `agent`. Antigravity's binary is `agy`. Native Codex is `--tool codex`; Pi `openai-codex/` model ids stay on `--tool pi`. Override a binary with `[tool.<name>] command` without changing the qc tool name.

Pi-only skill flags:

```console
$ qc summarize --no-skills
$ qc summarize --skill ~/.config/custom-skills/
```

On other tools those flags warn and are ignored. Live `text` prints the warning on stderr without `--debug`. Quiet omits it unless `--debug`:

```console
$ qc review --tool cursor --no-skills --debug
"""
<review text>
"""
[SUCCESS] cursor ⋅ <model> ⋅ 12s
[QC-SESSION] 260905-1549--cursor--f6g7h8
[WARNING] qc_no_skills is ignored for tool 'cursor'
```

### Shell for substitutions

```console
$ qc samples/system-status --shell /bin/bash
```

`-s` / `--shell` selects only the shell for `` !`command` ``. It does not change the agent binary. Order: `--shell`, project `shell`, global `shell`, `$SHELL`, `/bin/sh`.

## Sessions

After a **headless** run that observed a native id, qc writes `~/.qc/sessions/<id>.json` with `tool`, `native_id`, `cwd`, `created`, `updated`, and `warnings`. That includes failed, empty, and interrupted turns. Missing binary and preflight errors write no mapping.

Success stdout ends with:

```text
[QC-SESSION] 260905-1545--pi--a1b2c3
```

Fail stderr ends with `[QC-SESSION]` when a mapping was written. Live `text` then prints a copy-paste open command. When no mapping was written, live `text` prints the native binary instead (auth, login, or a broken install). `-q` stops after `[ERROR]` and optional child stderr:

```console
$ qc --append "competitor track" --tool pi
[ERROR] OpenAI API error (429): rate_limit_exceeded
[QC-SESSION] 260905-1545--pi--a1b2c3
qc -o 260905-1545--pi--a1b2c3
```

```console
$ qc -q --append "competitor track" --tool pi
[ERROR] OpenAI API error (429): rate_limit_exceeded
[QC-SESSION] 260905-1545--pi--a1b2c3
```

```console
$ qc review --tool cursor
[ERROR] cursor agent produced empty JSON output
Not logged in. Run `agent` to authenticate.
agent
```

Then `-c` or `-o` can resume that pretty id.

### Continue headless

`-c` / `--continue` needs a prompt or `--append`. The saved session locks the agent type. Cwd restores from the mapping unless `--workdir` or the prompt's `qc_workdir` overrides it.

```console
$ qc followup.md --continue 260905-1545--pi--a1b2c3
"""
<follow-up text>
"""
[SUCCESS] pi ⋅ <model> ⋅ 12s
[QC-SESSION] 260905-1545--pi--a1b2c3
```

The printed id is the same pretty id. The prompt body is the new turn.

```console
$ qc --continue 260905-1545--pi--a1b2c3 --append "Also add tests."
"""
<follow-up text>
"""
[SUCCESS] pi ⋅ <model> ⋅ 12s
[QC-SESSION] 260905-1545--pi--a1b2c3
```

Continue plus append-only uses the raw append path (no wrapper).

```console
$ qc --continue 260905-1545--pi--a1b2c3
[ERROR] a prompt reference or --append is required
```

```console
$ qc review --continue 260905-1545--pi--a1b2c3 --tool cursor
[ERROR] tool mismatch: session is 'pi' but resolved tool is 'cursor'
```

### Open the native TUI

`-o` / `--open` execs the provider's own interactive UI. It does not inject JSON flags or force-allow flags. qc prints nothing after the child starts. Exit code is the child's exit code.

```console
$ qc -o
```

Opens the newest stored session whose `cwd` matches the current directory (`path.resolve`, then `realpath` when the path exists). No ancestor walk. No `[tool] default` fallback.

```console
$ qc -o 260905-1545--pi--a1b2c3
```

Opens that pretty id. Spawn cwd is the mapping cwd.

```console
$ qc -o --tool pi
```

Newest session for this directory whose mapping tool is `pi`.

```console
$ qc -o
[ERROR] no session found for this directory
```

```console
$ qc -o --tool cursor
[ERROR] no cursor session found for this directory
```

```console
$ qc -o 260905-1545--pi--a1b2c3 --tool cursor
[ERROR] tool mismatch: session is 'pi' but resolved tool is 'cursor'
```

```console
$ qc -o 260905-1545--pi--zzzzzz
[ERROR] session not found: 260905-1545--pi--zzzzzz
```

`--tool` is the only flag allowed with `-o`. A prompt, `-c`, `-a`, `--output`, `--debug`, `--quiet`, `--model`, `--thinking`, `--workdir`, `--skill`, `--no-skills`, `-s`, help, version, and install are forbidden.

```console
$ qc review -o
[ERROR] --open cannot be combined with a prompt reference
```

```console
$ qc -o --open=260905-1545--pi--a1b2c3
[ERROR] unknown option '--open=260905-1545--pi--a1b2c3'
```

TUI resume arguments by tool: Pi `--session-id`, Cursor/Claude `--resume`, OpenCode `-s`, Antigravity `--conversation`, Codex `resume`. Details: [PROVIDERS.md](PROVIDERS.md).

## Output

Default TTY `text` shows a bouncing wait bar, the resolved tool, optional model, and elapsed seconds on stderr, then the success envelope on stdout. Non-TTY `text` skips the spinner and still prints live `[WARNING]` / `[ERROR]` on stderr. `-q` / `--quiet` restores the one-shot blob (empty stderr on success). Hard fail is empty stdout plus `[ERROR]` on stderr.

| Stream | Default `text` (TTY) | `-q` / `--quiet` | `--output json` | `-o/--open` |
| --- | --- | --- | --- | --- |
| stdout | Final envelope after stop: `"""` text, `[SUCCESS]`, optional `[USAGE]`, `[QC-SESSION]` | Today's one-shot blob | one JSON object | inherited by the TUI |
| stderr | bouncingBar + ` ⋅ <tool> ⋅ <model> ⋅ NNNs` (skip model when none); live `[WARNING]` / `[ERROR]` | empty on success | empty on success | silent after spawn. Pre-spawn failures still `[ERROR]` |
| Child stderr | buffered; printed after `[ERROR]` on fail when it adds information; hidden on success | same | same; in `debug` when `--debug` | inherited |
| Exit | child's exit | child's exit | child's exit | child's exit |

`--debug` adds `[WARNING]` and `[DEBUG]` to the success stdout blob. Live `text --debug` also prints `[WARNING]` on stderr during the wait. `-q --debug` is quiet: debug blob on stdout, empty stderr on success. `--output json` is always one-shot; `-q` is allowed and redundant. `--debug` and `--quiet` cannot combine with `-o`.

`[SUCCESS]` is `tool ⋅ model ⋅ <N>s`. Skip the model segment when none was resolved: `[SUCCESS] pi ⋅ 12s`. qc prints `[SUCCESS]` only when it got a non-empty assistant reply and the provider did not report a model or run error. A wrong model, a provider error, an empty reply, empty JSON, or an interrupt is `[ERROR]` on stderr with empty stdout. Provider stderr that adds information is printed next. When a native id is known, those runs still save a mapping and print `[QC-SESSION]` on stderr. Live `text` then prints `qc -o <id>`, or the native binary when nothing was saved. `-q` and json do not print those copy-paste lines. Missing binary writes `[ERROR]` only.

`[USAGE]` sits between `[SUCCESS]` and `[QC-SESSION]`. It is omitted when the provider reported nothing. Token segments that are present print as `in`, `out`, then `cache` / `think` / `total`. Counts `>= 1000` compact as `1.2k`. Cost is provider-reported only: `$0.012` when labeled USD, a bare number when unlabeled, `0.012 EUR` for another currency. Continue prints this turn's usage, not a session total. Cursor JSON has no usage today, so those runs omit `[USAGE]` and omit the JSON `usage` key.

```console
$ qc review --output json
{
  "tool": "pi",
  "session_id": "260905-1545--pi--a1b2c3",
  "native_id": "<provider session id>",
  "result": { },
  "warnings": [],
  "exit": 0,
  "model": "<resolved model or null>",
  "duration_s": 12,
  "usage": {
    "input_tokens": 1200,
    "output_tokens": 400,
    "cost": 0.012
  }
}
```

`result` is the provider's native JSON payload. `warnings` is always present. `debug` is present only with `--debug`. `exit` is the child exit code. `usage` is omitted when the provider reported nothing. No `[QC-SESSION]` line. Hard fail is still `[ERROR]` on stderr, not JSON.

```console
$ qc --output xml --append "hi"
[ERROR] --output must be 'text' or 'json'
```

There is no live token stream. Provider JSONL and child stderr stay buffered until the child exits.

```console
$ qc -q samples/joke
"""
Why don't scientists trust atoms? Because they make up everything.
"""
[SUCCESS] pi ⋅ <model> ⋅ 4s
[QC-SESSION] 260905-1545--pi--a1b2c3
```

## Invalid invocations

```console
$ qc -o -q
[ERROR] --open cannot be combined with --quiet
```

```console
$ qc review -q --quiet
[ERROR] --quiet was provided more than once
```

```console
$ qc
[ERROR] a prompt reference or --append is required
```

```console
$ qc --tool cursor
[ERROR] a prompt reference or --append is required
```

```console
$ qc --help --version
[ERROR] --help and --version cannot be combined
```

```console
$ qc review --help
[ERROR] --help cannot be combined with a prompt reference
```

```console
$ qc review ./other.md
[ERROR] only one prompt reference is allowed
```

```console
$ qc review --tool nope
[ERROR] unknown tool 'nope'
```

```console
$ qc review --tool pi --tool cursor
[ERROR] --tool was provided more than once
```

```console
$ qc review -a
[ERROR] --append requires a value
```

Missing binaries do not fall back to another provider:

```console
$ qc review --tool claude
[ERROR] claude executable 'claude' was not found on PATH. Install it and try again.
```

Corrupt `-o <id>` mappings fail closed (`corrupt session mapping` / `invalid session id`). A directory scan for bare `-o` skips unreadable or corrupt files.

## Safety

Invoking qc in a directory trusts that directory's `.qc/config.toml`. Exact `` !`command` `` expressions execute before the agent starts. Headless qc force-allows agent tools where the provider supports it. `qc -o` does not.

Review the prompt, `--append` text, config, and working directory before every run from a new project. Full rules: [README.md](README.md#security-and-safety).
