# Latest Ubuntu 22.04 arm64 AMI (Canonical) for SSM parameter initial value.
# The build-runner-ami workflow overwrites this with the Golden AMI;
# this data source only provides a valid AMI ID so Terraform can create
# the parameter with data_type = "aws:ec2:image" (which validates the value).
data "aws_ami" "ubuntu_arm64_latest" {
  most_recent = true
  owners      = ["099720109477"] # Canonical

  filter {
    name   = "name"
    values = ["ubuntu/images/hvm-ssd/ubuntu-jammy-22.04-arm64-server-*"]
  }

  filter {
    name   = "virtualization-type"
    values = ["hvm"]
  }
}

# SSM Parameter to store the Golden AMI ID.
# Value is managed by the build-runner-ami GitHub Actions workflow;
# Terraform only creates the parameter and ignores subsequent value changes.
#
# data_type = "aws:ec2:image" is REQUIRED: the EC2 Launch Template resolves
# the AMI via `resolve:ssm:` which only works with this data type.
# See: https://github.com/kent8192/reinhardt-web/issues/2023
resource "aws_ssm_parameter" "runner_ami_id" {
  name      = "/${var.prefix}/runner-ami-id"
  type      = "String"
  data_type = "aws:ec2:image"
  value     = data.aws_ami.ubuntu_arm64_latest.id

  tags = {
    Description = "Golden AMI ID for self-hosted GitHub Actions runners"
  }

  lifecycle {
    ignore_changes = [value]
  }
}
