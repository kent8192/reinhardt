#!/bin/bash
# Sync examples from main repository to this repository

set -e

MAIN_REPO="${MAIN_REPO:-https://github.com/kent8192/reinhardt-web.git}"
MAIN_BRANCH="${MAIN_BRANCH:-main}"
TEMP_DIR=$(mktemp -d)

echo "🔄 Syncing from main repository..."
echo "Repository: $MAIN_REPO"
echo "Branch: $MAIN_BRANCH"
echo "Temporary directory: $TEMP_DIR"

# Clone main repository (shallow)
echo "📥 Cloning main repository..."
git clone --depth 1 --branch "$MAIN_BRANCH" "$MAIN_REPO" "$TEMP_DIR"

# Check if examples directory exists in main repository
if [ ! -d "$TEMP_DIR/examples" ]; then
	echo "❌ Error: examples/ directory not found in main repository"
	rm -rf "$TEMP_DIR"
	exit 1
fi

# Copy examples directory
echo "📋 Copying examples..."
rsync -av --delete \
	--exclude '.git' \
	--exclude 'target' \
	--exclude 'Cargo.lock' \
	--exclude '.github' \
	--exclude 'scripts' \
	--exclude 'CONTRIBUTING.md' \
	--exclude 'CHANGELOG.md' \
	--exclude 'COMPATIBILITY.json' \
	--exclude 'SUBTREE_OPERATIONS.md' \
	"$TEMP_DIR/examples/" ./

# Cleanup
rm -rf "$TEMP_DIR"

echo ""
echo "✅ Sync complete!"
echo ""
echo "📝 Review changes:"
echo "   git status"
echo ""
echo "💾 Commit changes:"
echo "   git add ."
echo "   git commit -m 'chore: sync from main repository'"
echo "   git push origin main"
