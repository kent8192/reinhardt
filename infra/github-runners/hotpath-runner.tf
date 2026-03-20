# Always-on hotpath runner (t4g.micro, On-Demand).
# Runs lightweight CI control jobs that require immediate execution without
# queue delay: determine-runner, cancel-on-pr-close, cleanup-release-branch.
# Unlike the JIT ephemeral CI runners, this instance is persistent and
# uses a systemd service to keep the runner process alive across reboots.

# --- Moved blocks: rename cancel_runner → hotpath_runner ---
# These prevent Terraform from destroying and recreating existing resources.

moved {
  from = aws_ssm_parameter.cancel_runner_github_app_id
  to   = aws_ssm_parameter.hotpath_runner_github_app_id
}

moved {
  from = aws_ssm_parameter.cancel_runner_github_app_key
  to   = aws_ssm_parameter.hotpath_runner_github_app_key
}

moved {
  from = aws_ssm_parameter.cancel_runner_github_app_installation_id
  to   = aws_ssm_parameter.hotpath_runner_github_app_installation_id
}

moved {
  from = aws_security_group.cancel_runner
  to   = aws_security_group.hotpath_runner
}

moved {
  from = aws_iam_role.cancel_runner
  to   = aws_iam_role.hotpath_runner
}

moved {
  from = aws_iam_role_policy.cancel_runner_ssm_read
  to   = aws_iam_role_policy.hotpath_runner_ssm_read
}

moved {
  from = aws_iam_role_policy_attachment.cancel_runner_ssm_managed
  to   = aws_iam_role_policy_attachment.hotpath_runner_ssm_managed
}

moved {
  from = aws_iam_instance_profile.cancel_runner
  to   = aws_iam_instance_profile.hotpath_runner
}

moved {
  from = aws_instance.cancel_runner
  to   = aws_instance.hotpath_runner
}

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

# --- EC2 Instance ---

resource "aws_instance" "hotpath_runner" {
  count         = var.enable_hotpath_runner ? 1 : 0
  ami           = data.aws_ami.ubuntu_arm64_latest.id
  instance_type = var.hotpath_runner_instance_type

  subnet_id              = data.aws_subnets.default.ids[0]
  vpc_security_group_ids = [aws_security_group.hotpath_runner[0].id]
  iam_instance_profile   = aws_iam_instance_profile.hotpath_runner[0].name

  user_data = templatefile("${path.module}/hotpath-runner-userdata.sh", {
    aws_region        = var.aws_region
    prefix            = var.prefix
    github_owner      = var.github_owner
    github_repository = var.github_repository
    runner_labels     = "reinhardt-hotpath"
  })

  tags = {
    Name = "${var.prefix}-hotpath-runner"
  }

  # Don't replace instance when AMI updates (base Ubuntu AMI is fine).
  # user_data is NOT ignored: changes to the bootstrap script should
  # trigger instance replacement to maintain IaC correctness.
  lifecycle {
    ignore_changes        = [ami]
    create_before_destroy = true
  }
}
