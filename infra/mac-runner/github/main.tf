provider "github" {
  owner = var.github_owner
  token = var.github_token
}

resource "github_actions_variable" "mac_runner_enabled" {
  repository    = var.repository_name
  variable_name = "MAC_RUNNER_ENABLED"
  value         = var.mac_runner_enabled ? "true" : "false"
}
