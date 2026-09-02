#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]:-$0}")" && pwd -P)"
pnpm --dir "$SCRIPT_DIR" install --frozen-lockfile
pnpm --dir "$SCRIPT_DIR" run build
