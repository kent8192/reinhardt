#!/bin/bash
# init.sh: Generate backend.tfvars from terraform.tfvars, initialize Terraform,
#          and download pre-built Lambda zip files from GitHub releases.
#
# Usage:
#   cd infra/github-runners
#   cp terraform.examples.tfvars terraform.tfvars
#   # Edit terraform.tfvars (set aws_account_id, github_app_id, etc.)
#   ./init.sh
#
# What this script does:
#   1. Reads aws_account_id and aws_region from terraform.tfvars
#   2. Generates backend.tfvars with the correct S3 bucket name
#   3. Runs terraform init -backend-config=backend.tfvars
#   4. Detects the resolved module version from .terraform/modules/modules.json
#   5. Downloads pre-built Lambda zip files from GitHub releases into ./lambdas/

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

TFVARS_FILE="terraform.tfvars"
BACKEND_FILE="backend.tfvars"
STATE_KEY="github-runners/terraform.tfstate"
LAMBDA_DIR="lambdas"

# Verify terraform.tfvars exists
if [[ ! -f "$TFVARS_FILE" ]]; then
  echo "ERROR: $TFVARS_FILE not found."
  echo "       Copy terraform.examples.tfvars to terraform.tfvars and fill in the values."
  exit 1
fi

# Extract aws_account_id (required - no default)
ACCOUNT_ID=$(awk -F'"' '/^[[:space:]]*aws_account_id[[:space:]]*=/ {print $2}' "$TFVARS_FILE")
if [[ -z "$ACCOUNT_ID" ]]; then
  echo "ERROR: aws_account_id is not set in $TFVARS_FILE"
  exit 1
fi

# Extract aws_region (optional - default us-east-1)
REGION=$(awk -F'"' '/^[[:space:]]*aws_region[[:space:]]*=/ {print $2}' "$TFVARS_FILE")
REGION="${REGION:-us-east-1}"

BUCKET_NAME="reinhardt-ci-terraform-state-${ACCOUNT_ID}"

# Generate backend.tfvars
# The S3 state bucket lives in the CI sub-account, so the backend must also
# assume the OrganizationAccountAccessRole to access it.
ROLE_ARN="arn:aws:iam::${ACCOUNT_ID}:role/OrganizationAccountAccessRole"

cat > "$BACKEND_FILE" <<EOF
bucket       = "${BUCKET_NAME}"
key          = "${STATE_KEY}"
region       = "${REGION}"
use_lockfile = true
EOF

echo "Generated ${BACKEND_FILE}:"
echo "  bucket       = \"${BUCKET_NAME}\""
echo "  key          = \"${STATE_KEY}\""
echo "  region       = \"${REGION}\""
echo "  use_lockfile = true"
echo "  assume_role  = ${ROLE_ARN}"
echo ""

# Run terraform init with generated backend config.
# The assume_role block is passed via CLI because .tfvars files do not support
# nested blocks. This allows the S3 backend to access the sub-account bucket.
terraform init \
  -backend-config="$BACKEND_FILE" \
  -backend-config="assume_role={role_arn=\"${ROLE_ARN}\"}"
echo ""

# --- Download pre-built Lambda zip files ---
# Detect module version from .terraform/modules/modules.json (populated by terraform init)
MODULES_JSON=".terraform/modules/modules.json"
if [[ ! -f "$MODULES_JSON" ]]; then
  echo "ERROR: $MODULES_JSON not found. Run terraform init first."
  exit 1
fi

MODULE_VERSION=$(python3 -c "
import json, sys
data = json.load(open('$MODULES_JSON'))
for m in data.get('Modules', []):
    if m.get('Key') == 'github_runner':
        print(m['Version'])
        sys.exit(0)
sys.exit(1)
" 2>/dev/null || echo "")

if [[ -z "$MODULE_VERSION" ]]; then
  echo "ERROR: Could not detect module version from $MODULES_JSON"
  exit 1
fi

echo "Downloading Lambda zip files for module version v${MODULE_VERSION}..."
mkdir -p "$LAMBDA_DIR"

BASE_URL="https://github.com/github-aws-runners/terraform-aws-github-runner/releases/download/v${MODULE_VERSION}"

# Version tracking file to detect version changes across runs
VERSION_FILE="${LAMBDA_DIR}/.lambda-version"
CURRENT_TRACKED_VERSION=""
if [[ -f "$VERSION_FILE" ]]; then
  CURRENT_TRACKED_VERSION=$(cat "$VERSION_FILE")
fi

# If module version changed, re-download all Lambda zips
if [[ "$CURRENT_TRACKED_VERSION" != "$MODULE_VERSION" && -n "$CURRENT_TRACKED_VERSION" ]]; then
  echo "  [version-change] Lambda version changed: v${CURRENT_TRACKED_VERSION} -> v${MODULE_VERSION}"
  echo "  [cleanup] Removing outdated Lambda zip files..."
  for zip_name in webhook runners runner-binaries-syncer; do
    rm -f "${LAMBDA_DIR}/${zip_name}.zip"
  done
fi

# Download each required Lambda zip (skip if already downloaded for this version)
for LAMBDA_NAME in webhook runners runner-binaries-syncer; do
  DEST="${LAMBDA_DIR}/${LAMBDA_NAME}.zip"
  # Check if the file exists and is non-empty
  if [[ -f "$DEST" && -s "$DEST" ]]; then
    echo "  [skip] ${LAMBDA_NAME}.zip already exists (v${MODULE_VERSION})"
  else
    echo "  [download] ${LAMBDA_NAME}.zip from v${MODULE_VERSION}..."
    curl -fL --progress-bar -o "$DEST" "${BASE_URL}/${LAMBDA_NAME}.zip"
    echo "  [ok] ${LAMBDA_NAME}.zip ($(du -sh "$DEST" | cut -f1))"
  fi
done

# Record the current version for future runs
echo "$MODULE_VERSION" > "$VERSION_FILE"

echo ""
echo "All Lambda zip files are ready in ./${LAMBDA_DIR}/"
echo ""
echo "Next step: terraform plan"
