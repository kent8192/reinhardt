# Copy to terraform.tfvars and fill in values.
# NEVER commit terraform.tfvars (it is gitignored).
#
# Values marked <like-this> MUST be replaced before running terraform apply.
# Pre-filled values are sensible defaults and can be changed if needed.
#
# SETUP:
#   aws sts get-caller-identity --query Account --output text  # Get account ID
#   cp terraform.examples.tfvars terraform.tfvars
#   # Fill in aws_account_id below
#   terraform init && terraform plan && terraform apply

aws_region     = "us-east-1"
aws_account_id = "<aws-account-id>"  # 12-digit account ID: aws sts get-caller-identity --query Account --output text
