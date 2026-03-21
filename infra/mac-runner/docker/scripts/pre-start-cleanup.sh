#!/bin/bash
# Clean up stale workspace data from previous ephemeral job.
# Runner work directory persists via named volume across container restarts.

WORK_DIR="/home/runner/work"

if [ -d "$WORK_DIR" ]; then
	echo "[pre-start-cleanup] Cleaning workspace: $(du -sh "$WORK_DIR" 2>/dev/null | cut -f1)"
	find "$WORK_DIR" -mindepth 1 -maxdepth 1 -exec rm -rf {} + 2>/dev/null
	echo "[pre-start-cleanup] Workspace cleaned"
fi
