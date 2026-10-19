#!/usr/bin/env bash
# Tests for scripts/vendor-licenses.sh in an isolated copy, with a stub `gh`
# serving a fixture repository tree and a stub third-party.sh listing the
# crates without license files. No network.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fails=0

ok() { printf 'ok   %s\n' "$1"; }
fail() {
  printf 'FAIL %s\n' "$1"
  fails=$((fails + 1))
}
check() {
  local name="$1"
  shift
  if "$@"; then ok "$name"; else fail "$name"; fi
}

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
repo="$scratch/repo"
mkdir -p "$repo/scripts" "$repo/release/licenses"
cp "$here/../vendor-licenses.sh" "$repo/scripts/"

c1="$(printf '1%.0s' {1..40})" c2="$(printf '2%.0s' {1..40})" tagc="$(printf '3%.0s' {1..40})" headc="$(printf '4%.0s' {1..40})"
fx="$scratch/fixture"
put() { # project commit path content
  mkdir -p "$(dirname "$fx/$1/$2/$3")"
  printf '%s\n' "$4" >"$fx/$1/$2/$3"
}
# nested: crate folder has only a NOTICE; its parent has nothing; the root has the license.
put o/nested "$c1" crates/inner/Cargo.toml x
put o/nested "$c1" crates/inner/NOTICE "inner notice"
put o/nested "$c1" crates/README.md x
put o/nested "$c1" LICENSE-MIT "Copyright (c) 2019 Nested Holder"
# bare: no license anywhere.
put o/bare "$c2" src/lib.rs x
put o/bare "$c2" NOTICE "notice without a license"
# tagged and head: no published commit recorded.
put o/tagged "$tagc" COPYING "tagged copying"
put o/head "$headc" LICENSE "head license"
# flaky: two license files; the second download can be made to fail.
flakyc="$(printf '5%.0s' {1..40})"
put o/flaky "$flakyc" LICENSE-APACHE "flaky apache"
put o/flaky "$flakyc" LICENSE-MIT "flaky mit"
printf 'o/tagged refs/tags/v1.0.0 %s\no/head HEAD %s\n' "$tagc" "$headc" >"$fx/refs"

