#!/bin/bash
# Pre-installation hook for Golden AMI runners.
# All build tools, Docker images, and dependencies are baked into the AMI.
# This script only verifies Docker is running (required for TestContainers).

set -eo pipefail

# Verify Docker service is running (pre-installed in Golden AMI)
systemctl is-active docker || systemctl start docker
