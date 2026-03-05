variable "aws_region" {
	description = "AWS region for runner infrastructure"
	type        = string
	default     = "us-east-1"
}

variable "aws_account_id" {
	description = "AWS Account ID for the CI sub-account. Used by init.sh to auto-generate backend.tfvars with the correct S3 bucket name."
	type        = string
	# Retrieve with: aws sts get-caller-identity --query Account --output text
}

variable "github_app_id" {
	description = "GitHub App ID for runner registration token generation"
	type        = string
}

variable "github_app_installation_id" {
	description = "GitHub App installation ID (shown in GitHub App settings after installation)"
	type        = string
}

variable "github_app_key_base64" {
	description = "GitHub App private key encoded in base64 (cat key.pem | base64 | tr -d newline)"
	type        = string
	sensitive   = true
}

variable "github_owner" {
	description = "GitHub repository owner username"
	type        = string
	# Do not hardcode: specify in terraform.tfvars
}

variable "github_repository" {
	description = "GitHub repository name (without owner prefix)"
	type        = string
	default     = "reinhardt-web"
}

variable "runner_instance_types" {
	description = "EC2 Spot fleet instance type candidates (priority order). c6a first for cost."
	type        = list(string)
	default     = ["c6a.2xlarge", "c6i.2xlarge", "c5a.2xlarge"]
}

variable "runner_max_count" {
	description = "Maximum number of concurrent self-hosted runners. Set to cover observed max parallelism (~20 jobs) with headroom."
	type        = number
	default     = 20
}

variable "runner_extra_labels" {
	description = "Custom labels for self-hosted runners (used in runs-on)"
	type        = list(string)
	default     = ["reinhardt-ci"]
}

variable "runner_ebs_size_gb" {
	description = "EBS root volume size in GB (large enough to skip disk cleanup)"
	type        = number
	default     = 200
}

variable "prefix" {
	description = "Prefix for all AWS resource names"
	type        = string
	default     = "reinhardt-ci"
}

variable "monthly_budget_limit_usd" {
	description = "Monthly budget limit in USD. Triggers fallback to GitHub-hosted runners when exceeded."
	type        = string
	default     = "100"
}

variable "budget_alert_email" {
	description = "Email address for budget alert notifications"
	type        = string
}
