#!/usr/bin/env bash
# Update version references in documentation and examples after a release.
# Usage: ./scripts/update-version-refs.sh <new-version> [--dry-run]
# Example: ./scripts/update-version-refs.sh 0.1.0-rc.8

set -euo pipefail

# --- Argument parsing ---

DRY_RUN=false
NEW_VER=""

for arg in "$@"; do
	case "$arg" in
		--dry-run)
			DRY_RUN=true
			;;
		-*)
			echo "Error: Unknown option '$arg'" >&2
			echo "Usage: $0 <new-version> [--dry-run]" >&2
			exit 1
			;;
		*)
			if [ -z "$NEW_VER" ]; then
				NEW_VER="$arg"
			else
				echo "Error: Unexpected argument '$arg'" >&2
				echo "Usage: $0 <new-version> [--dry-run]" >&2
				exit 1
			fi
			;;
	esac
done

if [ -z "$NEW_VER" ]; then
	echo "Error: Version argument is required" >&2
	echo "Usage: $0 <new-version> [--dry-run]" >&2
	exit 1
fi

# --- Version format validation ---

if ! echo "$NEW_VER" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$'; then
	echo "Error: Invalid version format '$NEW_VER'" >&2
	echo "Expected: X.Y.Z or X.Y.Z-prerelease (e.g., 0.1.0, 0.1.0-rc.8)" >&2
	exit 1
fi

# --- Resolve repository root ---

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "Updating version references to $NEW_VER in $REPO_ROOT"

# Export version for perl to access via $ENV{NEW_VER}
# This avoids shell escaping issues with $1/$2 perl backreferences
export NEW_VER

# --- Update files ---

# 1. examples/Cargo.toml: workspace dependency
#    reinhardt = { version = "OLD", package = "reinhardt-web" }
perl -i -pe 's/(reinhardt\s*=\s*\{\s*version\s*=\s*")[^"]+(",\s*package\s*=\s*"reinhardt-web")/$1$ENV{NEW_VER}$2/g' "$REPO_ROOT/examples/Cargo.toml"

# 2. website/config.toml: template variable
#    reinhardt_version = "OLD"
perl -i -pe 's/(reinhardt_version\s*=\s*")[^"]+(")/$1$ENV{NEW_VER}$2/g' "$REPO_ROOT/website/config.toml"

# 3. README.md: umbrella crate references (with package = "reinhardt-web")
#    reinhardt = { version = "OLD", package = "reinhardt-web", ... }
#    Note: Does NOT match short version ranges like "0.1" (requires X.Y.Z format)
perl -i -pe 's/(reinhardt\s*=\s*\{[^}]*version\s*=\s*")[0-9]+\.[0-9]+\.[0-9]+[^"]*(",?\s*(?:package\s*=\s*"reinhardt-web"|default-features))/$1$ENV{NEW_VER}$2/g' "$REPO_ROOT/README.md"

# 4. README.md: individual crate references (reinhardt-xxx = "version")
perl -i -pe 's/(reinhardt-[a-z][-a-z]*\s*=\s*")[0-9]+\.[0-9]+\.[0-9]+[^"]*(")/$1$ENV{NEW_VER}$2/g' "$REPO_ROOT/README.md"

# 5. examples/CLAUDE.md: umbrella crate references
perl -i -pe 's/(reinhardt\s*=\s*\{[^}]*version\s*=\s*")[0-9]+\.[0-9]+\.[0-9]+[^"]*(",?\s*(?:package\s*=\s*"reinhardt-web"|default-features))/$1$ENV{NEW_VER}$2/g' "$REPO_ROOT/examples/CLAUDE.md"

# --- Show results ---

if $DRY_RUN; then
	echo ""
	echo "=== Dry run: showing diff ==="
	cd "$REPO_ROOT"
	git diff || true
	echo ""
	echo "=== Dry run complete. No changes committed. ==="
	# Revert changes in dry-run mode
	git checkout -- . 2>/dev/null || true
else
	echo ""
	echo "=== Updated files ==="
	cd "$REPO_ROOT"
	git diff --name-only || true
fi
