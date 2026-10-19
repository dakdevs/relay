#!/usr/bin/env bash
# CI only: installs the compiler pinned in rust-toolchain.toml (installing
# rustup first if the runner lacks it), plus an optional extra target.
#
#   scripts/ci-rust-toolchain.sh [TARGET]
#
# The channel is passed explicitly rather than relying on `rustup toolchain
# install` with no argument, which only reads rust-toolchain.toml in
# rustup 1.28 and later.
set -euo pipefail

[[ "${CI:-}" == true ]] || {
  echo "ci-rust-toolchain.sh: CI only (may install rustup)" >&2
  exit 1
}

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
channel="$(sed -n 's/^channel[[:space:]]*=[[:space:]]*"\([0-9][0-9.]*\)"[[:space:]]*$/\1/p' "$root/rust-toolchain.toml")"
[[ "$channel" =~ ^[0-9]+\.[0-9]+(\.[0-9]+)?$ ]] || {
  echo "ci-rust-toolchain.sh: no numeric channel in rust-toolchain.toml" >&2
  exit 1
}

bin="${CARGO_HOME:-$HOME/.cargo}/bin"
if [[ ! -x "$bin/rustup" ]]; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
fi
# rustup's proxies must win over any other rustc/cargo on the runner, in this
# step and in later ones.
export PATH="$bin:$PATH"
[[ -z "${GITHUB_PATH:-}" ]] || echo "$bin" >>"$GITHUB_PATH"

args=(toolchain install "$channel" --profile minimal --component rustfmt --component clippy)
[[ -z "${1:-}" ]] || args+=(--target "$1")
rustup "${args[@]}"
cd "$root"
rustc --version --verbose
[[ "$(rustc --version)" == "rustc $channel "* ]] || {
  echo "ci-rust-toolchain.sh: active compiler is not $channel" >&2
  exit 1
}
