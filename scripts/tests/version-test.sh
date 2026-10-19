#!/usr/bin/env bash
# Tests for scripts/version.sh. Expected values are written out by hand from
# the calendar and the documented mapping, not computed by the script under test.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
tool="$here/../version.sh"
fails=0

ok() { printf 'ok   %s\n' "$1"; }
fail() {
  printf 'FAIL %s\n' "$1"
  fails=$((fails + 1))
}
expect_out() { # name expected cmd...
  local name="$1" want="$2" got
  shift 2
  if got="$("$@" 2>/dev/null)" && [[ "$got" == "$want" ]]; then ok "$name"; else fail "$name: got '${got:-<error>}', want '$want'"; fi
}
expect_err() { # name cmd...
  local name="$1"
  shift
  if "$@" >/dev/null 2>&1; then fail "$name: expected failure"; else ok "$name"; fi
}

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
fixture() { # isolated root containing release/version
  local dir
  dir="$(mktemp -d "$scratch/root.XXXXXX")"
  mkdir -p "$dir/release"
  printf '%s\n' "$1" >"$dir/release/version"
  printf '%s\n' "$dir"
}

# Mapping.
expect_out "cargo form" "20261018.1.0" "$tool" cargo 20261018.1
expect_out "tag form" "v20261018.12" "$tool" tag 20261018.12
expect_out "tag form accepts v prefix" "v20261018.1" "$tool" tag v20261018.1
expect_out "macos form October" "2026.1018.1" "$tool" macos 20261018.1
expect_out "macos form drops month leading zero" "2026.105.3" "$tool" macos 20260105.3
expect_out "macos form December 31" "2026.1231.1" "$tool" macos 20261231.1

# Calendar validation.
expect_out "leap day in leap year" "20240229.1.0" "$tool" cargo 20240229.1
expect_out "leap day in 2000" "20000229.1.0" "$tool" cargo 20000229.1
expect_err "no leap day in 2023" "$tool" cargo 20230229.1
expect_err "no leap day in 2100" "$tool" cargo 21000229.1
expect_err "month 13" "$tool" cargo 20261301.1
expect_err "day 31 in a 30-day month" "$tool" cargo 20260931.1
expect_err "day 0" "$tool" cargo 20261000.1
expect_err "sequence 0" "$tool" cargo 20261018.0
expect_err "sequence leading zero" "$tool" cargo 20261018.01
expect_err "missing sequence" "$tool" cargo 20261018
expect_err "semver input" "$tool" cargo 2026.10.18
expect_err "trailing text" "$tool" cargo "20261018.1 x"
expect_err "shell metacharacters" "$tool" cargo '20261018.1;id'

# macOS bundle versions must be three integers and must increase over time.
prev=""
for v in 20261231.9 20270101.1 20270101.2 20270105.1 20271018.1; do
  m="$("$tool" macos "$v")"
  if [[ ! "$m" =~ ^[1-9][0-9]*\.[1-9][0-9]*\.[1-9][0-9]*$ ]]; then fail "macos $m shape"; fi
  if [[ -n "$prev" ]]; then
    IFS=. read -r a1 b1 c1 <<<"$prev"
    IFS=. read -r a2 b2 c2 <<<"$m"
    if ((a2 > a1 || (a2 == a1 && (b2 > b1 || (b2 == b1 && c2 > c1))))); then ok "macos $prev < $m"; else fail "macos $prev < $m"; fi
  fi
  prev="$m"
done

# check: release/version against Cargo.toml and the tag.
root="$(fixture 20261018.1)"
cat >"$root/Cargo.toml" <<'EOF'
[workspace]
members = []

[workspace.package]
version = "20261018.1.0"
edition = "2024"

[workspace.dependencies]
version = "not-this-one"
EOF
expect_out "check passes" "20261018.1" env RELAY_ROOT="$root" "$tool" check
expect_out "check with matching tag" "20261018.1" env RELAY_ROOT="$root" "$tool" check --tag v20261018.1
expect_err "check with other tag" env RELAY_ROOT="$root" "$tool" check --tag v20261018.2
sed -i.bak 's/^version = "20261018.1.0"/version = "20261018.2.0"/' "$root/Cargo.toml"
expect_err "check detects Cargo drift" env RELAY_ROOT="$root" "$tool" check

# set: rewrites both files, touching only the [workspace.package] version.
expect_out "set" "20261019.1" env RELAY_ROOT="$root" "$tool" set v20261019.1
if [[ "$(<"$root/release/version")" == "20261019.1" ]]; then ok "set writes release/version"; else fail "set writes release/version"; fi
if grep -qx 'version = "20261019.1.0"' "$root/Cargo.toml" && grep -qx 'version = "not-this-one"' "$root/Cargo.toml" && grep -qx 'edition = "2024"' "$root/Cargo.toml"; then
  ok "set edits only the workspace version"
else
  fail "set edits only the workspace version"
fi
expect_err "set rejects invalid" env RELAY_ROOT="$root" "$tool" set 20261332.1
if [[ "$(<"$root/release/version")" == "20261019.1" ]]; then ok "failed set changes nothing"; else fail "failed set changes nothing"; fi

# next: same day increments, new day restarts at 1, tags count, no going back.
root="$(fixture 20261018.1)"
expect_out "next same day" "20261018.2" env RELAY_ROOT="$root" "$tool" next --date 20261018
expect_out "next new day" "20261019.1" env RELAY_ROOT="$root" "$tool" next --date 20261019
expect_err "next before current" env RELAY_ROOT="$root" "$tool" next --date 20261017
expect_err "next invalid date" env RELAY_ROOT="$root" "$tool" next --date 20261032
git -C "$root" init -q
git -C "$root" -c user.name=t -c user.email=t@example.invalid commit -q --allow-empty -m init
for t in v20261018.1 v20261018.4 v20261018.10 not-a-version; do git -C "$root" tag "$t"; done
expect_out "next uses highest tag numerically" "20261018.11" env RELAY_ROOT="$root" "$tool" next --date 20261018
git -C "$root" tag v20261020.1
expect_err "next refuses date before newest tag" env RELAY_ROOT="$root" "$tool" next --date 20261019

# Repository state: the committed version is valid and matches Cargo.toml.
expect_out "repository version is consistent" "$("$tool" show)" "$tool" check

if ((fails)); then
  printf '%d version test(s) failed\n' "$fails"
  exit 1
fi
printf 'version tests passed\n'
