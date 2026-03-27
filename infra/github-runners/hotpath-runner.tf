# Hotpath runner: Spot ASG (min=1, max=2, desired=1) with capacity rebalancing.
#
# Runs lightweight CI control jobs that require immediate execution without
# queue delay: determine-runner, cancel-on-pr-close, cleanup-release-branch.
#
# Architecture:
#   - Launch Template defines the instance spec (t4g.micro Spot, base Ubuntu AMI)
#   - ASG maintains exactly 1 running instance at all times
#   - capacity_rebalance proactively launches a replacement before Spot interruption,
#     so the switchover happens with near-zero downtime
#   - max_size=2 allows temporary over-provisioning during rebalancing
#   - Multiple instance type overrides (t4g.micro, t4g.small) improve Spot availability
#
# Cost: ~$2.83/month (t4g.micro Spot $2.19 + 8GB gp3 EBS $0.64)
# Recovery: Spot rebalance recommendation → new instance launched while old still
#           runs → config.sh --replace switches runner registration → old terminated.

# --- SSM Parameters: GitHub App credentials for runner registration ---
# The runner needs these at first boot to generate a registration token.
# Subsequent boots reuse the persisted runner config (no re-registration).

resource "aws_ssm_parameter" "hotpath_runner_github_app_id" {
  count = var.enable_hotpath_runner ? 1 : 0
  name  = "/${var.prefix}/hotpath-runner/github-app-id"
  type  = "String"
  value = var.github_app_id

  tags = {
    Description = "GitHub App ID for hotpath runner registration"
  }

  lifecycle {
    ignore_changes = [value]
  }
}

resource "aws_ssm_parameter" "hotpath_runner_github_app_key" {
  count = var.enable_hotpath_runner ? 1 : 0
  name  = "/${var.prefix}/hotpath-runner/github-app-key"
  type  = "SecureString"
  value = base64decode(var.github_app_key_base64)

  tags = {
    Description = "GitHub App private key for hotpath runner registration"
  }

  lifecycle {
    ignore_changes = [value]
  }
}

resource "aws_ssm_parameter" "hotpath_runner_github_app_installation_id" {
  count = var.enable_hotpath_runner ? 1 : 0
  name  = "/${var.prefix}/hotpath-runner/github-app-installation-id"
  type  = "String"
  value = var.github_app_installation_id

  tags = {
    Description = "GitHub App installation ID for hotpath runner registration"
  }

  lifecycle {
    ignore_changes = [value]
  }
}

# --- Security Group: egress only ---

resource "aws_security_group" "hotpath_runner" {
  count       = var.enable_hotpath_runner ? 1 : 0
  name        = "${var.prefix}-hotpath-runner"
  description = "Hotpath runner - egress only (HTTPS to GitHub, HTTP for apt)"
  vpc_id      = data.aws_vpc.default.id

  # HTTPS: GitHub API, runner downloads, SSM endpoints
  egress {
    from_port   = 443
    to_port     = 443
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
    description = "HTTPS (GitHub API, SSM, runner downloads)"
  }

  # HTTP: apt package repositories
  egress {
    from_port   = 80
    to_port     = 80
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
    description = "HTTP (apt repositories)"
  }

  # DNS
  egress {
    from_port   = 53
    to_port     = 53
    protocol    = "udp"
    cidr_blocks = ["0.0.0.0/0"]
    description = "DNS resolution"
  }
  egress {
    from_port   = 53
    to_port     = 53
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
    description = "DNS resolution (TCP fallback)"
  }

  tags = {
    Name = "${var.prefix}-hotpath-runner"
  }
}

# --- IAM Role ---

resource "aws_iam_role" "hotpath_runner" {
  count = var.enable_hotpath_runner ? 1 : 0
  name  = "${var.prefix}-hotpath-runner"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect    = "Allow"
      Principal = { Service = "ec2.amazonaws.com" }
      Action    = "sts:AssumeRole"
    }]
  })
}

resource "aws_iam_role_policy" "hotpath_runner_ssm_read" {
  count = var.enable_hotpath_runner ? 1 : 0
  name  = "ssm-read-github-app-credentials"
  role  = aws_iam_role.hotpath_runner[0].name

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = [
        "ssm:GetParameter",
        "ssm:GetParameters",
      ]
      Resource = [
        aws_ssm_parameter.hotpath_runner_github_app_id[0].arn,
        aws_ssm_parameter.hotpath_runner_github_app_key[0].arn,
        aws_ssm_parameter.hotpath_runner_github_app_installation_id[0].arn,
      ]
    }]
  })
}

# SSM Session Manager for shell debugging (no SSH port needed)
resource "aws_iam_role_policy_attachment" "hotpath_runner_ssm_managed" {
  count      = var.enable_hotpath_runner ? 1 : 0
  role       = aws_iam_role.hotpath_runner[0].name
  policy_arn = "arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore"
}

resource "aws_iam_instance_profile" "hotpath_runner" {
  count = var.enable_hotpath_runner ? 1 : 0
  name  = "${var.prefix}-hotpath-runner"
  role  = aws_iam_role.hotpath_runner[0].name
}

# --- Launch Template ---

resource "aws_launch_template" "hotpath_runner" {
  count                  = var.enable_hotpath_runner ? 1 : 0
  name                   = "${var.prefix}-hotpath-runner"
  update_default_version = true

  image_id = data.aws_ami.ubuntu_arm64_latest.id

  iam_instance_profile {
    name = aws_iam_instance_profile.hotpath_runner[0].name
  }

  vpc_security_group_ids = [aws_security_group.hotpath_runner[0].id]

  user_data = base64encode(templatefile("${path.module}/hotpath-runner-userdata.sh", {
    aws_region        = var.aws_region
    prefix            = var.prefix
    github_owner      = var.github_owner
    github_repository = var.github_repository
    runner_labels     = "reinhardt-hotpath"
  }))

  tag_specifications {
    resource_type = "instance"
    tags = {
      Name = "${var.prefix}-hotpath-runner"
    }
  }

  # Don't replace launch template when AMI updates; the base Ubuntu AMI
  # is only used for initial provisioning. user_data changes DO trigger
  # a new template version (update_default_version = true).
  lifecycle {
    ignore_changes = [image_id]
  }
}

# --- Auto Scaling Group (Spot, capacity rebalancing) ---

resource "aws_autoscaling_group" "hotpath_runner" {
  count = var.enable_hotpath_runner ? 1 : 0
  name  = "${var.prefix}-hotpath-runner"

  min_size         = 1
  max_size         = 2
  desired_capacity = 1

  vpc_zone_identifier = data.aws_subnets.default.ids

  # Proactive Spot replacement: when AWS detects elevated interruption risk,
  # the ASG launches a new instance while the old one is still running.
  # max_size=2 allows this temporary over-provisioning during switchover.
  capacity_rebalance = true

  # Allow 5 minutes for user-data to complete before health checks start
  health_check_type         = "EC2"
  health_check_grace_period = 300

  mixed_instances_policy {
    instances_distribution {
      on_demand_base_capacity                  = 0
      on_demand_percentage_above_base_capacity = 0
      spot_allocation_strategy                 = "price-capacity-optimized"
    }

    launch_template {
      launch_template_specification {
        launch_template_id = aws_launch_template.hotpath_runner[0].id
        version            = "$Latest"
      }

      # Primary instance type; fallback provides higher Spot availability
      override {
        instance_type = var.hotpath_runner_instance_type
      }
      override {
        instance_type = "t4g.small"
      }
    }
  }

  tag {
    key                 = "Name"
    value               = "${var.prefix}-hotpath-runner"
    propagate_at_launch = true
  }
}
