variable "github_owner" {
  description = "GitHub repository owner"
  type        = string
  default     = "kent8192"
}

variable "github_token" {
  description = "GitHub PAT with repo scope"
  type        = string
  sensitive   = true
}

variable "repository_name" {
  description = "GitHub repository name"
  type        = string
  default     = "reinhardt"
}

variable "mac_runner_enabled" {
  description = "Enable Mac local runner in CI workflows"
  type        = bool
  default     = true
}
