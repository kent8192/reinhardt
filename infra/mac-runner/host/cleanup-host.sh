#!/bin/bash
# Host-level Docker cleanup for Mac runner infrastructure.
# Monitors disk usage and runs Docker cleanup when threshold is exceeded.
#
# Install via launchd plist or crontab:
#   crontab: 0 4 * * * /path/to/cleanup-host.sh >> /tmp/mac-runner-cleanup.log 2>&1

set -euo pipefail

THRESHOLD_PERCENT=80
LOG_PREFIX="[mac-runner-cleanup]"

# Get disk usage percentage for the root filesystem
USAGE=$(df -h / | awk 'NR==2 {gsub(/%/,""); print $5}')

echo "$LOG_PREFIX $(date): Disk usage: ${USAGE}%"

if [ "$USAGE" -ge "$THRESHOLD_PERCENT" ]; then
	echo "$LOG_PREFIX Threshold ${THRESHOLD_PERCENT}% exceeded. Running cleanup..."

	# Prune host-level Docker artifacts
	docker system prune -f --filter "until=48h"

	NEW_USAGE=$(df -h / | awk 'NR==2 {gsub(/%/,""); print $5}')
	echo "$LOG_PREFIX Cleanup complete. Disk usage: ${NEW_USAGE}%"

	# If still above threshold, run aggressive cleanup
	if [ "$NEW_USAGE" -ge "$THRESHOLD_PERCENT" ]; then
		echo "$LOG_PREFIX Still above threshold. Running aggressive cleanup..."
		docker system prune -af --filter "until=24h"
		docker volume prune -f

		FINAL_USAGE=$(df -h / | awk 'NR==2 {gsub(/%/,""); print $5}')
		echo "$LOG_PREFIX Aggressive cleanup complete. Disk usage: ${FINAL_USAGE}%"
	fi
else
	echo "$LOG_PREFIX Disk usage OK, no cleanup needed."
fi
