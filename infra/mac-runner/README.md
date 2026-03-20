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

### Weekly maintenance

```bash
cd infra/mac-runner/terraform
terraform taint docker_image.runner
terraform apply
docker system prune -f
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
