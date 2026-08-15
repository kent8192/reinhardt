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

expect_status_and_error() {
	local name="$1"
	local expected_status="$2"
	local expected="$3"
	shift 3
	set +e
	"$@" >"$FIXTURE_DIR/out.log" 2>"$FIXTURE_DIR/err.log"
	local status=$?
	set -e
	[[ "$status" -eq "$expected_status" ]] \
		|| fail "$name returned $status instead of $expected_status"
	grep -Fq "$expected" "$FIXTURE_DIR/err.log" \
		|| fail "$name did not report: $expected"
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

cat >"$FIXTURE_DIR/unit.lcov" <<'LCOV'
SF:crates/reinhardt-example/src/lib.rs
DA:4,1
DA:5,0
end_of_record
LCOV

cat >"$FIXTURE_DIR/integration.lcov" <<'LCOV'
SF:crates/reinhardt-example/src/lib.rs
DA:4,0
DA:5,1
end_of_record
LCOV

cat >"$FIXTURE_DIR/ignored.lcov" <<'LCOV'
SF:crates/reinhardt-test/src/lib.rs
DA:4,0
end_of_record
LCOV

cat >"$FIXTURE_DIR/other.lcov" <<'LCOV'
SF:crates/reinhardt-other/src/lib.rs
DA:4,0
end_of_record
LCOV

expect_status_and_error \
	"complete union requires all tracked lines" \
	1 \
	"crates/reinhardt-example/src/lib.rs:5" \
	"$VALIDATOR" --require-complete "$FIXTURE_DIR/unit.lcov"

OUTPUT=$("$VALIDATOR" --require-complete \
	"$FIXTURE_DIR/unit.lcov" "$FIXTURE_DIR/integration.lcov" "$FIXTURE_DIR/ignored.lcov")
[[ "$OUTPUT" == "LCOV complete: files=1 tracked_lines=2 hit_lines=2 misses=0" ]] \
	|| fail "complete union returned unexpected output: $OUTPUT"
pass "complete union combines reports and ignores configured paths"

OUTPUT=$("$VALIDATOR" --require-complete --path crates/reinhardt-example/src \
	"$FIXTURE_DIR/unit.lcov" "$FIXTURE_DIR/integration.lcov" "$FIXTURE_DIR/other.lcov")
[[ "$OUTPUT" == "LCOV complete: files=1 tracked_lines=2 hit_lines=2 misses=0" ]] \
	|| fail "complete path union returned unexpected output: $OUTPUT"
pass "complete union filters paths"

expect_status_and_error \
	"unknown option" \
	2 \
	"Usage:" \
	"$VALIDATOR" --unknown "$FIXTURE_DIR/hits.lcov"

expect_status_and_error \
	"missing path value" \
	2 \
	"Usage:" \
	"$VALIDATOR" --path
