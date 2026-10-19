#!/usr/bin/env bash
# Runs the release-tooling checks: syntax, shellcheck and the script tests.
# Offline and side-effect free: everything runs in temporary directories.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
scripts=(scripts/*.sh scripts/tests/*.sh)

for f in "${scripts[@]}"; do
  bash -n "$f"
  if [[ -x /bin/bash ]]; then /bin/bash -n "$f"; fi
done
echo "syntax ok"

if command -v shellcheck >/dev/null; then
  shellcheck "${scripts[@]}"
  echo "shellcheck ok"
else
  echo "shellcheck not installed; skipped" >&2
  [[ -z "${CI:-}" ]] || exit 1
fi

scripts/tests/version-test.sh | grep -v '^ok '
scripts/tests/install-test.sh | grep -v '^ok '
scripts/tests/third-party-test.sh | grep -v '^ok '
scripts/tests/linkage-test.sh | grep -v '^ok '
scripts/tests/vendor-licenses-test.sh | grep -v '^ok '
