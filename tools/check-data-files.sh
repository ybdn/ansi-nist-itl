#!/usr/bin/env bash
# DATA-04: fails if the repository tracks a NIST file (.nist, .eft, .an2,
# .int, any case) that tests/data/ALLOWLIST does not list.
#
# Usage (from the repository root): tools/check-data-files.sh [--staged]
#
# With --staged, checks the files staged for commit instead, for a
# pre-commit hook:
#
#     printf '#!/bin/sh\nexec tools/check-data-files.sh --staged\n' > .git/hooks/pre-commit
#     chmod +x .git/hooks/pre-commit
set -euo pipefail

allowlist=tests/data/ALLOWLIST
if [[ "${1:-}" == "--staged" ]]; then
    files=$(git diff --cached --name-only --diff-filter=ACMR)
else
    files=$(git ls-files)
fi

allowed=$(sed -e 's/#.*//' -e 's/[[:space:]]*$//' -e '/^$/d' "$allowlist" 2>/dev/null || true)
status=0
while IFS= read -r file; do
    [[ -z "$file" ]] && continue
    lower=$(printf '%s' "$file" | tr '[:upper:]' '[:lower:]')
    case "$lower" in
        *.nist | *.eft | *.an2 | *.int) ;;
        *) continue ;;
    esac
    if ! grep -Fxq -- "$file" <<<"$allowed"; then
        echo "error: $file is not listed in $allowlist (DATA-04): real NIST files must never be committed" >&2
        status=1
    fi
done <<<"$files"
exit "$status"
