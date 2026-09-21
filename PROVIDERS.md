---
title: QuickCall provider guide
description: Choose a QuickCall provider and verify its tool name, binary, models, thinking support, sessions, and native headless behavior.
updated: 2026-09-21
---

# QuickCall provider guide

QuickCall wraps six native agent CLIs behind one command. Choose the provider with `--tool` or `qc_tool`; QuickCall translates shared settings into that provider's headless JSON arguments.

Use qc flags and prompt frontmatter with QuickCall. Do not pass native provider flags through `qc`.

Public manuals next to this file: `README.md` (install and map), `USAGE.md` (commands, prompts, expected output). This catalog is tools, models, thinking, and native flags.

## Choose a provider

| Provider | qc tool name | Binary | Main difference |
| --- | --- | --- | --- |
| Pi | `pi` | `pi` | Built-in default; supports every documented qc thinking level and Pi skill controls |
| Cursor Agent | `cursor` | `agent` | Ignores `qc_thinking`; effort and fast mode belong in the model slug |
| Claude Code | `claude` | `claude` | Maps five qc thinking levels to `--effort` |
| OpenCode | `opencode` | `opencode` | Uses `provider/model` IDs and maps thinking to `--variant` |
| Antigravity | `antigravity` | `agy` | Maps `low`, `medium`, and `high` to `--effort` |
| Codex | `codex` | `codex` | Native Codex CLI (not Pi `openai-codex/`); maps thinking to `model_reasoning_effort` |

Pi is the built-in default. These first two commands are equivalent when `[tool] default` is unset or set to `pi`:

```bash
qc review
qc review --tool pi

qc review --tool cursor --model composer-2.5
qc review --tool claude --model sonnet --thinking high
qc review --tool opencode --model opencode-go/deepseek-v4-flash --thinking high
qc review --tool antigravity --thinking medium
qc review --tool codex --model gpt-5.6-luna --thinking medium
```

Write `--tool cursor`, not `--tool agent`. Cursor's binary is `agent`; Antigravity's binary is `agy`. Native `--tool codex` is binary `codex` with native model ids. Pi `openai-codex/<id>` is a Pi transport, not this adapter. A `[tool.<name>] command` setting can override a binary name without changing the qc tool name. The value is one executable name or path, not a shell prefix such as `FOO=bar opencode`.

QuickCall does not fall back to another provider when a binary is missing.

## Shared interface

| qc flag | Frontmatter | Role |
| --- | --- | --- |
| `--tool` | `qc_tool` | Adapter name (`pi`, `cursor`, `claude`, `opencode`, `antigravity`, `codex`), not the binary |
| `--model` | `qc_model` | Native model / slug (`--model` or OpenCode `-m`) |
| `--thinking` | `qc_thinking` | Map provider-specific thinking behavior; Cursor warns and ignores it |
| `--workdir` | `qc_workdir` | Spawn cwd + native cwd flag when the tool has one |
| `-c` / `--continue` | None | Resume via stored native id; no frontmatter field |
| `-o` / `--open` | None | Open the stored native session in the provider TUI; `--tool` is the only companion |
| `--no-skills` / `--skill` | `qc_no_skills` / `qc_skill_path` | Pi only; other tools warn + ignore |
| `--output` | none | Select qc text or JSON stdout; native execution always uses JSON |

Use an absolute `qc_workdir`; this field does not expand `~`. `--continue` resumes from qc's saved native session mapping in headless JSON. `-o/--open` resumes the same mapping in the provider's interactive TUI. Neither has a frontmatter equivalent.

Without `--continue`, tool selection resolves from `--tool` or `qc_tool`, then `[tool] default`, and finally `pi`. A continued session locks its saved tool; a different `--tool` or `qc_tool` fails. Model and thinking settings resolve from command flags, prompt frontmatter, project `[tool.<name>]`, then global `[tool.<name>]`. The configured command resolves from project settings, global settings, then the built-in binary. Workdir resolves from `--workdir` or `qc_workdir`, a continued session's saved directory, then the invocation directory.

## Thinking levels

Canonical `qc_thinking` values are `off`, `minimal`, `low`, `medium`, `high`, `xhigh`, and `max`.

