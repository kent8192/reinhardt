output "webhook_endpoint" {
  description = "GitHub App webhook URL - configure this in GitHub App settings under Webhook URL"
  value       = module.github_runner.webhook.endpoint
}

output "webhook_secret" {
  description = "GitHub App webhook secret - configure this in GitHub App settings"
  value       = random_password.webhook_secret.result
  sensitive   = true
}

output "runner_labels" {
  description = "Labels to use in GitHub Actions runs-on for self-hosted runners"
  value       = jsonencode(concat(["self-hosted", "linux", "arm64"], var.runner_extra_labels))
}

# Manual webhook setup guide.
# The GitHub Terraform provider does not support GitHub App webhook configuration,
# and shell-based JWT generation is fragile across OS environments. Configure once
# after the first successful terraform apply using the instructions below.
output "webhook_setup_guide" {
  description = "One-time manual steps to configure the GitHub App webhook after terraform apply"
  value       = <<-EOT
		=== GitHub App Webhook Setup (run once after terraform apply) ===

		1. Get credentials:
		   Webhook URL:    terraform output -raw webhook_endpoint
		   Webhook Secret: terraform output -raw webhook_secret

		2. Configure via GitHub App settings:
		   https://github.com/settings/apps/<YOUR_APP_NAME>
		   - Enable "Active" checkbox under Webhook
		   - Set Webhook URL to the value from step 1
		   - Set Webhook Secret to the value from step 1
		   - Under "Permissions & events" > "Subscribe to events": enable "Workflow jobs"
		   - Save changes

		3. Verify: Push a commit to trigger CI and check AWS CloudWatch for Lambda invocations
	EOT
}

output "cancel_runner_instance_id" {
  description = "EC2 instance ID of the cancel runner (empty if disabled)"
  value       = var.enable_cancel_runner ? aws_instance.cancel_runner[0].id : ""
}

output "cancel_runner_private_ip" {
  description = "Private IP of the cancel runner (for SSM Session Manager access)"
  value       = var.enable_cancel_runner ? aws_instance.cancel_runner[0].private_ip : ""
}

output "github_actions_oidc_provider_arn" {
  description = "ARN of the GitHub Actions OIDC identity provider"
  value       = aws_iam_openid_connect_provider.github_actions.arn
}

output "github_actions_ami_builder_role_arn" {
  description = "ARN of the IAM role used by the AMI builder workflow"
  value       = aws_iam_role.github_actions_ami_builder.arn
}
