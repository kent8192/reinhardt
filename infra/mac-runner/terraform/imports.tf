# Idempotent import blocks for recovering Terraform state.
# These blocks are no-ops when resources are already managed by Terraform.
# They activate only when state is lost and Docker resources still exist.
#
# Covered:
#   - Volumes (stable, predictable names — preserves caches on state recovery)
#
# Not covered (safe to recreate):
#   - docker_network  — import requires Docker network ID (SHA256), not name
#   - docker_container — ephemeral; IDs change on every recreate
#   - docker_image     — provider does not support import

import {
	to = docker_volume.dind_certs_ca
	id = "mac-runner-dind-certs-ca"
}

import {
	to = docker_volume.dind_certs_client
	id = "mac-runner-dind-certs-client"
}

import {
	for_each = { for i in range(var.runner_replicas) : tostring(i) => i }
	to       = docker_volume.runner_work[each.value]
	id       = "mac-runner-work-${each.value}"
}
