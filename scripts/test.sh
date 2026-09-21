#!/usr/bin/env bash
set -euo pipefail

SOURCE="${BASH_SOURCE[0]:-$0}"
while [ -L "$SOURCE" ]; do
  DIR="$(cd -- "$(dirname -- "$SOURCE")" && pwd)"
  SOURCE="$(readlink "$SOURCE")"
  [[ "$SOURCE" != /* ]] && SOURCE="$DIR/$SOURCE"
done
SCRIPT_DIR="$(cd -- "$(dirname -- "$SOURCE")" && pwd -P)"

usage() {
  cat <<'EOF'
Usage: scripts/test.sh [unit|integration|smoke|package|differential|performance|all]
         [--legacy-root <path>]
         [--mode host|final-artifact]
         [--artifact <path>]
         [--checksum <sha256>]

Child Cargo layer runner.

  unit          cargo test --locked --test unit (enables qc-test-agent)
  integration   cargo test --locked --test integration --features test-agent
  smoke         cargo test --locked --test smoke --features test-agent
  package       cargo test --locked --test package --features test-agent
  differential  cargo test --locked --test differential (requires --legacy-root)
  performance   cargo test --locked --test performance (release-artifact timings; not in all)
  all           unit, integration, smoke, then host-mode package (default; excludes differential and performance)
EOF
}

if ! command -v cargo >/dev/null 2>&1; then
  printf 'Error: cargo not on PATH\nPATH=%s\n' "$PATH" >&2
  exit 127
fi

run_unit() {
  (cd -- "$PACKAGE_ROOT" && cargo test --locked --test unit --manifest-path "$PACKAGE_ROOT/Cargo.toml" --features test-agent)
}

run_integration() {
  (cd -- "$PACKAGE_ROOT" && cargo test --locked --test integration --manifest-path "$PACKAGE_ROOT/Cargo.toml" --features test-agent -- --test-threads=1)
}

run_smoke() {
  (cd -- "$PACKAGE_ROOT" && cargo test --locked --test smoke --manifest-path "$PACKAGE_ROOT/Cargo.toml" --features test-agent -- --test-threads=1)
}

run_package() {
  (cd -- "$PACKAGE_ROOT" && cargo test --locked --test package --manifest-path "$PACKAGE_ROOT/Cargo.toml" --features test-agent -- --test-threads=1)
}

run_differential() {
  local legacy_root="${QC_LEGACY_ROOT:-}"
  if [[ -z "$legacy_root" ]]; then
    printf 'Error: differential requires --legacy-root <path>\n' >&2
    exit 2
  fi
  if [[ ! -f "$legacy_root/dist/cli.js" ]]; then
    printf 'Error: legacy oracle missing at %s/dist/cli.js\n' "$legacy_root" >&2
    exit 1
  fi
  (
    cd -- "$PACKAGE_ROOT"
    QC_LEGACY_ROOT="$legacy_root" cargo test --locked --test differential --manifest-path "$PACKAGE_ROOT/Cargo.toml" --features test-agent -- --test-threads=1
  )
}

run_performance() {
  (cd -- "$PACKAGE_ROOT" && cargo test --locked --test performance --manifest-path "$PACKAGE_ROOT/Cargo.toml" --features test-agent -- --nocapture --test-threads=1)
}

PACKAGE_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd -P)"
LAYER="${1:-all}"
if [[ $# -gt 0 ]]; then
  shift
fi
LEGACY_ROOT=""
PACKAGE_MODE=""
PACKAGE_ARTIFACT=""
PACKAGE_CHECKSUM=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --legacy-root)
      LEGACY_ROOT="${2:?--legacy-root requires a path}"
      shift 2
      ;;
    --mode)
      PACKAGE_MODE="${2:?--mode requires host or final-artifact}"
      shift 2
      ;;
    --artifact)
      PACKAGE_ARTIFACT="${2:?--artifact requires a path}"
      shift 2
      ;;
    --checksum)
      PACKAGE_CHECKSUM="${2:?--checksum requires a sha256}"
      shift 2
      ;;
    *)
      printf 'Error: unknown argument %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done
if [[ -n "$LEGACY_ROOT" ]]; then
  export QC_LEGACY_ROOT="$LEGACY_ROOT"
fi
if [[ -n "$PACKAGE_MODE" ]]; then
  export QC_PACKAGE_MODE="$PACKAGE_MODE"
fi
if [[ "${PACKAGE_MODE:-}" == "final-artifact" ]]; then
  if [[ -z "$PACKAGE_ARTIFACT" || -z "$PACKAGE_CHECKSUM" ]]; then
    printf 'Error: final-artifact requires --artifact <path> and --checksum <sha256>\n' >&2
    exit 2
  fi
fi
if [[ -n "$PACKAGE_ARTIFACT" ]]; then
  export QC_PACKAGE_ARTIFACT="$PACKAGE_ARTIFACT"
fi
if [[ -n "$PACKAGE_CHECKSUM" ]]; then
  export QC_PACKAGE_CHECKSUM="$PACKAGE_CHECKSUM"
fi

case "$LAYER" in
  unit)
    run_unit
    ;;
  integration)
    run_integration
    ;;
  smoke)
    run_smoke
    ;;
  package)
    run_package
    ;;
  differential)
    run_differential
    ;;
  performance)
    run_performance
    ;;
  all)
    run_unit
    run_integration
    run_smoke
    run_package
    ;;
  -h|--help)
    usage
    exit 0
    ;;
  *)
    printf 'Error: unknown layer %s\n' "$LAYER" >&2
    usage >&2
    exit 2
    ;;
esac
