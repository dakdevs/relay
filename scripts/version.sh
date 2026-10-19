#!/usr/bin/env bash
# Relay release version tool.
#
# Display version: YYYYMMDD.N, stored in release/version. The date is the
# release-cut date in the maintainer's local calendar, fixed when the version is
# set and committed; CI never derives it from its own clock. N starts at 1 and
# counts releases cut on that date.
#
# Derived forms:
#   cargo   YYYYMMDD.N.0   root Cargo.toml [workspace.package] version
#   macos   YYYY.MMDD.N    CFBundleShortVersionString and CFBundleVersion
#   tag     vYYYYMMDD.N
set -euo pipefail

root="${RELAY_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
version_file="$root/release/version"
cargo_file="$root/Cargo.toml"

die() {
  printf 'version.sh: %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'EOF'
Usage: scripts/version.sh <command> [args]

  show                    Print the display version from release/version.
  check [--tag vVERSION]  Validate release/version, the Cargo.toml workspace
                          version and, if given, the release tag.
  next [--date YYYYMMDD]  Print the next version for the date (default: today,
                          local time), after release/version and git tags.
  set VERSION             Write VERSION to release/version and Cargo.toml.
  cargo VERSION           Print the Cargo form (YYYYMMDD.N.0).
  macos VERSION           Print the macOS bundle form (YYYY.MMDD.N).
  tag VERSION             Print the tag form (vYYYYMMDD.N).
EOF
}

# valid_date YYYYMMDD: a real Gregorian calendar date from 2000 to 9999.
valid_date() {
  local d="$1"
  [[ "$d" =~ ^([0-9]{4})([0-9]{2})([0-9]{2})$ ]] || return 1
  local y=$((10#${BASH_REMATCH[1]})) m=$((10#${BASH_REMATCH[2]})) day=$((10#${BASH_REMATCH[3]}))
  ((y >= 2000 && m >= 1 && m <= 12 && day >= 1)) || return 1
  local max
  case "$m" in
  1 | 3 | 5 | 7 | 8 | 10 | 12) max=31 ;;
  4 | 6 | 9 | 11) max=30 ;;
  2)
    if ((y % 4 == 0 && (y % 100 != 0 || y % 400 == 0))); then max=29; else max=28; fi
    ;;
  esac
  ((day <= max))
}

# parse VERSION: accept YYYYMMDD.N with optional leading v; set date and seq.
parse() {
  local v="${1#v}"
  [[ "$v" =~ ^([0-9]{8})\.([1-9][0-9]{0,3})$ ]] || die "invalid version '$1' (expected YYYYMMDD.N, N from 1 without leading zeros)"
  date="${BASH_REMATCH[1]}"
  seq="${BASH_REMATCH[2]}"
  valid_date "$date" || die "invalid date in version '$1'"
}

read_file_version() {
  [[ -f "$version_file" ]] || die "missing $version_file"
  local v
  v="$(<"$version_file")"
  v="${v%$'\n'}"
  parse "$v"
  printf '%s.%s' "$date" "$seq"
}

