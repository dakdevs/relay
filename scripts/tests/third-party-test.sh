#!/usr/bin/env bash
# Tests for scripts/third-party.sh against hand-written `cargo metadata` output
# served by a stub `cargo`, in an isolated copy of the script with a fixture
# upstream-license index. No network and no real Cargo involved.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
real_licenses="$here/../../release/licenses"
fails=0

ok() { printf 'ok   %s\n' "$1"; }
fail() {
  printf 'FAIL %s\n' "$1"
  fails=$((fails + 1))
}
refuse() { if "$@"; then return 1; fi; }
lacks() { ! grep -q -- "$1" <<<"$2"; } # pattern text
check() {
  local name="$1"
  shift
  if "$@"; then ok "$name"; else fail "$name"; fi
}
sha() { openssl dgst -sha256 -r "$1" | awk '{ print $1 }'; }

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

# Isolated repository: the script, the canonical texts, a fixture upstream store.
repo="$scratch/repo"
mkdir -p "$repo/scripts" "$repo/release/licenses/upstream"
cp "$here/../third-party.sh" "$repo/scripts/"
cp "$real_licenses"/*.txt "$real_licenses/SHA256SUMS" "$repo/release/licenses/"
tool="$repo/scripts/third-party.sh"
printf 'MIT License\n\nCopyright (c) 2018 Upstream Holder Inc.\n' >"$scratch/up-mit"
printf 'Apache notice from upstream default branch\n' >"$scratch/up-head"
up_mit="$(sha "$scratch/up-mit")" up_head="$(sha "$scratch/up-head")"
cp "$scratch/up-mit" "$repo/release/licenses/upstream/$up_mit"
cp "$scratch/up-head" "$repo/release/licenses/upstream/$up_head"
{
  printf '# name\tversion\trepository\tcommit\tupstream_path\tsha256\tsource\n'
  printf 'vendored\t1.0.0\thttps://example.invalid/vendored\t%s\tLICENSE\t%s\tpublished-commit\n' "$(printf 'a%.0s' {1..40})" "$up_mit"
  printf 'from-head\t1.0.0\thttps://example.invalid/from-head\t%s\tLICENSE-APACHE\t%s\tdefault-branch\n' "$(printf 'b%.0s' {1..40})" "$up_head"
} >"$repo/release/licenses/upstream.tsv"

reg="$scratch/registry"
crate() { # dir-name [license-file-content]
  mkdir -p "$reg/$1"
  if [[ $# -ge 2 ]]; then printf '%s\n' "$2" >"$reg/$1/LICENSE-MIT"; fi
}
for c in dual-slash apache-only zlib-first gpl-only and-expr dev-only no-authors vendored from-head no-repo; do crate "$c"; done
crate has-file "Copyright (c) 2020 Has File Authors -- original text"

pkg() { # id license authors-json [repository-json]
  printf '{"id":"%s","name":"%s","version":"1.0.0","license":%s,"repository":%s,"homepage":"https://example.invalid/home/%s","source":"registry+https://github.com/rust-lang/crates.io-index","manifest_path":"%s/%s/Cargo.toml","authors":%s}' \
    "$1" "$1" "$2" "${4:-\"https://example.invalid/$1\"}" "$1" "$reg" "$1" "$3"
}
dep() { printf '{"pkg":"%s","dep_kinds":[{"kind":%s}]}' "$1" "$2"; }
metadata() { # app deps (JSON objects, comma-separated)
  cat <<EOF
{"packages":[
{"id":"app","name":"relay-desktop","version":"1.0.0","license":"MIT","source":null,"manifest_path":"/ws/Cargo.toml","authors":[]},
$(pkg has-file '"MIT"' '["Has File <a@example.invalid>"]'),
$(pkg dual-slash '"MIT/Apache-2.0"' '["Alice", "Bob"]'),
$(pkg apache-only '"Apache-2.0"' '["Carol"]'),
$(pkg zlib-first '"Zlib OR Apache-2.0 OR MIT"' '["Dan"]'),
$(pkg gpl-only '"GPL-3.0-only"' '["Eve"]'),
$(pkg and-expr '"MIT AND Apache-2.0"' '["Frank"]'),
$(pkg dev-only '"GPL-3.0-only"' '["Grace"]'),
$(pkg no-authors '"MIT"' '[]'),
$(pkg vendored '"MIT"' '["Heidi"]'),
$(pkg from-head '"Apache-2.0"' '["Ivan"]'),
$(pkg no-repo '"MIT"' '["Judy"]' null)
],
"resolve":{"nodes":[
{"id":"app","deps":[$1]},
{"id":"has-file","deps":[]},{"id":"dual-slash","deps":[]},{"id":"apache-only","deps":[]},
{"id":"zlib-first","deps":[]},{"id":"gpl-only","deps":[]},{"id":"and-expr","deps":[]},
{"id":"dev-only","deps":[]},{"id":"no-authors","deps":[$(dep apache-only null)]},
{"id":"vendored","deps":[]},{"id":"from-head","deps":[]},{"id":"no-repo","deps":[]}
]}}
EOF
}

mkdir "$scratch/bin"
# shellcheck disable=SC2016 # expanded by the stub at run time
printf '#!/bin/sh\ncat "$STUB_METADATA"\n' >"$scratch/bin/cargo"
chmod +x "$scratch/bin/cargo"
run() { # metadata-file out [extra args]
  local m="$1" o="$2"
  shift 2
  STUB_METADATA="$m" PATH="$scratch/bin:$PATH" "$tool" --target x86_64-unknown-linux-gnu --out "$o" "$@" 2>"$scratch/err"
}

good="$(dep has-file null),$(dep dual-slash null),$(dep zlib-first null),$(dep no-authors null),$(dep dev-only '"dev"'),$(dep vendored null),$(dep from-head null),$(dep no-repo null)"
metadata "$good" >"$scratch/good.json"
out="$scratch/good.md"
check "permissive closure succeeds" run "$scratch/good.json" "$out" --missing "$scratch/missing"
section() { awk -v h="## $1 1.0.0" '$0 == h { on = 1; print; next } on && /^## / { exit } on' "$out"; }

check "package license file copied verbatim" grep -q 'Copyright (c) 2020 Has File Authors -- original text' "$out"
check "package license file: no gap note" lacks "No license file ships" "$(section has-file)"
check "vendored upstream notice preserved" grep -q 'Copyright (c) 2018 Upstream Holder Inc.' <<<"$(section vendored)"
check "vendored file names repository and commit" grep -q "from <https://example.invalid/vendored> at commit \`aaaaaaaa" <<<"$(section vendored)"
check "vendored file says published commit" grep -q 'the commit this crate version was published from' <<<"$(section vendored)"
check "vendored crate is not a gap" lacks "No license file ships" "$(section vendored)"
check "default-branch source is labelled" grep -q 'may differ from the one current at release' <<<"$(section from-head)"
check "gap crate names standard text" grep -q 'declares MIT/Apache-2.0; the standard MIT text' <<<"$(section dual-slash)"
check "gap crate states no notice" grep -q 'No copyright notice is available.' <<<"$(section dual-slash)"
check "gap crate lists Cargo authors as authors" grep -qx 'Authors listed in Cargo.toml: Alice, Bob' <<<"$(section dual-slash)"
check "no copyright line synthesized from authors" lacks 'Copyright (c) Alice' "$(cat "$out")"
check "no-authors gap says none" grep -qx 'Authors listed in Cargo.toml: none' <<<"$(section no-authors)"
check "Zlib OR Apache-2.0 OR MIT prefers MIT" grep -q 'the standard MIT text' <<<"$(section zlib-first)"
check "transitive Apache-only crate included" grep -q 'the standard Apache-2.0 text' <<<"$(section apache-only)"
check "dev-only dependency excluded" lacks "dev-only" "$(cat "$out")"
check "workspace crate excluded" lacks "^## relay-desktop" "$(cat "$out")"
check "MIT text appended once" test "$(grep -c '^### MIT$' "$out")" -eq 1
check "Apache-2.0 text appended once" test "$(grep -c '^### Apache-2.0$' "$out")" -eq 1
check "appended MIT text is the canonical file" grep -qF "$(sed -n 5p "$real_licenses/MIT.txt")" "$out"
check "unused Zlib text not appended" lacks '^### Zlib$' "$(cat "$out")"
check "summary counts gaps" grep -q '(5 without any license file)' "$scratch/err"

# --missing lists every crate whose package lacks a file, keeping empty fields.
check "--missing lists package-less crates" test "$(cut -d$'\x1f' -f1 "$scratch/missing" | LC_ALL=C sort | tr '\n' ' ')" == "apache-only dual-slash from-head no-authors no-repo vendored zlib-first "
check "--missing keeps empty repository field" test "$(grep '^no-repo' "$scratch/missing" | cut -d$'\x1f' -f4,6)" == $'\x1fhttps://example.invalid/home/no-repo'

metadata "$good,$(dep gpl-only null)" >"$scratch/gpl.json"
if run "$scratch/gpl.json" "$scratch/gpl.md"; then fail "GPL-only crate without file fails"; else
  check "GPL-only crate without file fails" grep -q 'gpl-only 1.0.0 has no license file' "$scratch/err"
fi
metadata "$good,$(dep and-expr null)" >"$scratch/and.json"
check "AND expression without file fails" refuse run "$scratch/and.json" "$scratch/and.md"

printf 'tampered\n' >>"$repo/release/licenses/upstream/$up_mit"
if run "$scratch/good.json" "$scratch/t.md"; then fail "tampered upstream file is refused"; else
  check "tampered upstream file is refused" grep -q 'does not match its checksum' "$scratch/err"
fi
cp "$scratch/up-mit" "$repo/release/licenses/upstream/$up_mit"
printf 'tampered\n' >>"$repo/release/licenses/MIT.txt"
if run "$scratch/good.json" "$scratch/t.md"; then fail "tampered canonical text is refused"; else
  check "tampered canonical text is refused" grep -q 'do not match' "$scratch/err"
fi

if ((fails)); then
  printf '%d third-party test(s) failed\n' "$fails"
  exit 1
fi
printf 'third-party tests passed\n'
