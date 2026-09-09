#!/usr/bin/env bash
# Run the Node policy suite through the shared shell-test entry point.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel)"

cd "$ROOT_DIR"
exec node --test "$SCRIPT_DIR/test-breaking-change-target.cjs"
