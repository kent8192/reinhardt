#!/usr/bin/env bash
# Collect release data (CHANGELOG, PRs, Breaking Changes Discussions) into JSON.
# Usage: ./scripts/collect-release-data.sh <tag> [--output <file>]
# Example: ./scripts/collect-release-data.sh reinhardt-web@v0.1.0-rc.15

set -euo pipefail

# --- Constants ---

REPO_OWNER="kent8192"
REPO_NAME="reinhardt-web"
TAG_PREFIX="reinhardt-web@v"
BC_CATEGORY_ID="DIC_kwDOP9Jw0c4C6kgx"

# Bot accounts to exclude from PR comments
BOT_AUTHORS="github-actions\\[bot\\]|copilot\\[bot\\]|reinhardt-release-plz\\[bot\\]|dependabot\\[bot\\]"

# --- Argument parsing ---

TAG=""
OUTPUT_FILE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output)
      OUTPUT_FILE="$2"
      shift 2
      ;;
    -*)
      echo "Error: Unknown option '$1'" >&2
      echo "Usage: $0 <tag> [--output <file>]" >&2
      exit 1
      ;;
    *)
      if [ -z "$TAG" ]; then
        TAG="$1"
      else
        echo "Error: Unexpected argument '$1'" >&2
        exit 1
      fi
      shift
      ;;
  esac
done

if [ -z "$TAG" ]; then
  echo "Error: Tag argument is required" >&2
  echo "Usage: $0 <tag> [--output <file>]" >&2
  exit 1
fi

# Extract version from tag: reinhardt-web@v0.1.0-rc.15 → 0.1.0-rc.15
VERSION="${TAG#"$TAG_PREFIX"}"

if [ -z "$VERSION" ] || [ "$VERSION" = "$TAG" ]; then
  echo "Error: Tag must start with '$TAG_PREFIX'" >&2
  exit 1
fi

if [ -z "$OUTPUT_FILE" ]; then
  OUTPUT_FILE="release-data-${VERSION}.json"
fi

# --- Resolve repository root ---

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "Collecting release data for $TAG..."

# --- Find previous tag ---

PREV_TAG=""
FOUND_CURRENT=false

while IFS= read -r t; do
  if [ "$t" = "$TAG" ]; then
    FOUND_CURRENT=true
    continue
  fi
  if $FOUND_CURRENT; then
    PREV_TAG="$t"
    break
  fi
done < <(git -C "$REPO_ROOT" tag --list "${TAG_PREFIX}*" --sort=-version:refname)

if ! $FOUND_CURRENT; then
  echo "Error: Tag '$TAG' not found in repository" >&2
  exit 1
fi

if [ -z "$PREV_TAG" ]; then
  echo "No previous tag found (this is the first release)"
  COMMIT_RANGE="$TAG"
else
  echo "Previous tag: $PREV_TAG"
  COMMIT_RANGE="${PREV_TAG}..${TAG}"
fi

# --- Get release date from tag ---

RELEASE_DATE=$(git -C "$REPO_ROOT" log -1 --format=%cs "$TAG")

# --- Extract CHANGELOG section ---

CHANGELOG_FILE="$REPO_ROOT/CHANGELOG.md"
CHANGELOG_SECTION=""