mkdir "$scratch/bin"
cat >"$scratch/bin/gh" <<'EOF'
#!/bin/sh
# Minimal `gh api` replay: contents listings, raw file reads, commit lookups.
raw=0 path=""
for a in "$@"; do
  case "$a" in
  Accept:*raw) raw=1 ;;
  repos/*) path="${a#repos/}" ;;
  esac
done
project="$(printf '%s' "$path" | cut -d/ -f1-2)"
rest="$(printf '%s' "$path" | cut -d/ -f3-)"
case "$rest" in
commits/*)
  ref="${rest#commits/}"
  awk -v p="$project" -v r="$ref" '$1 == p && $2 == r { print $3; found = 1 } END { exit !found }' "$FIX/refs" && exit 0
  echo '{"message":"No commit found"}'
  exit 1
  ;;
contents*)
  sub="${rest#contents}"
  sub="${sub#/}"
  ref="${sub##*\?ref=}"
  sub="${sub%%\?ref=*}"
  target="$FIX/$project/$ref/$sub"
  if [ "$raw" = 1 ]; then
    [ -n "${FAIL_RAW:-}" ] && [ "$sub" = "$FAIL_RAW" ] && exit 1
    [ -f "$target" ] && exec cat "$target"
    exit 1
  fi
  [ -d "$target" ] || { echo '{"message":"Not Found"}'; exit 1; }
  for f in "$target"/*; do [ -f "$f" ] && basename "$f"; done
  exit 0
  ;;
esac
exit 1
EOF
chmod +x "$scratch/bin/gh"
# The fixture has no GitLab project; curl must not reach the network.
printf '#!/bin/sh\nexit 7\n' >"$scratch/bin/curl"
chmod +x "$scratch/bin/curl"

reg="$scratch/registry"
mkdir -p "$reg/nested" "$reg/bare" "$reg/tagged" "$reg/head"
printf '{"git":{"sha1":"%s"},"path_in_vcs":"crates/inner"}\n' "$c1" >"$reg/nested/.cargo_vcs_info.json"
printf '{"git":{"sha1":"%s"},"path_in_vcs":""}\n' "$c2" >"$reg/bare/.cargo_vcs_info.json"
row() { printf '%s\x1f1.0.0\x1fMIT\x1f%s\x1f%s\x1f%s\n' "$1" "$2" "$reg/$1" "${3:-}"; }
{
  row nested https://github.com/o/nested
  row bare http://github.com/o/bare/
  row tagged "" https://github.com/o/tagged/tree/main/crate
  row head https://github.com/o/head.git
} >"$scratch/missing"
# Stub third-party.sh: writes the fixture list to --missing.
cat >"$repo/scripts/third-party.sh" <<EOF
#!/bin/sh
while [ \$# -gt 0 ]; do [ "\$1" = --missing ] && cp "$scratch/missing" "\$2"; shift; done
exit 1
EOF
chmod +x "$repo/scripts/third-party.sh"

out="$scratch/out"
check "vendoring runs" env FIX="$fx" PATH="$scratch/bin:$PATH" "$repo/scripts/vendor-licenses.sh" x86_64-unknown-linux-gnu >"$out" 2>&1
index="$repo/release/licenses/upstream.tsv"
rows() { awk -F'\t' -v n="$1" '$1 == n { print $5 "|" $4 "|" $7 }' "$index" | LC_ALL=C sort | tr '\n' ' '; }

check "walks past a NOTICE-only folder to the root license" test "$(rows nested)" == "LICENSE-MIT|$c1|published-commit crates/inner/NOTICE|$c1|published-commit "
check "stored file keeps the upstream notice" grep -qx 'Copyright (c) 2019 Nested Holder' "$repo/release/licenses/upstream/$(awk -F'\t' '$1 == "nested" && $5 == "LICENSE-MIT" { print $6 }' "$index")"
named_by_hash() {
  local f
  for f in "$1"/*; do [[ "$(openssl dgst -sha256 -r "$f" | cut -c1-64)" == "$(basename "$f")" ]] || return 1; done
}
check "store name is the file's SHA-256" named_by_hash "$repo/release/licenses/upstream"
check "NOTICE alone is a gap, not a license" test -z "$(rows bare)"
check "gap names the crate and ignored NOTICE" grep -q 'gap: bare 1.0.0: no LICENSE, COPYING or UNLICENSE file .*NOTICE files ignored: NOTICE' "$out"
check "version tag used when no commit is recorded" test "$(rows tagged)" == "COPYING|$tagc|version-tag "
check "repository taken from homepage when missing" grep -q $'^tagged\t1.0.0\thttps://github.com/o/tagged\t' "$index"
check "default branch used, and labelled, without commit or tag" test "$(rows head)" == "LICENSE|$headc|default-branch "
check "summary counts one gap" grep -qx 'vendor-licenses.sh: 1 crate(s) without an upstream license file' "$out"

cp "$index" "$scratch/index.before"
env FIX="$fx" PATH="$scratch/bin:$PATH" "$repo/scripts/vendor-licenses.sh" x86_64-unknown-linux-gnu >/dev/null 2>&1
check "rerun leaves the index unchanged" cmp -s "$scratch/index.before" "$index"

# A crate whose download fails part-way is left out entirely and retried.
mkdir -p "$reg/flaky"
printf '{"git":{"sha1":"%s"},"path_in_vcs":""}\n' "$flakyc" >"$reg/flaky/.cargo_vcs_info.json"
row flaky https://github.com/o/flaky >>"$scratch/missing"
env FIX="$fx" FAIL_RAW=LICENSE-MIT PATH="$scratch/bin:$PATH" "$repo/scripts/vendor-licenses.sh" x86_64-unknown-linux-gnu >"$out" 2>&1 || true
check "failed download is reported for retry" grep -q 'gap: flaky 1.0.0: could not download LICENSE-MIT .*rerun to retry' "$out"
check "failed download leaves no partial rows" test -z "$(rows flaky)"
check "failed download leaves the rest of the index alone" cmp -s "$scratch/index.before" "$index"
env FIX="$fx" PATH="$scratch/bin:$PATH" "$repo/scripts/vendor-licenses.sh" x86_64-unknown-linux-gnu >"$out" 2>&1
check "rerun vendors the whole crate" test "$(rows flaky)" == "LICENSE-APACHE|$flakyc|published-commit LICENSE-MIT|$flakyc|published-commit "

rm "$repo/scripts/third-party.sh"
printf '#!/bin/sh\necho "broken" >&2\nexit 1\n' >"$repo/scripts/third-party.sh"
chmod +x "$repo/scripts/third-party.sh"
if env FIX="$fx" PATH="$scratch/bin:$PATH" "$repo/scripts/vendor-licenses.sh" x86_64-unknown-linux-gnu >"$out" 2>&1; then
  fail "failed dependency scan stops vendoring"
else
  check "failed dependency scan stops vendoring" grep -q 'third-party.sh failed' "$out"
fi

if ((fails)); then
  printf '%d vendor-licenses test(s) failed\n' "$fails"
  exit 1
fi
printf 'vendor-licenses tests passed\n'
