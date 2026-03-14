#!/bin/bash
# User data for the housekeeping runner (t4g.nano, always-on).
# First boot: registers as a GitHub Actions runner using GitHub App credentials.
# Subsequent boots: systemd restarts the runner service automatically.
set -euo pipefail
exec > >(tee /var/log/user-data.log | logger -t user-data) 2>&1

RUNNER_DIR="/opt/actions-runner"
RUNNER_USER="ubuntu"

# --- Reboot path: runner already configured, systemd handles restart ---
if [ -f "$RUNNER_DIR/.runner" ]; then
  echo "Runner already configured, systemd service will handle startup"
  exit 0
fi

# --- First boot: install and register runner ---

apt-get update -y
apt-get install -y jq curl openssl unzip

# Install AWS CLI v2 (arm64) - not included in base Ubuntu AMI
curl -sL "https://awscli.amazonaws.com/awscli-exe-linux-aarch64.zip" -o /tmp/awscliv2.zip
unzip -q /tmp/awscliv2.zip -d /tmp
/tmp/aws/install
rm -rf /tmp/awscliv2.zip /tmp/aws

# Fetch GitHub App credentials from SSM
APP_ID=$(aws ssm get-parameter \
  --region "${aws_region}" \
  --name "/${prefix}/housekeeping/github-app-id" \
  --query 'Parameter.Value' --output text)

APP_KEY=$(aws ssm get-parameter \
  --region "${aws_region}" \
  --name "/${prefix}/housekeeping/github-app-key" \
  --with-decryption \
  --query 'Parameter.Value' --output text)

INSTALLATION_ID=$(aws ssm get-parameter \
  --region "${aws_region}" \
  --name "/${prefix}/housekeeping/github-app-installation-id" \
  --query 'Parameter.Value' --output text)

# Generate JWT from GitHub App private key (valid for 10 minutes)
NOW=$(date +%s)
IAT=$((NOW - 60))
EXP=$((NOW + 300))

HEADER=$(echo -n '{"alg":"RS256","typ":"JWT"}' | openssl base64 -e -A | tr '+/' '-_' | tr -d '=')
PAYLOAD=$(echo -n "{\"iat\":$IAT,\"exp\":$EXP,\"iss\":\"$APP_ID\"}" | openssl base64 -e -A | tr '+/' '-_' | tr -d '=')
SIGNATURE=$(echo -n "$HEADER.$PAYLOAD" | openssl dgst -sha256 -sign <(echo "$APP_KEY") | openssl base64 -e -A | tr '+/' '-_' | tr -d '=')
JWT="$HEADER.$PAYLOAD.$SIGNATURE"

# Get installation access token
INSTALLATION_TOKEN=$(curl -sf -X POST \
  -H "Authorization: Bearer $JWT" \
  -H "Accept: application/vnd.github+json" \
  "https://api.github.com/app/installations/$INSTALLATION_ID/access_tokens" | jq -r '.token')

# Get runner registration token
REG_TOKEN=$(curl -sf -X POST \
  -H "Authorization: token $INSTALLATION_TOKEN" \
  -H "Accept: application/vnd.github+json" \
  "https://api.github.com/repos/${github_owner}/${github_repository}/actions/runners/registration-token" | jq -r '.token')

# Download latest GitHub Actions Runner (arm64)
mkdir -p "$RUNNER_DIR"
cd "$RUNNER_DIR"

RUNNER_VERSION=$(curl -sf https://api.github.com/repos/actions/runner/releases/latest | jq -r '.tag_name' | sed 's/^v//')
curl -sL "https://github.com/actions/runner/releases/download/v$RUNNER_VERSION/actions-runner-linux-arm64-$RUNNER_VERSION.tar.gz" | tar xz
chown -R "$RUNNER_USER:$RUNNER_USER" "$RUNNER_DIR"

# Configure runner (non-interactive, replace if name already registered)
sudo -u "$RUNNER_USER" ./config.sh \
  --url "https://github.com/${github_owner}/${github_repository}" \
  --token "$REG_TOKEN" \
  --name "housekeeping-runner" \
  --labels "self-hosted,linux,arm64,${runner_labels}" \
  --unattended \
  --replace

# Install as systemd service and start
./svc.sh install "$RUNNER_USER"
./svc.sh start

echo "Housekeeping runner registered and started successfully"
