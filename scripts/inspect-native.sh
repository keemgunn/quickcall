#!/usr/bin/env bash
# Inspect one staged native pair. Compile+inspect is not native execution proof.
set -euo pipefail

SOURCE="${BASH_SOURCE[0]:-$0}"
while [ -L "$SOURCE" ]; do
  DIR="$(cd -- "$(dirname -- "$SOURCE")" && pwd)"
  SOURCE="$(readlink "$SOURCE")"
  [[ "$SOURCE" != /* ]] && SOURCE="$DIR/$SOURCE"
done
SCRIPT_DIR="$(cd -- "$(dirname -- "$SOURCE")" && pwd -P)"
PACKAGE_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd -P)"
# shellcheck source=/dev/null
source "$SCRIPT_DIR/native-targets.sh"

TRIPLE=""
BIN_DIR=""
OUT=""

usage() {
  echo "usage: scripts/inspect-native.sh --triple <triple> [--bin-dir <dir>] [--out <file>]" >&2
  exit 2
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --triple) TRIPLE="${2:?}"; shift 2 ;;
    --bin-dir) BIN_DIR="${2:?}"; shift 2 ;;
    --out) OUT="${2:?}"; shift 2 ;;
    -h|--help) usage ;;
    *) echo "unknown argument: $1" >&2; usage ;;
  esac
done

[[ -n "$TRIPLE" ]] || usage
if [[ -z "$BIN_DIR" ]]; then
  BIN_DIR="$PACKAGE_ROOT/dist/$TRIPLE"
fi
if [[ "$BIN_DIR" != /* ]]; then
  BIN_DIR="$PWD/$BIN_DIR"
fi

known=0
for candidate in "${NATIVE_TARGETS[@]}"; do
  if [[ "$candidate" == "$TRIPLE" ]]; then
    known=1
    break
  fi
done
if [[ "$known" -ne 1 ]]; then
  printf 'Error: unsupported triple %s\n' "$TRIPLE" >&2
  exit 2
fi

QC="$BIN_DIR/qc"
BOOTSTRAP="$BIN_DIR/qc-bootstrap"

if [[ ! -f "$QC" || ! -f "$BOOTSTRAP" ]]; then
  printf 'Error: missing qc/qc-bootstrap under %s\n' "$BIN_DIR" >&2
  exit 1
fi
if [[ -e "$BIN_DIR/qc-test-agent" ]]; then
  printf 'Error: qc-test-agent must not ship in dist/%s\n' "$TRIPLE" >&2
  exit 1
fi

emit() {
  if [[ -n "$OUT" ]]; then
    mkdir -p -- "$(dirname -- "$OUT")"
    printf '%s\n' "$1" >>"$OUT"
  fi
  printf '%s\n' "$1"
}

if [[ -n "$OUT" ]]; then
  : >"$OUT"
fi

mode_of() {
  python3 -c 'import os,stat,sys; m=os.stat(sys.argv[1]).st_mode; print("0755" if (m & 0o777)==0o755 else oct(m & 0o777))' "$1"
}

is_exec() {
  [[ -x "$1" ]]
}

glibc_versions() {
  local bin="$1"
  # Imported GLIBC_* symbol versions. Empty on static/musl.
  strings "$bin" 2>/dev/null | grep -oE 'GLIBC_[0-9]+\.[0-9]+' | sort -u -V || true
}

max_glibc() {
  local versions="$1"
  if [[ -z "$versions" ]]; then
    printf ''
    return 0
  fi
  printf '%s\n' "$versions" | sort -V | tail -1
}

version_gt() {
  # true if $1 > $2 (dotted numeric)
  local IFS=.
  local -a a b
  read -r -a a <<<"$1"
  read -r -a b <<<"$2"
  local i
  for i in 0 1 2; do
    local av="${a[i]:-0}"
    local bv="${b[i]:-0}"
    if ((10#$av > 10#$bv)); then
      return 0
    fi
    if ((10#$av < 10#$bv)); then
      return 1
    fi
  done
  return 1
}

fail=0
for bin_name in "${NATIVE_BINARIES[@]}"; do
  path="$BIN_DIR/$bin_name"
  emit "=== $TRIPLE $bin_name ==="
  emit "path=$path"
  if ! is_exec "$path"; then
    emit "ERROR: not executable"
    fail=1
  fi
  mode="$(mode_of "$path")"
  emit "mode=$mode"
  if [[ "$mode" != "0755" ]]; then
    emit "ERROR: expected mode 0755"
    fail=1
  fi
  if command -v file >/dev/null 2>&1; then
    emit "file=$(file -b "$path")"
  fi
  case "$TRIPLE" in
    *-apple-darwin)
      if command -v otool >/dev/null 2>&1; then
        emit "otool-h:"
        otool -hv "$path" 2>/dev/null | while IFS= read -r line; do emit "  $line"; done
        minos="$(otool -l "$path" 2>/dev/null | awk '/minos/{print $2; exit}')"
        emit "minos=${minos:-unknown}"
        if [[ -n "${minos:-}" && "$minos" != "unknown" ]] && version_gt "13.5" "$minos"; then
          emit "ERROR: minos $minos is below MACOSX_DEPLOYMENT_TARGET 13.5"
          fail=1
        fi
      fi
      case "$TRIPLE" in
        aarch64-*)
          if command -v file >/dev/null 2>&1 && ! file "$path" | grep -qi 'arm64'; then
            emit "ERROR: expected arm64 Mach-O"
            fail=1
          fi
          ;;
        x86_64-*)
          if command -v file >/dev/null 2>&1 && ! file "$path" | grep -qi 'x86_64'; then
            emit "ERROR: expected x86_64 Mach-O"
            fail=1
          fi
          ;;
      esac
      ;;
    *-unknown-linux-gnu)
      versions="$(glibc_versions "$path")"
      if [[ -z "$versions" ]]; then
        emit "ERROR: no GLIBC_* symbols (expected dynamically linked gnu)"
        fail=1
      else
        emit "glibc-symbols:"
        printf '%s\n' "$versions" | while IFS= read -r v; do emit "  $v"; done
        max="$(max_glibc "$versions")"
        emit "glibc-max=$max (floor $GLIBC_FLOOR)"
        ver="${max#GLIBC_}"
        if version_gt "$ver" "$GLIBC_FLOOR"; then
          emit "ERROR: imported GLIBC $ver exceeds floor $GLIBC_FLOOR"
          fail=1
        fi
      fi
      if command -v file >/dev/null 2>&1 && file "$path" | grep -qi 'statically linked'; then
        emit "ERROR: gnu target must not be fully static"
        fail=1
      fi
      if command -v llvm-readobj >/dev/null 2>&1; then
        emit "llvm-readobj-file-header:"
        llvm-readobj --file-headers "$path" 2>/dev/null | head -40 | while IFS= read -r line; do emit "  $line"; done
      fi
      ;;
    *-unknown-linux-musl)
      if command -v file >/dev/null 2>&1; then
        desc="$(file -b "$path")"
        emit "linkage-note=$desc"
        if ! printf '%s' "$desc" | grep -qi 'statically linked'; then
          emit "WARN: musl binary not reported as statically linked; treating as pending confirmation"
        fi
      fi
      versions="$(glibc_versions "$path")"
      if [[ -n "$versions" ]]; then
        emit "ERROR: musl binary imports GLIBC symbols"
        fail=1
      fi
      if command -v llvm-readobj >/dev/null 2>&1; then
        emit "llvm-readobj-file-header:"
        llvm-readobj --file-headers "$path" 2>/dev/null | head -40 | while IFS= read -r line; do emit "  $line"; done
      fi
      ;;
  esac
done

emit "native-run=pending-unless-host"
if [[ "$fail" -ne 0 ]]; then
  emit "result=FAIL"
  exit 1
fi
emit "result=PASS-inspect"
exit 0
