# GitHub repository environments for deployment protection.

# Look up the reviewer's GitHub user ID for environment protection rules.
data "github_user" "reviewer" {
  username = var.environment_reviewer
}

# Gate for terraform plan on fork PRs.
# Requires manual approval before CI runs plan on external contributions.
resource "github_repository_environment" "fork_review" {
  environment = "fork-review"
  repository  = var.repository_name
  wait_timer  = 0

  prevent_self_review = true

  reviewers {
    users = [data.github_user.reviewer.id]
  }
}

# Gate for terraform apply post-merge.
# Requires manual approval before applying infrastructure changes to production.
resource "github_repository_environment" "production" {
  environment = "production"
  repository  = var.repository_name
  wait_timer  = 0

  prevent_self_review = true

  reviewers {
    users = [data.github_user.reviewer.id]
  }
}
