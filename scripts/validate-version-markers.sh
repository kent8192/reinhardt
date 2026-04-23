#!/usr/bin/env bash
# scripts/validate-version-markers.sh
# Lint for the reinhardt-version-sync marker convention.
#
# Reports:
#   ORPHAN_MARKER   - marker found with no version on next non-blank,
#                     non-fence line.
#   UNMARKED        - Reinhardt-looking hardcoded version with no marker
#                     directly above it.
#
# Exit 0 on clean, 1 on any finding.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

DEFAULT_TARGETS=(
	"README.md"
	"examples/Cargo.toml"
	"examples/CLAUDE.md"
	"website/config.toml"
)

if [ -n "${REINHARDT_VERSION_SYNC_TARGETS:-}" ]; then
	read -r -a TARGETS <<< "$REINHARDT_VERSION_SYNC_TARGETS"
else
	TARGETS=("${DEFAULT_TARGETS[@]}")
fi

AWK_PROG='
BEGIN {
	marker_re  = "^[[:space:]]*(#|//)[[:space:]]*reinhardt-version-sync[[:space:]]*$"
	marker_re2 = "^[[:space:]]*<!--[[:space:]]*reinhardt-version-sync[[:space:]]*-->[[:space:]]*$"
	# Hints that a line carries a Reinhardt version we should have marked.
	hint_re    = "(reinhardt[a-z-]*[[:space:]]*=|reinhardt_version[[:space:]]*=|package[[:space:]]*=[[:space:]]*\"reinhardt-web\")"
	version_re = "[0-9]+\\.[0-9]+\\.[0-9]+(-[a-zA-Z0-9.]+)?"
	fence_re   = "^[[:space:]]*```"
	blank_re   = "^[[:space:]]*$"
	state = "SCANNING"
	findings = 0
	marker_line = 0
}
{
	if (state == "SCANNING") {
		if ($0 ~ marker_re || $0 ~ marker_re2) {
			state = "ARMED"
			marker_line = NR
			next
		}
		# Unmarked hardcoded version detection.
		if ($0 ~ hint_re && match($0, version_re)) {
			printf("UNMARKED %s:%d: no preceding marker: %s\n", FILENAME, NR, $0) > "/dev/stderr"
			findings++
		}
		next
	}
	# ARMED
	if ($0 ~ fence_re || $0 ~ blank_re) next
	if (match($0, version_re)) {
		state = "SCANNING"
		next
	}
	printf("ORPHAN_MARKER %s:%d: no version follows marker\n", FILENAME, marker_line) > "/dev/stderr"
	findings++
	state = "SCANNING"
}
END { if (findings > 0) exit 1 }
'

FAIL=0
for rel in "${TARGETS[@]}"; do
	path="$REPO_ROOT/$rel"
	if [ ! -f "$path" ]; then
		continue
	fi
	if ! awk "$AWK_PROG" "$path"; then
		FAIL=1
	fi
done

if [ "$FAIL" -eq 0 ]; then
	echo "version-markers: OK (${#TARGETS[@]} file(s) scanned)"
fi
exit "$FAIL"
