#!/bin/bash
# Run pre-start cleanup, then delegate to the original entrypoint.
/usr/local/bin/pre-start-cleanup.sh
exec /entrypoint.sh "$@"
