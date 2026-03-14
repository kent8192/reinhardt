# Repository issue labels managed by Terraform.
# For the full label catalog, see .github/labels.yml (managed outside Terraform).

resource "github_issue_label" "infrastructure" {
  repository  = var.repository_name
  name        = "infrastructure"
  color       = "0e8a16"
  description = "Infrastructure and Terraform changes"
}
