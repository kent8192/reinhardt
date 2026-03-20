output "runner_names" {
  description = "Names of runner containers"
  value       = [for r in docker_container.runner : r.name]
}

output "dind_name" {
  description = "Name of DinD container"
  value       = docker_container.dind.name
}

output "network_name" {
  description = "Name of runner network"
  value       = docker_network.runner_net.name
}

output "runner_count" {
  description = "Number of runner containers"
  value       = var.runner_replicas
}
