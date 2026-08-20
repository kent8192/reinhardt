#!/usr/bin/env bash
# scripts/tests/test-validate-version-markers.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SCRIPT="$SCRIPT_DIR/validate-version-markers.sh"

FAIL=0
pass() { echo "  PASS: $1"; }
fail() { echo "  FAIL: $1" >&2; FAIL=1; }

run_validate_case() {
	local name="$1"
	local input_file="$2"
	local target_rel="$3"
	local expected_rc="$4"
	local expected_stderr_substr="$5"

	local tmpdir
	tmpdir=$(mktemp -d)
	mkdir -p "$tmpdir/scripts" "$(dirname "$tmpdir/$target_rel")"
	cp "$SCRIPT" "$tmpdir/scripts/validate-version-markers.sh"
	cp "$input_file" "$tmpdir/$target_rel"

	set +e
	REINHARDT_VERSION_SYNC_TARGETS="$target_rel" \
		bash "$tmpdir/scripts/validate-version-markers.sh" \
		>"$tmpdir/out.log" 2>"$tmpdir/err.log"
	rc=$?
	set -e

	if [ "$rc" -ne "$expected_rc" ]; then
		fail "$name (rc=$rc, expected $expected_rc)"
		cat "$tmpdir/err.log" >&2
		rm -rf "$tmpdir"
		return
	fi

	if [ -n "$expected_stderr_substr" ]; then
		if grep -q "$expected_stderr_substr" "$tmpdir/err.log"; then
			pass "$name"
		else
			fail "$name (stderr missing: $expected_stderr_substr)"
			cat "$tmpdir/err.log" >&2
		fi
	else
		pass "$name"
	fi
	rm -rf "$tmpdir"
}

fx_dir=$(mktemp -d)
trap 'rm -rf "$fx_dir"' EXIT

# Clean: marker directly above version -> rc 0
cat > "$fx_dir/clean.toml" <<'EOF'
# reinhardt-version-sync
reinhardt = { version = "0.1.0-rc.17", package = "reinhardt-web" }
EOF
run_validate_case "V1 clean" "$fx_dir/clean.toml" "ok.toml" 0 ""

# Orphan: marker with no version on next non-blank line -> rc 1
cat > "$fx_dir/orphan.md" <<'EOF'
<!-- reinhardt-version-sync -->
Just prose, no version here.
EOF
run_validate_case "V2 orphan marker" "$fx_dir/orphan.md" "orphan.md" 1 "ORPHAN_MARKER"

# Unmarked: Reinhardt version present but no preceding marker -> rc 1
cat > "$fx_dir/unmarked.toml" <<'EOF'
[dependencies]
reinhardt = { version = "0.1.0-rc.17", package = "reinhardt-web" }
EOF
run_validate_case "V3 unmarked hardcoded version" "$fx_dir/unmarked.toml" "bad.toml" 1 "UNMARKED"

# Unmarked security policy release reference -> rc 1, UNMARKED
cat > "$fx_dir/unmarked-security.md" <<'EOF'
Security fixes target the current supported release, `0.3.8`, and the current development line as appropriate.
EOF
run_validate_case "V3b unmarked security release" "$fx_dir/unmarked-security.md" "SECURITY.md" 1 "UNMARKED"

# V4: marker inside a Markdown fenced code block -> rc 1, MARKER_IN_CODE_BLOCK
cat > "$fx_dir/v4-marker-in-block.md" <<'MD_EOF'
# Example

```toml
# reinhardt-version-sync
reinhardt = { version = "0.1.0-rc.17", package = "reinhardt-web" }
```
MD_EOF
run_validate_case "V4 marker in code block" "$fx_dir/v4-marker-in-block.md" "bad.md" 1 "MARKER_IN_CODE_BLOCK"

# Default targets include the root security policy.
run_default_security_policy_case() {
	local tmpdir
	tmpdir=$(mktemp -d)
	mkdir -p "$tmpdir/scripts" "$tmpdir/crates" "$tmpdir/docs" "$tmpdir/website/content"
	cp "$SCRIPT" "$tmpdir/scripts/validate-version-markers.sh"
	cat > "$tmpdir/SECURITY.md" <<'EOF'
# Security Policy

<!-- reinhardt-version-sync -->
Security fixes are handled through the security advisory process.
EOF

	set +e
	REINHARDT_REPO_ROOT="$tmpdir" \
		bash "$tmpdir/scripts/validate-version-markers.sh" \
		>"$tmpdir/out.log" 2>"$tmpdir/err.log"
	local rc=$?
	set -e
	if [ "$rc" -eq 1 ] && grep -q 'ORPHAN_MARKER .*SECURITY.md' "$tmpdir/err.log"; then
		pass "V5 default target root SECURITY.md"
	else
		fail "V5 default target root SECURITY.md (rc=$rc)"
		cat "$tmpdir/err.log" >&2
	fi
	rm -rf "$tmpdir"
}

run_default_security_policy_case

exit "$FAIL"
