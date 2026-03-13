resource "cloudflare_pages_project" "website" {
  account_id        = var.cloudflare_account_id
  name              = var.pages_project_name
  production_branch = var.production_branch
}

# Custom domain — registers domain with the Pages project
resource "cloudflare_pages_domain" "website" {
  account_id   = var.cloudflare_account_id
  project_name = cloudflare_pages_project.website.name
  name         = var.custom_domain
}

# Look up the existing Cloudflare zone for the custom domain
data "cloudflare_zone" "website" {
  filter = {
    name = var.custom_domain
  }
}

# Apex domain CNAME record pointing to Cloudflare Pages
resource "cloudflare_dns_record" "website_apex" {
  zone_id = data.cloudflare_zone.website.zone_id
  name    = "@"
  type    = "CNAME"
  content = "${cloudflare_pages_project.website.name}.pages.dev"
  ttl     = 1
  proxied = true

  depends_on = [cloudflare_pages_domain.website]
}

# Custom domain for www subdomain — registers www with the Pages project
resource "cloudflare_pages_domain" "website_www" {
  account_id   = var.cloudflare_account_id
  project_name = cloudflare_pages_project.website.name
  name         = "www.${var.custom_domain}"
}

# www subdomain CNAME record pointing to Cloudflare Pages
resource "cloudflare_dns_record" "website_www" {
  zone_id = data.cloudflare_zone.website.zone_id
  name    = "www"
  type    = "CNAME"
  content = "${cloudflare_pages_project.website.name}.pages.dev"
  ttl     = 1
  proxied = true

  depends_on = [cloudflare_pages_domain.website_www]
}

# Google Search Console domain verification
resource "cloudflare_dns_record" "google_verification" {
  zone_id = data.cloudflare_zone.website.zone_id
  name    = "@"
  type    = "TXT"
  content = var.google_site_verification
  ttl     = 3600
}
