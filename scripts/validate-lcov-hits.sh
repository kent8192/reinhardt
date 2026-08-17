#!/usr/bin/env bash
set -euo pipefail

usage() {
	echo "Usage: $0 [--require-complete] [--path PREFIX] <lcov-file> [<lcov-file>...]" >&2
	exit 2
}

REQUIRE_COMPLETE=0
PATH_PREFIX=""
PATH_PREFIX_SET=0
LCOV_FILES=()

while [[ "$#" -gt 0 ]]; do
	case "$1" in
		--require-complete)
			REQUIRE_COMPLETE=1
			;;
		--path)
			[[ "$#" -gt 1 && "$2" != --* ]] || usage
			PATH_PREFIX="$2"
			PATH_PREFIX_SET=1
			shift
			;;
		--*)
			usage
			;;
		*)
			LCOV_FILES+=("$1")
			;;
	esac
	shift
done

[[ "${#LCOV_FILES[@]}" -gt 0 ]] || usage
if [[ "$REQUIRE_COMPLETE" -eq 0 \
	&& ( "$PATH_PREFIX_SET" -eq 1 || "${#LCOV_FILES[@]}" -ne 1 ) ]]; then
	usage
fi

for LCOV_FILE in "${LCOV_FILES[@]}"; do
	if [[ ! -s "$LCOV_FILE" ]]; then
		echo "LCOV report is missing or empty: $LCOV_FILE" >&2
		exit 1
	fi
done

if [[ "$REQUIRE_COMPLETE" -eq 0 ]]; then
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
' "${LCOV_FILES[0]}"
	exit 0
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

awk -v config="$REPO_ROOT/codecov.yml" -v repo_root="$REPO_ROOT" -v path_prefix="$PATH_PREFIX" '
function normalize(path) {
	sub("^" repo_root "/", "", path)
	if (path ~ /^\// && match(path, /\/(crates|src)\//)) {
		path = substr(path, RSTART + 1)
	}
	return path
}
function glob_regex(pattern, i, ch, regex) {
	regex = ""
	for (i = 1; i <= length(pattern); i++) {
		ch = substr(pattern, i, 1)
		if (ch == "*") {
			if (substr(pattern, i + 1, 1) == "*") {
				regex = regex ".*"
				i++
			} else {
				regex = regex "[^/]*"
			}
		} else if (ch == "?") {
			regex = regex "[^/]"
		} else if (index("\\.^$|()[]{}+", ch)) {
			regex = regex "\\" ch
		} else {
			regex = regex ch
		}
	}
	return "^" regex "$"
}
function ignored(path, i) {
	for (i = 1; i <= ignore_count; i++) {
		if (path ~ glob_regex(ignores[i])) {
			return 1
		}
	}
	return 0
}
function included(path) {
	return (path ~ /^src\// || path ~ /^crates\/[^\/]+\/src\//) && !ignored(path) && \
		(path_prefix == "" || path == path_prefix || index(path, path_prefix "/") == 1)
}
BEGIN {
	in_ignore = 0
	while ((getline config_line < config) > 0) {
		if (config_line ~ /^  ignore:[[:space:]]*$/) {
			in_ignore = 1
			continue
		}
		if (in_ignore && config_line ~ /^  [[:alnum:]_]+:/) {
			in_ignore = 0
		}
		if (in_ignore && config_line ~ /^    - /) {
			pattern = config_line
			sub(/^    -[[:space:]]*/, "", pattern)
			sub(/[[:space:]]*#.*/, "", pattern)
			first = substr(pattern, 1, 1)
			last = substr(pattern, length(pattern), 1)
			if ((first == "\"" || first == sprintf("%c", 39)) && first == last) {
				pattern = substr(pattern, 2, length(pattern) - 2)
			}
			ignores[++ignore_count] = pattern
		}
	}
	close(config)
}
/^SF:/ {
	current_path = normalize(substr($0, 4))
	current_included = included(current_path)
	if (current_included) {
		files[current_path] = 1
	}
	next
}
current_included && /^DA:/ {
	split(substr($0, 4), fields, ",")
	key = current_path SUBSEP fields[1]
	lines[key] = 1
	line_path[key] = current_path
	line_number[key] = fields[1]
	if ((fields[2] + 0) > 0) {
		hits[key] += fields[2] + 0
	}
}
/^end_of_record$/ {
	current_included = 0
}
END {
	for (path in files) {
		production_files++
	}
	if (production_files == 0) {
		print "LCOV report contains no workspace production source files" > "/dev/stderr"
		exit 1
	}
	for (key in lines) {
		tracked_lines++
		if (hits[key] > 0) {
			hit_lines++
		} else {
			misses++
			print line_path[key] ":" line_number[key] > "/dev/stderr"
		}
	}
	if (hit_lines == 0) {
		print "LCOV report contains no executed workspace production lines" > "/dev/stderr"
		exit 1
	}
	if (misses > 0) {
		exit 1
	}
	printf "LCOV complete: files=%d tracked_lines=%d hit_lines=%d misses=0\n", production_files, tracked_lines, hit_lines
}
' "${LCOV_FILES[@]}"
