//! URL reverse support for versioned URLs
//!
//! Provides utilities for generating versioned URLs that can be used
//! in responses, redirects, and API documentation.

use super::BaseVersioning;
use std::collections::HashMap;
use std::sync::Arc;

/// Builder for creating versioned URLs
pub struct VersionedUrlBuilder {
	versioning: Arc<dyn BaseVersioning>,
	base_url: String,
	version: Option<String>,
	strategy: VersioningStrategy,
}

impl VersionedUrlBuilder {
	/// Create a new versioned URL builder
	pub fn new(versioning: Arc<dyn BaseVersioning>, base_url: &str) -> Self {
		Self {
			versioning,
			base_url: base_url.to_string(),
			version: None,
			strategy: VersioningStrategy::URLPath, // Default
		}
	}

	/// Create a new versioned URL builder with explicit strategy
	pub fn with_strategy(
		versioning: Arc<dyn BaseVersioning>,
		base_url: &str,
		strategy: VersioningStrategy,
	) -> Self {
		Self {
			versioning,
			base_url: base_url.to_string(),
			version: None,
			strategy,
		}
	}

	/// Set a specific version for the URL
	pub fn with_version(mut self, version: &str) -> Self {
		self.version = Some(version.to_string());
		self
	}

	/// Build a versioned URL for a given path
	pub fn build(&self, path: &str) -> String {
		let version = self
			.version
			.as_deref()
			.or_else(|| self.versioning.default_version())
			.unwrap_or("1.0");

		self.build_with_version(path, version)
	}

	/// Build a versioned URL with a specific version
	pub fn build_with_version(&self, path: &str, version: &str) -> String {
		// Remove leading slash from path if present
		let clean_path = path.strip_prefix('/').unwrap_or(path);

		// Ensure base_url doesn't end with slash
		let base = self.base_url.trim_end_matches('/');

		// Build versioned path based on versioning strategy
		match self.strategy {
			VersioningStrategy::URLPath => {
				format!("{}/v{}/{}", base, version, clean_path)
			}
			VersioningStrategy::AcceptHeader => {
				format!("{}/{}", base, clean_path)
			}
			VersioningStrategy::QueryParameter => {
				format!("{}/{}?version={}", base, clean_path, version)
			}
			VersioningStrategy::HostName => {
				format!("https://v{}.{}", version, self.extract_domain())
			}
			VersioningStrategy::Namespace => {
				format!("{}/v{}/{}", base, version, clean_path)
			}
		}
	}

	/// Build multiple versioned URLs for the same path
	pub fn build_all_versions(&self, path: &str) -> HashMap<String, String> {
		let mut urls = HashMap::new();

		if let Some(allowed_versions) = self.versioning.allowed_versions() {
			for version in allowed_versions {
				urls.insert(version.clone(), self.build_with_version(path, version));
			}
		} else {
			// If no allowed versions, build with default
			let default_version = self.versioning.default_version().unwrap_or("1.0");
			urls.insert(
				default_version.to_string(),
				self.build_with_version(path, default_version),
			);
		}

		urls
	}

	/// Extract domain from base URL for hostname versioning
	fn extract_domain(&self) -> String {
		if let Some(domain) = self
			.base_url
			.strip_prefix("https://")
			.or_else(|| self.base_url.strip_prefix("http://"))
		{
			domain.to_string()
		} else {
			self.base_url.clone()
		}
	}
}

/// Versioning strategy for URL building
#[derive(Debug, Clone, PartialEq)]
pub enum VersioningStrategy {
	/// Version embedded in the URL path (e.g., `/v1/users/`).
	URLPath,
	/// Version specified in the Accept header.
	AcceptHeader,
	/// Version specified as a query parameter.
	QueryParameter,
	/// Version extracted from the hostname subdomain.
	HostName,
	/// Version extracted from the URL namespace.
	Namespace,
}

/// URL reverse manager for handling multiple versioning strategies
pub struct UrlReverseManager {
	builders: HashMap<String, VersionedUrlBuilder>,
	default_builder: Option<VersionedUrlBuilder>,
}

impl UrlReverseManager {
	/// Create a new URL reverse manager
	pub fn new() -> Self {
		Self {
			builders: HashMap::new(),
			default_builder: None,
		}
	}

