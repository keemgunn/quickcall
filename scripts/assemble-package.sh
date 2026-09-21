#!/usr/bin/env bash
# Assemble one npm layout from six already-built native pairs. Never builds.
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

DIST_ROOT="$PACKAGE_ROOT/dist"
OUT_DIR=""
EXPECTED_VERSION=""
EXPECTED_REVISION=""

usage() {
  echo "usage: scripts/assemble-package.sh [--dist-root <dir>] [--out <dir>] [--version <semver>] [--revision <sha>]" >&2
  exit 2
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dist-root) DIST_ROOT="${2:?}"; shift 2 ;;
    --out) OUT_DIR="${2:?}"; shift 2 ;;
    --version) EXPECTED_VERSION="${2:?}"; shift 2 ;;
    --revision) EXPECTED_REVISION="${2:?}"; shift 2 ;;
    -h|--help) usage ;;
    *) echo "unknown argument: $1" >&2; usage ;;
  esac
done

if [[ "$DIST_ROOT" != /* ]]; then
  DIST_ROOT="$PWD/$DIST_ROOT"
fi
if [[ -z "$OUT_DIR" ]]; then
  OUT_DIR="$PACKAGE_ROOT/.tmp/assembled-package"
fi
if [[ "$OUT_DIR" != /* ]]; then
  OUT_DIR="$PWD/$OUT_DIR"
fi

if ! command -v npm >/dev/null 2>&1; then
  printf 'Error: npm not on PATH\nPATH=%s\n' "$PATH" >&2
  exit 127
fi
if ! command -v node >/dev/null 2>&1; then
  printf 'Error: node not on PATH\nPATH=%s\n' "$PATH" >&2
  exit 127
fi

mkdir -p -- "$OUT_DIR"
LAYOUT="$OUT_DIR/layout"
rm -rf -- "$LAYOUT"
mkdir -p -- "$LAYOUT/dist" "$LAYOUT/bin" "$LAYOUT/scripts"

cp -- "$PACKAGE_ROOT/package.json" "$LAYOUT/"
if [[ -f "$PACKAGE_ROOT/pnpm-lock.yaml" ]]; then
  cp -- "$PACKAGE_ROOT/pnpm-lock.yaml" "$LAYOUT/"
fi
cp -- "$PACKAGE_ROOT/bin/qc" "$LAYOUT/bin/qc"
chmod 755 "$LAYOUT/bin/qc"
for name in native.mjs postinstall.mjs build.sh test.sh native-targets.sh inspect-native.sh assemble-package.sh; do
  if [[ -f "$PACKAGE_ROOT/scripts/$name" ]]; then
    cp -- "$PACKAGE_ROOT/scripts/$name" "$LAYOUT/scripts/$name"
  fi
done
chmod 755 "$LAYOUT/scripts/"*.sh 2>/dev/null || true
cp -R -- "$PACKAGE_ROOT/share" "$LAYOUT/share"
for name in README.md USAGE.md PROVIDERS.md LICENSE; do
  if [[ -f "$PACKAGE_ROOT/$name" ]]; then
    cp -- "$PACKAGE_ROOT/$name" "$LAYOUT/$name"
  fi
done

manifest_version="$(node -p 'JSON.parse(require("fs").readFileSync(process.argv[1],"utf8")).version' "$LAYOUT/package.json")"
if [[ -n "$EXPECTED_VERSION" && "$manifest_version" != "$EXPECTED_VERSION" ]]; then
  printf 'Error: package.json version %s does not match expected %s\n' "$manifest_version" "$EXPECTED_VERSION" >&2
  exit 1
fi

seen_triples=""
for triple in "${NATIVE_TARGETS[@]}"; do
  src="$DIST_ROOT/$triple"
  if [[ ! -d "$src" ]]; then
    printf 'Error: missing target directory %s\n' "$src" >&2
    exit 1
  fi
  case " $seen_triples " in
    *" $triple "*) printf 'Error: duplicate target %s\n' "$triple" >&2; exit 1 ;;
  esac
  seen_triples="$seen_triples $triple"

  dest="$LAYOUT/dist/$triple"
  mkdir -p -- "$dest"
  for bin_name in "${NATIVE_BINARIES[@]}"; do
    if [[ ! -f "$src/$bin_name" ]]; then
      printf 'Error: missing %s/%s\n' "$src" "$bin_name" >&2
      exit 1
    fi
    if [[ ! -x "$src/$bin_name" ]]; then
      printf 'Error: %s/%s is not executable\n' "$src" "$bin_name" >&2
      exit 1
    fi
    # Reject leftover debug filenames if a build dropped them here.
    if [[ -e "$src/${bin_name}.d" ]]; then
      printf 'Error: debug companion present at %s/%s.d\n' "$src" "$bin_name" >&2
      exit 1
    fi
    cp -- "$src/$bin_name" "$dest/$bin_name"
    chmod 755 "$dest/$bin_name"
  done
  if [[ -e "$src/qc-test-agent" || -e "$dest/qc-test-agent" ]]; then
    printf 'Error: qc-test-agent must not enter the assembled package\n' >&2
    exit 1
  fi
  if [[ -f "$src/metadata.json" ]]; then
    meta_profile="$(node -p 'JSON.parse(require("fs").readFileSync(process.argv[1],"utf8")).profile || ""' "$src/metadata.json")"
    meta_target="$(node -p 'JSON.parse(require("fs").readFileSync(process.argv[1],"utf8")).target || ""' "$src/metadata.json")"
    meta_rev="$(node -p 'JSON.parse(require("fs").readFileSync(process.argv[1],"utf8")).revision || ""' "$src/metadata.json")"
    if [[ "$meta_profile" != "release" ]]; then
      printf 'Error: %s metadata profile is %s, expected release\n' "$triple" "${meta_profile:-empty}" >&2
      exit 1
    fi
    if [[ -n "$meta_target" && "$meta_target" != "$triple" ]]; then
      printf 'Error: %s metadata target is %s\n' "$triple" "$meta_target" >&2
      exit 1
    fi
    if [[ -n "$EXPECTED_REVISION" && -n "$meta_rev" && "$meta_rev" != "$EXPECTED_REVISION" ]]; then
      printf 'Error: %s revision %s does not match %s\n' "$triple" "$meta_rev" "$EXPECTED_REVISION" >&2
      exit 1
    fi
  fi
done

pair_count="$(find "$LAYOUT/dist" -type f \( -name qc -o -name qc-bootstrap \) | wc -l | tr -d ' ')"
if [[ "$pair_count" != "12" ]]; then
  printf 'Error: expected 12 native executables, found %s\n' "$pair_count" >&2
  exit 1
fi
if find "$LAYOUT" -name 'qc-test-agent' | grep -q .; then
  printf 'Error: fixture binary leaked into layout\n' >&2
  exit 1
fi

PACK_DIR="$OUT_DIR/pack"
rm -rf -- "$PACK_DIR"
mkdir -p -- "$PACK_DIR"
(
  cd -- "$LAYOUT"
  npm pack --pack-destination "$PACK_DIR"
)

shopt -s nullglob
tgz_files=("$PACK_DIR"/*.tgz)
shopt -u nullglob
if [[ "${#tgz_files[@]}" -ne 1 ]]; then
  printf 'Error: expected exactly one packed tarball in %s\n' "$PACK_DIR" >&2
  exit 1
fi
TGZ="${tgz_files[0]}"
CHECKSUM_FILE="$OUT_DIR/package.sha256"
if command -v shasum >/dev/null 2>&1; then
  (cd -- "$(dirname -- "$TGZ")" && shasum -a 256 "$(basename -- "$TGZ")") >"$CHECKSUM_FILE"
else
  (cd -- "$(dirname -- "$TGZ")" && sha256sum "$(basename -- "$TGZ")") >"$CHECKSUM_FILE"
fi

printf 'assembled %s\n' "$TGZ"
printf 'checksum %s\n' "$(cat "$CHECKSUM_FILE")"
printf 'host-note: local assemble of six pairs is not remote native-run proof\n'
