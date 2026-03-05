output "account_id" {
	description = "AWS Account ID for the CI sub-account. Use this in github-runners/terraform.tfvars as aws_account_id."
	value       = aws_organizations_account.ci_runners.id
}

output "assume_role_arn" {
	description = "IAM role ARN to assume for deploying resources in the CI sub-account"
	value       = "arn:aws:iam::${aws_organizations_account.ci_runners.id}:role/OrganizationAccountAccessRole"
}

output "switch_role_hint" {
	description = "AWS CLI command to assume the CI sub-account role"
	value       = "aws sts assume-role --role-arn arn:aws:iam::${aws_organizations_account.ci_runners.id}:role/OrganizationAccountAccessRole --role-session-name ci-deploy"
}