| Tool | Native mapping |
| --- | --- |
| `pi` | All seven map to `--thinking` |
| `cursor` | Ignored with a warning; encode effort, thinking, or fast mode in the exact model slug |
| `claude` | `low`, `medium`, `high`, `xhigh`, and `max` map to `--effort`; `off` and `minimal` warn and are ignored |
| `opencode` | Every value maps to `--variant`, not OpenCode's display-only `--thinking` flag |
| `antigravity` | `low`, `medium`, and `high` map to `--effort`; other values warn and are ignored |
| `codex` | `off` maps to Codex `none`; `minimal`, `low`, `medium`, `high`, `xhigh`, and `max` pass through as `model_reasoning_effort`; other values warn and are ignored |

Cursor warns and ignores every supplied thinking value. Claude and Antigravity warn and ignore values outside their mapped subsets. Codex maps `off` to `none` and passes the remaining mapped values through `-c model_reasoning_effort`; other values warn and are ignored. Pi forwards the supplied value to `--thinking`; OpenCode forwards it to `--variant`. For example, Claude's native `ultracode` effort is not mapped by QuickCall and is ignored with a warning when supplied as `qc_thinking`.

## Automatic execution and trust

Headless QuickCall always injects JSON and each provider's force-allow or auto-trust flags. These behaviors are not user-selectable. Native provider deny rules may still block an operation.

`-o/--open` is a normal interactive resume. It does not inject JSON/print flags or force-allow flags (`-a`, `--yolo`, `--dangerously-skip-permissions`, `--auto`, `--trust`, `--force`, `--approve-mcps`, `--dangerously-bypass-approvals-and-sandbox`, `--dangerously-bypass-hook-trust`). It also does not pass `--model`, `--thinking`, `--workspace`, `--dir`, `--add-dir`, or `--cd`.

Review the prompt and working directory before every run. The provider tables below show the exact automatic flags and the TUI resume argument.

In the mapping tables, **Native** is the argument QuickCall sends. **Automatic** means QuickCall always injects the behavior and offers no qc flag for it. **None** means there is no matching frontmatter field.

## Pi (`pi`)

Pi is the default adapter. It receives the prompt on stdin and uses qc's printable session id as its native session id.

```bash
qc review --tool pi --model openai-codex/gpt-5.6-sol --thinking high
```

| Item | Native | qc flag | Frontmatter |
| --- | --- | --- | --- |
| Adapter | binary `pi` | `--tool pi` | `qc_tool: pi` |
| Headless JSON | `--mode json`; prompt on stdin | Automatic | None |
| Force-allow | `-a` (project trust for `.pi/`) | Automatic | None |
| Session create | `--session-id <pretty-id>` | qc generates the id | None |
| Session resume | Same `--session-id` | `-c <pretty-id>` | None |
| TUI open | `--session-id <native_id>` (no `--mode json`, no `-a`) | `-o` / `-o <pretty-id>` | None |
| Cwd | spawn cwd only (no `--cwd`) | `--workdir` | `qc_workdir` |
| Thinking | `--thinking` (all seven values) | `--thinking` | `qc_thinking` |
| Model | `--model <provider/id>` (`openai-codex/`, `opencode-go/`, other prefixes) | `--model` | `qc_model` |
| No skills | `--no-skills` | `--no-skills` | `qc_no_skills: true` |
| Skill path | `--skill <path>` | `--skill` | `qc_skill_path` |

### Models and documentation

Pi model ids are `provider/id`. Prefix the id:

- `openai-codex/` for OpenAI Codex models from a ChatGPT subscription. Example: `openai-codex/gpt-5.6-sol`
- `opencode-go/` for OpenCode Go models. Example: `opencode-go/glm-5.3-flash`

QuickCall does not ship a full Pi catalog. Other prefixes exist. Auth and availability vary. Run the installed CLI before saving a model:

```bash
pi --list-models
```

