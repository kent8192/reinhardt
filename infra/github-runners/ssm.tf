# SSM Parameter to store the Golden AMI ID.
# Value is managed by the build-runner-ami GitHub Actions workflow;
# Terraform only creates the parameter and ignores subsequent value changes.
resource "aws_ssm_parameter" "runner_ami_id" {
	name  = "/${var.prefix}/runner-ami-id"
	type  = "String"
	value = "ami-placeholder"

	tags = {
		Description = "Golden AMI ID for self-hosted GitHub Actions runners"
	}

	lifecycle {
		ignore_changes = [value]
	}
}