	/// Add a versioned URL builder for a specific name
	pub fn add_builder(mut self, name: &str, builder: VersionedUrlBuilder) -> Self {
		self.builders.insert(name.to_string(), builder);
		self
	}

	/// Set the default builder
	pub fn with_default_builder(mut self, builder: VersionedUrlBuilder) -> Self {
		self.default_builder = Some(builder);
		self
	}

	/// Build a versioned URL using the named builder
	pub fn build_url(&self, name: &str, path: &str) -> Option<String> {
		self.builders.get(name).map(|builder| builder.build(path))
	}

	/// Build a versioned URL using the default builder
	pub fn build_default_url(&self, path: &str) -> Option<String> {
		self.default_builder
			.as_ref()
			.map(|builder| builder.build(path))
	}

	/// Build URLs for all available builders
	pub fn build_all_urls(&self, path: &str) -> HashMap<String, String> {
		let mut urls = HashMap::new();

		for (name, builder) in &self.builders {
			urls.insert(name.clone(), builder.build(path));
		}

		if let Some(default_builder) = &self.default_builder {
			urls.insert("default".to_string(), default_builder.build(path));
		}

		urls
	}
}

impl Default for UrlReverseManager {
	fn default() -> Self {
		Self::new()
	}
}

/// Utility for creating versioned API documentation URLs
pub struct ApiDocUrlBuilder {
	base_url: String,
	version: String,
	format: ApiDocFormat,
}

/// The format for API documentation output.
#[derive(Debug, Clone, PartialEq)]
pub enum ApiDocFormat {
	/// OpenAPI 3.0 specification format.
	OpenApi,
	/// Swagger UI format.
	Swagger,
	/// ReDoc documentation format.
	ReDoc,
	/// A custom documentation format identified by name.
	Custom(String),
}

impl ApiDocUrlBuilder {
	/// Create a new API documentation URL builder
	pub fn new(base_url: &str, version: &str) -> Self {
		Self {
			base_url: base_url.to_string(),
			version: version.to_string(),
			format: ApiDocFormat::OpenApi,
		}
	}

	/// Set the documentation format
	pub fn with_format(mut self, format: ApiDocFormat) -> Self {
		self.format = format;
		self
	}

	/// Build the documentation URL
	pub fn build(&self) -> String {
		let base = self.base_url.trim_end_matches('/');

		match self.format {
			ApiDocFormat::OpenApi => {
				format!("{}/v{}/openapi.json", base, self.version)
			}
			ApiDocFormat::Swagger => {
				format!("{}/v{}/swagger-ui/", base, self.version)
			}
			ApiDocFormat::ReDoc => {
				format!("{}/v{}/redoc/", base, self.version)
			}
			ApiDocFormat::Custom(ref format) => {
				format!("{}/v{}/{}", base, self.version, format)
			}
		}
	}
}

