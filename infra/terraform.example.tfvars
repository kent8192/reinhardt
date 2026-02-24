# --- Sensitive values (NEVER commit terraform.tfvars) ---
cloudflare_api_token  = ""
cloudflare_account_id = ""
github_token          = ""

# --- Non-sensitive values ---
github_owner       = "kent8192"
github_repository  = "reinhardt-web"
pages_project_name = "reinhardt-web"
custom_domain      = "reinhardt-web.dev"
production_branch  = "main"

# --- Import IDs (Cloudflare DNS record IDs for resource import) ---
dns_record_apex_id = ""
dns_record_www_id  = ""
