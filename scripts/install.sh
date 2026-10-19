#!/usr/bin/env bash
# Relay installer for macOS and Linux. Installs into your home directory and
# never uses sudo.
# Options: see usage() below or run with --help.
#
# Every archive is checked against a SHA-256 checksum before extraction:
# release copies of this script carry the checksums inline (pinned); the
# repository copy reads SHA256SUMS from the same release. The archive must
# contain only regular files and folders, and the binary must report the
# expected version. Relay is not started.
set -euo pipefail

REPO="dakdevs/relay"
BUNDLE_ID="dev.dak.relay"
# Filled in by scripts/package.sh when a release is cut.
PINNED_VERSION=""
PINNED_SHA256=""

say() { printf 'relay-install: %s\n' "$*" >&2; }
die() {
  say "error: $*"
  exit 1
}

usage() {
  cat <<'EOF'
Usage: install.sh [--version vYYYYMMDD.N | latest] [--force] [--source auto|gh|curl]
                  [--app-dir DIR] [--prefix DIR] [--from DIR]

  --version   Release to install (default: the pinned or latest release)
  --force     Replace an existing installation
  --source    gh (authenticated; works while the repository is private), curl, or auto
  --app-dir   macOS folder for Relay.app (default: ~/Applications)
  --prefix    Linux prefix for bin/relay and desktop files (default: ~/.local)
  --from DIR  Install from archives and SHA256SUMS already in DIR
EOF
}

version="" force=0 source="auto" app_dir="" prefix="" from=""
while (($#)); do
  case "$1" in
  --force) force=1 ;;
  -h | --help)
    usage
    exit 0
    ;;
  --version | --source | --app-dir | --prefix | --from)
    [[ $# -ge 2 && -n "$2" ]] || die "$1 needs a value"
    case "$1" in
    --version) version="$2" ;;
    --source) source="$2" ;;
    --app-dir) app_dir="$2" ;;
    --prefix) prefix="$2" ;;
    --from) from="$2" ;;
    esac
    shift
    ;;
  *) die "unknown argument '$1' (see --help)" ;;
  esac
  shift
done

case "$source" in auto | gh | curl) ;; *) die "--source must be auto, gh or curl" ;; esac
[[ -n "${HOME:-}" && -d "$HOME" ]] || die "HOME is not set to a directory"

# --- Platform -----------------------------------------------------------------
# RELAY_INSTALL_OS / RELAY_INSTALL_ARCH override detection for tests.
os_name="${RELAY_INSTALL_OS:-$(uname -s)}"
arch_name="${RELAY_INSTALL_ARCH:-$(uname -m)}"
case "$os_name" in
Darwin) os="macos" ;;
Linux) os="linux" ;;
*) die "unsupported OS '$os_name' (Relay supports macOS and Linux)" ;;
esac
case "$arch_name" in
x86_64 | amd64) arch="x86_64" ;;
arm64 | aarch64) arch="aarch64" ;;
*) die "unsupported CPU '$arch_name'" ;;
esac
# A Rosetta shell on Apple Silicon reports x86_64; install the native build.
if [[ "$os" == macos && "$arch" == x86_64 && -z "${RELAY_INSTALL_ARCH:-}" ]] &&
  [[ "$(sysctl -in hw.optional.arm64 2>/dev/null || true)" == 1 ]]; then
  arch="aarch64"
fi

# --- Version ------------------------------------------------------------------
valid_version() { [[ "$1" =~ ^[0-9]{8}\.[1-9][0-9]{0,3}$ ]]; }

version="${version#v}"
if [[ -n "$PINNED_VERSION" ]]; then
  if [[ -n "$version" && "$version" != latest && "$version" != "$PINNED_VERSION" ]]; then
    die "this installer is pinned to $PINNED_VERSION; use the installer from release v$version"
  fi
  version="$PINNED_VERSION"
fi

have_gh() { command -v gh >/dev/null && gh auth status --hostname github.com >/dev/null 2>&1; }
if [[ -n "$from" ]]; then
  source="local"
elif [[ "$source" == auto ]]; then
  if have_gh; then source="gh"; else source="curl"; fi
fi
[[ "$source" != gh ]] || command -v gh >/dev/null || die "gh is not installed"
[[ "$source" != curl ]] || command -v curl >/dev/null || die "curl is not installed"

curl_get() { curl --proto '=https' --tlsv1.2 -fsSL --retry 3 "$@"; }

