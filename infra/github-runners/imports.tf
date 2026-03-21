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

# Golden AMI SSM parameter (see #2023).
# Idempotent: no-op if already managed by this workspace.
import {
  to = aws_ssm_parameter.runner_ami_id
  id = "/${var.prefix}/runner-ami-id"
}

# AMI builder resources (re-integrated after state rm, see #2060).
# Idempotent: no-op if already managed by this workspace.
import {
  to = aws_iam_openid_connect_provider.github_actions
  id = "arn:aws:iam::${var.aws_account_id}:oidc-provider/token.actions.githubusercontent.com"
}

import {
  to = aws_iam_role.github_actions_ami_builder
  id = "${var.prefix}-gha-ami-builder"
}

import {
  to = aws_iam_role_policy.ami_builder_ec2
  id = "${var.prefix}-gha-ami-builder:packer-ec2-ami-build"
}

import {
  to = aws_iam_role_policy.ami_builder_ssm
  id = "${var.prefix}-gha-ami-builder:ssm-put-parameter-ami"
}

import {
  to = github_actions_secret.aws_role_arn
  id = "${var.github_repository}:AWS_ROLE_ARN"
}
