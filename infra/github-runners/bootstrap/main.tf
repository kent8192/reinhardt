# Bootstrap: Creates S3 state bucket for the main github-runners Terraform config.
# Uses LOCAL state (run once, state file stored locally in this directory).
# After applying, use the outputs to configure backend.tfvars in the parent directory.

terraform {
	required_version = ">= 1.10"

	required_providers {
		aws = {
			source  = "hashicorp/aws"
			version = "~> 5.0"
		}
	}
	# Intentionally local backend: this IS the bootstrap for remote state
}

provider "aws" {
	region = var.aws_region

	default_tags {
		tags = {
			Project   = "reinhardt"
			ManagedBy = "terraform"
			Component = "github-runners-bootstrap"
		}
	}
}

locals {
	state_bucket_name = "reinhardt-ci-terraform-state-${var.aws_account_id}"
}

# S3 bucket for Terraform state (private, versioned, encrypted)
resource "aws_s3_bucket" "terraform_state" {
	bucket = local.state_bucket_name

	lifecycle {
		prevent_destroy = true
	}
}

resource "aws_s3_bucket_versioning" "terraform_state" {
	bucket = aws_s3_bucket.terraform_state.id

	versioning_configuration {
		status = "Enabled"
	}
}

resource "aws_s3_bucket_server_side_encryption_configuration" "terraform_state" {
	bucket = aws_s3_bucket.terraform_state.id

	rule {
		apply_server_side_encryption_by_default {
			sse_algorithm = "AES256"
		}
	}
}

resource "aws_s3_bucket_public_access_block" "terraform_state" {
	bucket = aws_s3_bucket.terraform_state.id

	block_public_acls       = true
	block_public_policy     = true
	ignore_public_acls      = true
	restrict_public_buckets = true
}

# Note: Terraform >= 1.10 supports native S3 locking (no DynamoDB needed).
# The main module uses use_lockfile = true in the backend config.
