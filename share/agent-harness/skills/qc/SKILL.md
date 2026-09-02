---
name: qc
description: Invoke trusted reusable Markdown prompts through the qc wrapper around Pi print mode. Load when an agent needs qc prompt lookup, metadata, append text, shell-output expansion, or failure handling.
compatibility: agnostic
metadata:
  audience: agents
  domain: cli
---

# QuickCall

Use `qc` to resolve one Markdown prompt, apply canonical `qc_*` settings, optionally append user text, expand shell output, and send the result to the required `pi` executable.

## Ground Truth

Read the package README (`README.md` after `npm install -g @keemgunn/quickcall`) for installation and current user-facing behavior. Install packaged harness assets with `qc --install-agent-harness` (skills only) or `qc --install-agent-harness <framework> ...` (commands plus skills).

## Invoke Safely

1. Identify the invocation cwd, prompt reference, optional append text, and whether each source is trusted.
2. Inspect `<cwd>/.qc/config.toml` when present. It can select a shell and override global command permissions.
3. Inspect the prompt and append text for exact shell-output expressions before running qc.
4. Invoke `qc <prompt-reference>` with only required `--shell <path>` or `--append <text>` options.
5. Treat qc/Pi output and status as the command result; do not rewrite or hide failures.

```bash
qc some-category/some-prompt
qc ./prompts/task.md --append "Limit the report to changed files."
qc some-prompt --shell /usr/bin/bash
```

## Lookup

- Absolute paths, `./...`, `../...`, and references ending `.md` are direct paths only.
- Other references are aliases checked at `<cwd>/.qc/prompts/<alias>.md`, then `~/.qc/prompts/<alias>.md`.
- Project aliases override global aliases. Nested aliases are supported but cannot escape their prompt root.

## Prompt Metadata

Use only canonical fields when authoring or reviewing prompts:

| Field | Meaning |
| --- | --- |
| `qc_cli` | Runtime backend; v1 allows only `pi` |
| `qc_model` | Pi model value |
| `qc_thinking` | Pi thinking value |
| `qc_no_skills` | Boolean; `true` adds `--no-skills` |
| `qc_skill_path` | One Pi skill path |
| `qc_approve` | Boolean approval choice |
| `description` | Documentation only |

Never use legacy `qpi_*`, `pqi_*`, or `pi_*` prompt keys.

## Execution Risk

The exact form below executes through the selected shell:

```markdown
!`command`
```

It can appear in the prompt or `--append` text. qc checks every parsed command segment against ordered global then project permissions before execution. With configured permissions, unmatched or denied segments stop the run; without either table, substitutions are allowed. Permission checks do not make untrusted prompts or project config safe.

## Failures

- Preserve `qc: error:` diagnostics and fix the named argument, path, config, frontmatter, permission, shell, or Pi prerequisite.
- Do not bypass a denial, weaken permission rules, or replace the selected shell without explicit user approval.
- Do not retry a prompt that may have produced side effects until its state is inspected.
- Pi stdout/stderr and non-zero status pass through; distinguish those from qc-owned diagnostics.
