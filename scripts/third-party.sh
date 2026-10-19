#!/usr/bin/env bash
# Writes THIRD-PARTY-LICENSES.md for the crates linked into one package on one
# target: the normal (non-dev, non-build) dependency closure from
# `cargo metadata`, with each crate's declared license and its bundled license
# files.
#
#   scripts/third-party.sh --target TRIPLE [--package relay-desktop] --out FILE
#                          [--missing TSV]
#
# For a crate whose package ships no license file, the upstream license files
# vendored by scripts/vendor-licenses.sh (release/licenses/upstream.tsv, taken
# from the repository at the commit the crate was published from) are used
# verbatim. If upstream has none either, the crate is reported as a gap: the
# entry states that no copyright notice is available, lists the Cargo.toml
# authors as authors (not as copyright holders), and the standard SPDX text of
# its license is appended (for OR: the first of MIT, Apache-2.0, Zlib,
# BSD-3-Clause, BSD-2-Clause, ISC). The script fails if such a crate offers
# none of those or uses AND or WITH. --missing writes the crates whose package
# has no license file (name, version, license, repository, dir, homepage;
# separated by U+001F).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
texts="$root/release/licenses"
preferred=(MIT Apache-2.0 Zlib BSD-3-Clause BSD-2-Clause ISC)
upstream="$texts/upstream"
upstream_index="$texts/upstream.tsv"
target="" package="relay-desktop" out="" missing_out=""

die() {
  printf 'third-party.sh: %s\n' "$*" >&2
  exit 1
}

while (($#)); do
  [[ $# -ge 2 ]] || die "$1 needs a value"
  case "$1" in
  --target) target="$2" ;;
  --package) package="$2" ;;
  --out) out="$2" ;;
  --missing) missing_out="$2" ;;
  *) die "unknown argument '$1'" ;;
  esac
  shift 2
done
[[ -n "$target" && -n "$out" ]] || die "needs --target and --out"
command -v jq >/dev/null || die "jq is required"

sha256() { if command -v sha256sum >/dev/null; then sha256sum "$@"; else shasum -a 256 "$@"; fi; }
(cd "$texts" && sha256 --check --quiet SHA256SUMS) >/dev/null 2>&1 ||
  die "release/licenses texts do not match release/licenses/SHA256SUMS"
# Vendored upstream files are stored under their own SHA-256.
[[ -f "$upstream_index" ]] || die "missing $upstream_index"
while IFS=$'\t' read -r _ _ _ _ _ up_sha _; do
  [[ "$up_sha" =~ ^[0-9a-f]{64}$ && -f "$upstream/$up_sha" ]] || die "upstream.tsv names a missing file: $up_sha"
  [[ "$(sha256 "$upstream/$up_sha" | awk '{ print $1 }')" == "$up_sha" ]] || die "upstream/$up_sha does not match its checksum"
done < <(grep -v '^#' "$upstream_index" | grep . || true)
[[ -z "$missing_out" ]] || : >"$missing_out"

metadata="$(mktemp)"
body="$(mktemp)"
trap 'rm -f "$metadata" "$body"' EXIT
cargo metadata --manifest-path "$root/Cargo.toml" --format-version 1 --locked --filter-platform "$target" >"$metadata"

