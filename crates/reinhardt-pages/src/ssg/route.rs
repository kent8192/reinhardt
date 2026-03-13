//! SSG route definition for static site generation.

use crate::component::Page;
use crate::ssr::SsrOptions;

/// A route to be rendered as a static HTML file during SSG.
///
/// Each `SsgRoute` maps a URL path to a rendering function that produces
/// a `Page`. The renderer function is called at build time to generate
/// the static HTML output.
pub struct SsgRoute {
	/// The URL path for this route (e.g., `"/about/"`)
	pub path: String,
	/// The rendering function that produces a `Page` for this route
	renderer: Box<dyn Fn() -> Page + Send + Sync>,
	/// Optional per-route SSR options override
	pub ssr_options: Option<SsrOptions>,
	/// Priority hint for sitemap (0.0 to 1.0)
	pub sitemap_priority: f32,
	/// Change frequency hint for sitemap
	pub change_frequency: ChangeFrequency,
}

impl SsgRoute {
	/// Creates a new SSG route with the given path and renderer.
	///
	/// # Arguments
	///
	/// * `path` - The URL path (e.g., `"/"`, `"/about/"`)
	/// * `renderer` - A function that returns a `Page` for this route
	pub fn new(
		path: impl Into<String>,
		renderer: impl Fn() -> Page + Send + Sync + 'static,
	) -> Self {
		let path = path.into();
		let normalized = normalize_path(&path);
		Self {
			path: normalized,
			renderer: Box::new(renderer),
			ssr_options: None,
			sitemap_priority: 0.5,
			change_frequency: ChangeFrequency::Weekly,
		}
	}

	/// Sets custom SSR options for this route.
	pub fn with_ssr_options(mut self, options: SsrOptions) -> Self {
		self.ssr_options = Some(options);
		self
	}

	/// Sets the sitemap priority for this route.
	pub fn with_priority(mut self, priority: f32) -> Self {
		self.sitemap_priority = priority.clamp(0.0, 1.0);
		self
	}

	/// Sets the sitemap change frequency for this route.
	pub fn with_change_frequency(mut self, freq: ChangeFrequency) -> Self {
		self.change_frequency = freq;
		self
	}

	/// Renders this route to a `Page`.
	pub fn render(&self) -> Page {
		(self.renderer)()
	}

	/// Returns the file system path for the output HTML file.
	///
	/// URL paths are mapped to directory structures:
	/// - `"/"` -> `"index.html"`
	/// - `"/about/"` -> `"about/index.html"`
	/// - `"/blog/post-1/"` -> `"blog/post-1/index.html"`
	pub fn output_path(&self) -> std::path::PathBuf {
		let trimmed = self.path.trim_matches('/');
		if trimmed.is_empty() {
			std::path::PathBuf::from("index.html")
		} else {
			std::path::PathBuf::from(trimmed).join("index.html")
		}
	}
}

/// Change frequency hint for sitemap entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeFrequency {
	/// Content changes every time it is accessed.
	Always,
	/// Content changes hourly.
	Hourly,
	/// Content changes daily.
	Daily,
	/// Content changes weekly.
	Weekly,
	/// Content changes monthly.
	Monthly,
	/// Content changes yearly.
	Yearly,
	/// Content is archived and will not change.
	Never,
}

impl std::fmt::Display for ChangeFrequency {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Always => write!(f, "always"),
			Self::Hourly => write!(f, "hourly"),
			Self::Daily => write!(f, "daily"),
			Self::Weekly => write!(f, "weekly"),
			Self::Monthly => write!(f, "monthly"),
			Self::Yearly => write!(f, "yearly"),
			Self::Never => write!(f, "never"),
		}
	}
}

/// Normalizes a URL path to ensure consistent formatting.
///
/// Ensures the path starts with `/` and ends with `/` for directory-style URLs.
fn normalize_path(path: &str) -> String {
	let mut normalized = path.to_string();

	// Ensure leading slash
	if !normalized.starts_with('/') {
		normalized.insert(0, '/');
	}

	// Ensure trailing slash for directory-style URLs
	// (skip if path looks like a file, e.g., "/robots.txt")
	if !normalized.ends_with('/')
		&& !normalized
			.rsplit('/')
			.next()
			.is_some_and(|seg| seg.contains('.'))
	{
		normalized.push('/');
	}

	normalized
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::component::Page;
	use rstest::rstest;

	fn dummy_page() -> Page {
		Page::text("Hello, SSG!")
	}

	#[rstest]
	fn test_ssg_route_new() {
		// Arrange
		let path = "/about/";

		// Act
		let route = SsgRoute::new(path, dummy_page);

		// Assert
		assert_eq!(route.path, "/about/");
		assert_eq!(route.sitemap_priority, 0.5);
	}

	#[rstest]
	fn test_ssg_route_render() {
		// Arrange
		let route = SsgRoute::new("/", || Page::text("Home"));

		// Act
		let page = route.render();

		// Assert
		let html = page.render_to_string();
		assert_eq!(html, "Home");
	}

	#[rstest]
	#[case("/", "index.html")]
	#[case("/about/", "about/index.html")]
	#[case("/blog/post-1/", "blog/post-1/index.html")]
	fn test_ssg_route_output_path(#[case] path: &str, #[case] expected: &str) {
		// Arrange
		let route = SsgRoute::new(path, dummy_page);

		// Act
		let output = route.output_path();

		// Assert
		assert_eq!(output, std::path::PathBuf::from(expected));
	}

	#[rstest]
	fn test_ssg_route_with_priority() {
		// Arrange & Act
		let route = SsgRoute::new("/", dummy_page).with_priority(0.9);

		// Assert
		assert!((route.sitemap_priority - 0.9).abs() < f32::EPSILON);
	}

	#[rstest]
	fn test_ssg_route_priority_clamped() {
		// Arrange & Act
		let route = SsgRoute::new("/", dummy_page).with_priority(1.5);

		// Assert
		assert!((route.sitemap_priority - 1.0).abs() < f32::EPSILON);
	}

	#[rstest]
	fn test_ssg_route_with_change_frequency() {
		// Arrange & Act
		let route = SsgRoute::new("/", dummy_page).with_change_frequency(ChangeFrequency::Daily);

		// Assert
		assert_eq!(route.change_frequency, ChangeFrequency::Daily);
	}

	#[rstest]
	#[case("about", "/about/")]
	#[case("/about", "/about/")]
	#[case("/about/", "/about/")]
	#[case("/", "/")]
	fn test_normalize_path(#[case] input: &str, #[case] expected: &str) {
		// Arrange & Act
		let normalized = normalize_path(input);

		// Assert
		assert_eq!(normalized, expected);
	}

	#[rstest]
	fn test_normalize_path_preserves_file_extension() {
		// Arrange & Act
		let normalized = normalize_path("/robots.txt");

		// Assert
		assert_eq!(normalized, "/robots.txt");
	}

	#[rstest]
	fn test_change_frequency_display() {
		// Arrange & Act & Assert
		assert_eq!(ChangeFrequency::Always.to_string(), "always");
		assert_eq!(ChangeFrequency::Hourly.to_string(), "hourly");
		assert_eq!(ChangeFrequency::Daily.to_string(), "daily");
		assert_eq!(ChangeFrequency::Weekly.to_string(), "weekly");
		assert_eq!(ChangeFrequency::Monthly.to_string(), "monthly");
		assert_eq!(ChangeFrequency::Yearly.to_string(), "yearly");
		assert_eq!(ChangeFrequency::Never.to_string(), "never");
	}
}
