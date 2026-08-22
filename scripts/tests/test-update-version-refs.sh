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

# Fixture 03: Markdown with toml block containing multiple versions
cat > "$fx_dir/03-input.md" <<'MD_EOF'
```toml
# Core components
# reinhardt-version-sync
reinhardt-http = "0.1.0-rc.17"
# reinhardt-version-sync
reinhardt-urls = "0.1.0-rc.17"

# Optional: Database
# reinhardt-version-sync
reinhardt-db = "0.1.0-rc.17"
```
MD_EOF

cat > "$fx_dir/03-expected.md" <<'MD_EOF'
```toml
# Core components
# reinhardt-version-sync
reinhardt-http = "0.1.0-rc.99"
# reinhardt-version-sync
reinhardt-urls = "0.1.0-rc.99"

# Optional: Database
# reinhardt-version-sync
reinhardt-db = "0.1.0-rc.99"
```
MD_EOF

run_case "03 multi-version block" \
	"$fx_dir/03-input.md" \
	"$fx_dir/03-expected.md" \
	"0.1.0-rc.99" \
	"multi.md"

# Fixture 04: Markdown marker outside a bash block updates comment and command
cat > "$fx_dir/04-input.md" <<'MD_EOF'
<!-- reinhardt-version-sync:2 -->
```bash
# Pre-release: --version is required. Once 0.1.0-rc.17 stable ships, --version
# becomes optional. The literal below is auto-bumped by release-plz.
cargo install reinhardt-admin-cli --version "0.1.0-rc.17"
```
MD_EOF

cat > "$fx_dir/04-expected.md" <<'MD_EOF'
<!-- reinhardt-version-sync:2 -->
```bash
# Pre-release: --version is required. Once 0.1.0-rc.99 stable ships, --version
# becomes optional. The literal below is auto-bumped by release-plz.
cargo install reinhardt-admin-cli --version "0.1.0-rc.99"
```
MD_EOF

run_case "04 markdown bash block counted marker" \
	"$fx_dir/04-input.md" \
	"$fx_dir/04-expected.md" \
	"0.1.0-rc.99" \
	"install.md"

# Orphan marker case (expects non-zero exit)

run_orphan_case() {
	local name="$1"
	local input_file="$2"
	local target_rel="$3"

	local tmpdir
	tmpdir=$(mktemp -d)
	mkdir -p "$tmpdir/scripts" "$(dirname "$tmpdir/$target_rel")"
	cp "$SCRIPT" "$tmpdir/scripts/update-version-refs.sh"
	cp "$input_file" "$tmpdir/$target_rel"

	set +e
	REINHARDT_VERSION_SYNC_TARGETS="$target_rel" \
		bash "$tmpdir/scripts/update-version-refs.sh" "0.1.0-rc.99" \
		>/dev/null 2>"$tmpdir/err.log"
	rc=$?
	set -e

	if [ "$rc" -eq 2 ] && grep -q "ORPHAN_MARKER" "$tmpdir/err.log"; then
		pass "$name"
	else
		fail "$name (rc=$rc, stderr below)"
		cat "$tmpdir/err.log" >&2
	fi
	rm -rf "$tmpdir"
}

# Fixture 05: marker with no following version
cat > "$fx_dir/05-input.md" <<'MD_EOF'
<!-- reinhardt-version-sync -->
This paragraph has no version on the next line.
MD_EOF

run_orphan_case "05 orphan marker" "$fx_dir/05-input.md" "bad.md"

# Fixture 06: counter marker N=3 — HTML comment arms three replacements
cat > "$fx_dir/06-input.md" <<'MD_EOF'
<!-- reinhardt-version-sync:3 -->
```toml
reinhardt-http = "0.1.0-rc.17"
reinhardt-urls = "0.1.0-rc.17"
reinhardt-db   = "0.1.0-rc.17"
```
MD_EOF

