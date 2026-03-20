# GitHub Personal Access Token (repo scope)
# Generate at: https://github.com/settings/tokens
# Required scopes: repo (for private repos) or public_repo (for public repos)
github_token = "ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"

# Repository URL for runner registration
repo_url = "https://github.com/kent8192/reinhardt"

# Number of parallel runner containers (1-8)
# Default: 4 (recommended for M4 Pro 48GB)
runner_replicas = 4

# Memory limit per runner container (MB)
# Default: 8192 (8GB)
runner_memory_mb = 8192

# Memory limit for DinD container (MB)
# Default: 6144 (6GB)
dind_memory_mb = 6144

# Runner labels (comma-separated)
runner_labels = "self-hosted,linux,arm64,mac-local"
