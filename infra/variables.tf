# --- Sensitive (values in terraform.tfvars, NOT committed) ---
variable "cloudflare_api_token" {
  description = "Cloudflare API token with Pages and DNS permissions"
  type        = string
  sensitive   = true
}

variable "cloudflare_account_id" {
  description = "Cloudflare account ID"
  type        = string
  sensitive   = true
}

variable "github_token" {
  description = "GitHub PAT with repository secrets permission"
  type        = string
  sensitive   = true
}

# --- Non-sensitive ---
variable "github_owner" {
  description = "GitHub repository owner"
  type        = string
}

variable "github_repository" {
  description = "GitHub repository name"
  type        = string
}

variable "pages_project_name" {
  description = "Cloudflare Pages project name"
  type        = string
}

variable "custom_domain" {
  description = "Custom domain for the Pages project"
  type        = string
}

variable "production_branch" {
  description = "Git branch for production deployments"
  type        = string
  default     = "main"
}

# --- Import IDs (for importing existing Cloudflare resources) ---
variable "dns_record_apex_id" {
  description = "Cloudflare DNS record ID for the apex domain"
  type        = string
}

variable "dns_record_www_id" {
  description = "Cloudflare DNS record ID for the www subdomain"
  type        = string
}
