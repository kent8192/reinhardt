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

# --- Terraform Plan CI Secrets ---
resource "github_actions_secret" "tf_plan_aws_access_key_id" {
  repository      = var.github_repository
  secret_name     = "TF_AWS_ACCESS_KEY_ID"
  plaintext_value = var.tf_plan_aws_access_key_id

  lifecycle {
    ignore_changes = [plaintext_value]
  }
}

resource "github_actions_secret" "tf_plan_aws_secret_access_key" {
  repository      = var.github_repository
  secret_name     = "TF_AWS_SECRET_ACCESS_KEY"
  plaintext_value = var.tf_plan_aws_secret_access_key

  lifecycle {
    ignore_changes = [plaintext_value]
  }
}

resource "github_actions_secret" "tf_plan_aws_region" {
  repository      = var.github_repository
  secret_name     = "TF_AWS_REGION"
  plaintext_value = var.aws_region

  lifecycle {
    ignore_changes = [plaintext_value]
  }
}

resource "github_actions_secret" "tf_plan_state_bucket" {
  repository      = var.github_repository
  secret_name     = "TF_STATE_BUCKET"
  plaintext_value = "reinhardt-ci-terraform-state-${var.aws_account_id}"

  lifecycle {
    ignore_changes = [plaintext_value]
  }
}

resource "github_actions_secret" "tf_plan_github_app_id" {
  repository      = var.github_repository
  secret_name     = "TF_GITHUB_APP_ID"
  plaintext_value = var.github_app_id

  lifecycle {
    ignore_changes = [plaintext_value]
  }
}

resource "github_actions_secret" "tf_plan_github_app_installation_id" {
  repository      = var.github_repository
  secret_name     = "TF_GITHUB_APP_INSTALLATION_ID"
  plaintext_value = var.github_app_installation_id

  lifecycle {
    ignore_changes = [plaintext_value]
  }
}

resource "github_actions_secret" "tf_plan_github_app_key_base64" {
  repository      = var.github_repository
  secret_name     = "TF_GITHUB_APP_KEY_BASE64"
  plaintext_value = var.github_app_key_base64

  lifecycle {
    ignore_changes = [plaintext_value]
  }
}