/// Macro for easy URL building
#[macro_export]
macro_rules! versioned_url {
	($builder:expr, $path:expr) => {
		$builder.build($path)
	};

	($builder:expr, $path:expr, $version:expr) => {
		$builder.build_with_version($path, $version)
	};
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::versioning::{AcceptHeaderVersioning, QueryParameterVersioning, URLPathVersioning};

	#[test]
	fn test_versioned_url_builder_url_path() {
		let versioning = Arc::new(
			URLPathVersioning::new()
				.with_default_version("1.0")
				.with_allowed_versions(vec!["1.0", "2.0"]),
		);

		let builder = VersionedUrlBuilder::new(versioning, "https://api.example.com");

		let url = builder.build("/users/");
		assert_eq!(url, "https://api.example.com/v1.0/users/");

		let url = builder.build_with_version("/users/", "2.0");
		assert_eq!(url, "https://api.example.com/v2.0/users/");
	}

	#[test]
	fn test_versioned_url_builder_query_parameter() {
		let versioning = Arc::new(
			QueryParameterVersioning::new()
				.with_default_version("1.0")
				.with_allowed_versions(vec!["1.0", "2.0"]),
		);

		let builder = VersionedUrlBuilder::with_strategy(
			versioning,
			"https://api.example.com",
			VersioningStrategy::QueryParameter,
		);

		let url = builder.build("/users/");
		assert_eq!(url, "https://api.example.com/users/?version=1.0");

		let url = builder.build_with_version("/users/", "2.0");
		assert_eq!(url, "https://api.example.com/users/?version=2.0");
	}

	#[test]
	fn test_versioned_url_builder_accept_header() {
		let versioning = Arc::new(
			AcceptHeaderVersioning::new()
				.with_default_version("1.0")
				.with_allowed_versions(vec!["1.0", "2.0"]),
		);

		let builder = VersionedUrlBuilder::with_strategy(
			versioning,
			"https://api.example.com",
			VersioningStrategy::AcceptHeader,
		);

		let url = builder.build("/users/");
		assert_eq!(url, "https://api.example.com/users/");

		let url = builder.build_with_version("/users/", "2.0");
		assert_eq!(url, "https://api.example.com/users/");
	}

	#[test]
	fn test_versioned_url_builder_all_versions() {
		let versioning = Arc::new(
			URLPathVersioning::new()
				.with_default_version("1.0")
				.with_allowed_versions(vec!["1.0", "2.0", "3.0"]),
		);

		let builder = VersionedUrlBuilder::new(versioning, "https://api.example.com");

		let urls = builder.build_all_versions("/users/");
		assert_eq!(urls.len(), 3);
		assert_eq!(
			urls.get("1.0"),
			Some(&"https://api.example.com/v1.0/users/".to_string())
		);
		assert_eq!(
			urls.get("2.0"),
			Some(&"https://api.example.com/v2.0/users/".to_string())
		);
		assert_eq!(
			urls.get("3.0"),
			Some(&"https://api.example.com/v3.0/users/".to_string())
		);
	}

	#[test]
	fn test_url_reverse_manager() {
		let versioning1 = Arc::new(URLPathVersioning::new().with_default_version("1.0"));
		let versioning2 = Arc::new(QueryParameterVersioning::new().with_default_version("2.0"));

		let builder1 = VersionedUrlBuilder::with_strategy(
			versioning1,
			"https://api.example.com",
			VersioningStrategy::URLPath,
		);
		let builder2 = VersionedUrlBuilder::with_strategy(
			versioning2,
			"https://api.example.com",
			VersioningStrategy::QueryParameter,
		);

		let manager = UrlReverseManager::new()
			.add_builder("url_path", builder1)
			.add_builder("query_param", builder2)
			.with_default_builder(VersionedUrlBuilder::with_strategy(
				Arc::new(URLPathVersioning::new().with_default_version("1.0")),
				"https://api.example.com",
				VersioningStrategy::URLPath,
			));

		let url1 = manager.build_url("url_path", "/users/").unwrap();
		assert_eq!(url1, "https://api.example.com/v1.0/users/");

		let url2 = manager.build_url("query_param", "/users/").unwrap();
		assert_eq!(url2, "https://api.example.com/users/?version=2.0");

		let default_url = manager.build_default_url("/users/").unwrap();
		assert_eq!(default_url, "https://api.example.com/v1.0/users/");
	}

	#[test]
	fn test_api_doc_url_builder() {
		let builder = ApiDocUrlBuilder::new("https://api.example.com", "1.0")
			.with_format(ApiDocFormat::OpenApi);

		let url = builder.build();
		assert_eq!(url, "https://api.example.com/v1.0/openapi.json");

		let builder = builder.with_format(ApiDocFormat::Swagger);
		let url = builder.build();
		assert_eq!(url, "https://api.example.com/v1.0/swagger-ui/");

		let builder = builder.with_format(ApiDocFormat::Custom("docs".to_string()));
		let url = builder.build();
		assert_eq!(url, "https://api.example.com/v1.0/docs");
	}

	#[test]
	fn test_versioned_url_macro() {
		let versioning = Arc::new(URLPathVersioning::new().with_default_version("1.0"));
		let builder = VersionedUrlBuilder::new(versioning, "https://api.example.com");

		let url = versioned_url!(builder, "/users/");
		assert_eq!(url, "https://api.example.com/v1.0/users/");

		let url = versioned_url!(builder, "/users/", "2.0");
		assert_eq!(url, "https://api.example.com/v2.0/users/");
	}
}
