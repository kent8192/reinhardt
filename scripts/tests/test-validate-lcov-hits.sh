#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
VALIDATOR="$SCRIPT_DIR/validate-lcov-hits.sh"
FIXTURE_DIR=$(mktemp -d)
trap 'rm -rf "$FIXTURE_DIR"' EXIT

pass() { echo "  PASS: $1"; }
fail() { echo "  FAIL: $1" >&2; exit 1; }

expect_failure() {
	local name="$1"
	local expected="$2"
	shift 2
	set +e
	"$@" >"$FIXTURE_DIR/out.log" 2>"$FIXTURE_DIR/err.log"
	local status=$?
	set -e
	if [[ "$status" -eq 0 ]]; then
		fail "$name unexpectedly succeeded"
	fi
	if ! grep -Fq "$expected" "$FIXTURE_DIR/err.log"; then
		cat "$FIXTURE_DIR/err.log" >&2
		fail "$name did not report: $expected"
	fi
	pass "$name"
}

set +e
"$VALIDATOR" >"$FIXTURE_DIR/out.log" 2>"$FIXTURE_DIR/err.log"
USAGE_STATUS=$?
set -e
[[ "$USAGE_STATUS" -eq 2 ]] \
	|| fail "invalid usage returned $USAGE_STATUS instead of 2"
grep -Fq "Usage:" "$FIXTURE_DIR/err.log" \
	|| fail "invalid usage did not print usage information"
pass "invalid usage"

expect_failure \
	"missing report" \
	"LCOV report is missing or empty" \
	"$VALIDATOR" "$FIXTURE_DIR/missing.lcov"

: >"$FIXTURE_DIR/empty.lcov"
expect_failure \
	"empty report" \
	"LCOV report is missing or empty" \
	"$VALIDATOR" "$FIXTURE_DIR/empty.lcov"

cat >"$FIXTURE_DIR/tests-only.lcov" <<'LCOV'
SF:/workspace/crates/reinhardt-example/tests/integration.rs
DA:4,1
end_of_record
LCOV
expect_failure \
	"no production source" \
	"LCOV report contains no workspace production source files" \
	"$VALIDATOR" "$FIXTURE_DIR/tests-only.lcov"

cat >"$FIXTURE_DIR/all-zero.lcov" <<'LCOV'
SF:/workspace/crates/reinhardt-example/src/lib.rs
DA:4,0
DA:5,0
end_of_record
LCOV
expect_failure \
	"all-zero production source" \
	"LCOV report contains no executed workspace production lines" \
	"$VALIDATOR" "$FIXTURE_DIR/all-zero.lcov"

cat >"$FIXTURE_DIR/hits.lcov" <<'LCOV'
SF:crates/reinhardt-example/src/lib.rs
DA:4,0
DA:5,3
end_of_record
LCOV
OUTPUT=$("$VALIDATOR" "$FIXTURE_DIR/hits.lcov")
[[ "$OUTPUT" == "LCOV production hits: files=1 hit_lines=1" ]] \
	|| fail "positive report returned unexpected output: $OUTPUT"
pass "positive production hits"
