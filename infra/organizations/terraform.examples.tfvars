# Copy to terraform.tfvars and fill in values.
# NEVER commit terraform.tfvars (it is gitignored).
#
# Values marked <like-this> MUST be replaced before running terraform apply.
# Pre-filled values are sensible defaults and can be changed if needed.

aws_region    = "us-east-1"
account_name  = "reinhardt-ci-runners"
account_email = "<your-email+reinhardt-ci@example.com>"  # Must be unique across all AWS accounts

# Set to existing account ID if importing an already-created sub-account.
# Leave empty ("") to create a new sub-account via terraform apply.
existing_account_id = ""