if [[ -z "$version" || "$version" == latest ]]; then
  [[ -z "$from" ]] || die "--from needs --version"
  if [[ "$source" == gh ]]; then
    tag="$(gh release view --repo "$REPO" --json tagName --jq .tagName)" || die "could not read the latest release with gh"
  else
    # The /releases/latest page redirects to /releases/tag/<tag>.
    location="$(curl --proto '=https' --tlsv1.2 -fsSI "https://github.com/$REPO/releases/latest" | tr -d '\r' | awk 'tolower($1) == "location:" { print $2 }' | tail -n 1)" ||
      die "could not reach github.com/$REPO (the repository may be private: install and log in to gh, then rerun)"
    tag="${location##*/}"
  fi
  version="${tag#v}"
fi
valid_version "$version" || die "invalid version '$version' (expected YYYYMMDD.N)"
tag="v$version"
archive="relay-$version-$os-$arch.tar.gz"

# --- Download and verify ------------------------------------------------------
tmp="$(mktemp -d "${TMPDIR:-/tmp}/relay-install.XXXXXXXX")"
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/dl" "$tmp/x"

if [[ -n "$from" ]]; then
  [[ -f "$from/$archive" ]] || die "$from/$archive not found"
  cp "$from/$archive" "$tmp/dl/"
  [[ -n "$PINNED_SHA256" || ! -f "$from/SHA256SUMS" ]] || cp "$from/SHA256SUMS" "$tmp/dl/"
else
  say "downloading $archive ($tag, via $source)"
  if [[ "$source" == gh ]]; then
    patterns=(--pattern "$archive")
    [[ -n "$PINNED_SHA256" ]] || patterns+=(--pattern SHA256SUMS)
    gh release download "$tag" --repo "$REPO" "${patterns[@]}" --dir "$tmp/dl" ||
      die "gh could not download $tag (check the version and your access to $REPO)"
  else
    base="https://github.com/$REPO/releases/download/$tag"
    curl_get -o "$tmp/dl/$archive" "$base/$archive" ||
      die "download failed: $base/$archive (if the repository is private, use --source gh)"
    [[ -n "$PINNED_SHA256" ]] || curl_get -o "$tmp/dl/SHA256SUMS" "$base/SHA256SUMS" ||
      die "download failed: $base/SHA256SUMS"
  fi
fi

if [[ -n "$PINNED_SHA256" ]]; then
  sums="$PINNED_SHA256"
else
  [[ -f "$tmp/dl/SHA256SUMS" ]] || die "SHA256SUMS not found for $tag"
  sums="$(<"$tmp/dl/SHA256SUMS")"
fi
expected="$(awk -v f="$archive" '$2 == f && $1 ~ /^[0-9a-f]+$/ && length($1) == 64 { print $1 }' <<<"$sums")"
[[ -n "$expected" && "$expected" != *$'\n'* ]] || die "no single checksum listed for $archive"
if command -v sha256sum >/dev/null; then
  actual="$(sha256sum "$tmp/dl/$archive" | awk '{ print $1 }')"
else
  actual="$(shasum -a 256 "$tmp/dl/$archive" | awk '{ print $1 }')"
fi
[[ "$actual" == "$expected" ]] || die "checksum mismatch for $archive (expected $expected, got $actual)"
say "checksum verified"

