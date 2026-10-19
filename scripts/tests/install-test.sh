#!/usr/bin/env bash
# End-to-end tests for scripts/package.sh and scripts/install.sh using a stub
# `relay` binary, temporary HOME directories and local archives (--from). No
# network access, no real installation, nothing is launched.
#
# Oracles are independent of the code under test: expected archive listings and
# install paths are written out by hand, and checksums are recomputed with
# openssl rather than sha256sum/shasum.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
package="$root/scripts/package.sh"
version="$("$root/scripts/version.sh" check)"
# Run the installer with the system bash too: macOS ships bash 3.2.
installer_shells=(bash)
[[ -x /bin/bash ]] && installer_shells+=(/bin/bash)
fails=0

ok() { printf 'ok   %s\n' "$1"; }
fail() {
  printf 'FAIL %s\n' "$1"
  fails=$((fails + 1))
}
check() { # name cmd...
  local name="$1"
  shift
  if "$@"; then ok "$name"; else fail "$name"; fi
}

refuse() { if "$@"; then return 1; fi; }

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

stub() { # path reported-version
  # shellcheck disable=SC2016 # $1 belongs to the generated script
  printf '#!/bin/sh\n[ "$1" = --version ] && echo "relay %s" && exit 0\nexit 3\n' "$2" >"$1"
  chmod 0755 "$1"
}
stub "$scratch/relay" "$version"
printf 'stub licenses\n' >"$scratch/licenses.md"

dist="$scratch/dist"
for target in aarch64-apple-darwin x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu; do
  "$package" archive --target "$target" --binary "$scratch/relay" --out "$dist" --licenses "$scratch/licenses.md" >/dev/null
done
"$package" sums "$dist" >/dev/null
"$package" installer "$dist" >/dev/null

# --- Archives -----------------------------------------------------------------
listing() { tar -tzf "$1" | sed 's#/$##' | LC_ALL=C sort | tr '\n' ' '; }
want_linux="relay-$version-linux-x86_64 relay-$version-linux-x86_64/LICENSE relay-$version-linux-x86_64/NOTICE.md relay-$version-linux-x86_64/THIRD-PARTY-LICENSES.md relay-$version-linux-x86_64/bin relay-$version-linux-x86_64/bin/relay relay-$version-linux-x86_64/share relay-$version-linux-x86_64/share/applications relay-$version-linux-x86_64/share/applications/dev.dak.relay.desktop relay-$version-linux-x86_64/share/icons relay-$version-linux-x86_64/share/icons/hicolor relay-$version-linux-x86_64/share/icons/hicolor/128x128 relay-$version-linux-x86_64/share/icons/hicolor/128x128/apps relay-$version-linux-x86_64/share/icons/hicolor/128x128/apps/dev.dak.relay.png relay-$version-linux-x86_64/share/icons/hicolor/256x256 relay-$version-linux-x86_64/share/icons/hicolor/256x256/apps relay-$version-linux-x86_64/share/icons/hicolor/256x256/apps/dev.dak.relay.png relay-$version-linux-x86_64/share/icons/hicolor/512x512 relay-$version-linux-x86_64/share/icons/hicolor/512x512/apps relay-$version-linux-x86_64/share/icons/hicolor/512x512/apps/dev.dak.relay.png "
check "linux archive layout" test "$(listing "$dist/relay-$version-linux-x86_64.tar.gz")" == "$want_linux"

mac_list="$(listing "$dist/relay-$version-macos-aarch64.tar.gz")"
for f in Relay.app/Contents/Info.plist Relay.app/Contents/MacOS/relay Relay.app/Contents/PkgInfo Relay.app/Contents/Resources/relay.icns Relay.app/Contents/Resources/LICENSE Relay.app/Contents/Resources/THIRD-PARTY-LICENSES.md; do
  check "macOS archive has $f" grep -q " $f " <<<" $mac_list"
done
check "macOS archive has no AppleDouble files" test -z "$(grep -o '\._[^ ]*' <<<"$mac_list" || true)"

