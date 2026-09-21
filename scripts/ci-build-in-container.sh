#!/usr/bin/env bash
# Install a pinned Rust toolchain inside an AlmaLinux 8 or Alpine container, then
# build one native pair with scripts/build.sh. Invoked by npm-publish.yml via docker.
set -euo pipefail

SOURCE="${BASH_SOURCE[0]:-$0}"
while [ -L "$SOURCE" ]; do
  DIR="$(cd -- "$(dirname -- "$SOURCE")" && pwd)"
  SOURCE="$(readlink "$SOURCE")"
  [[ "$SOURCE" != /* ]] && SOURCE="$DIR/$SOURCE"
done
SCRIPT_DIR="$(cd -- "$(dirname -- "$SOURCE")" && pwd -P)"
PACKAGE_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd -P)"

TARGET="${1:-}"
if [[ -z "$TARGET" ]]; then
  echo "usage: scripts/ci-build-in-container.sh <triple>" >&2
  exit 2
fi

if [[ -f /etc/os-release ]]; then
  # shellcheck disable=SC1091
  . /etc/os-release
  case "${ID:-}" in
    almalinux|rhel|centos|rocky)
      dnf install -y gcc gcc-c++ make git tar gzip curl ca-certificates
      ;;
    alpine)
      apk add --no-cache bash curl gcc musl-dev git tar gzip
      ;;
  esac
fi

if ! command -v rustup >/dev/null 2>&1 && [[ ! -x "${HOME}/.cargo/bin/rustup" ]]; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none
fi
export PATH="${HOME}/.cargo/bin:${PATH}"

cd -- "$PACKAGE_ROOT"
# rust-toolchain.toml pins 1.98.1; fetch it and the requested target std.
rustup show >/dev/null
rustup target add "$TARGET"

bash "$SCRIPT_DIR/build.sh" --profile release --target "$TARGET"