# Prints the [workspace.package] version from Cargo.toml, or nothing.
read_cargo_version() {
  awk '
    /^\[/ { in_pkg = ($0 == "[workspace.package]") ; next }
    in_pkg && /^version[ \t]*=/ {
      sub(/^version[ \t]*=[ \t]*"/, ""); sub(/".*$/, ""); print; exit
    }
  ' "$cargo_file"
}

cmd_check() {
  local tag=""
  while (($#)); do
    case "$1" in
    --tag)
      [[ $# -ge 2 ]] || die "--tag needs a value"
      tag="$2"
      shift 2
      ;;
    *) die "unknown argument '$1'" ;;
    esac
  done
  local v
  v="$(read_file_version)"
  if [[ -f "$cargo_file" ]]; then
    local c
    c="$(read_cargo_version)"
    [[ -n "$c" ]] || die "Cargo.toml has no [workspace.package] version"
    [[ "$c" == "$v.0" ]] || die "Cargo.toml workspace version '$c' does not match release/version '$v' (expected '$v.0')"
  fi
  if [[ -n "$tag" ]]; then
    [[ "$tag" == "v$v" ]] || die "tag '$tag' does not match release/version (expected 'v$v')"
  fi
  printf '%s\n' "$v"
}

cmd_next() {
  local today=""
  while (($#)); do
    case "$1" in
    --date)
      [[ $# -ge 2 ]] || die "--date needs a value"
      today="$2"
      shift 2
      ;;
    *) die "unknown argument '$1'" ;;
    esac
  done
  [[ -n "$today" ]] || today="$(date +%Y%m%d)"
  valid_date "$today" || die "invalid date '$today'"

  local candidates=() highest_date="" max_seq=0 v
  [[ -f "$version_file" ]] && candidates+=("$(read_file_version)")
  if git -C "$root" rev-parse --git-dir >/dev/null 2>&1; then
    while IFS= read -r v; do
      [[ "$v" =~ ^v[0-9]{8}\.[1-9][0-9]*$ ]] && candidates+=("${v#v}")
    done < <(git -C "$root" tag --list 'v*')
  fi
  for v in ${candidates[@]+"${candidates[@]}"}; do
    parse "$v"
    [[ -z "$highest_date" || "$date" > "$highest_date" ]] && highest_date="$date"
    if [[ "$date" == "$today" ]] && ((seq > max_seq)); then max_seq=$seq; fi
  done
  if [[ -n "$highest_date" && "$highest_date" > "$today" ]]; then
    die "latest version date $highest_date is after $today; pass --date"
  fi
  printf '%s.%s\n' "$today" $((max_seq + 1))
}

cmd_set() {
  [[ $# -eq 1 ]] || die "set needs exactly one VERSION"
  parse "$1"
  local v="$date.$seq"
  if [[ -f "$cargo_file" ]]; then
    [[ -n "$(read_cargo_version)" ]] || die "Cargo.toml has no [workspace.package] version line to update"
    local tmp
    tmp="$(mktemp "$cargo_file.XXXXXX")"
    awk -v ver="$v.0" '
      /^\[/ { in_pkg = ($0 == "[workspace.package]") }
      in_pkg && !done && /^version[ \t]*=/ { print "version = \"" ver "\""; done = 1; next }
      { print }
    ' "$cargo_file" >"$tmp"
    mv "$tmp" "$cargo_file"
    # Workspace member versions are recorded in Cargo.lock; CI builds --locked.
    if [[ -f "$root/Cargo.lock" ]]; then
      cargo update --workspace --offline --quiet --manifest-path "$cargo_file" ||
        printf 'version.sh: warning: run "cargo update --workspace" to refresh Cargo.lock\n' >&2
    fi
  fi
  mkdir -p "$(dirname "$version_file")"
  printf '%s\n' "$v" >"$version_file"
  printf '%s\n' "$v"
}

cmd="${1:-}"
[[ $# -gt 0 ]] && shift
case "$cmd" in
show) read_file_version && printf '\n' ;;
check) cmd_check "$@" ;;
next) cmd_next "$@" ;;
set) cmd_set "$@" ;;
cargo)
  [[ $# -eq 1 ]] || die "cargo needs VERSION"
  parse "$1"
  printf '%s.%s.0\n' "$date" "$seq"
  ;;
macos)
  [[ $# -eq 1 ]] || die "macos needs VERSION"
  parse "$1"
  printf '%s.%s.%s\n' "${date:0:4}" "$((10#${date:4:4}))" "$seq"
  ;;
tag)
  [[ $# -eq 1 ]] || die "tag needs VERSION"
  parse "$1"
  printf 'v%s.%s\n' "$date" "$seq"
  ;;
-h | --help | help) usage ;;
*)
  usage >&2
  exit 2
  ;;
esac
