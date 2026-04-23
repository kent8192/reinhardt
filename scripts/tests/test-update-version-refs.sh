#!/usr/bin/env bash
# scripts/tests/test-update-version-refs.sh
# Self-contained tests for scripts/update-version-refs.sh.
# Each test creates fixtures in a tempdir, invokes the script with
# REPO_ROOT pointing at the tempdir, and asserts the result.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SCRIPT="$SCRIPT_DIR/update-version-refs.sh"

# --- Test harness ---

FAIL=0
pass() { echo "  PASS: $1"; }
fail() { echo "  FAIL: $1" >&2; FAIL=1; }

run_case() {
	local name="$1"
	local input_file="$2"
	local expected_file="$3"
	local new_ver="$4"
	local target_rel="$5"

	local tmpdir
	tmpdir=$(mktemp -d)
	trap 'rm -rf "$tmpdir"' RETURN

	# Lay out the tempdir as a fake REPO_ROOT
	mkdir -p "$tmpdir/scripts"
	cp "$SCRIPT" "$tmpdir/scripts/update-version-refs.sh"

	# Place the fixture at the target path declared by the test
	mkdir -p "$(dirname "$tmpdir/$target_rel")"
	cp "$input_file" "$tmpdir/$target_rel"

	# Override TARGET_FILES via env for testability (see Task 3 hook)
	REINHARDT_VERSION_SYNC_TARGETS="$target_rel" \
		bash "$tmpdir/scripts/update-version-refs.sh" "$new_ver" >/dev/null

	if diff -q "$expected_file" "$tmpdir/$target_rel" >/dev/null; then
		pass "$name"
	else
		fail "$name (diff below)"
		diff -u "$expected_file" "$tmpdir/$target_rel" || true
	fi
}

# --- Fixtures ---

fx_dir=$(mktemp -d)
trap 'rm -rf "$fx_dir"' EXIT

# Fixture 01: plain TOML, single marker, single version
cat > "$fx_dir/01-input.toml" <<'EOF'
[package]
name = "demo"

# reinhardt-version-sync
reinhardt = { version = "0.1.0-rc.17", package = "reinhardt-web" }
EOF

cat > "$fx_dir/01-expected.toml" <<'EOF'
[package]
name = "demo"

# reinhardt-version-sync
reinhardt = { version = "0.1.0-rc.99", package = "reinhardt-web" }
EOF

run_case "01 plain TOML single marker" \
	"$fx_dir/01-input.toml" \
	"$fx_dir/01-expected.toml" \
	"0.1.0-rc.99" \
	"demo.toml"

# Fixture 02: Markdown with toml fenced block, marker inside the block
cat > "$fx_dir/02-input.md" <<'MD_EOF'
# Demo

Install it:

```toml
# reinhardt-version-sync
reinhardt = { version = "0.1.0-rc.17", package = "reinhardt-web" }
```

Done.
MD_EOF

cat > "$fx_dir/02-expected.md" <<'MD_EOF'
# Demo

Install it:

```toml
# reinhardt-version-sync
reinhardt = { version = "0.1.0-rc.99", package = "reinhardt-web" }
```

Done.
MD_EOF

run_case "02 markdown toml block single marker" \
	"$fx_dir/02-input.md" \
	"$fx_dir/02-expected.md" \
	"0.1.0-rc.99" \
	"demo.md"

exit "$FAIL"