x="$scratch/plist"
mkdir "$x"
tar -xzf "$dist/relay-$version-macos-aarch64.tar.gz" -C "$x"
plist="$x/Relay.app/Contents/Info.plist"
plist_value() { awk -v k="<key>$1</key>" 'found { gsub(/^[ \t]*<string>|<\/string>[ \t]*$/, ""); print; exit } index($0, k) { found = 1 }' "$plist"; }
mac_version="${version:0:4}.$((10#${version:4:4})).${version#*.}"
check "plist CFBundleShortVersionString" test "$(plist_value CFBundleShortVersionString)" == "$mac_version"
check "plist CFBundleVersion" test "$(plist_value CFBundleVersion)" == "$mac_version"
check "plist RelayDisplayVersion" test "$(plist_value RelayDisplayVersion)" == "$version"
check "plist CFBundleExecutable" test "$(plist_value CFBundleExecutable)" == relay
check "plist CFBundleIdentifier" test "$(plist_value CFBundleIdentifier)" == dev.dak.relay
check "plist LSUIElement true" grep -A1 -q '<key>LSUIElement</key>' "$plist"
if command -v plutil >/dev/null; then check "plist lints" plutil -lint -s "$plist"; fi
if command -v codesign >/dev/null; then check "bundle ad-hoc signature verifies" codesign --verify --strict "$x/Relay.app"; fi

# --- Reproducibility --------------------------------------------------------------
again="$scratch/again"
sleep 1
touch "$scratch/relay" "$scratch/licenses.md"
for target in aarch64-apple-darwin x86_64-unknown-linux-gnu; do
  "$package" archive --target "$target" --binary "$scratch/relay" --out "$again" --licenses "$scratch/licenses.md" >/dev/null
