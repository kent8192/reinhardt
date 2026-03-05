variable "aws_region" {
	description = "AWS region"
	type        = string
	default     = "us-east-1"
}

variable "aws_account_id" {
	description = "AWS Account ID for the CI sub-account. Used to construct the globally unique S3 state bucket name (reinhardt-ci-terraform-state-<account_id>)."
	type        = string
	# Retrieve with: aws sts get-caller-identity --query Account --output text
}
