#!/usr/bin/env bash
# scripts/tests/run-all.sh — entry point for all shell tests in this PR series.
# Exit 0 on success, non-zero on first failing test file.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
FAIL=0

for test in "$SCRIPT_DIR"/test-*.sh; do
	[ -f "$test" ] || continue
	echo "=== $(basename "$test") ==="
	if ! bash "$test"; then
		echo "FAIL: $(basename "$test")" >&2
		FAIL=1
	fi
done

if [ "$FAIL" -eq 0 ]; then
	echo "All script tests passed."
fi
exit "$FAIL"
