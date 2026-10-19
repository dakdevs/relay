#!/usr/bin/env bash
# CI only: smoke-tests one packaged release archive on its own runner.
#
#   scripts/smoke.sh DIST_DIR TARGET
#
# 1. Installs the archive with scripts/install.sh --from into a temporary HOME
#    (checksum, extraction and `relay --version` checks).
# 2. Platform checks: plutil and codesign (macOS); desktop-file-validate and
#    unresolved shared libraries (Linux).
# 3. Starts the app for 15 s with every data path in a temporary RELAY_HOME
#    (Linux under Xvfb with the lavapipe Vulkan driver) and requires it to still
#    be running. It refuses to run outside CI because it opens a window.
set -euo pipefail

[[ "${CI:-}" == true ]] || {
  echo "smoke.sh: CI only (launches the app); use scripts/test.sh locally" >&2
  exit 1
}

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dist="${1:?DIST_DIR}"
target="${2:?TARGET}"
version="$("$root/scripts/version.sh" check)"

case "$target" in
*-apple-darwin) os=Darwin ;;
*-linux-gnu) os=Linux ;;
*)
  echo "smoke.sh: unsupported target $target" >&2
  exit 1
  ;;
esac
arch="${target%%-*}"

sandbox="$(mktemp -d)"
trap 'rm -rf "$sandbox"' EXIT
export HOME="$sandbox/home" RELAY_HOME="$sandbox/relay-home"
export XDG_CONFIG_HOME="$HOME/.config" XDG_DATA_HOME="$HOME/.local/share" XDG_STATE_HOME="$HOME/.local/state" XDG_CACHE_HOME="$HOME/.cache"
mkdir -p "$HOME" "$RELAY_HOME"

echo "::group::install"
RELAY_INSTALL_OS="$os" RELAY_INSTALL_ARCH="$arch" "$root/scripts/install.sh" --from "$dist" --version "$version"
echo "::endgroup::"

if [[ "$os" == Darwin ]]; then
  app="$HOME/Applications/Relay.app"
  bin="$app/Contents/MacOS/relay"
  plutil -lint "$app/Contents/Info.plist"
  codesign --verify --strict --verbose=2 "$app"
  lipo -archs "$bin" | grep -qx "${arch/aarch64/arm64}"
else
  bin="$HOME/.local/bin/relay"
  desktop-file-validate "$HOME/.local/share/applications/dev.dak.relay.desktop"
  # Direct dependencies must come from documented runtime packages; nothing
  # in the full closure may be missing.
  "$root/scripts/linkage.sh" "$bin" "$root/release/linux/runtime-packages.txt"
  echo "glibc symbol floor: $(objdump -T "$bin" | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -n 1)"
fi
[[ "$("$bin" --version)" == "relay $version" ]]
"$bin" --help >/dev/null

echo "::group::startup"
log="$sandbox/relay.log"
if [[ "$os" == Linux ]]; then
  xvfb-run -a "$bin" >"$log" 2>&1 &
else
  "$bin" >"$log" 2>&1 &
fi
pid=$!
sleep 15
if kill -0 "$pid" 2>/dev/null; then
  echo "relay still running after 15 s"
  pkill -TERM -P "$pid" 2>/dev/null || true
  kill -TERM "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
  status=0
else
  wait "$pid" && code=0 || code=$?
  echo "smoke.sh: relay exited early with status $code" >&2
  status=1
fi
cat "$log"
echo "::endgroup::"
exit "$status"
