output "pages_project_name" {
  description = "Cloudflare Pages project name"
  value       = cloudflare_pages_project.website.name
}

output "pages_subdomain" {
  description = "Default *.pages.dev subdomain"
  value       = "${cloudflare_pages_project.website.name}.pages.dev"
}

output "custom_domain" {
  description = "Custom domain for the Pages project"
  value       = cloudflare_pages_domain.website.name
}

output "zone_id" {
  description = "Cloudflare Zone ID for the custom domain"
  value       = data.cloudflare_zone.website.zone_id
  sensitive   = true
}
