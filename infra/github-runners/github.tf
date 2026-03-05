# Repository variable: controls whether self-hosted runners are enabled.
# Set to "false" manually via GitHub repo Settings > Variables when monthly budget is exceeded.
# Reset to "true" at the start of each new month if budget was exceeded.
resource "github_actions_variable" "self_hosted_enabled" {
	repository    = var.github_repository
	variable_name = "SELF_HOSTED_ENABLED"
	value         = "true"

	lifecycle {
		# Do not override if manually set to "false" for budget control
		ignore_changes = [value]
	}
}
