//! SSG builder for orchestrating static site generation.

use super::output::SsgOutput;
use super::route::SsgRoute;
use super::sitemap::SitemapGenerator;
use crate::ssr::{SsrOptions, SsrRenderer};
use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during SSG build.
#[derive(Debug, Error)]
pub enum SsgError {
	/// An I/O error occurred while writing files.
	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),

	/// No routes were provided for generation.
	#[error("no routes provided for SSG build")]
	NoRoutes,

	/// The output directory path is invalid.
	#[error("invalid output directory: {0}")]
	InvalidOutputDir(String),
}

/// Builder for configuring and executing static site generation.
///
/// `SsgBuilder` orchestrates the SSG pipeline:
/// 1. Enumerate routes to render
/// 2. Render each route to HTML using `SsrRenderer`
/// 3. Write HTML files mirroring the URL structure
/// 4. Optionally generate a `sitemap.xml`
pub struct SsgBuilder {
	/// Output directory for generated files.
	output_dir: PathBuf,
	/// Routes to render.
	routes: Vec<SsgRoute>,
	/// Default SSR options applied to all routes (unless overridden per-route).
	default_ssr_options: SsrOptions,
	/// Base URL for sitemap generation.
	base_url: Option<String>,
	/// Whether to generate a sitemap.
	generate_sitemap: bool,
	/// Whether to clean the output directory before building.
	clean_output: bool,
}

impl SsgBuilder {
	/// Creates a new SSG builder targeting the given output directory.
	pub fn new(output_dir: impl Into<PathBuf>) -> Self {
		Self {
			output_dir: output_dir.into(),
			routes: Vec::new(),
			default_ssr_options: SsrOptions::new().no_hydration(),
			base_url: None,
			generate_sitemap: true,
			clean_output: false,
		}
	}

