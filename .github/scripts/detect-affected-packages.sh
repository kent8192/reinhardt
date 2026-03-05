#!/bin/bash
set -euo pipefail

BASE_REF="${1:-origin/main}"

# In a PR context (PR_NUMBER and GITHUB_REPOSITORY are set), use the GitHub
# REST API to get the list of changed files. This is immune to the issue where
# git diff returns empty after update-branch has merged the base branch into
# the PR branch, making the three-dot merge base equal to the current base
# branch tip. Issue #1836.
#
# In a non-PR context (push to main, manual trigger, etc.), fall back to
# git diff three-dot notation. The HEAD_REF env var contains the PR branch
# name when available; we use 'origin/$HEAD_REF' rather than 'HEAD', because
# in GitHub Actions the checkout ref is a synthetic merge commit
# (refs/pull/N/merge). Issue #1822.
if [[ -n "${PR_NUMBER:-}" && -n "${GITHUB_REPOSITORY:-}" ]]; then
  if ! GH_OUT=$(gh api \
      "repos/$GITHUB_REPOSITORY/pulls/$PR_NUMBER/files" \
      --paginate \
      --jq '.[].filename'); then
    echo "::error::gh api call failed: repos/$GITHUB_REPOSITORY/pulls/$PR_NUMBER/files" >&2
    exit 1
  fi
  CHANGED_FILES=$(echo "$GH_OUT" | grep -v '^$' | sort -u || true)
else
  if [[ -n "${HEAD_REF:-}" ]]; then
    COMPARE_REF="origin/$HEAD_REF"
  else
    COMPARE_REF="HEAD"
  fi
  CHANGED_FILES=$(git diff --name-only "$BASE_REF...$COMPARE_REF" \
    | grep -v '^$' | sort -u || true)
fi

FILE_COUNT=$(echo "$CHANGED_FILES" | wc -l | tr -d ' ')
echo "::notice::Changed files ($FILE_COUNT total): $(echo "$CHANGED_FILES" | head -5 | tr '\n' ', ')"

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
# Resolve symlinks in workspace root so paths match cargo metadata's
# manifest_path (which always uses the canonical path). This matters on
# macOS where /tmp -> /private/tmp.
WORKSPACE_ROOT=$(cd "$(pwd)" && pwd -P)
if ! METADATA=$(cargo metadata --format-version 1 --no-deps) \
    || [[ -z "$METADATA" ]]; then
  echo "run-all=true" >> "$GITHUB_OUTPUT"
  echo "has-affected=true" >> "$GITHUB_OUTPUT"
  echo "nextest-filter=" >> "$GITHUB_OUTPUT"
  echo "affected-packages=" >> "$GITHUB_OUTPUT"
  exit 0
fi

PKG_COUNT=$(echo "$METADATA" | jq '.packages | length')
echo "::notice::Workspace packages: $PKG_COUNT"

declare -A AFFECTED_MAP
while IFS= read -r file; do
  ABS_FILE="$WORKSPACE_ROOT/$file"
  # Map file to the most specific (longest path match) workspace package.
  # Bind the package object to $pkg so that .manifest_path resolves correctly
  # inside the startswith() call. Issue #1843
  PKG=$(echo "$METADATA" | jq -r --arg f "$ABS_FILE" '
    [.packages[]
    | . as $pkg
    | ($pkg.manifest_path | rtrimstr("/Cargo.toml")) as $dir
    | select(
        try ($f | startswith($dir))
        catch false
      )
    | {name: $pkg.name, dirlen: ($dir | length)}]
    | sort_by(-.dirlen)
    | .[0].name // empty' || true)
  echo "::notice::File mapping: $file -> ${PKG:-<no match>}"
  if [[ -n "$PKG" && "$PKG" != "null" ]]; then
    AFFECTED_MAP["$PKG"]=1
  fi
done <<< "$CHANGED_FILES"

AFFECTED_PKGS=(${!AFFECTED_MAP[@]})

echo "::notice::Affected packages (${#AFFECTED_PKGS[@]}): ${AFFECTED_PKGS[*]}"

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
