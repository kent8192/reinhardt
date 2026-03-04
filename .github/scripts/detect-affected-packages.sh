#!/bin/bash
set -euo pipefail

BASE_REF="${1:-origin/main}"

# Collect changed files by comparing the merge base of BASE_REF and COMPARE_REF.
# We use 'git diff --name-only BASE...COMPARE' (three-dot) so that only files
# that differ in the final state are considered. This avoids false positives from
# commits that add then revert a file within the same branch.
#
# In a PR context, the HEAD_REF env var contains the PR branch name. We use
# 'origin/$HEAD_REF' (the actual PR branch ref) rather than 'HEAD', because in
# GitHub Actions the checkout ref is a synthetic merge commit (refs/pull/N/merge).
# The origin/$HEAD_REF approach (Issue #1822) makes git diff safe to use here;
# previously git log was needed to work around the synthetic merge ref, but that
# caused false RUN_ALL=true for add-then-revert patterns (Issue #1833).
if [[ -n "${HEAD_REF:-}" ]]; then
  COMPARE_REF="origin/$HEAD_REF"
else
  COMPARE_REF="HEAD"
fi
CHANGED_FILES=$(git diff --name-only "$BASE_REF...$COMPARE_REF" \
  | grep -v '^$' | sort -u || true)

if [[ -z "$CHANGED_FILES" ]]; then
  echo "run-all=false" >> "$GITHUB_OUTPUT"
  echo "has-affected=false" >> "$GITHUB_OUTPUT"
  echo "nextest-filter=" >> "$GITHUB_OUTPUT"
  echo "affected-packages=" >> "$GITHUB_OUTPUT"
  exit 0
fi

# Global file patterns that trigger full test run
RUN_ALL=false
while IFS= read -r file; do
  case "$file" in
    Cargo.toml|Cargo.lock|rust-toolchain*|Makefile.toml)
      RUN_ALL=true; break ;;
    .cargo/*|.config/*|.github/*)
      RUN_ALL=true; break ;;
  esac
done <<< "$CHANGED_FILES"

if [[ "$RUN_ALL" == "true" ]]; then
  echo "run-all=true" >> "$GITHUB_OUTPUT"
  echo "has-affected=true" >> "$GITHUB_OUTPUT"
  echo "nextest-filter=" >> "$GITHUB_OUTPUT"
  echo "affected-packages=" >> "$GITHUB_OUTPUT"
  exit 0
fi

# Map changed files -> workspace packages using cargo metadata.
# Use explicit exit-code check because VAR=$(failing_cmd) does not trigger
# set -e in bash — the assignment itself succeeds even when the subshell fails.
# Fall back to a full test run if cargo metadata is unavailable or returns
# empty output. # Issue #1819
WORKSPACE_ROOT=$(pwd)
if ! METADATA=$(cargo metadata --format-version 1 --no-deps 2>/dev/null) \
    || [[ -z "$METADATA" ]]; then
  echo "run-all=true" >> "$GITHUB_OUTPUT"
  echo "has-affected=true" >> "$GITHUB_OUTPUT"
  echo "nextest-filter=" >> "$GITHUB_OUTPUT"
  echo "affected-packages=" >> "$GITHUB_OUTPUT"
  exit 0
fi

declare -A AFFECTED_MAP
while IFS= read -r file; do
  ABS_FILE="$WORKSPACE_ROOT/$file"
  # Use try-catch in jq so that a single malformed package entry does not abort
  # the entire scan. # Issue #1819
  PKG=$(echo "$METADATA" | jq -r --arg f "$ABS_FILE" '
    .packages[]
    | select(
        try ($f | startswith((.manifest_path | rtrimstr("/Cargo.toml"))))
        catch false
      )
    | .name' 2>/dev/null | head -1 || true)
  if [[ -n "$PKG" && "$PKG" != "null" ]]; then
    AFFECTED_MAP["$PKG"]=1
  fi
done <<< "$CHANGED_FILES"

AFFECTED_PKGS=(${!AFFECTED_MAP[@]})

if [[ ${#AFFECTED_PKGS[@]} -eq 0 ]]; then
  echo "run-all=false" >> "$GITHUB_OUTPUT"
  echo "has-affected=false" >> "$GITHUB_OUTPUT"
  echo "nextest-filter=" >> "$GITHUB_OUTPUT"
  echo "affected-packages=" >> "$GITHUB_OUTPUT"
  exit 0
fi

# Build nextest rdeps() filter expression
NEXTEST_FILTER=""
for pkg in "${AFFECTED_PKGS[@]}"; do
  if [[ -z "$NEXTEST_FILTER" ]]; then
    NEXTEST_FILTER="rdeps(=$pkg)"
  else
    NEXTEST_FILTER="$NEXTEST_FILTER + rdeps(=$pkg)"
  fi
done

echo "run-all=false" >> "$GITHUB_OUTPUT"
echo "has-affected=true" >> "$GITHUB_OUTPUT"
echo "nextest-filter=$NEXTEST_FILTER" >> "$GITHUB_OUTPUT"

# affected-packages as multi-line output (for doc-test per-package execution)
{
  echo "affected-packages<<AFFECTED_EOF"
  for pkg in "${AFFECTED_PKGS[@]}"; do
    echo "$pkg"
  done
  echo "AFFECTED_EOF"
} >> "$GITHUB_OUTPUT"
