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

if [ -n "${REINHARDT_VERSION_SYNC_TARGETS:-}" ]; then
	read -r -a TARGETS <<< "$REINHARDT_VERSION_SYNC_TARGETS"
else
	TARGETS=(
		"README.md"
		"examples/Cargo.toml"
		"examples/CLAUDE.md"
		"website/config.toml"
	)
	while IFS= read -r f; do
		TARGETS+=("$f")
	done < <(find "$REPO_ROOT/crates" -name "README.md" -maxdepth 2 | sort | \
	          sed "s|$REPO_ROOT/||")
	while IFS= read -r f; do
		TARGETS+=("$f")
	done < <(find "$REPO_ROOT/docs" -name "*.md" | sort | \
	          sed "s|$REPO_ROOT/||")
fi

AWK_PROG='
BEGIN {
	marker_re      = "^[[:space:]]*(#|//)[[:space:]]*reinhardt-version-sync[[:space:]]*$"
	marker_html_re = "^[[:space:]]*<!--[[:space:]]*reinhardt-version-sync[[:space:]]*-->[[:space:]]*$"
	marker_html_n  = "^[[:space:]]*<!--[[:space:]]*reinhardt-version-sync:[0-9]+[[:space:]]*-->[[:space:]]*$"
	# Hints that a line carries a Reinhardt version we should have marked.
	hint_re    = "(reinhardt[a-z-]*[[:space:]]*=|reinhardt_version[[:space:]]*=|package[[:space:]]*=[[:space:]]*\"reinhardt-web\")"
	version_re = "[0-9]+\\.[0-9]+\\.[0-9]+(-[a-zA-Z0-9.]+)?"
	fence_re   = "^[[:space:]]*```"
	blank_re   = "^[[:space:]]*$"
	state       = "SCANNING"
	armed_count = 0
	findings    = 0
	marker_line = 0
	in_code_block = 0
	is_md_file    = 0
}
FNR == 1 {
	# Detect if file is a Markdown file by checking FILENAME suffix
	is_md_file = (FILENAME ~ /\.md$/)
	in_code_block = 0
}
{
	# Track fenced code block state for Markdown files
	if (is_md_file && $0 ~ fence_re) {
		in_code_block = !in_code_block
	}

	if (state == "SCANNING") {
		# In Markdown, # or // markers inside a code block are migration errors
		if (is_md_file && in_code_block && $0 ~ marker_re) {
			printf("MARKER_IN_CODE_BLOCK %s:%d: use '\''<!-- reinhardt-version-sync[:N] -->'\'' outside the code block instead: %s\n", FILENAME, NR, $0) > "/dev/stderr"
			findings++
			next
		}
		if ($0 ~ marker_re || $0 ~ marker_html_re) {
			armed_count = 1
			state = "ARMED"
			marker_line = NR
			next
		}
		if ($0 ~ marker_html_n) {
			tmp = $0
			if (match(tmp, ":[0-9]+")) {
				armed_count = int(substr(tmp, RSTART + 1, RLENGTH - 1))
			} else {
				armed_count = 1
			}
			if (armed_count < 1) armed_count = 1
			state = "ARMED"
			marker_line = NR
			next
		}
		# Unmarked hardcoded version detection (skip inside md code blocks).
		if (!(is_md_file && in_code_block) && $0 ~ hint_re && match($0, version_re)) {
			printf("UNMARKED %s:%d: no preceding marker: %s\n", FILENAME, NR, $0) > "/dev/stderr"
			findings++
		}
		next
	}
	# ARMED: skip fences, blanks, and non-version lines that precede the version
	# (handles both legacy in-fence markers and new outside-fence markers where
	# the opening ```, comment-only lines, and section headers appear first)
	if ($0 ~ fence_re || $0 ~ blank_re) next
	if (match($0, version_re)) {
		armed_count--
		if (armed_count <= 0) state = "SCANNING"
		next
	}
	# Non-version, non-fence, non-blank line while ARMED.
	# If we are inside a code block, continue scanning (the version may be on a later line).
	if (in_code_block) next
	# Outside a code block and no version found: orphan marker.
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
