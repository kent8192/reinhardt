#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 1 ]]; then
	echo "Usage: $0 <lcov-file>" >&2
	exit 2
fi

LCOV_FILE="$1"
if [[ ! -s "$LCOV_FILE" ]]; then
	echo "LCOV report is missing or empty: $LCOV_FILE" >&2
	exit 1
fi

awk '
BEGIN {
	in_production_source = 0
	production_files = 0
	hit_lines = 0
}
/^SF:/ {
	path = substr($0, 4)
	in_production_source = path ~ /(^|\/)crates\/[^\/]+\/src\//
	if (in_production_source) {
		production_files++
	}
	next
}
in_production_source && /^DA:/ {
	split(substr($0, 4), fields, ",")
	if ((fields[2] + 0) > 0) {
		hit_lines++
	}
}
END {
	if (production_files == 0) {
		print "LCOV report contains no workspace production source files" > "/dev/stderr"
		exit 1
	}
	if (hit_lines == 0) {
		print "LCOV report contains no executed workspace production lines" > "/dev/stderr"
		exit 1
	}
	printf "LCOV production hits: files=%d hit_lines=%d\n", production_files, hit_lines
}
' "$LCOV_FILE"
