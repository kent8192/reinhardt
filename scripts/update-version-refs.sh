#!/usr/bin/env bash
# Update version references in documentation and non-release-plz-managed
# manifests after a version bump. Replacement is guided by explicit
# '<lang-comment> reinhardt-version-sync' markers on the line immediately
# above each target (see docs/superpowers/specs/2026-04-23-release-plz-
# version-refs-sync-design.md).
#
# Usage: ./scripts/update-version-refs.sh <new-version> [--dry-run]
# Example: ./scripts/update-version-refs.sh 0.1.0-rc.20

set -euo pipefail

# --- Argument parsing ---

DRY_RUN=false
NEW_VER=""

for arg in "$@"; do
	case "$arg" in
		--dry-run) DRY_RUN=true ;;
		-*)
			echo "Error: Unknown option '$arg'" >&2
			echo "Usage: $0 <new-version> [--dry-run]" >&2
			exit 1
			;;
		*)
			if [ -z "$NEW_VER" ]; then
				NEW_VER="$arg"
			else
				echo "Error: Unexpected argument '$arg'" >&2
				exit 1
			fi
			;;
	esac
done

if [ -z "$NEW_VER" ]; then
	echo "Usage: $0 <new-version> [--dry-run]" >&2
	exit 1
fi

# Validate version format: X.Y.Z or X.Y.Z-prerelease
if ! echo "$NEW_VER" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$'; then
	echo "Error: Invalid version format '$NEW_VER'" >&2
	echo "Expected: X.Y.Z or X.Y.Z-prerelease" >&2
	exit 1
fi

# --- Resolve paths and target list ---

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Default in-scope files for PR-1. PR-2 will extend this list via a
# follow-up commit. Tests override via the env var below.
DEFAULT_TARGETS=(
	"README.md"
	"examples/Cargo.toml"
	"examples/CLAUDE.md"
	"website/config.toml"
)

if [ -n "${REINHARDT_VERSION_SYNC_TARGETS:-}" ]; then
	# Space-separated override (for tests only)
	read -r -a TARGETS <<< "$REINHARDT_VERSION_SYNC_TARGETS"
else
	TARGETS=("${DEFAULT_TARGETS[@]}")
fi

echo "Updating version references to $NEW_VER in $REPO_ROOT"

# --- Awk program: marker-based replacer ---
#
# States:
#   SCANNING: looking for a marker line
#   ARMED:    marker seen, next non-blank non-fence line must carry a version
#
# Exit codes:
#   0 = all files processed without orphan markers
#   2 = at least one orphan marker encountered

AWK_PROG='
BEGIN {
	marker_re      = "^[[:space:]]*(#|//)[[:space:]]*reinhardt-version-sync[[:space:]]*$"
	marker_html_re = "^[[:space:]]*<!--[[:space:]]*reinhardt-version-sync[[:space:]]*-->[[:space:]]*$"
	marker_html_n  = "^[[:space:]]*<!--[[:space:]]*reinhardt-version-sync:[0-9]+[[:space:]]*-->[[:space:]]*$"
	version_re     = "[0-9]+\\.[0-9]+\\.[0-9]+(-[a-zA-Z0-9.]+)?"
	fence_re       = "^[[:space:]]*```"
	blank_re       = "^[[:space:]]*$"
	state        = "SCANNING"
	armed_count  = 0
	in_code_block = 0
	orphans      = 0
}
{
	# Track fenced code block state (used in ARMED to skip non-version block content)
	if ($0 ~ fence_re) in_code_block = !in_code_block

	if (state == "SCANNING") {
		print
		if ($0 ~ marker_re || $0 ~ marker_html_re) {
			armed_count = 1
			state = "ARMED"
		} else if ($0 ~ marker_html_n) {
			# Extract the integer N from <!-- reinhardt-version-sync:N -->
			tmp = $0
			if (match(tmp, ":[0-9]+")) {
				armed_count = int(substr(tmp, RSTART + 1, RLENGTH - 1))
			} else {
				armed_count = 1
			}
			if (armed_count < 1) armed_count = 1
			state = "ARMED"
		}
		next
	}
	# ARMED: pass fence and blank lines through without consuming count
	if ($0 ~ fence_re || $0 ~ blank_re) { print; next }
	if (match($0, version_re)) {
		prefix = substr($0, 1, RSTART - 1)
		suffix = substr($0, RSTART + RLENGTH)
		print prefix new_ver suffix
		armed_count--
		if (armed_count <= 0) state = "SCANNING"
		next
	}
	# Non-version, non-fence, non-blank line while ARMED.
	# If inside a code block, pass through and keep scanning (more lines to come).
	if (in_code_block) { print; next }
	# Outside any code block and no version found: report orphan.
	printf("ORPHAN_MARKER %s:%d: next line has no version: %s\n", FILENAME, NR, $0) > "/dev/stderr"
	print
	orphans++
	state = "SCANNING"
}
END { if (orphans > 0) exit 2 }
'

# --- Apply to each target ---

CHANGED_FILES=()
ORPHAN_FAIL=0

for rel in "${TARGETS[@]}"; do
	path="$REPO_ROOT/$rel"
	if [ ! -f "$path" ]; then
		echo "Warning: target not found: $rel" >&2
		continue
	fi
	tmp=$(mktemp)
	set +e
	awk -v new_ver="$NEW_VER" "$AWK_PROG" "$path" > "$tmp"
	rc=$?
	set -e

	if [ "$rc" -eq 2 ]; then
		ORPHAN_FAIL=1
	fi

	if ! diff -q "$path" "$tmp" >/dev/null 2>&1; then
		if $DRY_RUN; then
			diff -u "$path" "$tmp" || true
		else
			cp "$tmp" "$path"
		fi
		CHANGED_FILES+=("$rel")
	fi
	rm -f "$tmp"
done

# --- Summary ---

if [ "${#CHANGED_FILES[@]}" -eq 0 ]; then
	echo "No changes required."
else
	echo "Updated files:"
	for f in "${CHANGED_FILES[@]}"; do
		echo "  - $f"
	done
fi

if [ "$ORPHAN_FAIL" -ne 0 ]; then
	echo "Error: orphan markers detected (see stderr)." >&2
	exit 2
fi
