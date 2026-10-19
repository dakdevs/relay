#!/usr/bin/env bash
# Checks a Linux binary's shared-library dependencies against a package list.
#
#   scripts/linkage.sh BINARY PACKAGE_LIST
#
# - Fails if `ldd` reports any library in the full closure as "not found".
# - For each library the binary links directly (DT_NEEDED, from readelf -d),
#   finds the owning Debian package with dpkg -S and fails unless it is in
#   PACKAGE_LIST (one name per line, # comments) or is libc6 / libgcc-s1.
#   Libraries only needed by those libraries are left to apt's Depends.
#
# READELF, LDD and DPKG may name substitute commands (used by the tests).
set -euo pipefail

bin="${1:?BINARY}"
list="${2:?PACKAGE_LIST}"
readelf_cmd="${READELF:-readelf}"
ldd_cmd="${LDD:-ldd}"
dpkg_cmd="${DPKG:-dpkg}"

allowed="$(grep -v '^#' "$list" | grep . || true)"
allowed="$allowed"$'\nlibc6\nlibgcc-s1'

needed="$("$readelf_cmd" -d "$bin" | sed -n 's/.*(NEEDED).*\[\(.*\)\].*/\1/p')"
[[ -n "$needed" ]] || {
  echo "linkage.sh: $bin has no NEEDED entries (static or not ELF?)" >&2
  exit 1
}
closure="$("$ldd_cmd" "$bin")"

problems=0
missing="$(awk '$2 == "=>" && $3 == "not" { print $1 }' <<<"$closure")"
if [[ -n "$missing" ]]; then
  while IFS= read -r lib; do echo "linkage.sh: $lib: not found" >&2; done <<<"$missing"
  problems=1
fi

# Path ldd resolved for a soname: "name => /path (addr)" or "/path/name (addr)".
resolve() {
  awk -v n="$1" '
    $2 == "=>" && $1 == n && $3 ~ /^\// { print $3; exit }
    $1 ~ /^\// { k = split($1, p, "/"); if (p[k] == n) { print $1; exit } }
  ' <<<"$closure"
}

# Package owning a path; dpkg may record it under /lib or /usr/lib (merged
# /usr). Prints the name without an architecture suffix such as ":amd64".
owner() {
  local p hit
  for p in "$1" "$(readlink -f "$1" 2>/dev/null || printf '%s' "$1")" "${1#/usr}" "/usr${1#/usr}"; do
    hit="$("$dpkg_cmd" -S "$p" 2>/dev/null | grep -E '^[^ ]+: /' | head -n 1)" || continue
    if [[ -n "$hit" ]]; then
      hit="${hit%%: /*}"
      printf '%s\n' "${hit%%:*}"
      return 0
    fi
  done
  return 1
}

while IFS= read -r soname; do
  path="$(resolve "$soname")"
  if [[ -z "$path" ]]; then
    grep -qxF "$soname" <<<"$missing" || echo "linkage.sh: $soname: not resolved by ldd" >&2
    problems=1
    continue
  fi
  pkg="$(owner "$path" || true)"
  printf 'needs %-28s %-48s %s\n' "$soname" "$path" "${pkg:-unknown}"
  if ! grep -qxF "${pkg:-unknown}" <<<"$allowed"; then
    echo "linkage.sh: $soname comes from '${pkg:-unknown}', which is not in $list" >&2
    problems=1
  fi
done <<<"$needed"

exit "$problems"