# One row (fields separated by U+001F) per third-party crate:
# name, version, license, repository, dir, authors, homepage.
rows="$(jq -r --arg pkg "$package" '
  (.packages | map({key: .id, value: .}) | from_entries) as $pkgs
  | (.resolve.nodes | map({key: .id, value: .}) | from_entries) as $nodes
  | ([.packages[] | select(.name == $pkg and .source == null) | .id] | first
     // error("package \($pkg) not found in workspace")) as $root
  | {todo: [$root], seen: {}}
  | until(.todo | length == 0;
      .todo[0] as $id
      | .todo |= .[1:]
      | if .seen[$id] then .
        else .seen[$id] = true
          | .todo += [$nodes[$id].deps[]
              | select(any(.dep_kinds[]; .kind == null)) | .pkg]
        end)
  | .seen | keys[] | $pkgs[.]
  | select(.source != null)
  | [.name, .version, (.license // ""), (.repository // ""),
     (.manifest_path | sub("/Cargo.toml$"; "")), ((.authors // []) | join(", ")), (.homepage // "")]
  | join("\u001f")
' "$metadata" | LC_ALL=C sort -t$'\x1f' -k1,1 -k2,2V)"
[[ -n "$rows" ]] || die "no third-party crates found for $package on $target"

# Prints the canonical license id to use for a crate without license files.
choose_text() {
  local expr="$1" id option
  [[ "$expr" != *" AND "* && "$expr" != *" WITH "* && "$expr" != *"("* ]] || return 1
  expr="${expr//\// OR }"
  for id in "${preferred[@]}"; do
    while IFS= read -r option; do
      option="${option#"${option%%[![:space:]]*}"}"
      option="${option%"${option##*[![:space:]]}"}"
      if [[ "$option" == "$id" ]]; then
        printf '%s\n' "$id"
        return 0
      fi
    done < <(awk '{ gsub(/ OR /, "\n"); print }' <<<"$expr")
  done
  return 1
}

fence='````'
failed=0 gaps=0 used=()
while IFS=$'\x1f' read -r name ver license repo dir authors homepage; do
  {
    printf '## %s %s\n\n' "$name" "$ver"
    printf 'License: %s\n' "${license:-not declared}"
    [[ -z "$repo" ]] || printf 'Repository: <%s>\n' "$repo"
    printf '\n'
  } >>"$body"
  found=0
  while IFS= read -r -d '' file; do
    found=1
    printf '### %s\n\n%stext\n%s\n%s\n\n' "$(basename "$file")" "$fence" "$(cat "$file")" "$fence" >>"$body"
  done < <(find -L "$dir" -maxdepth 1 -type f \( -iname 'LICENSE*' -o -iname 'LICENCE*' -o -iname 'COPYING*' -o -iname 'NOTICE*' -o -iname 'UNLICENSE*' \) -print0 | LC_ALL=C sort -z)
  ((found == 0)) || continue
  [[ -z "$missing_out" ]] || printf '%s\x1f%s\x1f%s\x1f%s\x1f%s\x1f%s\n' "$name" "$ver" "$license" "$repo" "$dir" "$homepage" >>"$missing_out"

  # Upstream license files vendored by scripts/vendor-licenses.sh.
  while IFS=$'\t' read -r _ _ up_repo up_commit up_path up_sha up_source; do
    found=1
    case "$up_source" in
    published-commit) origin="the commit this crate version was published from" ;;
    version-tag) origin="the tag for this version; the package records no commit" ;;
    *) origin="the default branch; this version has no recorded commit or tag, so the file may differ from the one current at release" ;;
    esac
    # shellcheck disable=SC2016 # backticks are Markdown
    printf '### %s\n\nUpstream file `%s` from <%s> at commit `%s` (%s).\n\n%stext\n%s\n%s\n\n' \
      "$(basename "$up_path")" "$up_path" "$up_repo" "$up_commit" "$origin" "$fence" "$(cat "$upstream/$up_sha")" "$fence" >>"$body"
  done < <(awk -F'\t' -v n="$name" -v v="$ver" '$1 == n && $2 == v' "$upstream_index")
  ((found == 0)) || continue

  gaps=$((gaps + 1))
  if id="$(choose_text "$license")"; then
    used+=("$id")
    printf 'No license file ships in this crate package or was found in its upstream repository at the published commit or version tag. It declares %s; the standard %s text is under [License texts](#license-texts). No copyright notice is available.\n\nAuthors listed in Cargo.toml: %s\n\n' \
      "$license" "$id" "${authors:-none}" >>"$body"
    printf 'third-party.sh: warning: %s %s has no license file in the package or upstream\n' "$name" "$ver" >&2
  else
    printf 'third-party.sh: %s %s has no license file and declares "%s"\n' "$name" "$ver" "${license:-nothing}" >&2
    failed=$((failed + 1))
  fi
done <<<"$rows"

{
  printf '# Third-party licenses\n\n'
  # shellcheck disable=SC2016 # backticks are Markdown
  printf 'Rust crates linked into `%s` for `%s`, from `Cargo.lock`.\n\n' "$package" "$target"
  printf '| Crate | Version | License |\n| --- | --- | --- |\n'
  while IFS=$'\x1f' read -r name ver license _; do
    printf '| %s | %s | %s |\n' "$name" "$ver" "${license:-not declared}"
  done <<<"$rows"
  printf '\n'
  cat "$body"
  if ((${#used[@]})); then
    printf '## License texts\n\nStandard SPDX texts (license-list-data v3.29.0) for crates above without any license file.\n\n'
    while IFS= read -r id; do
      printf '### %s\n\n%stext\n%s\n%s\n\n' "$id" "$fence" "$(cat "$texts/$id.txt")" "$fence"
    done < <(printf '%s\n' "${used[@]}" | LC_ALL=C sort -u)
  fi
} >"$out"

count="$(grep -c . <<<"$rows")"
printf 'third-party.sh: %s crates written to %s (%s without any license file)\n' "$count" "$out" "$gaps" >&2
((failed == 0)) || die "$failed crates without usable license text"
