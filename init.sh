#!/usr/bin/env bash
set -euo pipefail

SOURCE="${BASH_SOURCE[0]:-$0}"
while [ -L "$SOURCE" ]; do
  DIR="$(cd -- "$(dirname -- "$SOURCE")" && pwd)"
  SOURCE="$(readlink "$SOURCE")"
  [[ "$SOURCE" != /* ]] && SOURCE="$DIR/$SOURCE"
done
SCRIPT_DIR="$(cd -- "$(dirname -- "$SOURCE")" && pwd -P)"

if ! command -v pnpm >/dev/null 2>&1; then
  printf 'Error: pnpm not on PATH\nPATH=%s\n' "$PATH" >&2
  exit 127
fi
if ! command -v cargo >/dev/null 2>&1; then
  printf 'Error: cargo not on PATH\nPATH=%s\n' "$PATH" >&2
  exit 127
fi

# Isolated HOME so developer setup cannot refresh the real ~/.qc.
INIT_TMP="$SCRIPT_DIR/.tmp/init-$$"
mkdir -p -- "$INIT_TMP/home"
cleanup_init() {
  rm -rf -- "$INIT_TMP"
}
trap cleanup_init EXIT

HOME="$INIT_TMP/home" pnpm --dir "$SCRIPT_DIR" install --frozen-lockfile --ignore-scripts
(cd -- "$SCRIPT_DIR" && cargo fetch --locked --manifest-path "$SCRIPT_DIR/Cargo.toml")
"$SCRIPT_DIR/scripts/build.sh" --profile debug