- [Pi usage](https://pi.dev/docs/latest/usage)
- [Pi JSON mode](https://pi.dev/docs/latest/json)
- [Pi sessions](https://pi.dev/docs/latest/sessions)
- [Pi providers](https://pi.dev/docs/latest/providers)
- [Pi models](https://pi.dev/docs/latest/models)

### OpenAI Codex model snapshot

<details>
<summary>Show the 7 IDs verified on 2026-09-03</summary>

One-machine account snapshot from `pi --list-models openai-codex`. Always re-run that command before relying on an id. Use each id as `openai-codex/<id>`.

Latest: [OpenAI Codex models](https://developers.openai.com/codex/models)

- `gpt-5.3-codex-spark`
- `gpt-5.4`
- `gpt-5.4-mini`
- `gpt-5.5`
- `gpt-5.6-luna`
- `gpt-5.6-sol`
- `gpt-5.6-terra`

</details>

## Cursor Agent (`cursor`, binary `agent`)

Use `cursor` as the qc tool name. QuickCall runs the `agent` binary and passes the prompt as the final argument. Cursor returns a UUID in JSON `session_id`; qc stores it behind qc's printable session id.

`qc_thinking` is ignored with a warning. Encode effort / thinking / fast in the exact `--model` slug. Do not invent slugs; account availability is dynamic.

```bash
qc review --tool cursor --model composer-2.5
```

| Item | Native | qc flag | Frontmatter |
| --- | --- | --- | --- |
| Adapter | binary `agent` | `--tool cursor` | `qc_tool: cursor` |
| Headless JSON | `-p --output-format json`; prompt as final argument | Automatic | None |
| Force-allow | `--force --yolo --approve-mcps --trust` (deny rules still bind) | Automatic | None |
| Session create | JSON `session_id` UUID | qc stores the native id | None |
| Session resume | `--resume <uuid>` | `-c <pretty-id>` | None |
| TUI open | `--resume <uuid>` (no `-p`, no yolo/trust) | `-o` / `-o <pretty-id>` | None |
| Cwd | `--workspace <path>` (+ spawn cwd) | `--workdir` | `qc_workdir` |
| Thinking | Not a flag; encode in the exact slug (`-thinking-...`, `-fast`) | `--thinking` warns and is ignored | `qc_thinking` warns and is ignored |
| Model | `--model <exact slug>` from `agent --list-models` | `--model` | `qc_model` |

### Models and documentation

```bash
agent --list-models
```

- [Cursor headless CLI](https://cursor.com/docs/cli/headless)
- [Cursor CLI parameters](https://cursor.com/docs/cli/reference/parameters)
- [Cursor output formats](https://cursor.com/docs/cli/reference/output-format)
- [Cursor models and pricing](https://cursor.com/docs/models-and-pricing), for pricing context rather than exact CLI slugs

### Cursor slug snapshot

<details>
<summary>Show the account snapshot from 2026-09-03</summary>

One-machine account snapshot. Always re-run `agent --list-models` before relying on a slug.

Latest: [Cursor models and pricing](https://cursor.com/docs/models-and-pricing). Product notes: [Cursor changelog](https://cursor.com/changelog).

Claude Opus families generally use `{model}-{effort}` and `{model}-thinking-{effort}`, with optional `-fast`. Confirmed efforts vary by family. Use only a slug returned for your account.

| Family | IDs |
| --- | --- |
| `auto` | `auto` |
| `gpt-5.3-codex` | - `gpt-5.3-codex-low`<br>- `gpt-5.3-codex-low-fast`<br>- `gpt-5.3-codex`<br>- `gpt-5.3-codex-fast`<br>- `gpt-5.3-codex-high`<br>- `gpt-5.3-codex-high-fast`<br>- `gpt-5.3-codex-xhigh`<br>- `gpt-5.3-codex-xhigh-fast` |
| `gpt-5.2` | - `gpt-5.2`<br>- `gpt-5.2-low`<br>- `gpt-5.2-low-fast`<br>- `gpt-5.2-fast`<br>- `gpt-5.2-high`<br>- `gpt-5.2-high-fast`<br>- `gpt-5.2-xhigh`<br>- `gpt-5.2-xhigh-fast` |
| `cursor-grok-4.6` | - `cursor-grok-4.6-low`<br>- `cursor-grok-4.6-low-fast`<br>- `cursor-grok-4.6-medium`<br>- `cursor-grok-4.6-medium-fast`<br>- `cursor-grok-4.6-high`<br>- `cursor-grok-4.6-high-fast`<br>- `cursor-grok-4.6-xhigh`<br>- `cursor-grok-4.6-xhigh-fast` |
| `cursor-grok-4.5` | - `cursor-grok-4.5-low`<br>- `cursor-grok-4.5-low-fast`<br>- `cursor-grok-4.5-medium`<br>- `cursor-grok-4.5-medium-fast`<br>- `cursor-grok-4.5-high`<br>- `cursor-grok-4.5-high-fast` |
| `composer` | - `composer-2.5`<br>- `composer-2.5-fast` |
| `claude-opus-5` (confirmed) | - `claude-opus-5-low`<br>- `claude-opus-5-low-fast`<br>- `claude-opus-5-medium`<br>- `claude-opus-5-medium-fast`<br>- `claude-opus-5-high`<br>- `claude-opus-5-high-fast`<br>- `claude-opus-5-thinking-low`<br>- `claude-opus-5-thinking-low-fast`<br>- `claude-opus-5-thinking-medium`<br>- `claude-opus-5-thinking-medium-fast`<br>- `claude-opus-5-thinking-high`<br>- `claude-opus-5-thinking-high-fast`<br>- `claude-opus-5-thinking-xhigh`<br>- `claude-opus-5-thinking-xhigh-fast`<br>- `claude-opus-5-thinking-max`<br>- `claude-opus-5-thinking-max-fast` |
| `claude-opus-4-8` (full low to max, with optional thinking and fast modes) | - `claude-opus-4-8-low`<br>- `claude-opus-4-8-low-fast`<br>- `claude-opus-4-8-medium`<br>- `claude-opus-4-8-medium-fast`<br>- `claude-opus-4-8-high`<br>- `claude-opus-4-8-high-fast`<br>- `claude-opus-4-8-xhigh`<br>- `claude-opus-4-8-xhigh-fast`<br>- `claude-opus-4-8-max`<br>- `claude-opus-4-8-max-fast`<br>- `claude-opus-4-8-thinking-low`<br>- `claude-opus-4-8-thinking-low-fast`<br>- `claude-opus-4-8-thinking-medium`<br>- `claude-opus-4-8-thinking-medium-fast`<br>- `claude-opus-4-8-thinking-high`<br>- `claude-opus-4-8-thinking-high-fast`<br>- `claude-opus-4-8-thinking-xhigh`<br>- `claude-opus-4-8-thinking-xhigh-fast`<br>- `claude-opus-4-8-thinking-max`<br>- `claude-opus-4-8-thinking-max-fast` |
| `claude-opus-4-7` (same matrix as 4-8) | - `claude-opus-4-7-low`<br>- `claude-opus-4-7-low-fast`<br>- `claude-opus-4-7-medium`<br>- `claude-opus-4-7-medium-fast`<br>- `claude-opus-4-7-high`<br>- `claude-opus-4-7-high-fast`<br>- `claude-opus-4-7-xhigh`<br>- `claude-opus-4-7-xhigh-fast`<br>- `claude-opus-4-7-max`<br>- `claude-opus-4-7-max-fast`<br>- `claude-opus-4-7-thinking-low`<br>- `claude-opus-4-7-thinking-low-fast`<br>- `claude-opus-4-7-thinking-medium`<br>- `claude-opus-4-7-thinking-medium-fast`<br>- `claude-opus-4-7-thinking-high`<br>- `claude-opus-4-7-thinking-high-fast`<br>- `claude-opus-4-7-thinking-xhigh`<br>- `claude-opus-4-7-thinking-xhigh-fast`<br>- `claude-opus-4-7-thinking-max`<br>- `claude-opus-4-7-thinking-max-fast` |
| `claude-fable-5` | - `claude-fable-5-low`<br>- `claude-fable-5-medium`<br>- `claude-fable-5-high`<br>- `claude-fable-5-xhigh`<br>- `claude-fable-5-max`<br>- `claude-fable-5-thinking-low`<br>- `claude-fable-5-thinking-medium`<br>- `claude-fable-5-thinking-high`<br>- `claude-fable-5-thinking-xhigh`<br>- `claude-fable-5-thinking-max` |
| `claude-fable-5-1` | - `claude-fable-5-1-low`<br>- `claude-fable-5-1-medium`<br>- `claude-fable-5-1-high`<br>- `claude-fable-5-1-xhigh`<br>- `claude-fable-5-1-max`<br>- `claude-fable-5-1-thinking-low`<br>- `claude-fable-5-1-thinking-medium`<br>- `claude-fable-5-1-thinking-high`<br>- `claude-fable-5-1-thinking-xhigh`<br>- `claude-fable-5-1-thinking-max` |
| `claude-sonnet-5` | - `claude-sonnet-5-low`<br>- `claude-sonnet-5-medium`<br>- `claude-sonnet-5-high`<br>- `claude-sonnet-5-xhigh`<br>- `claude-sonnet-5-max`<br>- `claude-sonnet-5-thinking-low`<br>- `claude-sonnet-5-thinking-medium`<br>- `claude-sonnet-5-thinking-high`<br>- `claude-sonnet-5-thinking-xhigh`<br>- `claude-sonnet-5-thinking-max` |
| `gpt-5.6-sol` | - `gpt-5.6-sol-none`<br>- `gpt-5.6-sol-none-fast`<br>- `gpt-5.6-sol-low`<br>- `gpt-5.6-sol-low-fast`<br>- `gpt-5.6-sol-medium`<br>- `gpt-5.6-sol-medium-fast`<br>- `gpt-5.6-sol-high`<br>- `gpt-5.6-sol-high-fast`<br>- `gpt-5.6-sol-xhigh`<br>- `gpt-5.6-sol-xhigh-fast`<br>- `gpt-5.6-sol-max`<br>- `gpt-5.6-sol-max-fast` |
| `gpt-5.6-terra` | - `gpt-5.6-terra-none`<br>- `gpt-5.6-terra-none-fast`<br>- `gpt-5.6-terra-low`<br>- `gpt-5.6-terra-low-fast`<br>- `gpt-5.6-terra-medium`<br>- `gpt-5.6-terra-medium-fast`<br>- `gpt-5.6-terra-high`<br>- `gpt-5.6-terra-high-fast`<br>- `gpt-5.6-terra-xhigh`<br>- `gpt-5.6-terra-xhigh-fast`<br>- `gpt-5.6-terra-max`<br>- `gpt-5.6-terra-max-fast` |
| `gpt-5.6-luna` | - `gpt-5.6-luna-none`<br>- `gpt-5.6-luna-none-fast`<br>- `gpt-5.6-luna-low`<br>- `gpt-5.6-luna-low-fast`<br>- `gpt-5.6-luna-medium`<br>- `gpt-5.6-luna-medium-fast`<br>- `gpt-5.6-luna-high`<br>- `gpt-5.6-luna-high-fast`<br>- `gpt-5.6-luna-xhigh`<br>- `gpt-5.6-luna-xhigh-fast`<br>- `gpt-5.6-luna-max`<br>- `gpt-5.6-luna-max-fast` |
| `gpt-5.5` | - `gpt-5.5-none`<br>- `gpt-5.5-none-fast`<br>- `gpt-5.5-low`<br>- `gpt-5.5-low-fast`<br>- `gpt-5.5-medium`<br>- `gpt-5.5-medium-fast`<br>- `gpt-5.5-high`<br>- `gpt-5.5-high-fast`<br>- `gpt-5.5-extra-high`<br>- `gpt-5.5-extra-high-fast` |
| `gpt-5.4` | - `gpt-5.4-low`<br>- `gpt-5.4-medium`<br>- `gpt-5.4-medium-fast`<br>- `gpt-5.4-high`<br>- `gpt-5.4-high-fast`<br>- `gpt-5.4-xhigh`<br>- `gpt-5.4-xhigh-fast` |
| `gpt-5.4-mini` | - `gpt-5.4-mini-none`<br>- `gpt-5.4-mini-low`<br>- `gpt-5.4-mini-medium`<br>- `gpt-5.4-mini-high`<br>- `gpt-5.4-mini-xhigh` |
| `gpt-5.4-nano` | - `gpt-5.4-nano-none`<br>- `gpt-5.4-nano-low`<br>- `gpt-5.4-nano-medium`<br>- `gpt-5.4-nano-high`<br>- `gpt-5.4-nano-xhigh` |
| `gemini-3.8-flash` | - `gemini-3.8-flash-low`<br>- `gemini-3.8-flash-medium`<br>- `gemini-3.8-flash-high` |
| `gemini-3.7-flash` | - `gemini-3.7-flash-low`<br>- `gemini-3.7-flash-medium`<br>- `gemini-3.7-flash-high` |
| `gemini-3.6-flash` | - `gemini-3.6-flash-minimal`<br>- `gemini-3.6-flash-low`<br>- `gemini-3.6-flash-medium`<br>- `gemini-3.6-flash-high` |
| gemini | - `gemini-3.1-pro`<br>- `gemini-3.5-flash`<br>- `gemini-3-flash` |
| `claude-4.6` | - `claude-4.6-sonnet-medium`<br>- `claude-4.6-sonnet-medium-thinking`<br>- `claude-4.6-opus-high`<br>- `claude-4.6-opus-max`<br>- `claude-4.6-opus-high-thinking`<br>- `claude-4.6-opus-max-thinking` |
| `claude-4.5` / `claude-4` | - `claude-4.5-opus-high`<br>- `claude-4.5-opus-high-thinking`<br>- `claude-4.5-sonnet`<br>- `claude-4.5-sonnet-thinking`<br>- `claude-4-sonnet`<br>- `claude-4-sonnet-thinking` |
| `gpt-5.1` / mini | - `gpt-5.1-low`<br>- `gpt-5.1`<br>- `gpt-5.1-high`<br>- `gpt-5-mini` |
| `kimi` / `glm` | - `kimi-k3-low`<br>- `kimi-k3-high`<br>- `kimi-k3-max`<br>- `kimi-k2.7-code`<br>- `glm-5.2-high`<br>- `glm-5.2-max` |

</details>

## Claude Code (`claude`)

QuickCall generates a UUID for Claude's `--session-id`, assigns qc's printable id through `--name`, and passes the prompt as the final argument. Resume uses the stored UUID. The `off` and `minimal` thinking levels warn and are ignored.

```bash
qc review --tool claude --model sonnet --thinking high
```

| Item | Native | qc flag | Frontmatter |
| --- | --- | --- | --- |
| Adapter | binary `claude` | `--tool claude` | `qc_tool: claude` |
| Headless JSON | `-p --output-format json`; prompt as final argument | Automatic | None |
| Force-allow | `--dangerously-skip-permissions` (deny rules and remaining print-mode prompts still bind) | Automatic | None |
| Session create | `--session-id <UUID>` + `--name <pretty-id>` | qc generates both ids | None |
| Session resume | `--resume <uuid>` | `-c <pretty-id>` | None |
| TUI open | `--resume <uuid>` (no `-p`, no `--dangerously-skip-permissions`) | `-o` / `-o <pretty-id>` | None |
| Cwd | spawn cwd (no native cwd flag) | `--workdir` | `qc_workdir` |
| Thinking | `--effort` (`low`\|`medium`\|`high`\|`xhigh`\|`max`) | `--thinking` | `qc_thinking` |
| Model | `--model <alias or full name>` | `--model` | `qc_model` |

### Models and documentation

Documented aliases include `default`, `best`, `fable`, `sonnet`, `opus`, `haiku`, `sonnet[1m]`, `opus[1m]`, and `opusplan`.

Full-name examples: `claude-opus-5`, `claude-sonnet-5`, `claude-fable-5`, `claude-haiku-4-5`.

- [Claude Code CLI reference](https://code.claude.com/docs/en/cli-reference)
- [Claude Code headless mode](https://code.claude.com/docs/en/headless)
- [Claude Code model configuration](https://code.claude.com/docs/en/model-config)
- [Claude Code sessions](https://code.claude.com/docs/en/sessions)

## OpenCode (`opencode`)

QuickCall passes the prompt as the final argument and stores OpenCode's native `ses_*` id. A new session uses qc's printable id as its title.

OpenCode's native `--thinking` displays thinking blocks. It does not control `qc_thinking`; QuickCall maps `qc_thinking` to `--variant`.

```bash
qc review --tool opencode --model opencode-go/deepseek-v4-flash --thinking high
```

| Item | Native | qc flag | Frontmatter |
| --- | --- | --- | --- |
| Adapter | binary `opencode` | `--tool opencode` | `qc_tool: opencode` |
| Headless JSON | `run --format json`; prompt as final argument | Automatic | None |
| Force-allow | `--auto` (auto-approve permissions not explicitly denied) | Automatic | None |
| Session create | Native `ses_*`; `--title <pretty-id>` | qc printable id becomes the title | None |
| Session resume | `-s <ses_*>` | `-c <pretty-id>` | None |
| TUI open | `-s <ses_*>` (default TUI, not `run`, not `--auto`) | `-o` / `-o <pretty-id>` | None |
| Cwd | `--dir <path>` | `--workdir` | `qc_workdir` |
| Thinking | `--variant <level>` | `--thinking` | `qc_thinking` |
| Model | `-m <provider/model>` | `--model` | `qc_model` |

### Models and documentation

Use `opencode/<id>` for Zen models and `opencode-go/<id>` for Go models.

Refresh the installed CLI's model list before saving a model:

```bash
opencode models
```

- [OpenCode CLI](https://opencode.ai/docs/cli/)
- [OpenCode permissions](https://opencode.ai/docs/permissions/)
- [OpenCode models](https://opencode.ai/docs/models/)
- [OpenCode Go](https://opencode.ai/docs/go/)
- [OpenCode Zen](https://opencode.ai/docs/zen/)

### OpenCode Go model snapshot

The [live OpenCode Go catalog](https://opencode.ai/zen/go/v1/models) is authoritative. The static documentation can lag behind it.

<details>
<summary>Show the 27 IDs and advertised `--variant` values verified on 2026-09-06</summary>

Use each id as `opencode-go/<id>`. `--thinking` maps to `--variant`. Values below are the keys from `opencode models opencode-go --verbose`. qc forwards any `--thinking` string; it does not check this table. `none` and `thinking` are native variant keys, not qc's canonical `off`.

Latest: [live OpenCode Go catalog](https://opencode.ai/zen/go/v1/models)

| ID | Advertised `--variant` |
| --- | --- |
| `deepseek-v4-flash` | `low`, `high`, `max` |
| `deepseek-v4-flash-vision-exp` | `low`, `high`, `max` |
| `deepseek-v4-pro` | `high`, `max` |
| `glm-5.1` | (empty) |
| `glm-5.2` | `high`, `max` |
| `glm-5.3` | `low`, `high`, `max` |
| `glm-5.3-flash` | `low`, `high`, `max` |
| `gpt-5.6-luna` | `none`, `low`, `medium`, `high`, `xhigh`, `max` |
| `grok-4.6` | `low`, `medium`, `high`, `xhigh` |
| `hy3` | `none`, `low`, `high` |
| `hy4-preview` | `none`, `high` |
| `kimi-k2.6` | (empty) |
| `kimi-k2.7-code` | (empty) |
| `kimi-k3` | `max` |
| `longcat-2.0` | `low`, `medium`, `high` |
| `mimo-v2.5` | (empty) |
| `mimo-v2.5-pro` | (empty) |
| `minimax-m2.7` | (empty) |
| `minimax-m3` | `none`, `thinking` |
| `muse-spark-1.2-contributor` | `minimal`, `low`, `medium`, `high`, `xhigh` |
| `muse-spark-1.3-contributor` | `minimal`, `low`, `medium`, `high`, `xhigh` |
| `omen-alpha` | `low`, `high` |
| `qwen3.6-plus` | (empty) |
| `qwen3.7-max` | (empty) |
| `qwen3.7-plus` | (empty) |
| `qwen3.8-flash` | `low`, `medium`, `xhigh` |
| `qwen3.8-max` | `low`, `medium`, `xhigh` |

</details>

## Antigravity (`antigravity`, binary `agy`)

Use `antigravity` as the qc tool name. QuickCall runs the `agy` binary with options first, then `-p` immediately followed by the prompt (final argv). `agy -p` takes the next token as the prompt value — a flag after `-p` becomes the prompt. Antigravity returns a native JSON `conversation_id`; qc stores it behind qc's printable session id. Thinking values other than `low`, `medium`, and `high` warn and are ignored.

```bash
qc review --tool antigravity --thinking medium
```

| Item | Native | qc flag | Frontmatter |
| --- | --- | --- | --- |
| Adapter | binary `agy` | `--tool antigravity` | `qc_tool: antigravity` |
| Headless JSON | `--output-format json`; options then `-p <prompt>` last | Automatic | None |
| Force-allow | `--dangerously-skip-permissions` | Automatic | None |
| Session create | JSON `conversation_id` | qc stores the native id | None |
| Session resume | `--conversation <id>` | `-c <pretty-id>` | None |
| TUI open | `--conversation <id>` (no `-p`, no json, no skip-permissions, no `--add-dir`) | `-o` / `-o <pretty-id>` | None |
| Cwd | spawn cwd + `--add-dir <path>` | `--workdir` | `qc_workdir` |
| Thinking | `--effort` (`low`\|`medium`\|`high` only) | `--thinking` | `qc_thinking` |
| Model | `--model <agy model id>` | `--model` | `qc_model` |

### Models and documentation

`qc_thinking` maps to `--effort`. Model slugs may still contain suffixes such as `-medium`; those are part of the exact `--model` id, not the thinking flag.

```bash
agy models
```

- [Antigravity headless CLI](https://antigravity.google/docs/cli/headless/)
- [Antigravity models](https://antigravity.google/docs/models/)
- [Antigravity permissions](https://antigravity.google/docs/cli/permissions/)

### Antigravity model snapshot

<details>
<summary>Show the account snapshot from 2026-09-03</summary>

One-machine account snapshot from `agy models`. Always re-run `agy models` before relying on a slug. Live stamp used `gemini-3.6-flash-medium`.

Latest: [Antigravity models](https://antigravity.google/docs/models/)

| Family | IDs |
| --- | --- |
| `gemini-3.8-flash` | - `gemini-3.8-flash-high`<br>- `gemini-3.8-flash-medium`<br>- `gemini-3.8-flash-low` |
| `gemini-3.7-flash` | - `gemini-3.7-flash-high`<br>- `gemini-3.7-flash-medium`<br>- `gemini-3.7-flash-low` |
| `gemini-3.6-flash` | - `gemini-3.6-flash-high`<br>- `gemini-3.6-flash-medium`<br>- `gemini-3.6-flash-low` |
| `gemini-3.1-pro` | - `gemini-3.1-pro-high`<br>- `gemini-3.1-pro-low` |
| `claude` | - `claude-sonnet-4-6`<br>- `claude-opus-4-6-thinking` |
| `gpt-oss` | - `gpt-oss-120b-medium` |

</details>

## Codex (`codex`)

Native OpenAI Codex CLI 0.151.0. Binary `codex`. This adapter is not Pi's `openai-codex/<id>` transport. `--tool codex` uses native model ids such as `gpt-5.6-luna`. Stay on `--tool pi` when you want Pi `openai-codex/` models.

QuickCall passes the prompt on stdin (`-`). It persists observed JSONL `thread.started.thread_id` only. It never stores the qc pretty id as the Codex native id. Sign in with ChatGPT / Codex CLI auth on the host before the first run.

```bash
qc review --tool codex --model gpt-5.6-luna --thinking medium
```

| Item | Native | qc flag | Frontmatter |
| --- | --- | --- | --- |
| Adapter | binary `codex` | `--tool codex` | `qc_tool: codex` |
| Headless JSON | `exec --json`; prompt on stdin (`-`) | Automatic | None |
| Force-allow | `--dangerously-bypass-approvals-and-sandbox --dangerously-bypass-hook-trust` | Automatic | None |
| Session create | JSONL `thread.started.thread_id` | qc stores the native id | None |
| Session resume | same `exec` controls, then `resume <thread-id> -` | `-c <pretty-id>` | None |
| TUI open | `resume <native_id>` (no JSON, model, thinking, `--cd`, or force-allow) | `-o` / `-o <pretty-id>` | None |
| Cwd | `--cd <path>` (+ spawn cwd) | `--workdir` | `qc_workdir` |
| Thinking | `-c model_reasoning_effort="<mapped>"` (`off` → `none`; `minimal`\|`low`\|`medium`\|`high`\|`xhigh`\|`max` pass through) | `--thinking` | `qc_thinking` |
| Model | `--model <native id>` | `--model` | `qc_model` |

Headless create with thinking and model: `-c model_reasoning_effort="<mapped>" exec --json --model <model> --cd <cwd> --dangerously-bypass-approvals-and-sandbox --dangerously-bypass-hook-trust -`. Resume inserts `resume <thread-id>` before `-`. Usage comes from JSONL `turn.completed.usage`: `input_tokens`, `output_tokens`, `cached_input_tokens` as cache, `reasoning_output_tokens` as think. No invented totals or cost. Fatal errors are `turn.failed.error.message` or top-level `error.message`. Missing binary writes no mapping.

### Models and documentation

Native Codex model ids are not Pi `openai-codex/` prefixes. Do not copy a Pi openai-codex snapshot as a native Codex id. Packaged starter: `[tool.codex] default_model = "gpt-5.6-luna"`, `default_thinking = "medium"`.

```bash
codex --help
```

- [OpenAI Codex models](https://developers.openai.com/codex/models)
