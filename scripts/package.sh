#!/usr/bin/env bash
# Packages a built `relay` binary into a release archive, and writes the
# release checksum list and the pinned installer.
#
#   scripts/package.sh archive --target TRIPLE --binary PATH --out DIR [--licenses FILE]
#   scripts/package.sh sums DIR
#   scripts/package.sh installer DIR
#
# Archives: relay-<version>-<os>-<arch>.tar.gz, where os is macos or linux and
# arch is x86_64 or aarch64.
#   macOS: Relay.app (ad-hoc signed, not Developer ID signed or notarized)
#   Linux: relay-<version>-linux-<arch>/{bin/relay, share/applications/*.desktop,
#          share/icons/hicolor/*/apps/dev.dak.relay.png}
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bundle_id="dev.dak.relay"
min_macos="12.0"

die() {
  printf 'package.sh: %s\n' "$*" >&2
  exit 1
}

version="$("$root/scripts/version.sh" check)"

# Platform pair for a Rust target triple, e.g. "macos aarch64".
platform_of() {
  case "$1" in
  aarch64-apple-darwin) echo "macos aarch64" ;;
  x86_64-apple-darwin) echo "macos x86_64" ;;
  x86_64-unknown-linux-gnu) echo "linux x86_64" ;;
  aarch64-unknown-linux-gnu) echo "linux aarch64" ;;
  *) die "unsupported target '$1'" ;;
  esac
}

# Reproducible archive: the same staged files give the same bytes. Entries are
# listed in byte order, owned by 0:0, dated 00:00 UTC on the version date,
# with permissions reduced to 0755/0644, in ustar format without macOS
# extended attributes, compressed by gzip without a name or timestamp.
make_tar() {
  local out="$1" dir="$2" entry="$3" list
  list="$(mktemp)"
  (
    cd "$dir"
    chmod -R u+rwX,go+rX,go-w "$entry"
    find "$entry" -exec env TZ=UTC touch -h -t "${version%%.*}0000.00" {} +
    find "$entry" -print | LC_ALL=C sort >"$list"
  )
  if tar --version 2>/dev/null | grep -q 'GNU tar'; then
    tar -C "$dir" --format=ustar --owner=0 --group=0 --numeric-owner --no-recursion -T "$list" -cf - | gzip -n -9 >"$out"
  else
    COPYFILE_DISABLE=1 tar -C "$dir" --format ustar --uid 0 --gid 0 --numeric-owner --no-xattrs --no-mac-metadata -n -T "$list" -cf - | gzip -n -9 >"$out"
  fi
  rm -f "$list"
}

sha256() {
  if command -v sha256sum >/dev/null; then sha256sum "$@"; else shasum -a 256 "$@"; fi
}

write_docs() {
  local dest="$1" licenses="$2"
  cp "$root/LICENSE" "$dest/LICENSE"
  cp "$root/NOTICE.md" "$dest/NOTICE.md"
  if [[ -n "$licenses" ]]; then
    cp "$licenses" "$dest/THIRD-PARTY-LICENSES.md"
  fi
}

info_plist() {
  local mac_version
  mac_version="$("$root/scripts/version.sh" macos "$version")"
  cat <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleDisplayName</key>
	<string>Relay</string>
	<key>CFBundleExecutable</key>
	<string>relay</string>
	<key>CFBundleIconFile</key>
	<string>relay</string>
	<key>CFBundleIdentifier</key>
	<string>$bundle_id</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>Relay</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>$mac_version</string>
	<key>CFBundleVersion</key>
	<string>$mac_version</string>
	<key>RelayDisplayVersion</key>
	<string>$version</string>
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.developer-tools</string>
	<key>LSMinimumSystemVersion</key>
	<string>$min_macos</string>
	<key>LSUIElement</key>
	<true/>
	<key>NSHighResolutionCapable</key>
	<true/>
</dict>
</plist>
EOF
}

