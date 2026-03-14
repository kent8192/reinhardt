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

# --- Terraform Plan CI Secrets ---
resource "github_actions_secret" "tf_plan_cloudflare_api_token" {
  repository      = var.github_repository
  secret_name     = "TF_CLOUDFLARE_API_TOKEN"
  plaintext_value = var.cloudflare_api_token

  lifecycle {
    ignore_changes = [plaintext_value]
  }
}

resource "github_actions_secret" "tf_plan_github_token" {
  repository      = var.github_repository
  secret_name     = "TF_GITHUB_TOKEN"
  plaintext_value = var.github_token

  lifecycle {
    ignore_changes = [plaintext_value]
  }
}

resource "github_actions_secret" "tf_plan_cloudflare_account_id" {
  repository      = var.github_repository
  secret_name     = "TF_CLOUDFLARE_ACCOUNT_ID"
  plaintext_value = var.cloudflare_account_id

  lifecycle {
    ignore_changes = [plaintext_value]
  }
}

resource "github_actions_secret" "tf_plan_pages_project_name" {
  repository      = var.github_repository
  secret_name     = "TF_PAGES_PROJECT_NAME"
  plaintext_value = var.pages_project_name

  lifecycle {
    ignore_changes = [plaintext_value]
  }
}

resource "github_actions_secret" "tf_plan_custom_domain" {
  repository      = var.github_repository
  secret_name     = "TF_CUSTOM_DOMAIN"
  plaintext_value = var.custom_domain

  lifecycle {
    ignore_changes = [plaintext_value]
  }
}

resource "github_actions_secret" "tf_plan_google_site_verification" {
  repository      = var.github_repository
  secret_name     = "TF_GOOGLE_SITE_VERIFICATION"
  plaintext_value = var.google_site_verification

  lifecycle {
    ignore_changes = [plaintext_value]
  }
}

resource "github_actions_secret" "tf_plan_dns_record_apex_id" {
  repository      = var.github_repository
  secret_name     = "TF_DNS_RECORD_APEX_ID"
  plaintext_value = var.dns_record_apex_id

  lifecycle {
    ignore_changes = [plaintext_value]
  }
}

resource "github_actions_secret" "tf_plan_dns_record_www_id" {
  repository      = var.github_repository
  secret_name     = "TF_DNS_RECORD_WWW_ID"
  plaintext_value = var.dns_record_www_id

  lifecycle {
    ignore_changes = [plaintext_value]
  }
}

resource "github_actions_secret" "tf_plan_dns_record_google_verification_id" {
  repository      = var.github_repository
  secret_name     = "TF_DNS_RECORD_GOOGLE_VERIFICATION_ID"
  plaintext_value = var.dns_record_google_verification_id

  lifecycle {
    ignore_changes = [plaintext_value]
  }
}
