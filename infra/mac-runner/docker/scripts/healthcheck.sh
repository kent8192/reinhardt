#!/bin/bash
# Check if the GitHub Actions Runner.Listener process is running
pgrep -f "Runner.Listener" > /dev/null 2>&1
