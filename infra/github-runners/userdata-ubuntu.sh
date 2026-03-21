#!/bin/bash -e
exec > >(tee /var/log/user-data.log | logger -t user-data -s 2>/dev/console) 2>&1

# Disable command tracing by default (tokens would be leaked)
set +x

%{ if enable_debug_logging }
set -x
%{ endif }

${pre_install}

# All system packages, AWS CLI v2, and CloudWatch agent are pre-installed
# in the Golden AMI (built by Packer). Only CloudWatch config is fetched here.
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
