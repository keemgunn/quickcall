---
description: Create a detailed, trusted qc Markdown prompt at a direct, project-alias, or global-alias location and report its validated invocation.
name: qc-create-prompt
disable-model-invocation: true
slash: true
metadata:
  opencode:
    autoinvoke: false
---

# qc-create-prompt

Create one reusable qc-compatible Markdown prompt. Write the file; do not merely return draft text. For a one-off instruction with no reusable file, use `qc -q --append` instead of creating a prompt.

## Gather

Derive existing answers from the request and workspace. Ask one compact question containing only missing essentials:

- Purpose: the repeatable job, expected result, and essential constraints.
- Location: direct file, current-project alias, or global alias.
- Settings: only explicit tool, model, thinking, workdir, or Pi skill behavior that must differ from defaults.

Do not ask for optional fields the prompt does not need. Do not invent model IDs, thinking values, skill paths, or shell commands.

## Choose Location

| Intent | File path | Invocation form |
| --- | --- | --- |
| Reusable only in the current project | `<project>/.qc/prompts/<alias>.md` | Run from that project: `qc -q <alias>` |
| Reusable across projects for this user | `~/.qc/prompts/<alias>.md` | `qc -q <alias>` when no cwd project alias shadows it |
| Explicit path or prompt outside alias stores | User-selected path ending `.md` | `qc -q <direct-path>.md` |

Aliases may contain nested categories but must not contain `..` or escape `prompts/` through symlinks. If scope is unclear, ask project, global, or direct. Never overwrite an existing prompt without explicit confirmation.

## Write The Prompt

Use top-of-file YAML frontmatter. Include `description` and only the canonical `qc_*` fields required by the requested behavior:

```yaml
---
description: <compact reusable purpose>
qc_tool: <pi|cursor|claude|opencode|antigravity|codex only when required>
qc_model: <model only when required>
qc_thinking: <thinking only when required>
qc_workdir: <path only when required>
qc_no_skills: <true only when required; pi only>
qc_skill_path: <one path only when required; pi only>
---
```

Delete every unused line from the template. Never emit empty settings or use alternative field prefixes, shell settings, arbitrary agent flags, `$1`, or `$ARGUMENTS`. Never write removed keys `qc_cli` or `qc_approve`.

Provider catalogs and thinking maps: sibling skill `qc`, `references/providers.md`.

Write a detailed task body with only useful sections. Prefer this order when each section adds information:

1. Clear task title.
2. Purpose and concrete outcome.
3. Required context or inputs available at runtime.
4. Ordered work instructions.
5. Constraints and trust limits.
6. Acceptance checks.
7. Required response/report shape.

Make instructions reusable, imperative, and specific. Use exact paths and commands only when known. Do not embed temporary chat context as a permanent assumption.

## Check Shell Output

Search the complete body for exact expressions in this form:

```markdown
!`command`
```

For each expression:

- Confirm its output is needed as prompt text.
- Confirm the command and prompt source are trusted.
- Inspect compound syntax, pipelines, subshells, and command substitutions for every executable segment.
- Explain that commands run in the effective workdir with inherited environment.

Remember that the caller's future `--append` text can contain the same executable syntax. Project `.qc/config.toml` is trusted executable configuration because it can select the shell.

## Validate

Before reporting:

1. Confirm the target is a readable regular `.md` file.
2. Parse the top frontmatter shape and verify every runtime field uses the canonical `qc_*` name and required string/boolean type.
3. Derive lookup from the intended invocation cwd: direct references resolve only to that path; aliases check project before global.
4. Reject traversal, symlink escape, or an unintended project alias shadowing a global alias.
5. Re-read the body for unresolved placeholders, accidental executable expressions, and missing acceptance criteria.
6. Construct a shell-quoted invocation example that includes `-q` plus only needed options.

Do not execute qc merely to validate it: qc launches an agent and may execute shell expressions. Run it only when the user explicitly asks to execute the new prompt.

## Report

Return:

- Created file path.
- Scope: direct, project alias, or global alias.
- Exact invocation example that includes `-q`, and required cwd when relevant.
- Included `qc_*` settings.
- Shell expressions and trust findings, or `none`.