cmd_archive() {
  local target="" binary="" out="" licenses=""
  while (($#)); do
    [[ $# -ge 2 ]] || die "$1 needs a value"
    case "$1" in
    --target) target="$2" ;;
    --binary) binary="$2" ;;
    --out) out="$2" ;;
    --licenses) licenses="$2" ;;
    *) die "unknown argument '$1'" ;;
    esac
    shift 2
  done
  [[ -n "$target" && -n "$binary" && -n "$out" ]] || die "archive needs --target, --binary and --out"
  [[ -x "$binary" ]] || die "binary '$binary' is not executable"
  [[ -z "$licenses" || -f "$licenses" ]] || die "licenses file '$licenses' not found"

  local os arch
  read -r os arch <<<"$(platform_of "$target")"
  local name="relay-$version-$os-$arch"
  mkdir -p "$out"
  out="$(cd "$out" && pwd)"
  local stage
  stage="$(mktemp -d)"
  trap 'rm -rf "$stage"' RETURN

  local entry
  if [[ "$os" == macos ]]; then
    entry="Relay.app"
    local app="$stage/$entry"
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    install -m 0755 "$binary" "$app/Contents/MacOS/relay"
    cp "$root/release/icons/relay.icns" "$app/Contents/Resources/relay.icns"
    info_plist >"$app/Contents/Info.plist"
    printf 'APPL????' >"$app/Contents/PkgInfo"
    write_docs "$app/Contents/Resources" "$licenses"
    if command -v plutil >/dev/null; then plutil -lint "$app/Contents/Info.plist" >/dev/null; fi
    # Ad-hoc signature over the whole bundle; Apple Silicon refuses unsigned code.
    if command -v codesign >/dev/null; then
      codesign --force --sign - --timestamp=none "$app"
      codesign --verify --strict "$app"
    fi
  else
    entry="$name"
    local dir="$stage/$entry"
    mkdir -p "$dir/bin" "$dir/share/applications"
    install -m 0755 "$binary" "$dir/bin/relay"
    install -m 0644 "$root/release/linux/$bundle_id.desktop" "$dir/share/applications/$bundle_id.desktop"
    local size
    for size in 128 256 512; do
      mkdir -p "$dir/share/icons/hicolor/${size}x${size}/apps"
      install -m 0644 "$root/release/icons/relay-$size.png" "$dir/share/icons/hicolor/${size}x${size}/apps/$bundle_id.png"
    done
    write_docs "$dir" "$licenses"
  fi

  make_tar "$out/$name.tar.gz" "$stage" "$entry"
  printf '%s\n' "$out/$name.tar.gz"
}

cmd_sums() {
  local dir="${1:?sums needs DIR}"
  (
    cd "$dir"
    shopt -s nullglob
    local archives=(relay-"$version"-*.tar.gz)
    ((${#archives[@]})) || die "no relay-$version-*.tar.gz archives in $dir"
    sha256 "${archives[@]}" | LC_ALL=C sort -k2 >SHA256SUMS
  )
  cat "$dir/SHA256SUMS"
}

# Copies scripts/install.sh with this release's version and archive checksums
# baked in, then appends the installer's own checksum to SHA256SUMS.
cmd_installer() {
  local dir="${1:?installer needs DIR}"
  [[ -f "$dir/SHA256SUMS" ]] || die "run 'sums' first"
  grep -q 'install.sh$' "$dir/SHA256SUMS" && die "SHA256SUMS already lists install.sh"
  local line
  while IFS= read -r line; do
    [[ "$line" =~ ^[0-9a-f]{64}\ \ relay-[0-9]{8}\.[0-9]+-(macos|linux)-(x86_64|aarch64)\.tar\.gz$ ]] || die "unexpected SHA256SUMS line: $line"
  done <"$dir/SHA256SUMS"
  awk -v ver="$version" -v sums="$dir/SHA256SUMS" '
    /^PINNED_VERSION=""$/ { print "PINNED_VERSION=\"" ver "\""; next }
    /^PINNED_SHA256=""$/ {
      print "PINNED_SHA256=\""
      while ((getline l < sums) > 0) print l
      print "\""
      next
    }
    { print }
  ' "$root/scripts/install.sh" >"$dir/install.sh"
  grep -q "^PINNED_VERSION=\"$version\"\$" "$dir/install.sh" || die "failed to pin version in install.sh"
  chmod 0755 "$dir/install.sh"
  (cd "$dir" && sha256 install.sh >>SHA256SUMS)
  printf '%s\n' "$dir/install.sh"
}

cmd="${1:-}"
[[ $# -gt 0 ]] && shift
case "$cmd" in
archive) cmd_archive "$@" ;;
sums) cmd_sums "$@" ;;
installer) cmd_installer "$@" ;;
*)
  sed -n '2,12p' "$0" >&2
  exit 2
  ;;
esac
