provider "docker" {
  host = "unix:///var/run/docker.sock"
}

# --- Network ---

resource "docker_network" "runner_net" {
  name   = "mac-runner-net"
  driver = "bridge"
}

# --- Volumes ---

resource "docker_volume" "dind_certs_ca" {
  name = "mac-runner-dind-certs-ca"
}

resource "docker_volume" "dind_certs_client" {
  name = "mac-runner-dind-certs-client"
}

resource "docker_volume" "runner_work" {
  count = var.runner_replicas
  name  = "mac-runner-work-${count.index}"
}

# --- DinD (Docker-in-Docker) ---

resource "docker_image" "dind" {
  name         = "docker:27-dind"
  keep_locally = true
}

resource "docker_container" "dind" {
  name  = "mac-runner-dind"
  image = docker_image.dind.image_id

  privileged = true

  # Set hostname so DinD TLS certificate SANs include the container name,
  # allowing runners to connect via tcp://mac-runner-dind:2376
  hostname = "mac-runner-dind"

  env = [
    "DOCKER_TLS_CERTDIR=/certs",
  ]

  volumes {
    volume_name    = docker_volume.dind_certs_ca.name
    container_path = "/certs/ca"
  }

  volumes {
    volume_name    = docker_volume.dind_certs_client.name
    container_path = "/certs/client"
  }

  # Mount all runner work directories so DinD can access them
  # (required for TestContainers volume mounts)
  dynamic "volumes" {
    for_each = docker_volume.runner_work
    content {
      volume_name    = volumes.value.name
      container_path = "/runner-work/${volumes.key}"
    }
  }

  networks_advanced {
    name = docker_network.runner_net.id
  }

  # memory expects bytes in kreuzwerker/docker provider
  memory = var.dind_memory_mb * 1024 * 1024

  cpu_shares = var.dind_cpu

  healthcheck {
    test     = ["CMD", "docker", "info"]
    interval = "10s"
    timeout  = "5s"
    retries  = 5
  }

  restart = "always"

  # Docker daemon injects default log_opts (max-file, max-size) into container
  # state, causing spurious replacement diffs when not set in config.
  lifecycle {
    ignore_changes = [log_driver, log_opts, memory_swap]
  }
}

# --- Runner Containers ---

resource "docker_image" "runner" {
  name = "mac-runner:latest"
  build {
    context    = "${path.module}/../docker"
    dockerfile = "Dockerfile"
  }
  triggers = {
    dockerfile_hash = filesha256("${path.module}/../docker/Dockerfile")
  }
}

resource "docker_container" "runner" {
  count = var.runner_replicas
  name  = "mac-runner-${count.index}"
  image = docker_image.runner.image_id

  env = [
    "RUNNER_NAME_PREFIX=mac-local-${count.index}",
    "RUNNER_SCOPE=repo",
    "REPO_URL=${var.repo_url}",
    "ACCESS_TOKEN=${var.github_token}",
    "LABELS=${var.runner_labels}",
    "EPHEMERAL=1",
    "DISABLE_AUTO_UPDATE=1",
    "DOCKER_HOST=tcp://mac-runner-dind:2376",
    "DOCKER_TLS_VERIFY=1",
    "DOCKER_CERT_PATH=/certs/client",
    "TESTCONTAINERS_HOST_OVERRIDE=mac-runner-dind",
  ]

  volumes {
    volume_name    = docker_volume.dind_certs_client.name
    container_path = "/certs/client"
    read_only      = true
  }

  volumes {
    volume_name    = docker_volume.runner_work[count.index].name
    container_path = "/home/runner/work"
  }

  networks_advanced {
    name = docker_network.runner_net.id
  }

  # memory expects bytes in kreuzwerker/docker provider
  memory = var.runner_memory_mb * 1024 * 1024

  cpu_shares = var.runner_cpu

  security_opts = ["no-new-privileges:true"]

  restart  = "always"
  must_run = true

  depends_on = [docker_container.dind]

  # Wait for DinD healthcheck before considering runner healthy
  healthcheck {
    test     = ["CMD", "/usr/local/bin/healthcheck.sh"]
    interval = "30s"
    timeout  = "10s"
    retries  = 3
  }

  # Docker daemon injects default log_opts (max-file, max-size) into container
  # state, causing spurious replacement diffs when not set in config.
  lifecycle {
    ignore_changes = [log_driver, log_opts, memory_swap]
  }
}
