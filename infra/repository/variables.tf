variable "github_owner" {
  description = "GitHub organization or user that owns the repository."
  type        = string
  default     = "kent8192"
}

variable "github_token" {
  description = "GitHub personal access token with admin repository permissions."
  type        = string
  sensitive   = true
}

variable "repository_name" {
  description = "Name of the GitHub repository to manage."
  type        = string
  default     = "reinhardt"
}

variable "environment_reviewer" {
  description = "GitHub username for environment deployment approvals."
  type        = string
}
