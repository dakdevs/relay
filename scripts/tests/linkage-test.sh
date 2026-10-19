#!/usr/bin/env bash
# Tests for scripts/linkage.sh with stub readelf, ldd and dpkg replaying
# fixture output shaped like Ubuntu 22.04. Runs anywhere; needs no ELF binary.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
tool="$here/../linkage.sh"
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
mkdir "$scratch/bin"
# shellcheck disable=SC2016 # expanded by the stubs at run time
{
  printf '#!/bin/sh\ncat "$FIX_READELF"\n' >"$scratch/bin/readelf"
  printf '#!/bin/sh\ncat "$FIX_LDD"\n' >"$scratch/bin/ldd"
  # dpkg -S PATH: answers only for paths listed in $FIX_DPKG ("path package").
  printf '#!/bin/sh\nawk -v p="$2" '"'"'$1 == p { print $2 ": " $1; found = 1 } END { if (!found) { print "dpkg-query: no path found matching pattern " p > "/dev/stderr"; exit 1 } }'"'"' "$FIX_DPKG"\n' >"$scratch/bin/dpkg"
}
chmod +x "$scratch/bin/"*
: >"$scratch/relay"

printf '# runtime packages\nlibxkbcommon0\nlibxkbcommon-x11-0\nlibxcb1\nlibfontconfig1\n\n' >"$scratch/allowed"

L=/lib/x86_64-linux-gnu
readelf_out() { # extra NEEDED sonames...
  printf 'Dynamic section at offset 0x1 contains 30 entries:\n  Tag        Type                         Name/Value\n'
  printf ' 0x000000000000000e (SONAME)             Library soname: [librelay.so.1]\n'
  # shellcheck disable=SC2016 # a literal $ORIGIN
  printf ' 0x000000000000001d (RUNPATH)            Library runpath: [$ORIGIN]\n'
  local s
  for s in libxkbcommon.so.0 libxkbcommon-x11.so.0 libxcb.so.1 libfontconfig.so.1 libgcc_s.so.1 libm.so.6 libc.so.6 ld-linux-x86-64.so.2 "$@"; do
    printf ' 0x0000000000000001 (NEEDED)             Shared library: [%s]\n' "$s"
  done
}
ldd_out() { # extra lines...
  printf '\tlinux-vdso.so.1 (0x00007ffd)\n'
  local s
  for s in libxkbcommon.so.0 libxkbcommon-x11.so.0 libxcb.so.1 libfontconfig.so.1 libgcc_s.so.1 libm.so.6 libc.so.6 \
    libxcb-xkb.so.1 libexpat.so.1 libuuid.so.1 libpng16.so.16 libz.so.1 libbrotlidec.so.1 libXau.so.6 libXdmcp.so.6 libbsd.so.0 libmd.so.0; do
    printf '\t%s => %s/%s (0x00007f01)\n' "$s" "$L" "$s"
  done
  printf '\t/lib64/ld-linux-x86-64.so.2 (0x00007f02)\n'
  local extra
  for extra in "$@"; do printf '\t%s\n' "$extra"; done
}
{
  # Direct dependencies: some recorded under /usr/lib (merged /usr), arch suffixes.
  printf '/usr%s/libxkbcommon.so.0 libxkbcommon0:amd64\n' "$L"
  printf '%s/libxkbcommon-x11.so.0 libxkbcommon-x11-0:arm64\n' "$L"
  printf '%s/libxcb.so.1 libxcb1:amd64\n' "$L"
  printf '/usr%s/libfontconfig.so.1 libfontconfig1:amd64\n' "$L"
  printf '%s/libgcc_s.so.1 libgcc-s1:amd64\n' "$L"
  printf '%s/libm.so.6 libc6:amd64\n' "$L"
  printf '%s/libc.so.6 libc6:amd64\n' "$L"
  printf '/lib64/ld-linux-x86-64.so.2 libc6:amd64\n'
  # Transitive only: not in the allowed list.
  printf '%s/libxcb-xkb.so.1 libxcb-xkb1:amd64\n' "$L"
  printf '%s/libexpat.so.1 libexpat1:amd64\n' "$L"
  printf '%s/libuuid.so.1 libuuid1:amd64\n' "$L"
  printf '%s/libpng16.so.16 libpng16-16:amd64\n' "$L"
  printf '%s/libz.so.1 zlib1g:amd64\n' "$L"
} >"$scratch/dpkg"

run() { # readelf-fixture ldd-fixture
  FIX_READELF="$1" FIX_LDD="$2" FIX_DPKG="$scratch/dpkg" READELF="$scratch/bin/readelf" LDD="$scratch/bin/ldd" DPKG="$scratch/bin/dpkg" \
    "$tool" "$scratch/relay" "$scratch/allowed" >"$scratch/out" 2>"$scratch/err"
}

readelf_out >"$scratch/r.ok"
ldd_out >"$scratch/l.ok"
check "listed direct deps pass despite unlisted transitive deps" run "$scratch/r.ok" "$scratch/l.ok"
check "reports one line per NEEDED entry" test "$(grep -c '^needs ' "$scratch/out")" -eq 8
check "SONAME and RUNPATH are not dependencies" test -z "$(grep -E 'librelay|ORIGIN' "$scratch/out" || true)"
check "architecture suffix stripped" grep -qE '^needs libxkbcommon-x11\.so\.0 .* libxkbcommon-x11-0$' "$scratch/out"
check "merged-/usr owner found" grep -qE '^needs libxkbcommon\.so\.0 .* libxkbcommon0$' "$scratch/out"
check "dynamic loader resolved by absolute path" grep -qE '^needs ld-linux-x86-64\.so\.2 +/lib64/ld-linux-x86-64\.so\.2 +libc6$' "$scratch/out"
check "transitive libraries not checked" test -z "$(grep -E 'libexpat|libuuid|libpng' "$scratch/out" || true)"

readelf_out libuuid.so.1 >"$scratch/r.direct"
if run "$scratch/r.direct" "$scratch/l.ok"; then fail "unlisted direct dependency fails"; else
  check "unlisted direct dependency fails" grep -q "libuuid.so.1 comes from 'libuuid1'" "$scratch/err"
fi

ldd_out 'libbsd2.so.0 => not found' >"$scratch/l.missing"
if run "$scratch/r.ok" "$scratch/l.missing"; then fail "missing transitive library fails"; else
  check "missing transitive library fails" grep -qx 'linkage.sh: libbsd2.so.0: not found' "$scratch/err"
fi

readelf_out libgone.so.9 >"$scratch/r.gone"
ldd_out 'libgone.so.9 => not found' >"$scratch/l.gone"
if run "$scratch/r.gone" "$scratch/l.gone"; then fail "missing direct library fails"; else
  check "missing direct library reported once" test "$(grep -c 'libgone.so.9' "$scratch/err")" -eq 1
fi

readelf_out libghost.so.1 >"$scratch/r.ghost"
if run "$scratch/r.ghost" "$scratch/l.ok"; then fail "unresolved NEEDED entry fails"; else
  check "unresolved NEEDED entry fails" grep -q 'libghost.so.1: not resolved by ldd' "$scratch/err"
fi

printf 'Dynamic section at offset 0x1 contains 0 entries:\n' >"$scratch/r.static"
if run "$scratch/r.static" "$scratch/l.ok"; then fail "binary without NEEDED entries fails"; else
  check "binary without NEEDED entries fails" grep -q 'no NEEDED entries' "$scratch/err"
fi

if ((fails)); then
  printf '%d linkage test(s) failed\n' "$fails"
  exit 1
fi
printf 'linkage tests passed\n'
