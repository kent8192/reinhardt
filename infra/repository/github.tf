# --- Terraform Plan CI Secrets ---

resource "github_actions_secret" "tf_plan_environment_reviewer" {
  repository      = var.repository_name
  secret_name     = "TF_ENVIRONMENT_REVIEWER"
  plaintext_value = var.environment_reviewer

  lifecycle {
    ignore_changes = [plaintext_value]
  }
}

resource "github_actions_secret" "tf_plan_account_email" {
  repository      = var.repository_name
  secret_name     = "TF_ACCOUNT_EMAIL"
  plaintext_value = var.account_email

  lifecycle {
    ignore_changes = [plaintext_value]
  }
}