done
for a in "$again"/*.tar.gz; do
  check "rebuild of $(basename "$a") is byte-identical" test "$(openssl dgst -sha256 -r "$a" | cut -c1-64)" == "$(openssl dgst -sha256 -r "$dist/$(basename "$a")" | cut -c1-64)"
done
gzip_mtime() { od -An -tu4 -j4 -N4 "$1" | tr -d ' '; }
check "gzip header carries no timestamp" test "$(gzip_mtime "$dist/relay-$version-linux-x86_64.tar.gz")" == 0
want_epoch="$(TZ=UTC date -j -f '%Y%m%d%H%M%S' "${version%%.*}000000" +%s 2>/dev/null || date -u -d "${version%%.*}" +%s)"
mx="$scratch/mtime"
mkdir "$mx"
tar -xzf "$dist/relay-$version-linux-x86_64.tar.gz" -C "$mx"
# `date -r FILE` prints the modification time on both GNU and BSD; `stat -f`
# means different things on the two.
file_epoch="$(date -r "$mx/relay-$version-linux-x86_64/bin/relay" +%s)"
check "version-date oracle is a plain epoch" test -n "$want_epoch" -a -z "${want_epoch//[0-9]/}"
check "entries are dated 00:00 UTC on the version date" test "$file_epoch" == "$want_epoch"

# --- Checksums and pinned installer ---------------------------------------------
digest() { openssl dgst -sha256 -r "$1" | awk '{ print $1 }'; }
for a in "$dist"/relay-*.tar.gz; do
  name="$(basename "$a")"
  check "SHA256SUMS lists $name" grep -qx "$(digest "$a")  $name" "$dist/SHA256SUMS"
  check "install.sh pins $name" grep -qx "$(digest "$a")  $name" "$dist/install.sh"
done
check "SHA256SUMS lists install.sh" grep -qx "$(digest "$dist/install.sh")  install.sh" "$dist/SHA256SUMS"
check "install.sh pins version" grep -qx "PINNED_VERSION=\"$version\"" "$dist/install.sh"
check "repository install.sh stays unpinned" grep -qx 'PINNED_VERSION=""' "$root/scripts/install.sh"
check "installer cannot be rendered twice" refuse "$package" installer "$dist" 2>/dev/null

# --- Installer ------------------------------------------------------------------
run_install() { # shell os arch home installer args...
  local sh="$1" os="$2" arch="$3" home="$4" script="$5"
  shift 5
  env -i PATH="$PATH" HOME="$home" TMPDIR="$scratch" RELAY_INSTALL_OS="$os" RELAY_INSTALL_ARCH="$arch" "$sh" "$script" "$@" >/dev/null 2>"$scratch/err"
}
no_files_under() { test -z "$(find "$1" -type f -print -quit)"; }

for sh in "${installer_shells[@]}"; do
  tag="[$sh]"
  home="$(mktemp -d "$scratch/home.XXXXXX")"
  check "$tag linux install" run_install "$sh" Linux x86_64 "$home" "$dist/install.sh" --from "$dist"
  check "$tag linux binary installed" test -x "$home/.local/bin/relay"
  check "$tag installed binary reports version" test "$("$home/.local/bin/relay" --version)" == "relay $version"
  check "$tag desktop entry uses absolute Exec" grep -qx "Exec=$home/.local/bin/relay" "$home/.local/share/applications/dev.dak.relay.desktop"
  check "$tag desktop entry keeps Icon" grep -qx "Icon=dev.dak.relay" "$home/.local/share/applications/dev.dak.relay.desktop"
  check "$tag icon installed" test -f "$home/.local/share/icons/hicolor/256x256/apps/dev.dak.relay.png"
  check "$tag no staging leftovers" test -z "$(find "$home" -name '*.relay-install.*' -print -quit)"

  before="$(openssl dgst -sha256 -r "$home/.local/bin/relay")"
  if run_install "$sh" Linux x86_64 "$home" "$dist/install.sh" --from "$dist"; then
    fail "$tag existing install is refused"
  else
    check "$tag refusal explains --force" grep -q -- '--force' "$scratch/err"
  fi
  check "$tag refusal leaves binary untouched" test "$(openssl dgst -sha256 -r "$home/.local/bin/relay")" == "$before"
  check "$tag --force replaces" run_install "$sh" Linux x86_64 "$home" "$dist/install.sh" --from "$dist" --force

  home="$(mktemp -d "$scratch/home.XXXXXX")"
  check "$tag linux aarch64 custom prefix" run_install "$sh" Linux aarch64 "$home" "$dist/install.sh" --from "$dist" --prefix "$home/opt/relay"
  check "$tag custom prefix binary" test -x "$home/opt/relay/bin/relay"
  check "$tag unsafe prefix refused" refuse run_install "$sh" Linux x86_64 "$home" "$dist/install.sh" --from "$dist" --prefix "$home/a b"

  home="$(mktemp -d "$scratch/home.XXXXXX")"
  check "$tag macOS install" run_install "$sh" Darwin arm64 "$home" "$dist/install.sh" --from "$dist"
  check "$tag Relay.app installed" test -x "$home/Applications/Relay.app/Contents/MacOS/relay"
  check "$tag macOS x86_64 archive absent fails" refuse run_install "$sh" Darwin x86_64 "$(mktemp -d "$scratch/home.XXXXXX")" "$dist/install.sh" --from "$dist"

  home="$(mktemp -d "$scratch/home.XXXXXX")"
  check "$tag pinned installer rejects other version" refuse run_install "$sh" Linux x86_64 "$home" "$dist/install.sh" --from "$dist" --version 20200101.1
  check "$tag unsupported OS fails" refuse run_install "$sh" FreeBSD x86_64 "$home" "$dist/install.sh" --from "$dist"
  check "$tag unknown option fails" refuse run_install "$sh" Linux x86_64 "$home" "$dist/install.sh" --from "$dist" --bogus
  check "$tag failures install nothing" no_files_under "$home"
done

# --- Partial-install safety ----------------------------------------------------------
if [[ "$(id -u)" != 0 ]]; then # root ignores folder permissions
  home="$(mktemp -d "$scratch/home.XXXXXX")"
  mkdir -p "$home/.local/share/applications"
  chmod 0555 "$home/.local/share/applications"
  check "read-only applications folder is refused" refuse run_install bash Linux x86_64 "$home" "$dist/install.sh" --from "$dist"
  check "refusal names the folder" grep -q "share/applications is not a writable folder" "$scratch/err"
  check "refused fresh install writes no binary" test ! -e "$home/.local/bin/relay"
  chmod 0755 "$home/.local/share/applications"

  home="$(mktemp -d "$scratch/home.XXXXXX")"
  run_install bash Linux x86_64 "$home" "$dist/install.sh" --from "$dist"
  printf 'previous binary\n' >"$home/.local/bin/relay"
  printf 'previous entry\n' >"$home/.local/share/applications/dev.dak.relay.desktop"
  chmod 0555 "$home/.local/share/applications"
  check "forced reinstall into read-only folder is refused" refuse run_install bash Linux x86_64 "$home" "$dist/install.sh" --from "$dist" --force
  check "refused reinstall keeps previous binary" test "$(cat "$home/.local/bin/relay")" == "previous binary"
  chmod 0755 "$home/.local/share/applications"
  check "refused reinstall leaves no staged files" test -z "$(find "$home" -name '*.relay-install.*' -print -quit)"

  # A move that fails half-way restores everything already replaced.
  stubs="$scratch/stub-mv"
  mkdir -p "$stubs"
  # Fails only the move of the staged desktop entry into place.
  # shellcheck disable=SC2016 # expanded by the stub at run time
  printf '#!/bin/sh\ncase "$1|$2" in *.relay-install.*\|*.desktop) echo "mv: injected failure" >&2; exit 1 ;; esac\nexec /bin/mv "$@"\n' >"$stubs/mv"
  chmod +x "$stubs/mv"
  if env -i PATH="$stubs:$PATH" HOME="$home" TMPDIR="$scratch" RELAY_INSTALL_OS=Linux RELAY_INSTALL_ARCH=x86_64 bash "$dist/install.sh" --from "$dist" --force >/dev/null 2>"$scratch/err"; then
    fail "failed move is reported"
  else
    check "failed move is reported" grep -q 'previous files were restored' "$scratch/err"
  fi
  check "failed move restores previous binary" test "$(cat "$home/.local/bin/relay")" == "previous binary"
  check "failed move restores previous entry" test "$(cat "$home/.local/share/applications/dev.dak.relay.desktop")" == "previous entry"
  check "failed move leaves no staged or backup files" test -z "$(find "$home" \( -name '*.relay-install.*' -o -name '*.relay-old.*' \) -print -quit)"
  check "--force replaces every file" run_install bash Linux x86_64 "$home" "$dist/install.sh" --from "$dist" --force
  check "replaced binary is the new one" test "$("$home/.local/bin/relay" --version)" == "relay $version"
  check "replaced entry is the new one" grep -qx "Exec=$home/.local/bin/relay" "$home/.local/share/applications/dev.dak.relay.desktop"

  home="$(mktemp -d "$scratch/home.XXXXXX")"
  mkdir -p "$home/Applications"
  chmod 0555 "$home/Applications"
  check "read-only Applications folder is refused" refuse run_install bash Darwin arm64 "$home" "$dist/install.sh" --from "$dist"
  chmod 0755 "$home/Applications"
  check "refused macOS install writes nothing" test -z "$(find "$home/Applications" -mindepth 1 -print -quit)"
else
  echo "skip partial-install safety tests: running as root"
fi

# Unpinned (repository) installer reads SHA256SUMS next to the archive.
home="$(mktemp -d "$scratch/home.XXXXXX")"
check "unpinned installer uses SHA256SUMS" run_install bash Linux x86_64 "$home" "$root/scripts/install.sh" --from "$dist" --version "v$version"
check "unpinned installer needs --version with --from" refuse run_install bash Linux x86_64 "$(mktemp -d "$scratch/home.XXXXXX")" "$root/scripts/install.sh" --from "$dist"
check "invalid version rejected" refuse run_install bash Linux x86_64 "$(mktemp -d "$scratch/home.XXXXXX")" "$root/scripts/install.sh" --from "$dist" --version '1;touch /tmp/x'

# --- Hostile archives ------------------------------------------------------------
# Each case swaps the linux x86_64 archive for a crafted one with a matching
# SHA256SUMS entry, so only the content checks can reject it.
evil_case() { # name builder-function
  local name="$1" build="$2" d stage home
  d="$(mktemp -d "$scratch/evil.XXXXXX")"
  stage="$d/stage"
  mkdir -p "$stage/relay-$version-linux-x86_64/bin" "$stage/relay-$version-linux-x86_64/share/applications" "$stage/relay-$version-linux-x86_64/share/icons/hicolor/256x256/apps"
  stub "$stage/relay-$version-linux-x86_64/bin/relay" "$version"
  printf '[Desktop Entry]\nExec=relay\n' >"$stage/relay-$version-linux-x86_64/share/applications/dev.dak.relay.desktop"
  printf 'png' >"$stage/relay-$version-linux-x86_64/share/icons/hicolor/256x256/apps/dev.dak.relay.png"
  "$build" "$stage" "$d/relay-$version-linux-x86_64.tar.gz"
  printf '%s  %s\n' "$(digest "$d/relay-$version-linux-x86_64.tar.gz")" "relay-$version-linux-x86_64.tar.gz" >"$d/SHA256SUMS"
  home="$(mktemp -d "$scratch/home.XXXXXX")"
  if run_install bash Linux x86_64 "$home" "$root/scripts/install.sh" --from "$d" --version "$version"; then
    fail "$name is rejected"
  else
    ok "$name is rejected ($(sed 's/^relay-install: error: //' "$scratch/err" | tail -n 1))"
  fi
  check "$name installs nothing" no_files_under "$home"
}
plain_tar() { tar -C "$1" -czf "$2" "relay-$version-linux-x86_64"; }
build_ok() { plain_tar "$@"; }
build_symlink() {
  ln -s /etc/passwd "$1/relay-$version-linux-x86_64/share/link"
  plain_tar "$@"
}
build_extra_top() {
  printf 'x' >"$1/extra"
  tar -C "$1" -czf "$2" "relay-$version-linux-x86_64" extra
}
build_dotdot() {
  printf 'x' >"$1/outside"
  tar -C "$1" -czf "$2" "relay-$version-linux-x86_64" "relay-$version-linux-x86_64/../outside" 2>/dev/null
}
build_wrong_version() {
  stub "$1/relay-$version-linux-x86_64/bin/relay" 20000101.1
  plain_tar "$@"
}
build_missing_desktop() {
  rm "$1/relay-$version-linux-x86_64/share/applications/dev.dak.relay.desktop"
  plain_tar "$@"
}
build_tampered() {
  plain_tar "$@"
  printf 'x' >>"$2"
}

home="$(mktemp -d "$scratch/home.XXXXXX")"
d="$(mktemp -d "$scratch/good.XXXXXX")"
mkdir -p "$d/stage/relay-$version-linux-x86_64/bin" "$d/stage/relay-$version-linux-x86_64/share/applications" "$d/stage/relay-$version-linux-x86_64/share/icons/hicolor/256x256/apps"
stub "$d/stage/relay-$version-linux-x86_64/bin/relay" "$version"
printf '[Desktop Entry]\nExec=relay\n' >"$d/stage/relay-$version-linux-x86_64/share/applications/dev.dak.relay.desktop"
printf 'png' >"$d/stage/relay-$version-linux-x86_64/share/icons/hicolor/256x256/apps/dev.dak.relay.png"
build_ok "$d/stage" "$d/relay-$version-linux-x86_64.tar.gz"
printf '%s  %s\n' "$(digest "$d/relay-$version-linux-x86_64.tar.gz")" "relay-$version-linux-x86_64.tar.gz" >"$d/SHA256SUMS"
check "control: crafted well-formed archive installs" run_install bash Linux x86_64 "$home" "$root/scripts/install.sh" --from "$d" --version "$version"

evil_case "symlink in archive" build_symlink
evil_case "extra top-level entry" build_extra_top
evil_case "parent-directory entry" build_dotdot
evil_case "binary reporting another version" build_wrong_version
evil_case "archive missing desktop entry" build_missing_desktop

d="$(mktemp -d "$scratch/tamper.XXXXXX")"
cp "$dist/relay-$version-linux-x86_64.tar.gz" "$d/"
printf 'x' >>"$d/relay-$version-linux-x86_64.tar.gz"
home="$(mktemp -d "$scratch/home.XXXXXX")"
check "pinned checksum mismatch is rejected" refuse run_install bash Linux x86_64 "$home" "$dist/install.sh" --from "$d"
check "checksum message names mismatch" grep -q 'checksum mismatch' "$scratch/err"
check "checksum mismatch installs nothing" no_files_under "$home"

if ((fails)); then
  printf '%d install test(s) failed\n' "$fails"
  exit 1
fi
printf 'install tests passed\n'
