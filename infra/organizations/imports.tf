# Always-on import block: brings manually-created Organizations account under Terraform management.
# Pattern: same as infra/website/imports.tf (idempotent - safe to keep even after initial apply).
#
# USAGE:
#   New account:      Leave existing_account_id as "" in terraform.tfvars, run `terraform apply` to create.
#                     After creation, fill in the account ID below for future drift detection.
#   Existing account: Set existing_account_id to the existing account ID, then run `terraform init && terraform apply`.
#
# Get account ID: aws organizations list-accounts --query "Accounts[?Name=='reinhardt-ci-runners'].Id"
#
# NOTE: When existing_account_id is empty (""), skip this file or comment out the import block below.
#       Terraform will create a new account via the resource block in main.tf.

# Uncomment and set id when importing an existing account:
# import {
#   to = aws_organizations_account.ci_runners
#   id = "123456789012"  # Replace with actual account ID
# }
