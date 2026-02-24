resource "github_actions_secret" "cloudflare_api_token" {
  repository      = var.github_repository
  secret_name     = "CLOUDFLARE_API_TOKEN"
  plaintext_value = var.cloudflare_api_token

  lifecycle {
    ignore_changes = [plaintext_value]
  }
}

resource "github_actions_secret" "cloudflare_account_id" {
  repository      = var.github_repository
  secret_name     = "CLOUDFLARE_ACCOUNT_ID"
  plaintext_value = var.cloudflare_account_id

  lifecycle {
    ignore_changes = [plaintext_value]
  }
}
