# Copy to terraform.tfvars and fill in values.
# NEVER commit terraform.tfvars (it is gitignored).
#
# Values marked <like-this> MUST be replaced before running terraform apply.
# Pre-filled values are sensible defaults and can be changed if needed.
#
# SETUP (in order):
#   1. cp terraform.examples.tfvars terraform.tfvars
#   2. Fill in all <placeholder> values below
#   3. ./init.sh   <- auto-generates backend.tfvars and runs terraform init
#   4. terraform plan && terraform apply

aws_region     = "us-east-1"
aws_account_id = "<aws-account-id>"  # aws sts get-caller-identity --query Account --output text

# GitHub App credentials (created in GitHub App setup step)
github_app_id              = "<github-app-id>"           # Settings > Developer settings > GitHub Apps > About
github_app_installation_id = "<installation-id>"          # URL path after installing: /settings/installations/<id>
github_app_key_base64      = "<base64-encoded-pem-key>"  # cat key.pem | base64 | tr -d '\n'

# GitHub repository
github_owner      = "<github-username>"
github_repository = "reinhardt-web"

# Runner configuration (defaults tuned for reinhardt CI workload)
runner_instance_types = ["c6a.2xlarge", "c6i.2xlarge", "c5a.2xlarge"]
runner_max_count      = 30
runner_extra_labels   = ["reinhardt-ci"]
runner_ebs_size_gb    = 200

# Budget circuit breaker
monthly_budget_limit_usd = "100"
budget_alert_email       = "<your-email@example.com>"

prefix = "reinhardt-ci"
