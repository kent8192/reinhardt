# Repository variable: controls whether self-hosted runners are enabled.
# Set to "false" manually via GitHub repo Settings > Variables when monthly budget is exceeded.
# Reset to "true" at the start of each new month if budget was exceeded.
resource "github_actions_variable" "self_hosted_enabled" {
  repository    = var.github_repository
  variable_name = "SELF_HOSTED_ENABLED"
  value         = "true"

  lifecycle {
    # Do not override if manually set to "false" for budget control
    ignore_changes = [value]
  }
}

# GitHub Actions secret: AWS IAM role ARN for OIDC authentication
# Used by build-runner-ami.yml workflow to assume the AMI builder role
resource "github_actions_secret" "aws_role_arn" {
  repository      = var.github_repository
  secret_name     = "AWS_ROLE_ARN"
  plaintext_value = aws_iam_role.github_actions_ami_builder.arn
}