cat > "$fx_dir/06-expected.md" <<'MD_EOF'
<!-- reinhardt-version-sync:3 -->
```toml
reinhardt-http = "0.1.0-rc.99"
reinhardt-urls = "0.1.0-rc.99"
reinhardt-db   = "0.1.0-rc.99"
```
MD_EOF

run_case "06 counter marker N=3" \
	"$fx_dir/06-input.md" \
	"$fx_dir/06-expected.md" \
	"0.1.0-rc.99" \
	"counter3.md"

# Fixture 07: counter marker default (N=1, no colon) — equivalent to original HTML marker
cat > "$fx_dir/07-input.md" <<'MD_EOF'
<!-- reinhardt-version-sync -->
```toml
reinhardt = { version = "0.1.0-rc.17", package = "reinhardt-web" }
```
MD_EOF

cat > "$fx_dir/07-expected.md" <<'MD_EOF'
<!-- reinhardt-version-sync -->
```toml
reinhardt = { version = "0.1.0-rc.99", package = "reinhardt-web" }
```
MD_EOF

run_case "07 counter marker default N=1" \
	"$fx_dir/07-input.md" \
	"$fx_dir/07-expected.md" \
	"0.1.0-rc.99" \
	"counter1.md"

# Fixture 08: docs.rs URL keeps the package path and rewrites only the version
cat > "$fx_dir/08-input.toml" <<'EOF'
[extra]
# reinhardt-version-sync
docs_rs = "https://docs.rs/reinhardt-web/0.2.0-rc.4/reinhardt/"
EOF

cat > "$fx_dir/08-expected.toml" <<'EOF'
[extra]
# reinhardt-version-sync
docs_rs = "https://docs.rs/reinhardt-web/0.2.0-rc.5/reinhardt/"
EOF

run_case "08 docs.rs versioned URL" \
	"$fx_dir/08-input.toml" \
	"$fx_dir/08-expected.toml" \
	"0.2.0-rc.5" \
	"website/config.toml"

# Fixture 09: the default target set keeps the integration test package aligned
run_default_integration_manifest_case() {
	local name="$1"
	local tmpdir
	tmpdir=$(mktemp -d)
	mkdir -p "$tmpdir/scripts" "$tmpdir/tests/integration"
	cp "$SCRIPT" "$tmpdir/scripts/update-version-refs.sh"
	cat > "$tmpdir/tests/integration/Cargo.toml" <<'EOF'
[package]
name = "reinhardt-integration-tests"
# reinhardt-version-sync
version = "0.2.0-rc.2"
EOF

	REINHARDT_REPO_ROOT="$tmpdir" \
		bash "$tmpdir/scripts/update-version-refs.sh" "0.4.0-alpha.4" >/dev/null 2>&1

	if grep -q '^version = "0.4.0-alpha.4"$' "$tmpdir/tests/integration/Cargo.toml"; then
		pass "$name"
	else
		fail "$name"
		cat "$tmpdir/tests/integration/Cargo.toml" >&2
	fi
	rm -rf "$tmpdir"
}

run_default_integration_manifest_case "09 default integration manifest target"

# Fixture 10: the default target set includes the root security policy
run_default_security_policy_case() {
	local tmpdir
	tmpdir=$(mktemp -d)
	mkdir -p "$tmpdir/scripts" "$tmpdir/crates" "$tmpdir/docs" "$tmpdir/website/content"
	cp "$SCRIPT" "$tmpdir/scripts/update-version-refs.sh"
	cat > "$tmpdir/SECURITY.md" <<'EOF'
# Security Policy

<!-- reinhardt-version-sync -->
Security fixes target the current supported release, `0.3.8`.
EOF

	REINHARDT_REPO_ROOT="$tmpdir" \
		bash "$tmpdir/scripts/update-version-refs.sh" "9.8.7" >/dev/null
	if grep -q 'current supported release, `9.8.7`' "$tmpdir/SECURITY.md"; then
		pass "10 default target root SECURITY.md"
	else
		fail "10 default target root SECURITY.md"
	fi
	rm -rf "$tmpdir"
}

run_default_security_policy_case

exit "$FAIL"
