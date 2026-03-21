# Mac Local Runner

Docker-isolated, ephemeral self-hosted GitHub Actions runner on macOS.

## Prerequisites

- macOS with Apple Silicon (M1/M2/M3/M4)
- Docker Desktop installed and running
- Terraform >= 1.5
- GitHub PAT with `repo` scope

## Quick Start

### 1. Set up GitHub variable

```bash
cd infra/mac-runner/github
cp terraform.examples.tfvars terraform.tfvars
# Edit terraform.tfvars: set github_token
terraform init && terraform apply
```

### 2. Start runners

```bash
cd infra/mac-runner/terraform
cp terraform.examples.tfvars terraform.tfvars
# Edit terraform.tfvars: set github_token
terraform init && terraform apply
```

### 3. Verify runners are registered

```bash
docker ps --filter "name=mac-runner"
# Check GitHub: Settings → Actions → Runners
```

## Operations

### Scale runners

```bash
cd infra/mac-runner/terraform
terraform apply -var="runner_replicas=6"
```

### Go offline

```bash
cd infra/mac-runner/github
terraform apply -var="mac_runner_enabled=false"
cd ../terraform
terraform destroy
```

### Come back online

```bash
cd infra/mac-runner/terraform
terraform apply
cd ../github
terraform apply -var="mac_runner_enabled=true"
```

### Disk cleanup

Disk cleanup is automated at three layers:

| Layer | Mechanism | Frequency | Target |
|-------|-----------|-----------|--------|
| DinD internal | GitHub Actions workflow (`runner-cleanup.yml`) | Daily 05:00 UTC / Weekly Sun 03:00 UTC | TestContainers images, stopped containers, build cache |
| Runner workspace | Entrypoint script (`pre-start-cleanup.sh`) | Every job (on container restart) | Previous job's `target/`, git clones |
| Host Docker | launchd (`com.reinhardt.runner-cleanup.plist`) | Daily 04:00 (threshold-based) | Host-level dangling images, build cache |

**Manual trigger:**

```bash
# Trigger DinD cleanup via GitHub Actions
gh workflow run runner-cleanup.yml
gh workflow run runner-cleanup.yml -f aggressive=true

# Manual host cleanup
infra/mac-runner/host/cleanup-host.sh
```

**Install host-level launchd schedule:**

```bash
cp infra/mac-runner/host/com.reinhardt.runner-cleanup.plist ~/Library/LaunchAgents/
launchctl load ~/Library/LaunchAgents/com.reinhardt.runner-cleanup.plist
```

**Check cleanup logs:**

```bash
cat /tmp/mac-runner-cleanup.log
```

### Image rebuild

```bash
cd infra/mac-runner/terraform
terraform taint docker_image.runner
terraform apply
```

## Architecture

Runner containers connect to a DinD (Docker-in-Docker) sidecar for
TestContainers support. Host Docker socket is NOT shared with runners.

```
Runner containers ──TLS──> DinD daemon ──> TestContainers (PostgreSQL, etc.)
```

### Runner Priority

```
MAC_RUNNER_ENABLED=true + trusted actor → Mac local runner
AWS Spot opt-in checkbox                → AWS Spot runner
Fallback (fork PRs, etc.)              → GitHub-hosted ubuntu-latest
```

### Resource Allocation (M4 Pro 48GB)

| Component | Memory | CPU | Count |
|-----------|--------|-----|-------|
| Runner | 8GB | 2 cores | × 4 |
| DinD | 6GB | 1 core | × 1 |
| Headroom | ~10GB | ~3 cores | — |

## Security

- **Ephemeral**: each container handles one job then restarts
- **DinD isolation**: runners cannot access host Docker daemon
- **No-new-privileges**: privilege escalation blocked in runner containers
- **TLS**: runner-to-DinD communication encrypted via auto-generated certificates
- **Fork PRs**: always routed to GitHub-hosted runners (never to Mac runner)
- **Trusted actors only**: repo owner, release-plz branches, workflow_dispatch
