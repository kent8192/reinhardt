# Manages GitHub repository settings (labels, environments).
# Run with a GitHub token that has admin access to the repository.

terraform {
  required_version = ">= 1.5"

  required_providers {
    github = {
      source  = "integrations/github"
      version = "~> 6.0"
    }
  }
  # Local state: repository settings management is low-change, local state is sufficient.
}

provider "github" {
  owner = var.github_owner
  token = var.github_token
}
