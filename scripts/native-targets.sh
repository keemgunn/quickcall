# Sourceable list of published native triples.
# shellcheck shell=bash
NATIVE_TARGETS=(
  aarch64-apple-darwin
  x86_64-apple-darwin
  aarch64-unknown-linux-gnu
  x86_64-unknown-linux-gnu
  aarch64-unknown-linux-musl
  x86_64-unknown-linux-musl
)
NATIVE_BINARIES=(qc qc-bootstrap)
# Plan §9 / Node 24 envelope. Overrides rust-preferences USER.md 2.17.
GLIBC_FLOOR="2.28"
MACOSX_DEPLOYMENT_TARGET_VALUE="13.5"
