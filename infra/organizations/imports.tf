# Always-on import block: brings manually-created Organizations account under Terraform management.
# Pattern: same as infra/website/imports.tf (idempotent - safe to keep even after initial apply).
#
# USAGE:
#   New account:      Run `terraform apply` to create a new sub-account.
#   Existing account: Uncomment the import block below with the actual account ID,
#                     then run `terraform init && terraform apply`.
#
# Get account ID: aws organizations list-accounts --query "Accounts[?Name=='reinhardt-ci-runners'].Id"

# Uncomment and set id when importing an existing account:
# import {
#   to = aws_organizations_account.ci_runners
#   id = "123456789012"  # Replace with actual account ID
# }
