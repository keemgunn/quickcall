---
type: release_note
project: "quickcall"
description: "Native Rust qc, six providers including Codex, and faster startup."
date: "2026-09-21T14:50:02.938Z"
release_channel: "public"
version_code: "v0.2.0"
---

## Highlights

- `qc` is now a native Rust binary inside the same npm package `@keemgunn/quickcall`. You do not install Rust.
- Six providers: Pi, Cursor Agent, Claude Code, OpenCode, Antigravity, and OpenAI Codex CLI. Pi stays the default.
- Flags, prompts, TOML, session JSON, envelopes, and `-o/--open` are unchanged. Existing `~/.qc/` files keep working.
- npm `bin/qc` warm `--version` is about 31ms, down from about 66ms on the previous Node application. Native `qc` itself is about 3ms.

## New

- The package ships prebuilt `qc` and `qc-bootstrap` for macOS and Linux, x64 and ARM64, glibc and musl. Node.js 24+ only selects the host binary.
- `--tool codex` and prompt `qc_tool: codex` run a headless Codex turn and print the usual stdout envelope. Native `--tool codex` is distinct from Pi `openai-codex/` model ids.
- `-c <id>` resumes `codex exec … resume <thread-id>`. `-o <id>` opens `codex resume <thread-id>` without force-allow flags.
- Packaged Codex default is native model `gpt-5.6-luna` with thinking `medium`. Confirm ids in PROVIDERS.md.
- Codex thinking: qc `off` maps to Codex `none`. `minimal`, `low`, `medium`, `high`, `xhigh`, and `max` pass through. Other values are ignored with a warning.

## Improved

- Startup is faster because Node no longer runs the application. The POSIX `bin/qc` launcher execs `dist/<triple>/qc`.

## Upgrade notes

- Requires Node.js 24 or newer, a Unix host, and at least one supported agent CLI on `PATH`. QuickCall does not install those CLIs. No Rust toolchain is required to install or run `qc`.
- macOS 13.5 or newer. Linux gnu needs glibc 2.28 or newer. Musl binaries are static. Windows is still unsupported.
- Existing `~/.qc/config.toml` and `$HOME/.qc/sessions/*.json` stay as they are. No conversion step.
- Install the OpenAI Codex CLI and put `codex` on PATH before using `--tool codex`. Missing `codex` is a hard error. qc does not fall back to Pi.
- Re-run `qc --install-agent-harness` or reinstall with `--allow-scripts=@keemgunn/quickcall` so packaged skills pick up the six-provider catalog.
- Verify the install with `qc --version` after this tag lands. It should report `0.2.0`.
