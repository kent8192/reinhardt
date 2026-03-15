# JIT self-hosted runner infrastructure (multi-runner configuration).
# Source: github-aws-runners/terraform-aws-github-runner//modules/multi-runner
# Manages multiple runner pools (CI + release) with a single webhook endpoint.
# Handles: Webhook -> Lambda -> SQS -> EC2 Spot -> ephemeral runner -> terminate

# Shared runner configuration used by both CI and release runners.
# Avoids duplication of common settings (AMI, EBS, userdata, etc.).
locals {
  shared_runner_config = {
    runner_os           = "linux"
    runner_architecture = "arm64"
    runner_run_as       = "ubuntu"

    instance_target_capacity_type   = "spot"
    instance_allocation_strategy    = "price-capacity-optimized"
    create_service_linked_role_spot = true

    enable_ephemeral_runners = true
    enable_jit_config        = true
    enable_ssm_on_runners    = true
    enable_cloudwatch_agent  = true

    runner_boot_time_in_minutes     = 5
    minimum_running_time_in_minutes = 15
    scale_down_schedule_expression  = "cron(* * * * ? *)"

    ami = {
      id_ssm_parameter_arn = "arn:aws:ssm:${var.aws_region}:${var.aws_account_id}:parameter/${var.prefix}/runner-ami-id"
    }

    block_device_mappings = [{
      device_name           = "/dev/sda1"
      delete_on_termination = true
      volume_size           = var.runner_ebs_size_gb
      volume_type           = "gp3"
      iops                  = 3000
      throughput            = 125
      encrypted             = true
    }]

    userdata_template    = "${path.module}/userdata-ubuntu.sh"
    userdata_pre_install = "systemctl is-active docker || systemctl start docker"

    job_retry = {
      enable           = true
      delay_in_seconds = 120
      max_attempts     = 3
    }
  }
}

module "github_runner" {
  source  = "github-aws-runners/github-runner/aws//modules/multi-runner"
  version = "~> 6.1"

  aws_region = var.aws_region
  vpc_id     = data.aws_vpc.default.id
  subnet_ids = data.aws_subnets.default.ids
  prefix     = var.prefix

  # GitHub App authentication (more secure than PAT)
  github_app = {
    key_base64     = var.github_app_key_base64
    id             = var.github_app_id
    webhook_secret = random_password.webhook_secret.result
  }

  # Pre-built Lambda zip files (downloaded by init.sh from GitHub releases)
  webhook_lambda_zip                = "${path.module}/lambdas/webhook.zip"
  runners_lambda_zip                = "${path.module}/lambdas/runners.zip"
  runner_binaries_syncer_lambda_zip = "${path.module}/lambdas/runner-binaries-syncer.zip"

  # Disable reserved concurrency for scale-up Lambda (-1 = use unreserved pool).
  # New AWS accounts have a low Lambda concurrency limit; reserving concurrency
  # would reduce UnreservedConcurrentExecution below the AWS minimum of 10.
  # Note: multi-runner applies this globally to the shared scale-up Lambda.
  # Per-runner scale_up_reserved_concurrent_executions is set to -1 in each config.

  # Repository scope (not org-wide, for security isolation)
  # Note: enable_organization_runners is set per-runner in multi_runner_config.

  multi_runner_config = {
    # --- CI Runner (existing reinhardt-ci pool) ---
    "ci" = {
      matcherConfig = {
        labelMatchers = [["self-hosted", "linux", "arm64", "reinhardt-ci"]]
        exactMatch    = true
      }
      runner_config = merge(local.shared_runner_config, {
        runner_extra_labels                     = var.runner_extra_labels
        instance_types                          = var.runner_instance_types
        runners_maximum_count                   = var.runner_max_count
        scale_up_reserved_concurrent_executions = -1
        enable_organization_runners             = false
      })
    }

    # --- Release Runner (new reinhardt-release pool) ---
    "release" = {
      matcherConfig = {
        labelMatchers = [["self-hosted", "linux", "arm64", "reinhardt-release"]]
        exactMatch    = true
      }
      runner_config = merge(local.shared_runner_config, {
        runner_extra_labels                     = var.release_runner_extra_labels
        instance_types                          = var.release_runner_instance_types
        runners_maximum_count                   = var.release_runner_max_count
        scale_up_reserved_concurrent_executions = -1
        enable_organization_runners             = false
      })
    }
  }
}

# Supplemental IAM policy: grant ssm:GetParameter (singular) for AMI SSM parameter.
#
# The module grants ssm:GetParameters (plural) automatically, but EC2 CreateFleet
# with `resolve:ssm:` requires ssm:GetParameter (singular) to resolve the AMI ID
# from the SSM parameter. These are different IAM actions.
# See: https://github.com/kent8192/reinhardt-web/issues/2027
#
# In multi-runner mode, each runner config has its own scale-up role.
# Apply the policy to both CI and release runner roles.
resource "aws_iam_role_policy" "scale_up_ssm_get_parameter_ci" {
  name = "ssm-get-parameter-ami-resolve"
  role = module.github_runner.runners_map["ci"].role_scale_up.name

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect   = "Allow"
        Action   = ["ssm:GetParameter"]
        Resource = [aws_ssm_parameter.runner_ami_id.arn]
      }
    ]
  })
}

resource "aws_iam_role_policy" "scale_up_ssm_get_parameter_release" {
  name = "ssm-get-parameter-ami-resolve"
  role = module.github_runner.runners_map["release"].role_scale_up.name

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect   = "Allow"
        Action   = ["ssm:GetParameter"]
        Resource = [aws_ssm_parameter.runner_ami_id.arn]
      }
    ]
  })
}