if [ -f "$CHANGELOG_FILE" ]; then
  # Extract section between ## [VERSION] and next ## [
  ESCAPED_VERSION="${VERSION//./\\.}"
  CHANGELOG_SECTION=$(perl -0777 -ne "
    if (/^(## \\[${ESCAPED_VERSION}\\].*?)(?=^## \\[|\\z)/ms) {
      print \$1;
    }
  " "$CHANGELOG_FILE")
fi

if [ -z "$CHANGELOG_SECTION" ]; then
  echo "Warning: No CHANGELOG section found for version $VERSION" >&2
  CHANGELOG_SECTION="No CHANGELOG entry found for this version."
fi

# --- Extract PR numbers from commit range ---

echo "Extracting PRs from commit range: $COMMIT_RANGE"

PR_NUMBERS=()

# Merge commits: "Merge pull request #NNN from ..."
while IFS= read -r num; do
  [ -n "$num" ] && PR_NUMBERS+=("$num")
done < <(git -C "$REPO_ROOT" log "$COMMIT_RANGE" --merges --format="%s" 2>/dev/null \
  | grep -oE '#[0-9]+' | grep -oE '[0-9]+' || true)

# Squash commits: "feat(scope): description (#NNN)"
while IFS= read -r num; do
  [ -n "$num" ] && PR_NUMBERS+=("$num")
done < <(git -C "$REPO_ROOT" log "$COMMIT_RANGE" --no-merges --format="%s" 2>/dev/null \
  | grep -oE '\(#[0-9]+\)' | grep -oE '[0-9]+' || true)

# Deduplicate and sort
if [ ${#PR_NUMBERS[@]} -gt 0 ]; then
  mapfile -t PR_NUMBERS < <(printf '%s\n' "${PR_NUMBERS[@]}" | sort -un)
fi

echo "Found ${#PR_NUMBERS[@]} PRs in range"

# --- Fetch PR details ---
# Collect each PR as a separate JSON file, then combine with jq -s at the end.
# This avoids O(n^2) re-parsing of the accumulator on every iteration.

PRS_TMPDIR=$(mktemp -d)
trap 'rm -rf "$PRS_TMPDIR"' EXIT
PR_INDEX=0

for pr_num in "${PR_NUMBERS[@]}"; do
  echo "  Fetching PR #$pr_num..."

  # Get PR data
  PR_DATA=$(gh api "repos/$REPO_OWNER/$REPO_NAME/pulls/$pr_num" \
    --jq '{
      number: .number,
      title: .title,
      url: .html_url,
      body: (.body // ""),
      labels: [.labels[].name],
      author: .user.login
    }' 2>/dev/null || echo "null")

  if [ "$PR_DATA" = "null" ]; then
    echo "  Warning: Could not fetch PR #$pr_num, skipping" >&2
    continue
  fi

  # Check exclusions: skip release-plz PRs and bot authors
  PR_TITLE=$(printf '%s' "$PR_DATA" | jq -r '.title')
  PR_AUTHOR=$(printf '%s' "$PR_DATA" | jq -r '.author')

  if [ "$PR_TITLE" = "chore: release" ]; then
    echo "  Skipping release-plz PR #$pr_num"
    continue
  fi

  if printf '%s' "$PR_AUTHOR" | grep -qE '\[bot\]$'; then
    echo "  Skipping bot PR #$pr_num (author: $PR_AUTHOR)"
    continue
  fi

  # Get human comments (exclude bots)
  COMMENTS_TMPFILE=$(mktemp)
  gh api "repos/$REPO_OWNER/$REPO_NAME/issues/$pr_num/comments" \
    --jq "[.[] | select(.user.login | test(\"${BOT_AUTHORS}\") | not) | .body]" \
    > "$COMMENTS_TMPFILE" 2>/dev/null || printf '%s' "[]" > "$COMMENTS_TMPFILE"

  # Merge PR data with comments, write as individual file
  printf '%s' "$PR_DATA" | jq --slurpfile comments "$COMMENTS_TMPFILE" '. + {human_comments: $comments[0]}' \
    > "$PRS_TMPDIR/pr_$(printf '%04d' $PR_INDEX).json"
  PR_INDEX=$((PR_INDEX + 1))

  rm -f "$COMMENTS_TMPFILE"
done

# Combine all PR files into a single JSON array
if [ "$PR_INDEX" -gt 0 ]; then
  jq -s '.' "$PRS_TMPDIR"/pr_*.json > "$PRS_TMPDIR/all_prs.json"
else
  printf '%s' '[]' > "$PRS_TMPDIR/all_prs.json"
fi
PRS_JSON_FILE="$PRS_TMPDIR/all_prs.json"

# --- Fetch Breaking Changes Discussions ---

echo "Checking Breaking Changes Discussions..."

BC_DISCUSSIONS="[]"

if [ -n "$PREV_TAG" ]; then
  PREV_DATE=$(git -C "$REPO_ROOT" log -1 --format=%cI "$PREV_TAG")

  # Query Breaking Changes category discussions created after previous release
  BC_RESULT=$(gh api graphql -f query='
    query($owner: String!, $repo: String!, $categoryId: ID!) {
      repository(owner: $owner, name: $repo) {
        discussions(first: 50, categoryId: $categoryId, orderBy: {field: CREATED_AT, direction: DESC}) {
          nodes {
            number
            title
            url
            createdAt
          }
        }
      }
    }' \
    -f owner="$REPO_OWNER" \
    -f repo="$REPO_NAME" \
    -f categoryId="$BC_CATEGORY_ID" \
    --jq '.data.repository.discussions.nodes' 2>/dev/null || echo "[]")

  # Filter to discussions created after previous release date
  BC_DISCUSSIONS=$(printf '%s' "$BC_RESULT" | jq --arg after "$PREV_DATE" \
    '[.[] | select(.createdAt > $after) | {number, title, url}]')
fi

BC_COUNT=$(printf '%s' "$BC_DISCUSSIONS" | jq 'length')
echo "Found $BC_COUNT Breaking Changes Discussions since last release"

# --- Assemble output JSON ---
# Write intermediate data to temp files to avoid ARG_MAX limits with large PR data

TMPDIR_ASSEMBLE=$(mktemp -d)
trap 'rm -rf "$TMPDIR_ASSEMBLE" "$PRS_TMPDIR"' EXIT

printf '%s' "$BC_DISCUSSIONS" > "$TMPDIR_ASSEMBLE/bc.json"
printf '%s' "$CHANGELOG_SECTION" > "$TMPDIR_ASSEMBLE/changelog.txt"

jq -n \
  --arg version "$VERSION" \
  --arg tag "$TAG" \
  --arg previous_tag "$PREV_TAG" \
  --arg date "$RELEASE_DATE" \
  --rawfile changelog_section "$TMPDIR_ASSEMBLE/changelog.txt" \
  --slurpfile pull_requests "$PRS_JSON_FILE" \
  --slurpfile breaking_changes_discussions "$TMPDIR_ASSEMBLE/bc.json" \
  '{
    version: $version,
    tag: $tag,
    previous_tag: $previous_tag,
    date: $date,
    changelog_section: $changelog_section,
    pull_requests: $pull_requests[0],
    breaking_changes_discussions: $breaking_changes_discussions[0]
  }' > "$OUTPUT_FILE"

PR_COUNT=$(jq 'length' "$PRS_JSON_FILE")
echo "Release data written to $OUTPUT_FILE"
echo "  PRs: $PR_COUNT"
echo "  Breaking Changes Discussions: $BC_COUNT"
