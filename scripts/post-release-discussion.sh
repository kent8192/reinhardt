#!/usr/bin/env bash
# Post a release announcement to GitHub Discussions.
# Reads an announcement markdown file and creates a Discussion in the Release category.
# Usage: ./scripts/post-release-discussion.sh <announcement-file>
# Example: ./scripts/post-release-discussion.sh announcements/v0.1.0-rc.15.md

set -euo pipefail

# --- Constants ---

REPO_OWNER="kent8192"
REPO_NAME="reinhardt-web"
RELEASE_CATEGORY_ID="DIC_kwDOP9Jw0c4C2aMg"

# --- Argument parsing ---

if [ $# -lt 1 ]; then
  echo "Error: Announcement file argument is required" >&2
  echo "Usage: $0 <announcement-file>" >&2
  exit 1
fi

ANNOUNCEMENT_FILE="$1"

if [ ! -f "$ANNOUNCEMENT_FILE" ]; then
  echo "Error: File not found: $ANNOUNCEMENT_FILE" >&2
  exit 1
fi

# Extract version from filename: announcements/v0.1.0-rc.15.md → 0.1.0-rc.15
FILENAME=$(basename "$ANNOUNCEMENT_FILE" .md)
VERSION="${FILENAME#v}"
DISCUSSION_TITLE="reinhardt-web v${VERSION}"

echo "Posting announcement for $DISCUSSION_TITLE..."

# --- Get repository ID ---

REPO_ID=$(gh api graphql -f query='
  query($owner: String!, $repo: String!) {
    repository(owner: $owner, name: $repo) { id }
  }' \
  -f owner="$REPO_OWNER" \
  -f repo="$REPO_NAME" \
  --jq '.data.repository.id')

echo "Repository ID: $REPO_ID"

# --- Duplicate check ---

echo "Checking for existing Discussion..."

EXISTING=$(gh api graphql -f query='
  query($owner: String!, $repo: String!, $categoryId: ID!) {
    repository(owner: $owner, name: $repo) {
      discussions(first: 100, categoryId: $categoryId) {
        nodes { title url number }
      }
    }
  }' \
  -f owner="$REPO_OWNER" \
  -f repo="$REPO_NAME" \
  -f categoryId="$RELEASE_CATEGORY_ID" \
  --jq '.data.repository.discussions.nodes')

# Check exact match
EXACT_MATCH=$(echo "$EXISTING" | jq -r --arg title "$DISCUSSION_TITLE" \
  '.[] | select(.title == $title) | .url')

if [ -n "$EXACT_MATCH" ]; then
  echo "::notice::Discussion already exists: $EXACT_MATCH"
  echo "Skipping creation."
  exit 0
fi

# Check partial match (title contains version)
PARTIAL_MATCH=$(echo "$EXISTING" | jq -r --arg version "$VERSION" \
  '.[] | select(.title | contains($version)) | "\(.title) — \(.url)"')

if [ -n "$PARTIAL_MATCH" ]; then
  echo "::warning::Partial title match found. Manual check required:" >&2
  echo "$PARTIAL_MATCH" >&2
  exit 1
fi

# --- Create Discussion ---

BODY=$(cat "$ANNOUNCEMENT_FILE")

RESULT=$(gh api graphql -f query='
  mutation($repoId: ID!, $categoryId: ID!, $title: String!, $body: String!) {
    createDiscussion(input: {
      repositoryId: $repoId,
      categoryId: $categoryId,
      title: $title,
      body: $body
    }) {
      discussion { url number }
    }
  }' \
  -f repoId="$REPO_ID" \
  -f categoryId="$RELEASE_CATEGORY_ID" \
  -f title="$DISCUSSION_TITLE" \
  -f body="$BODY")

DISCUSSION_URL=$(echo "$RESULT" | jq -r '.data.createDiscussion.discussion.url')
DISCUSSION_NUM=$(echo "$RESULT" | jq -r '.data.createDiscussion.discussion.number')

echo "Discussion #$DISCUSSION_NUM created: $DISCUSSION_URL"
