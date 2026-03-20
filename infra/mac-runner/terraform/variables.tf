variable "repo_url" {
  description = "GitHub repository URL for runner registration"
  type        = string
  default     = "https://github.com/kent8192/reinhardt-web"
}

variable "github_token" {
  description = "GitHub PAT with repo scope for runner registration"
  type        = string
  sensitive   = true
}

variable "runner_replicas" {
  description = "Number of parallel runner containers"
  type        = number
  default     = 4

  validation {
    condition     = var.runner_replicas >= 1 && var.runner_replicas <= 8
    error_message = "Runner replicas must be between 1 and 8."
  }
}

variable "runner_memory_mb" {
  description = "Memory limit per runner container (MB)"
  type        = number
  default     = 8192
}

variable "runner_cpu" {
  description = "CPU shares per runner container (1024 = 1 core)"
  type        = number
  default     = 2048
}

variable "dind_memory_mb" {
  description = "Memory limit for DinD container (MB)"
  type        = number
  default     = 6144
}

variable "dind_cpu" {
  description = "CPU shares for DinD container (1024 = 1 core)"
  type        = number
  default     = 1024
}

variable "runner_labels" {
  description = "Comma-separated labels for runner registration"
  type        = string
  default     = "self-hosted,linux,arm64,mac-local"
}