# --- Extract ------------------------------------------------------------------
if [[ "$os" == macos ]]; then top="Relay.app"; else top="relay-$version-linux-$arch"; fi
while IFS= read -r entry; do
  case "$entry" in
  "$top" | "$top/" | "$top"/*) ;;
  *) die "unexpected archive entry '$entry'" ;;
  esac
  case "/$entry/" in */../*) die "unsafe archive entry '$entry'" ;; esac
done < <(tar -tzf "$tmp/dl/$archive")
tar -xzf "$tmp/dl/$archive" -C "$tmp/x"
odd="$(find "$tmp/x" ! -type f ! -type d -print -quit)"
[[ -z "$odd" ]] || die "archive contains a link or special file: ${odd#"$tmp/x/"}"

src="$tmp/x/$top"
if [[ "$os" == macos ]]; then
  bin="$src/Contents/MacOS/relay"
  required=("$bin" "$src/Contents/Info.plist" "$src/Contents/Resources/relay.icns")
else
  bin="$src/bin/relay"
  required=("$bin" "$src/share/applications/$BUNDLE_ID.desktop" "$src/share/icons/hicolor/256x256/apps/$BUNDLE_ID.png")
fi
for f in "${required[@]}"; do
  [[ -f "$f" ]] || die "archive is missing ${f#"$src/"}"
done
[[ -x "$bin" ]] || die "relay binary is not executable"
reported="$("$bin" --version 2>/dev/null)" || die "relay --version failed (wrong architecture or missing system libraries?)"
[[ "$reported" == "relay $version" ]] || die "binary reports '$reported', expected 'relay $version'"

# --- Install ------------------------------------------------------------------
# Every destination is checked before anything is written, then all files are
# staged next to their destinations, then moved into place. A failure while
# checking or staging changes nothing; a failure while moving restores what
# was replaced.
srcs=() dests=()
add() {
  srcs+=("$1")
  dests+=("$2")
}

refuse_existing() {
  local p existing=()
  for p in "${dests[@]}"; do
    if [[ -e "$p" || -L "$p" ]]; then existing+=("$p"); fi
  done
  if ((${#existing[@]})) && ((force == 0)); then
    say "already installed: ${existing[*]}"
    die "nothing was changed; rerun with --force to replace"
  fi
}

# The destination's folder must be writable, or creatable in a writable parent.
preflight() {
  local dest dir
  for dest in "${dests[@]}"; do
    dir="$(dirname "$dest")"
    while [[ ! -e "$dir" ]]; do dir="$(dirname "$dir")"; done
    [[ -d "$dir" && -w "$dir" ]] || die "$dir is not a writable folder; nothing was changed (choose another --prefix or --app-dir)"
  done
}

staged=() backups=()
cleanup_staged() {
  local p
  for p in ${staged[@]+"${staged[@]}"}; do rm -rf "$p" || true; done
}
install_all() {
  local i n="${#dests[@]}" stage
  for ((i = 0; i < n; i++)); do
    stage="$(dirname "${dests[i]}")/.$(basename "${dests[i]}").relay-install.$$"
    if ! { mkdir -p "$(dirname "${dests[i]}")" && rm -rf "$stage" && cp -R "${srcs[i]}" "$stage"; }; then
      cleanup_staged
      die "could not write next to ${dests[i]}; nothing was changed"
    fi
    staged+=("$stage")
  done
  for ((i = 0; i < n; i++)); do
    backups+=("")
    if [[ -e "${dests[i]}" || -L "${dests[i]}" ]]; then
      backups[i]="${dests[i]}.relay-old.$$"
      mv "${dests[i]}" "${backups[i]}" || {
        backups[i]=""
        rollback "$i"
      }
    fi
    mv "${staged[i]}" "${dests[i]}" || rollback "$((i + 1))"
  done
  for ((i = 0; i < n; i++)); do
    if [[ -n "${backups[i]}" ]]; then rm -rf "${backups[i]}"; fi
  done
}
# Undoes the first $1 moves, restores backups and removes staged files. Keeps
# going after an individual failure and names any file it could not restore.
rollback() {
  local i kept=""
  for ((i = 0; i < $1; i++)); do
    if [[ -e "${dests[i]}" && ! -e "${staged[i]}" ]]; then rm -rf "${dests[i]}" || true; fi
    if [[ -n "${backups[i]:-}" ]]; then
      mv "${backups[i]}" "${dests[i]}" || kept="$kept ${backups[i]}"
    fi
  done
  cleanup_staged
  [[ -z "$kept" ]] || die "installation failed; could not restore, previous files are kept at:$kept"
  die "installation failed while moving files into place; the previous files were restored"
}

if [[ "$os" == macos ]]; then
  app_dir="${app_dir:-$HOME/Applications}"
  dest="$app_dir/Relay.app"
  add "$src" "$dest"
  refuse_existing
  preflight
  install_all
  say "installed Relay $version to $dest"
  say "this build is ad-hoc signed and not notarized; it was not quarantined because it was not downloaded by a browser"
else
  prefix="${prefix:-$HOME/.local}"
  # The desktop entry stores the absolute binary path unquoted.
  [[ "$prefix" =~ ^/[A-Za-z0-9._/+-]*$ ]] || die "--prefix must be an absolute path of letters, digits and . _ / + -"
  prefix="${prefix%/}"
  dest_bin="$prefix/bin/relay"
  awk -v exe="$dest_bin" '/^Exec=/ { print "Exec=" exe; next } { print }' \
    "$src/share/applications/$BUNDLE_ID.desktop" >"$tmp/$BUNDLE_ID.desktop"
  add "$bin" "$dest_bin"
  add "$tmp/$BUNDLE_ID.desktop" "$prefix/share/applications/$BUNDLE_ID.desktop"
  for size in 128 256 512; do
    icon="$src/share/icons/hicolor/${size}x${size}/apps/$BUNDLE_ID.png"
    if [[ -f "$icon" ]]; then add "$icon" "$prefix/share/icons/hicolor/${size}x${size}/apps/$BUNDLE_ID.png"; fi
  done
  refuse_existing
  preflight
  install_all
  if command -v update-desktop-database >/dev/null; then
    update-desktop-database "$prefix/share/applications" >/dev/null 2>&1 || true
  fi
  say "installed Relay $version: $dest_bin"
  case ":$PATH:" in *":$prefix/bin:"*) ;; *) say "note: $prefix/bin is not on PATH" ;; esac
  say "Relay needs a Vulkan driver, the X11/Wayland and xkbcommon libraries, and a StatusNotifierItem host for its tray icon (GNOME: AppIndicator extension)"
fi
