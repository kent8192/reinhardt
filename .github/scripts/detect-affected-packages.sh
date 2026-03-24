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
  echo "cargo-packages=" >> "$GITHUB_OUTPUT"
  echo "partition-count=0" >> "$GITHUB_OUTPUT"
  echo "partitions-json=[]" >> "$GITHUB_OUTPUT"
  exit 0
fi

# Global file patterns that trigger full test run.
# Only match files that actually affect the build or test execution.
# Non-build files (labels.yml, issue templates, PR templates, CODEOWNERS,
# dependabot.yml, FUNDING.yml) are excluded. Issue #2880
RUN_ALL=false
while IFS= read -r file; do
  case "$file" in
    Cargo.toml|Cargo.lock|rust-toolchain*|Makefile.toml)
      RUN_ALL=true; break ;;
    .cargo/*|.config/*)
      RUN_ALL=true; break ;;
    .github/workflows/*|.github/actions/*|.github/scripts/*|.github/docker-images-*.txt)
      RUN_ALL=true; break ;;
  esac
done <<< "$CHANGED_FILES"

if [[ "$RUN_ALL" == "true" ]]; then
  echo "run-all=true" >> "$GITHUB_OUTPUT"
  echo "has-affected=true" >> "$GITHUB_OUTPUT"
  echo "nextest-filter=" >> "$GITHUB_OUTPUT"
  echo "affected-packages=" >> "$GITHUB_OUTPUT"
  echo "cargo-packages=" >> "$GITHUB_OUTPUT"
  echo "partition-count=8" >> "$GITHUB_OUTPUT"
  echo "partitions-json=[1,2,3,4,5,6,7,8]" >> "$GITHUB_OUTPUT"
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
  echo "cargo-packages=" >> "$GITHUB_OUTPUT"
  echo "partition-count=8" >> "$GITHUB_OUTPUT"
  echo "partitions-json=[1,2,3,4,5,6,7,8]" >> "$GITHUB_OUTPUT"
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
  echo "cargo-packages=" >> "$GITHUB_OUTPUT"
  echo "partition-count=0" >> "$GITHUB_OUTPUT"
  echo "partitions-json=[]" >> "$GITHUB_OUTPUT"
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

# Compute reverse dependencies (rdeps) of affected packages using cargo metadata.
# This determines which workspace packages need to be compiled (via -p flags)
# instead of compiling the entire workspace (--workspace). Issue #2878
#
# Algorithm: build a workspace-internal reverse dependency graph from the
# packages[].dependencies field, then compute the transitive closure starting
# from the affected packages.
RDEPS_PKGS=$(echo "$METADATA" | jq -r --argjson affected \
  "$(printf '%s\n' "${AFFECTED_PKGS[@]}" | jq -R . | jq -s .)" '
  # Get all workspace package names
  [.packages[].name] as $ws_names |

  # Build forward dependency graph (workspace-internal edges only)
  (reduce .packages[] as $p ({};
    . + {($p.name): [($p.dependencies[].name |
      select(. as $n | $ws_names | index($n) != null))]}
  )) as $fwd |

  # Build reverse dependency graph from forward deps
  (reduce ($fwd | keys[]) as $pkg ({};
    reduce ($fwd[$pkg][]) as $dep (.;
      .[$dep] = ((.[$dep] // []) + [$pkg])
    )
  )) as $rdeps |

  # Compute transitive closure from affected packages (BFS)
  {queue: $affected, visited: ($affected | map({(.): true}) | add // {})} |
  until(.queue | length == 0;
    .queue[0] as $current |
    .queue[1:] as $rest |
    ($rdeps[$current] // []) as $neighbors |
    reduce $neighbors[] as $n ({queue: $rest, visited: .visited};
      if .visited[$n] then .
      else {queue: (.queue + [$n]), visited: (.visited + {($n): true})}
      end
    )
  ) |
  .visited | keys[]
')

# Collect rdeps into array, falling back to affected packages if computation fails
declare -A RDEPS_MAP
if [[ -n "$RDEPS_PKGS" ]]; then
  while IFS= read -r pkg; do
    [[ -z "$pkg" ]] && continue
    RDEPS_MAP["$pkg"]=1
  done <<< "$RDEPS_PKGS"
else
  for pkg in "${AFFECTED_PKGS[@]}"; do
    RDEPS_MAP["$pkg"]=1
  done
fi
ALL_RDEPS_PKGS=(${!RDEPS_MAP[@]})
RDEPS_COUNT=${#ALL_RDEPS_PKGS[@]}

echo "::notice::Packages to compile (${RDEPS_COUNT} total, including rdeps): ${ALL_RDEPS_PKGS[*]}"

# Compute dynamic partition count based on rdeps scope. Issue #2881
# Fewer affected packages → fewer partitions to avoid wasting runners.
# Max partition count is 8 (matching the largest fixed partition count in CI).
if (( RDEPS_COUNT <= 2 )); then
  PARTITION_COUNT=2
elif (( RDEPS_COUNT <= 8 )); then
  PARTITION_COUNT=$RDEPS_COUNT
else
  PARTITION_COUNT=8
fi
# Build JSON array for dynamic matrix (e.g., [1,2,3])
PARTITIONS_JSON=$(seq 1 "$PARTITION_COUNT" | jq -cs .)
echo "::notice::Partition count: $PARTITION_COUNT (partitions: $PARTITIONS_JSON)"

echo "run-all=false" >> "$GITHUB_OUTPUT"
echo "has-affected=true" >> "$GITHUB_OUTPUT"
echo "nextest-filter=$NEXTEST_FILTER" >> "$GITHUB_OUTPUT"
echo "partition-count=$PARTITION_COUNT" >> "$GITHUB_OUTPUT"
echo "partitions-json=$PARTITIONS_JSON" >> "$GITHUB_OUTPUT"

# affected-packages as multi-line output (for doc-test per-package execution)
{
  echo "affected-packages<<AFFECTED_EOF"
  for pkg in "${AFFECTED_PKGS[@]}"; do
    echo "$pkg"
  done
  echo "AFFECTED_EOF"
} >> "$GITHUB_OUTPUT"

# cargo-packages as multi-line output (affected + rdeps, for -p flag compilation)
{
  echo "cargo-packages<<CARGO_PKG_EOF"
  for pkg in "${ALL_RDEPS_PKGS[@]}"; do
    echo "$pkg"
  done
  echo "CARGO_PKG_EOF"
} >> "$GITHUB_OUTPUT"
