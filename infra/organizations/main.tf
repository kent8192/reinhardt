# Manages the dedicated CI sub-account in AWS Organizations.
# Run with master account credentials (the account that owns the Organization).
# After apply, use outputs.account_id as the target account for all github-runners resources.

terraform {
	required_version = ">= 1.10"

	required_providers {
		aws = {
			source  = "hashicorp/aws"
			version = "~> 5.0"
		}
	}
	# Local state: Organizations account management is low-change, local state is sufficient.
	# If you prefer S3 backend, create a separate bootstrap bucket in the master account.
}

provider "aws" {
	region = var.aws_region

	default_tags {
		tags = {
			Project   = "reinhardt"
			ManagedBy = "terraform"
			Component = "organizations"
		}
	}
}

# CI-dedicated sub-account for isolated billing.
# Billing is automatically separated in AWS Organizations Cost Explorer.
resource "aws_organizations_account" "ci_runners" {
	name      = var.account_name
	email     = var.account_email
	role_name = "OrganizationAccountAccessRole"

	# Prevent accidental destruction of the account (accounts can only be closed, not deleted)
	lifecycle {
		prevent_destroy = true
	}
}
