//! Sitemap XML generation for SSG.

use super::route::SsgRoute;

/// Generates a `sitemap.xml` file from SSG routes.
pub struct SitemapGenerator {
	/// Base URL for the site (e.g., `"https://example.com"`)
	base_url: String,
}

impl SitemapGenerator {
	/// Creates a new sitemap generator with the given base URL.
	pub fn new(base_url: impl Into<String>) -> Self {
		let mut base = base_url.into();
		// Remove trailing slash from base URL
		if base.ends_with('/') {
			base.pop();
		}
		Self { base_url: base }
	}

	/// Generates a sitemap XML string from the given routes.
	pub fn generate(&self, routes: &[SsgRoute]) -> String {
		let mut xml = String::with_capacity(routes.len() * 200);

		xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
		xml.push_str("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");

		for route in routes {
			xml.push_str("  <url>\n");
			xml.push_str(&format!("    <loc>{}{}</loc>\n", self.base_url, route.path));
			xml.push_str(&format!(
				"    <changefreq>{}</changefreq>\n",
				route.change_frequency
			));
			xml.push_str(&format!(
				"    <priority>{:.1}</priority>\n",
				route.sitemap_priority
			));
			xml.push_str("  </url>\n");
		}

		xml.push_str("</urlset>\n");
		xml
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::component::Page;
	use crate::ssg::route::ChangeFrequency;
	use rstest::rstest;

	fn dummy_page() -> Page {
		Page::text("test")
	}

	#[rstest]
	fn test_sitemap_generator_basic() {
		// Arrange
		let generator = SitemapGenerator::new("https://example.com");
		let routes = vec![
			SsgRoute::new("/", dummy_page).with_priority(1.0),
			SsgRoute::new("/about/", dummy_page).with_priority(0.8),
		];

		// Act
		let xml = generator.generate(&routes);

		// Assert
		assert!(xml.starts_with("<?xml version=\"1.0\""));
		assert!(xml.contains("<loc>https://example.com/</loc>"));
		assert!(xml.contains("<loc>https://example.com/about/</loc>"));
		assert!(xml.contains("<priority>1.0</priority>"));
		assert!(xml.contains("<priority>0.8</priority>"));
		assert!(xml.contains("</urlset>"));
	}

	#[rstest]
	fn test_sitemap_generator_strips_trailing_slash_from_base_url() {
		// Arrange
		let generator = SitemapGenerator::new("https://example.com/");
		let routes = vec![SsgRoute::new("/", dummy_page)];

		// Act
		let xml = generator.generate(&routes);

		// Assert
		assert!(xml.contains("<loc>https://example.com/</loc>"));
		// Should not have double slashes
		assert!(!xml.contains("https://example.com//"));
	}

	#[rstest]
	fn test_sitemap_generator_change_frequency() {
		// Arrange
		let generator = SitemapGenerator::new("https://example.com");
		let routes = vec![
			SsgRoute::new("/", dummy_page).with_change_frequency(ChangeFrequency::Daily),
			SsgRoute::new("/archive/", dummy_page).with_change_frequency(ChangeFrequency::Never),
		];

		// Act
		let xml = generator.generate(&routes);

		// Assert
		assert!(xml.contains("<changefreq>daily</changefreq>"));
		assert!(xml.contains("<changefreq>never</changefreq>"));
	}

	#[rstest]
	fn test_sitemap_generator_empty_routes() {
		// Arrange
		let generator = SitemapGenerator::new("https://example.com");
		let routes: Vec<SsgRoute> = vec![];

		// Act
		let xml = generator.generate(&routes);

		// Assert
		assert!(xml.contains("<urlset"));
		assert!(xml.contains("</urlset>"));
		assert!(!xml.contains("<url>"));
	}
}
