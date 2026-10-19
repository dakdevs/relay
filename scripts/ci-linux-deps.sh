#!/usr/bin/env bash
# CI only: installs the Debian/Ubuntu packages GPUI and the ksni tray need to
# build relay-desktop, plus Xvfb and the lavapipe Vulkan driver for the
# startup smoke test, and frees runner disk space for the GPUI build.
#
#   scripts/ci-linux-deps.sh            build dependencies
#   scripts/ci-linux-deps.sh --runtime  only release/linux/runtime-packages.txt
set -euo pipefail

[[ "${CI:-}" == true ]] || {
  echo "ci-linux-deps.sh: CI only (uses sudo apt-get and deletes runner tool caches)" >&2
  exit 1
}

if [[ "${1:-}" == --runtime ]]; then
  root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
  mapfile -t packages < <(grep -v '^#' "$root/release/linux/runtime-packages.txt" | grep .)
  sudo apt-get update -q
  sudo apt-get install -y -q --no-install-recommends "${packages[@]}"
  exit 0
fi

# Preinstalled SDKs the build never uses; the GPUI release build needs the space.
sudo rm -rf /usr/share/dotnet /usr/local/lib/android /opt/ghc /usr/local/share/boost "${AGENT_TOOLSDIRECTORY:-/nonexistent}/CodeQL"
df -h /

sudo apt-get update -q
sudo apt-get install -y -q --no-install-recommends \
  pkg-config clang cmake \
  libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libvulkan-dev \
  libx11-dev libx11-xcb-dev libxcb1-dev libfontconfig-dev libfreetype-dev \
  libdbus-1-dev libzstd-dev \
  mesa-vulkan-drivers xvfb xauth desktop-file-utils
