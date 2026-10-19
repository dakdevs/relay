#!/usr/bin/env bash
# Maintainer tool (network): vendors upstream license files for dependency
# crates whose published package ships none, so THIRD-PARTY-LICENSES.md can
# carry the real notice instead of a generic text.
#
#   scripts/vendor-licenses.sh [TARGET...]   (default: all four release targets)
#
# For each such crate it reads the commit the crate was published from
# (.cargo_vcs_info.json; if absent, a version tag v<ver>, <ver>, <name>-v<ver>
# or <name>-<ver>; if neither exists, the default branch head, labelled as
# such in the output), lists the crate's directory in the upstream repository at
# that commit and then each parent directory up to the root. It stops at the
# first directory with a LICENSE, COPYING or UNLICENSE file and takes those,
# plus NOTICE files from every directory it passed. Files are stored as
# release/licenses/upstream/<sha256> and indexed in release/licenses/upstream.tsv
# (name, version, repository, commit, path, sha256, source). GitHub repositories are
# read with `gh api`; gitlab.redox-os.org through its public API. Crates it
# cannot resolve are printed as gaps and left out of the index.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
store="$root/release/licenses/upstream"
index="$root/release/licenses/upstream.tsv"
targets=("$@")
((${#targets[@]})) || targets=(aarch64-apple-darwin x86_64-apple-darwin x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu)
license_re='^(LICEN[CS]E|COPYING|NOTICE|UNLICENSE)'

for tool in gh jq curl; do
  command -v "$tool" >/dev/null || {
    echo "vendor-licenses.sh: needs $tool" >&2
    exit 1
  }
done
sha256() { if command -v sha256sum >/dev/null; then sha256sum "$@"; else shasum -a 256 "$@"; fi; }

mkdir -p "$store"
[[ -f "$index" ]] || printf '# name\tversion\trepository\tcommit\tupstream_path\tsha256\tsource\n' >"$index"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
for t in "${targets[@]}"; do
  # Non-zero is expected while crates lack usable license text; a missing list
  # means the script failed before reading the dependency graph.
  "$root/scripts/third-party.sh" --target "$t" --out "$work/out.md" --missing "$work/missing-$t.tsv" 2>"$work/err-$t" || true
  [[ -f "$work/missing-$t.tsv" ]] || {
    cat "$work/err-$t" >&2
    echo "vendor-licenses.sh: third-party.sh failed for $t" >&2
    exit 1
  }
done
cat "$work"/missing-*.tsv | LC_ALL=C sort -u >"$work/missing.tsv"


# list HOST PROJECT COMMIT DIR -> license file names in DIR, one per line.
# Fails when the listing itself fails (unknown commit, network error).
list() {
  local host="$1" project="$2" commit="$3" dir="$4" all
  if [[ "$host" == github.com ]]; then
    all="$(gh api "repos/$project/contents/$dir?ref=$commit" --jq '.[] | select(.type == "file" or .type == "symlink") | .name' 2>/dev/null)" || return 1
  else
    all="$(curl -fsS --retry 3 "https://$host/api/v4/projects/${project//\//%2F}/repository/tree?ref=$commit&path=$dir&per_page=100" | jq -r '.[] | select(.type == "blob") | .name')" || return 1
  fi
  grep -iE "$license_re" <<<"$all" || true
}
fetch() { # HOST PROJECT COMMIT PATH OUT
  local host="$1" project="$2" commit="$3" path="$4" out="$5"
  if [[ "$host" == github.com ]]; then
    gh api -H 'Accept: application/vnd.github.raw' "repos/$project/contents/$path?ref=$commit" >"$out"
  else
    curl -fsS "https://$host/api/v4/projects/${project//\//%2F}/repository/files/${path//\//%2F}/raw?ref=$commit" >"$out"
  fi
}
github_sha() { # PROJECT REF -> commit sha; gh prints error bodies on stdout
  local sha
  sha="$(gh api "repos/$1/commits/$2" --jq .sha 2>/dev/null)" || return 1
  [[ "$sha" =~ ^[0-9a-f]{40}$ ]] && printf '%s\n' "$sha"
}
resolve_tag() { # PROJECT NAME VERSION -> commit sha (GitHub only)
  local tag
  for tag in "v$3" "$3" "$2-v$3" "$2-$3"; do
    github_sha "$1" "refs/tags/$tag" && return 0
  done
  return 1
}

gaps=0
while IFS=$'\x1f' read -r name ver _ repo dir homepage; do
  if awk -F'\t' -v n="$name" -v v="$ver" '$1 == n && $2 == v { found = 1 } END { exit !found }' "$index"; then
    continue
  fi
  [[ -n "$repo" ]] || repo="$homepage"
  repo="${repo/#http:/https:}"
  [[ ! "$repo" =~ ^(https://github\.com/[^/]+/[^/]+)/tree/ ]] || repo="${BASH_REMATCH[1]}"
  repo="${repo%/}"
  repo="${repo%.git}"
  if [[ ! "$repo" =~ ^https://(github\.com|gitlab\.redox-os\.org)/([A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+)$ ]]; then
    echo "gap: $name $ver: unsupported or missing repository '${repo:-none}'"
    gaps=$((gaps + 1))
    continue
  fi
  host="${BASH_REMATCH[1]}" project="${BASH_REMATCH[2]}"
  commit="" sub="" how="published-commit"
  if [[ -f "$dir/.cargo_vcs_info.json" ]]; then
    commit="$(jq -r '.git.sha1 // empty' "$dir/.cargo_vcs_info.json")"
    sub="$(jq -r '.path_in_vcs // empty' "$dir/.cargo_vcs_info.json")"
  fi
  if [[ -z "$commit" && "$host" == github.com ]]; then
    if commit="$(resolve_tag "$project" "$name" "$ver")"; then
      how="version-tag"
    elif commit="$(github_sha "$project" HEAD)"; then
      how="default-branch"
    fi
  fi
  if [[ ! "$commit" =~ ^[0-9a-f]{40}$ ]]; then
    echo "gap: $name $ver: no published commit or version tag in $repo"
    gaps=$((gaps + 1))
    continue
  fi
  # Walk up from the crate's directory. Stop at the first directory with a
  # license proper (LICENSE, COPYING, UNLICENSE); NOTICE files seen on the way,
  # including there, are kept as supplements.
  paths="" notices="" listed=1 licensed=0
  d="$sub"
  while :; do
    names="$(list "$host" "$project" "$commit" "$d")" || {
      echo "gap: $name $ver: could not list $repo at $commit (commit missing upstream, or a network error)"
      listed=0
      break
    }
    while IFS= read -r file; do
      [[ -n "$file" ]] || continue
      if grep -qiE '^NOTICE' <<<"$file"; then
        notices+="${d:+$d/}$file"$'\n'
      else
        paths+="${d:+$d/}$file"$'\n'
        licensed=1
      fi
    done <<<"$names"
    ((licensed == 0)) || break
    [[ -n "$d" ]] || break
    if [[ "$d" == */* ]]; then d="${d%/*}"; else d=""; fi
  done
  if ((licensed == 0)); then
    if ((listed == 1)); then
      echo "gap: $name $ver: no LICENSE, COPYING or UNLICENSE file in $repo at $commit${notices:+ (NOTICE files ignored: ${notices//$'\n'/ })}"
    fi
    gaps=$((gaps + 1))
    continue
  fi
  # All of a crate's files or none: a crate with a failed download stays out
  # of the index, so the next run retries it instead of skipping it.
  : >"$work/rows"
  complete=1
  while IFS= read -r path; do
    [[ -n "$path" ]] || continue
    if ! fetch "$host" "$project" "$commit" "$path" "$work/f" 2>/dev/null || [[ ! -s "$work/f" ]]; then
      echo "gap: $name $ver: could not download $path from $repo at $commit; rerun to retry"
      complete=0
      break
    fi
    sum="$(sha256 "$work/f" | awk '{ print $1 }')"
    cp "$work/f" "$work/store-$sum"
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$name" "$ver" "$repo" "$commit" "$path" "$sum" "$how" >>"$work/rows"
  done <<<"$paths$notices"
  if ((complete == 0)); then
    rm -f "$work"/store-*
    gaps=$((gaps + 1))
    continue
  fi
  for f in "$work"/store-*; do mv "$f" "$store/${f##*/store-}"; done
  cat "$work/rows" >>"$index"
  while IFS=$'\t' read -r _ _ _ _ path _ _; do
    echo "vendored: $name $ver $path ($how ${commit:0:12})"
  done <"$work/rows"
done <"$work/missing.tsv"

{
  head -n 1 "$index"
  tail -n +2 "$index" | LC_ALL=C sort -t$'\t' -k1,1 -k2,2 -k5,5
} >"$work/index" && mv "$work/index" "$index"
echo "vendor-licenses.sh: $gaps crate(s) without an upstream license file"
