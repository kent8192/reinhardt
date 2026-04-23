variable "aws_region" {
  description = "AWS region for runner infrastructure"
  type        = string
  default     = "us-east-1"
}

variable "excluded_zone_ids" {
  description = "Zone IDs to exclude (AZs that do not support required Graviton instance types). Use zone IDs (e.g. 'use1-az3') for cross-account consistency."
  type        = list(string)
  default     = []
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
  description = "EC2 Spot fleet Graviton instance type candidates (priority order)."
  type        = list(string)
  default     = ["c7g.2xlarge", "c6g.2xlarge"]
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

variable "runner_ami_ssm_parameter_name" {
  description = "SSM Parameter name storing the Golden AMI ID (managed by build-runner-ami workflow)"
  type        = string
  default     = "/reinhardt-ci/runner-ami-id"
}

variable "enable_hotpath_runner" {
  description = "Enable the always-on hotpath runner for lightweight CI control jobs"
  type        = bool
  default     = true
}

variable "hotpath_runner_instance_type" {
  description = "EC2 instance type for the hotpath runner (lightweight CI control jobs)"
  type        = string
  default     = "t4g.micro"
}


variable "tf_plan_aws_access_key_id" {
  description = "AWS access key ID for terraform-plan CI workflow (read-only IAM user recommended)"
  type        = string
  sensitive   = true
}

variable "tf_plan_aws_secret_access_key" {
  description = "AWS secret access key for terraform-plan CI workflow"
  type        = string
  sensitive   = true
}

variable "organizations_account_email" {
  description = "Email address for the CI sub-account (used by organizations module in terraform-plan CI)"
  type        = string
}

# ===== Orphan Detector (Issue #3903) =====

variable "orphan_detector_enabled" {
  description = "Enable the orphan job detector Lambda. Set to false to disable the scheduled scan."
  type        = bool
  default     = true
}

variable "orphan_detector_staleness_min" {
  description = "Jobs in queued state longer than this threshold (minutes) are considered orphaned. Default 60."
  type        = number
  default     = 60
  validation {
    condition     = var.orphan_detector_staleness_min > 0
    error_message = "orphan_detector_staleness_min must be a positive integer."
  }
}

variable "orphan_detector_circuit_breaker_margin" {
  description = "Circuit breaker fires when orphan count exceeds runner_max_count + this margin. Default 15."
  type        = number
  default     = 15
  validation {
    condition     = var.orphan_detector_circuit_breaker_margin >= 0
    error_message = "orphan_detector_circuit_breaker_margin must be non-negative."
  }
}

variable "orphan_detector_schedule_expression" {
  description = "EventBridge schedule expression for the orphan detector scan interval. Default every 10 minutes."
  type        = string
  default     = "rate(10 minutes)"
}

variable "orphan_detector_alert_email" {
  description = "Email for orphan detector circuit breaker alerts. Defaults to budget_alert_email if empty."
  type        = string
  default     = ""
}
