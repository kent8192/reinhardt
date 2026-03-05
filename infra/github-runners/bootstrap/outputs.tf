output "state_bucket_name" {
	description = "S3 bucket name for Terraform state (use in backend.tfvars)"
	value       = aws_s3_bucket.terraform_state.id
}

output "aws_region" {
	description = "AWS region (use in backend.tfvars)"
	value       = var.aws_region
}

output "backend_config_hint" {
	description = "backend.tfvars content to use in the parent directory"
	value       = <<-EOT
		bucket       = "${aws_s3_bucket.terraform_state.id}"
		key          = "github-runners/terraform.tfstate"
		region       = "${var.aws_region}"
		use_lockfile = true
	EOT
}
