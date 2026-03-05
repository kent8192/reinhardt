#!/bin/bash -e
exec > >(tee /var/log/user-data.log | logger -t user-data -s 2>/dev/console) 2>&1

# Disable command tracing by default (tokens would be leaked)
set +x

%{ if enable_debug_logging }
set -x
%{ endif }

${pre_install}

# Install system packages required for CI builds and runner dependencies
apt-get -q update
DEBIAN_FRONTEND=noninteractive apt-get install -q -y \
	build-essential \
	ca-certificates \
	curl \
	git \
	jq \
	unzip \
	wget

# Install AWS CLI v2 (required for S3 runner binary download and SSM)
curl -fsSL -o "awscliv2.zip" "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip"
unzip -q awscliv2.zip
aws/install
rm -rf aws awscliv2.zip

# Install and configure CloudWatch agent for log shipping
curl -fsSL -o "/tmp/amazon-cloudwatch-agent.deb" https://s3.amazonaws.com/amazoncloudwatch-agent/ubuntu/amd64/latest/amazon-cloudwatch-agent.deb
dpkg -i -E /tmp/amazon-cloudwatch-agent.deb
rm -f /tmp/amazon-cloudwatch-agent.deb
amazon-cloudwatch-agent-ctl -a fetch-config -m ec2 -s -c "ssm:${ssm_key_cloudwatch_agent_config}"

${install_runner}

${post_install}

cd /opt/actions-runner

%{ if hook_job_started != "" }
cat > /opt/actions-runner/hook_job_started.sh <<'EOF'
${hook_job_started}
EOF
echo ACTIONS_RUNNER_HOOK_JOB_STARTED=/opt/actions-runner/hook_job_started.sh | tee -a /opt/actions-runner/.env
%{ endif }

%{ if hook_job_completed != "" }
cat > /opt/actions-runner/hook_job_completed.sh <<'EOF'
${hook_job_completed}
EOF
echo ACTIONS_RUNNER_HOOK_JOB_COMPLETED=/opt/actions-runner/hook_job_completed.sh | tee -a /opt/actions-runner/.env
%{ endif }

${start_runner}
