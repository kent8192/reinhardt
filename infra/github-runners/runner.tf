# JIT self-hosted runner infrastructure.
# Source: github-aws-runners/terraform-aws-github-runner (migrated from philips-labs Jan 2025)
# Handles: Webhook -> Lambda -> SQS -> EC2 Spot -> ephemeral runner -> terminate
module "github_runner" {
	source  = "github-aws-runners/github-runner/aws"
	version = "~> 6.1"

	aws_region = var.aws_region

	# Use default VPC for simplicity
	vpc_id     = data.aws_vpc.default.id
	subnet_ids = data.aws_subnets.default.ids

	prefix = var.prefix

	# GitHub App authentication (more secure than PAT)
	github_app = {
		key_base64     = var.github_app_key_base64
		id             = var.github_app_id
		webhook_secret = random_password.webhook_secret.result
	}

	# Runner OS / architecture
	runner_os           = "linux"
	runner_architecture = "x64"

	# Ubuntu AMI uses 'ubuntu' user (module defaults to 'ec2-user' for Amazon Linux)
	runner_run_as = "ubuntu"

	# Ubuntu 22.04 LTS (Jammy) AMI - Canonical official images in us-east-1
	# Name pattern: ubuntu-jammy-22.04 (NOT ubuntu-22.04 which doesn't exist)
	ami = {
		filter = {
			name  = ["ubuntu/images/hvm-ssd/ubuntu-jammy-22.04-amd64-server-*"]
			state = ["available"]
		}
		owners = ["099720109477"] # Canonical
	}

	# Spot fleet with fallback instance types for availability
	instance_types                = var.runner_instance_types
	instance_target_capacity_type = "spot"
	instance_allocation_strategy  = "price-capacity-optimized"

	# Create EC2 Spot service-linked role if it does not already exist in the account
	create_service_linked_role_spot = true

	# Custom labels for runs-on matching in workflows
	runner_extra_labels = var.runner_extra_labels

	# Scale-to-zero: no idle runners = $0 when no CI runs
	runners_maximum_count = var.runner_max_count

	# Ephemeral: each runner executes exactly ONE job, then auto-terminates
	enable_ephemeral_runners = true

	# JIT config: short-lived token (auto-enabled for ephemeral; explicit for clarity)
	enable_jit_config = true

	# 200GB gp3 EBS: no need for free-disk-space cleanup step
	# Ubuntu AMI root device is /dev/sda1 (not /dev/xvda used by Amazon Linux)
	block_device_mappings = [{
		device_name           = "/dev/sda1"
		delete_on_termination = true
		volume_size           = var.runner_ebs_size_gb
		volume_type           = "gp3"
		iops                  = 3000 # gp3 baseline (included at no extra cost)
		throughput            = 125  # MB/s (included at no extra cost)
		encrypted             = true
	}]

	# Pre-built Lambda zip files (downloaded by init.sh from GitHub releases).
	# The module cannot build Node.js Lambdas at plan time; pre-built zips are required.
	webhook_lambda_zip                = "${path.module}/lambdas/webhook.zip"
	runners_lambda_zip                = "${path.module}/lambdas/runners.zip"
	runner_binaries_syncer_lambda_zip = "${path.module}/lambdas/runner-binaries-syncer.zip"

	# Custom Ubuntu userdata template (module default is Amazon Linux / dnf-based).
	# This template handles: AWS CLI, CloudWatch agent, runner install, and start.
	userdata_template = "${path.module}/userdata-ubuntu.sh"

	# Pre-installation script: Docker, Rust build tools, TestContainers images
	userdata_pre_install = file("${path.module}/runner-userdata.sh")

	# Repository scope (not org-wide, for security isolation)
	enable_organization_runners = false

	# CloudWatch Logs for debugging (ephemeral runners delete themselves)
	enable_cloudwatch_agent = true

	# SSM access for shell debugging (no SSH port needed)
	enable_ssm_on_runners = true

	# Scale-down: check every minute, terminate idle runners after grace period.
	# JIT runners need time to pick up jobs after registration; without a grace
	# period, scale-down terminates them before they become busy (race condition).
	scale_down_schedule_expression  = "cron(* * * * ? *)"
	minimum_running_time_in_minutes = 15
	runner_boot_time_in_minutes     = 10 # Allow 10 min for cold start

	# Disable reserved concurrency for scale-up Lambda (-1 = use unreserved pool).
	# New AWS accounts have a low Lambda concurrency limit; reserving concurrency
	# would reduce UnreservedConcurrentExecution below the AWS minimum of 10.
	scale_up_reserved_concurrent_executions = -1

	# Job retry: re-queue jobs that were not picked up by ephemeral runners.
	# Without this, queued jobs can deadlock when the initial SQS message is
	# consumed but the runner terminates before the job starts (e.g. when
	# runners_maximum_count is reached or spot interruption occurs).
	job_retry = {
		enable           = true
		delay_in_seconds = 120
		max_attempts     = 3
	}
}