	/// Sets the base URL for sitemap generation.
	pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
		self.base_url = Some(base_url.into());
		self
	}

	/// Sets the routes to render.
	pub fn with_routes(mut self, routes: Vec<SsgRoute>) -> Self {
		self.routes = routes;
		self
	}

	/// Adds a single route.
	pub fn add_route(mut self, route: SsgRoute) -> Self {
		self.routes.push(route);
		self
	}

	/// Sets the default SSR options for all routes.
	pub fn with_default_ssr_options(mut self, options: SsrOptions) -> Self {
		self.default_ssr_options = options;
		self
	}

	/// Enables or disables sitemap generation.
	pub fn with_sitemap(mut self, enabled: bool) -> Self {
		self.generate_sitemap = enabled;
		self
	}

	/// Enables or disables cleaning the output directory before building.
	pub fn with_clean_output(mut self, enabled: bool) -> Self {
		self.clean_output = enabled;
		self
	}

	/// Executes the SSG build pipeline.
	///
	/// Returns an `SsgOutput` with statistics about the generated files.
	pub fn build(self) -> Result<SsgOutput, SsgError> {
		if self.routes.is_empty() {
			return Err(SsgError::NoRoutes);
		}

		// Validate output directory path
		if self.output_dir.as_os_str().is_empty() {
			return Err(SsgError::InvalidOutputDir(
				"output directory path is empty".to_string(),
			));
		}

		// Clean output directory if requested
		if self.clean_output && self.output_dir.exists() {
			std::fs::remove_dir_all(&self.output_dir)?;
		}

		// Ensure output directory exists
		std::fs::create_dir_all(&self.output_dir)?;

		let mut output = SsgOutput::new(self.output_dir.clone());

		// Render each route
		for route in &self.routes {
			let html = self.render_route(route);
			let relative_path = route.output_path();
			let absolute_path = self.output_dir.join(&relative_path);

			// Ensure parent directory exists
			if let Some(parent) = absolute_path.parent() {
				std::fs::create_dir_all(parent)?;
			}

			let bytes = html.len() as u64;
			std::fs::write(&absolute_path, &html)?;
			output.record_file(relative_path, bytes);
		}

		// Generate sitemap
		if self.generate_sitemap
			&& let Some(ref base_url) = self.base_url
		{
			let sitemap = SitemapGenerator::new(base_url);
			let xml = sitemap.generate(&self.routes);
			let sitemap_path = self.output_dir.join("sitemap.xml");
			std::fs::write(&sitemap_path, &xml)?;
			output.sitemap_generated = true;
		}

		Ok(output)
	}

	/// Renders a single route to HTML.
	fn render_route(&self, route: &SsgRoute) -> String {
		let options = route
			.ssr_options
			.clone()
			.unwrap_or_else(|| self.default_ssr_options.clone());

		let mut renderer = SsrRenderer::with_options(options);
		let page = route.render();
		renderer.render_page_with_view_head(page)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::component::Page;
	use rstest::rstest;
	use tempfile::TempDir;

	fn home_page() -> Page {
		Page::text("Welcome Home")
	}

	fn about_page() -> Page {
		Page::text("About Us")
	}

	fn blog_page() -> Page {
		Page::text("Blog Posts")
	}

	#[rstest]
	fn test_ssg_builder_basic_build() {
		// Arrange
		let tmpdir = TempDir::new().unwrap();
		let output_dir = tmpdir.path().join("dist");

		// Act
		let output = SsgBuilder::new(&output_dir)
			.with_routes(vec![
				SsgRoute::new("/", home_page),
				SsgRoute::new("/about/", about_page),
			])
			.build()
			.unwrap();

		// Assert
		assert_eq!(output.files_written, 2);
		assert!(output_dir.join("index.html").exists());
		assert!(output_dir.join("about/index.html").exists());

		let home_content = std::fs::read_to_string(output_dir.join("index.html")).unwrap();
		assert!(home_content.contains("Welcome Home"));

		let about_content = std::fs::read_to_string(output_dir.join("about/index.html")).unwrap();
		assert!(about_content.contains("About Us"));
	}

	#[rstest]
	fn test_ssg_builder_generates_sitemap() {
		// Arrange
		let tmpdir = TempDir::new().unwrap();
		let output_dir = tmpdir.path().join("dist");

		// Act
		let output = SsgBuilder::new(&output_dir)
			.with_base_url("https://example.com")
			.with_routes(vec![
				SsgRoute::new("/", home_page).with_priority(1.0),
				SsgRoute::new("/about/", about_page).with_priority(0.8),
			])
			.build()
			.unwrap();

		// Assert
		assert!(output.sitemap_generated);
		let sitemap_path = output_dir.join("sitemap.xml");
		assert!(sitemap_path.exists());

		let sitemap_content = std::fs::read_to_string(sitemap_path).unwrap();
		assert!(sitemap_content.contains("https://example.com/"));
		assert!(sitemap_content.contains("https://example.com/about/"));
	}

	#[rstest]
	fn test_ssg_builder_no_sitemap_without_base_url() {
		// Arrange
		let tmpdir = TempDir::new().unwrap();
		let output_dir = tmpdir.path().join("dist");

		// Act
		let output = SsgBuilder::new(&output_dir)
			.with_routes(vec![SsgRoute::new("/", home_page)])
			.build()
			.unwrap();

		// Assert
		assert!(!output.sitemap_generated);
		assert!(!output_dir.join("sitemap.xml").exists());
	}

	#[rstest]
	fn test_ssg_builder_sitemap_disabled() {
		// Arrange
		let tmpdir = TempDir::new().unwrap();
		let output_dir = tmpdir.path().join("dist");

		// Act
		let output = SsgBuilder::new(&output_dir)
			.with_base_url("https://example.com")
			.with_sitemap(false)
			.with_routes(vec![SsgRoute::new("/", home_page)])
			.build()
			.unwrap();

		// Assert
		assert!(!output.sitemap_generated);
	}

	#[rstest]
	fn test_ssg_builder_no_routes_error() {
		// Arrange
		let tmpdir = TempDir::new().unwrap();
		let output_dir = tmpdir.path().join("dist");

		// Act
		let result = SsgBuilder::new(&output_dir).build();

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, SsgError::NoRoutes));
	}

	#[rstest]
	fn test_ssg_builder_clean_output() {
		// Arrange
		let tmpdir = TempDir::new().unwrap();
		let output_dir = tmpdir.path().join("dist");

		// Create a file that should be cleaned
		std::fs::create_dir_all(&output_dir).unwrap();
		std::fs::write(output_dir.join("old_file.html"), "old content").unwrap();

		// Act
		let _output = SsgBuilder::new(&output_dir)
			.with_clean_output(true)
			.with_routes(vec![SsgRoute::new("/", home_page)])
			.build()
			.unwrap();

		// Assert
		assert!(!output_dir.join("old_file.html").exists());
		assert!(output_dir.join("index.html").exists());
	}

	#[rstest]
	fn test_ssg_builder_nested_routes() {
		// Arrange
		let tmpdir = TempDir::new().unwrap();
		let output_dir = tmpdir.path().join("dist");

		// Act
		let output = SsgBuilder::new(&output_dir)
			.with_routes(vec![
				SsgRoute::new("/", home_page),
				SsgRoute::new("/blog/", blog_page),
				SsgRoute::new("/blog/post-1/", || Page::text("Post 1")),
				SsgRoute::new("/blog/post-2/", || Page::text("Post 2")),
			])
			.build()
			.unwrap();

		// Assert
		assert_eq!(output.files_written, 4);
		assert!(output_dir.join("index.html").exists());
		assert!(output_dir.join("blog/index.html").exists());
		assert!(output_dir.join("blog/post-1/index.html").exists());
		assert!(output_dir.join("blog/post-2/index.html").exists());
	}

	#[rstest]
	fn test_ssg_builder_total_bytes_tracked() {
		// Arrange
		let tmpdir = TempDir::new().unwrap();
		let output_dir = tmpdir.path().join("dist");

		// Act
		let output = SsgBuilder::new(&output_dir)
			.with_routes(vec![SsgRoute::new("/", home_page)])
			.build()
			.unwrap();

		// Assert
		assert!(output.total_bytes > 0);
	}

	#[rstest]
	fn test_ssg_builder_renders_full_html_page() {
		// Arrange
		let tmpdir = TempDir::new().unwrap();
		let output_dir = tmpdir.path().join("dist");

		// Act
		let _output = SsgBuilder::new(&output_dir)
			.with_routes(vec![SsgRoute::new("/", home_page)])
			.build()
			.unwrap();

		// Assert
		let content = std::fs::read_to_string(output_dir.join("index.html")).unwrap();
		assert!(content.contains("<!DOCTYPE html>"));
		assert!(content.contains("<html"));
		assert!(content.contains("Welcome Home"));
		assert!(content.contains("</html>"));
	}
}
