# Always-on import blocks: bring existing GitHub resources under Terraform management.
# Pattern: same as infra/website/imports.tf (idempotent - safe to keep permanently).
# These blocks are no-ops if the resources were created by Terraform in this workspace.
#
# GitHub Actions variable "SELF_HOSTED_ENABLED":
#   If the variable was manually created in GitHub repo settings before Terraform,
#   this import block prevents "already exists" errors on first apply.
#
# Format: "{owner}/{repository}/{variable_name}"
#
# NOTE: Uncomment only if SELF_HOSTED_ENABLED already exists in GitHub repo settings.
#       If Terraform creates it fresh, leave this commented out.

# import {
#   to = github_actions_variable.self_hosted_enabled
#   id = "${var.github_owner}/${var.github_repository}/SELF_HOSTED_ENABLED"
# }
