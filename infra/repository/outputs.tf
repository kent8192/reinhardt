output "fork_review_environment" {
  description = "Name of the fork review environment."
  value       = github_repository_environment.fork_review.environment
}

output "production_environment" {
  description = "Name of the production environment."
  value       = github_repository_environment.production.environment
}
