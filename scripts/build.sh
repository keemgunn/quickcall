#!/usr/bin/env bash
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

usage() {
  cat <<'EOF'
Usage: scripts/build.sh [--profile debug|release] [--target <triple>]

Build qc and qc-bootstrap, then stage an installed layout.

  --profile debug    (default) cargo debug; stage at build/dev/dist/<triple>/
                     plus build/dev/package.json and build/dev/share/
  --profile release  cargo release; stage binaries at dist/<triple>/
  --target <triple>  default: host triple from rustc

Linux gnu from macOS uses cargo zigbuild with glibc 2.28.
Linux musl from macOS uses cargo zigbuild (static).
Apple targets set MACOSX_DEPLOYMENT_TARGET=13.5.
Does not build or copy qc-test-agent. Raw target/ is not a CLI entry.
EOF
}

PROFILE="debug"
TARGET=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile)
      PROFILE="${2:?--profile requires debug or release}"
      shift 2
      ;;
    --target)
      TARGET="${2:?--target requires a triple}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'Error: unknown argument %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ "$PROFILE" != "debug" && "$PROFILE" != "release" ]]; then
  printf 'Error: --profile must be debug or release\n' >&2
  exit 2
fi

if ! command -v rustc >/dev/null 2>&1; then
  printf 'Error: rustc not on PATH\nPATH=%s\n' "$PATH" >&2
  exit 127
fi
if ! command -v cargo >/dev/null 2>&1; then
  printf 'Error: cargo not on PATH\nPATH=%s\n' "$PATH" >&2
  exit 127
fi

HOST_TRIPLE="$(cd -- "$PACKAGE_ROOT" && rustc -vV | awk '/^host:/{print $2}')"
if [[ -z "$TARGET" ]]; then
  TARGET="$HOST_TRIPLE"
fi
if [[ -z "$TARGET" ]]; then
  printf 'Error: could not determine host target triple\n' >&2
  exit 1
fi

known=0
for candidate in "${NATIVE_TARGETS[@]}"; do
  if [[ "$candidate" == "$TARGET" ]]; then
    known=1
    break
  fi
done
if [[ "$known" -ne 1 ]]; then
  printf 'Error: unsupported target %s\n' "$TARGET" >&2
  exit 2
fi

CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PACKAGE_ROOT/target}"
if [[ "$PROFILE" == "release" ]]; then
  ARTIFACT_DIR="$CARGO_TARGET_DIR/$TARGET/release"
  STAGE_BIN_DIR="$PACKAGE_ROOT/dist/$TARGET"
else
  ARTIFACT_DIR="$CARGO_TARGET_DIR/$TARGET/debug"
  STAGE_ROOT="$PACKAGE_ROOT/build/dev"
  STAGE_BIN_DIR="$STAGE_ROOT/dist/$TARGET"
fi

# Drop caller host-only CPU flags before setting platform floors.
unset RUSTFLAGS || true

CARGO_CMD=(cargo)
CARGO_SUBCOMMAND=build
CARGO_TARGET_FLAG="$TARGET"
USE_ZIGBUILD=0

case "$TARGET" in
  *-apple-darwin)
    export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-$MACOSX_DEPLOYMENT_TARGET_VALUE}"
    export RUSTFLAGS="-C link-arg=-mmacosx-version-min=${MACOSX_DEPLOYMENT_TARGET}"
    ;;
  *-unknown-linux-gnu)
    # Cross from a non-matching host (this Mac) must zigbuild at glibc 2.28.
    # Native GNU CI (AlmaLinux 8) already is the floor; plain cargo build.
    if [[ "$HOST_TRIPLE" != "$TARGET" ]]; then
      USE_ZIGBUILD=1
      CARGO_TARGET_FLAG="${TARGET}.${GLIBC_FLOOR}"
    fi
    ;;
  *-unknown-linux-musl)
    if [[ "$HOST_TRIPLE" != "$TARGET" ]]; then
      USE_ZIGBUILD=1
    fi
    ;;
esac

if [[ "$USE_ZIGBUILD" -eq 1 ]]; then
  CARGO_SUBCOMMAND=zigbuild
fi

CARGO_ARGS=(
  "$CARGO_SUBCOMMAND"
  --locked
  --bin qc
  --bin qc-bootstrap
  --manifest-path "$PACKAGE_ROOT/Cargo.toml"
  --target "$CARGO_TARGET_FLAG"
)
if [[ "$PROFILE" == "release" ]]; then
  CARGO_ARGS+=(--release)
fi

if [[ "$USE_ZIGBUILD" -eq 1 ]]; then
  if ! cargo zigbuild --help >/dev/null 2>&1; then
    printf 'Error: cargo zigbuild is required to build %s from host %s\n' "$TARGET" "$HOST_TRIPLE" >&2
    printf 'PATH=%s\n' "$PATH" >&2
    exit 127
  fi
fi

# Force cargo to emit the requested pair (deployment-target / zig floor changes
# do not always invalidate a previous fingerprint).
rm -f -- "$ARTIFACT_DIR/qc" "$ARTIFACT_DIR/qc-bootstrap"

(
  cd -- "$PACKAGE_ROOT"
  "${CARGO_CMD[@]}" "${CARGO_ARGS[@]}"
)

if [[ ! -f "$ARTIFACT_DIR/qc" || ! -f "$ARTIFACT_DIR/qc-bootstrap" ]]; then
  printf 'Error: expected qc and qc-bootstrap under %s\n' "$ARTIFACT_DIR" >&2
  exit 1
fi
if [[ -e "$ARTIFACT_DIR/qc-test-agent" ]]; then
  printf 'Error: qc-test-agent must not be built into the product artifact dir\n' >&2
  exit 1
fi

mkdir -p -- "$STAGE_BIN_DIR"
cp -- "$ARTIFACT_DIR/qc" "$STAGE_BIN_DIR/qc"
cp -- "$ARTIFACT_DIR/qc-bootstrap" "$STAGE_BIN_DIR/qc-bootstrap"
chmod 755 "$STAGE_BIN_DIR/qc" "$STAGE_BIN_DIR/qc-bootstrap"
rm -f -- "$STAGE_BIN_DIR/qc-test-agent"

if [[ "$PROFILE" == "debug" ]]; then
  cp -- "$PACKAGE_ROOT/package.json" "$STAGE_ROOT/package.json"
  rm -rf -- "$STAGE_ROOT/share"
  cp -R -- "$PACKAGE_ROOT/share" "$STAGE_ROOT/share"
fi

printf 'staged %s %s -> %s\n' "$PROFILE" "$TARGET" "$STAGE_BIN_DIR"
