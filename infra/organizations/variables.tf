variable "aws_region" {
	description = "AWS region for the Organizations API (us-east-1 is always available)"
	type        = string
	default     = "us-east-1"
}

variable "account_name" {
	description = "Display name for the CI sub-account in AWS Organizations"
	type        = string
	default     = "reinhardt-ci-runners"
}

variable "account_email" {
	description = "Unique email address for the CI sub-account (use + alias if needed)"
	type        = string
	# Example: "ci-runners+reinhardt@yourdomain.com"
}

variable "existing_account_id" {
	description = "Existing AWS account ID to import (leave empty for new account creation)"
	type        = string
	default     = "" # Set to account ID (e.g. "123456789012") if importing existing account
}
