#!/bin/bash
set -euo pipefail

BASE_REF="${1:-origin/main}"
MERGE_BASE=$(git merge-base HEAD "$BASE_REF")
CHANGED_FILES=$(git diff --name-only "$MERGE_BASE"...HEAD)

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

# Map changed files -> workspace packages using cargo metadata
WORKSPACE_ROOT=$(pwd)
METADATA=$(cargo metadata --format-version 1 --no-deps 2>/dev/null)

declare -A AFFECTED_MAP
while IFS= read -r file; do
  ABS_FILE="$WORKSPACE_ROOT/$file"
  PKG=$(echo "$METADATA" | jq -r --arg f "$ABS_FILE" '
    .packages[]
    | select(
        ($f | startswith((.manifest_path | rtrimstr("/Cargo.toml"))))
      )
    | .name' | head -1)
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
