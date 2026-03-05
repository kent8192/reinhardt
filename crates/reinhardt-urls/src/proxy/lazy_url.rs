//! Lazy URL resolution for deferred URL generation
//!
//! This module provides Django-style lazy URL objects that defer URL resolution
//! until the actual URL string is needed.

use crate::proxy::url_resolver::UrlResolver;
use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};

/// A lazy URL that defers resolution until needed
#[derive(Clone)]
pub struct LazyUrl {
	/// Pattern name to resolve
	name: String,
	/// Parameters for URL generation
	kwargs: HashMap<String, String>,
	/// Shared resolver instance
	resolver: Arc<UrlResolver>,
	/// Cached resolved URL
	cached_url: Arc<RwLock<Option<String>>>,
}

impl LazyUrl {
	/// Creates a new lazy URL without parameters
	///
	/// # Arguments
	///
	/// * `name` - Pattern name to resolve (e.g., "home", "admin:users")
	/// * `resolver` - Shared resolver instance
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::{LazyUrl, UrlResolver, UrlPattern};
	/// use std::sync::Arc;
	///
	/// let mut resolver = UrlResolver::new();
	/// resolver.add_pattern(UrlPattern::new("home", "/", None));
	/// let resolver = Arc::new(resolver);
	///
	/// let home_url = LazyUrl::new("home", resolver.clone());
	/// assert_eq!(home_url.try_resolve().unwrap(), "/");
	/// ```
	pub fn new(name: impl Into<String>, resolver: Arc<UrlResolver>) -> Self {
		Self {
			name: name.into(),
			kwargs: HashMap::new(),
			resolver,
			cached_url: Arc::new(RwLock::new(None)),
		}
	}

	/// Creates a new lazy URL with parameters
	///
	/// # Arguments
	///
	/// * `name` - Pattern name to resolve
	/// * `kwargs` - Parameters for URL generation
	/// * `resolver` - Shared resolver instance
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::{LazyUrl, UrlResolver, UrlPattern};
	/// use std::sync::Arc;
	/// use std::collections::HashMap;
	///
	/// let mut resolver = UrlResolver::new();
	/// resolver.add_pattern(UrlPattern::new("user-detail", "/users/<id>/", None));
	/// let resolver = Arc::new(resolver);
	///
	/// let mut kwargs = HashMap::new();
	/// kwargs.insert("id".to_string(), "123".to_string());
	///
	/// let user_url = LazyUrl::with_kwargs("user-detail", kwargs, resolver.clone());
	/// assert_eq!(user_url.try_resolve().unwrap(), "/users/123/");
	/// ```
	pub fn with_kwargs(
		name: impl Into<String>,
		kwargs: HashMap<String, String>,
		resolver: Arc<UrlResolver>,
	) -> Self {
		Self {
			name: name.into(),
			kwargs,
			resolver,
			cached_url: Arc::new(RwLock::new(None)),
		}
	}

	/// Checks if the URL has been resolved
	///
	/// Returns `false` if the internal lock is poisoned (instead of panicking).
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::{LazyUrl, UrlResolver, UrlPattern};
	/// use std::sync::Arc;
	///
	/// let mut resolver = UrlResolver::new();
	/// resolver.add_pattern(UrlPattern::new("home", "/", None));
	/// let resolver = Arc::new(resolver);
	///
	/// let home_url = LazyUrl::new("home", resolver.clone());
	/// assert!(!home_url.is_resolved());
	///
	/// let _ = home_url.try_resolve();
	/// assert!(home_url.is_resolved());
	/// ```
	pub fn is_resolved(&self) -> bool {
		self.cached_url
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.is_some()
	}

	/// Attempts to resolve the URL, returning a `Result` instead of panicking.
	///
	/// This method will cache the result for subsequent calls.
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - Pattern name is not found in the resolver
	/// - Required parameters are missing
	/// - Internal lock is poisoned
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::{LazyUrl, UrlResolver, UrlPattern};
	/// use std::sync::Arc;
	///
	/// let mut resolver = UrlResolver::new();
	/// resolver.add_pattern(UrlPattern::new("home", "/", None));
	/// let resolver = Arc::new(resolver);
	///
	/// let home_url = LazyUrl::new("home", resolver.clone());
	/// let url = home_url.try_resolve().unwrap();
	/// assert_eq!(url, "/");
	///
	/// // Non-existent pattern returns an error
	/// let bad_url = LazyUrl::new("nonexistent", resolver.clone());
	/// assert!(bad_url.try_resolve().is_err());
	/// ```
	pub fn try_resolve(&self) -> Result<String, String> {
		// Check if already cached (recovers from lock poisoning)
		{
			let cached = self
				.cached_url
				.read()
				.unwrap_or_else(PoisonError::into_inner);
			if let Some(url) = cached.as_ref() {
				return Ok(url.clone());
			}
		}

		// Resolve the URL
		let url = self
			.resolver
			.reverse(&self.name, self.kwargs.clone())
			.map_err(|e| format!("Failed to resolve URL '{}': {}", self.name, e))?;

		// Cache the result (recovers from lock poisoning)
		{
			let mut cached = self
				.cached_url
				.write()
				.unwrap_or_else(PoisonError::into_inner);
			*cached = Some(url.clone());
		}

		Ok(url)
	}

	/// Resolves the URL to its string representation.
	///
	/// This is a convenience wrapper around [`try_resolve`](Self::try_resolve)
	/// that panics on failure. **Prefer `try_resolve` in library and production
	/// code** to avoid panics. This method is provided for cases where a missing
	/// URL pattern indicates a programming error that should be caught early.
	///
	/// # Panics
	///
	/// Panics if URL resolution fails (pattern not found or missing parameters).
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::{LazyUrl, UrlResolver, UrlPattern};
	/// use std::sync::Arc;
	///
	/// let mut resolver = UrlResolver::new();
	/// resolver.add_pattern(UrlPattern::new("home", "/", None));
	/// let resolver = Arc::new(resolver);
	///
	/// let home_url = LazyUrl::new("home", resolver.clone());
	/// assert_eq!(home_url.resolve(), "/");
	/// ```
	pub fn resolve(&self) -> String {
		self.try_resolve().unwrap_or_else(|e| panic!("{}", e))
	}
}

impl std::fmt::Display for LazyUrl {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self.try_resolve() {
			Ok(url) => write!(f, "{}", url),
			Err(e) => write!(f, "<unresolved: {}>", e),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::proxy::url_pattern::UrlPattern;
	use rstest::rstest;

	fn setup_resolver() -> Arc<UrlResolver> {
		let mut resolver = UrlResolver::new();
		resolver.add_pattern(UrlPattern::new("home", "/", None));
		resolver.add_pattern(UrlPattern::new("about", "/about/", None));
		resolver.add_pattern(UrlPattern::new("user-detail", "/users/<id>/", None));
		Arc::new(resolver)
	}

	#[rstest]
	fn test_lazy_url_creation() {
		// Arrange
		let resolver = setup_resolver();

		// Act
		let home_url = LazyUrl::new("home", resolver.clone());

		// Assert
		assert!(!home_url.is_resolved());
	}

	#[rstest]
	fn test_lazy_url_resolve() {
		// Arrange
		let resolver = setup_resolver();
		let home_url = LazyUrl::new("home", resolver.clone());

		// Act
		let url = home_url.resolve();

		// Assert
		assert_eq!(url, "/");
		assert!(home_url.is_resolved());
	}

	#[rstest]
	fn test_lazy_url_try_resolve_success() {
		// Arrange
		let resolver = setup_resolver();
		let home_url = LazyUrl::new("home", resolver.clone());

		// Act
		let result = home_url.try_resolve();

		// Assert
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), "/");
		assert!(home_url.is_resolved());
	}

	#[rstest]
	fn test_lazy_url_try_resolve_pattern_not_found() {
		// Arrange
		let resolver = setup_resolver();
		let invalid_url = LazyUrl::new("nonexistent", resolver.clone());

		// Act
		let result = invalid_url.try_resolve();

		// Assert
		assert!(result.is_err());
		assert_eq!(
			result.unwrap_err(),
			"Failed to resolve URL 'nonexistent': URL pattern 'nonexistent' not found"
		);
	}

	#[rstest]
	fn test_lazy_url_try_resolve_missing_parameter() {
		// Arrange
		let resolver = setup_resolver();
		let user_url = LazyUrl::new("user-detail", resolver.clone());

		// Act
		let result = user_url.try_resolve();

		// Assert
		assert!(result.is_err());
		assert!(result.unwrap_err().contains("Failed to resolve URL"));
	}

	#[rstest]
	fn test_lazy_url_with_kwargs() {
		// Arrange
		let resolver = setup_resolver();
		let mut kwargs = HashMap::new();
		kwargs.insert("id".to_string(), "123".to_string());

		// Act
		let user_url = LazyUrl::with_kwargs("user-detail", kwargs, resolver.clone());

		// Assert
		assert_eq!(user_url.resolve(), "/users/123/");
	}

	#[rstest]
	fn test_lazy_url_with_kwargs_try_resolve() {
		// Arrange
		let resolver = setup_resolver();
		let mut kwargs = HashMap::new();
		kwargs.insert("id".to_string(), "123".to_string());
		let user_url = LazyUrl::with_kwargs("user-detail", kwargs, resolver.clone());

		// Act
		let result = user_url.try_resolve();

		// Assert
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), "/users/123/");
	}

	#[rstest]
	fn test_lazy_url_caching() {
		// Arrange
		let resolver = setup_resolver();
		let home_url = LazyUrl::new("home", resolver.clone());

		// Act
		let url1 = home_url.resolve();
		let url2 = home_url.resolve();

		// Assert
		assert!(home_url.is_resolved());
		assert_eq!(url1, url2);
	}

	#[rstest]
	fn test_lazy_url_display_success() {
		// Arrange
		let resolver = setup_resolver();
		let home_url = LazyUrl::new("home", resolver.clone());

		// Act
		let display = format!("{}", home_url);

		// Assert
		assert_eq!(display, "/");
	}

	#[rstest]
	fn test_lazy_url_display_failure() {
		// Arrange
		let resolver = setup_resolver();
		let invalid_url = LazyUrl::new("nonexistent", resolver.clone());

		// Act
		let display = format!("{}", invalid_url);

		// Assert
		assert!(display.starts_with("<unresolved:"));
	}

	#[rstest]
	#[should_panic(expected = "Failed to resolve URL")]
	fn test_lazy_url_pattern_not_found() {
		// Arrange
		let resolver = setup_resolver();
		let invalid_url = LazyUrl::new("nonexistent", resolver.clone());

		// Act (panics)
		invalid_url.resolve();
	}

	#[rstest]
	#[should_panic(expected = "Failed to resolve URL")]
	fn test_lazy_url_missing_parameter() {
		// Arrange
		let resolver = setup_resolver();
		let user_url = LazyUrl::new("user-detail", resolver.clone());

		// Act (panics)
		user_url.resolve();
	}

	#[rstest]
	fn test_try_resolve_recovers_from_poisoned_cache_lock() {
		// Arrange
		let resolver = setup_resolver();
		let lazy_url = LazyUrl::new("home", resolver.clone());

		// Poison the cached_url RwLock by panicking while holding a write guard
		let cached_clone = lazy_url.cached_url.clone();
		let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			let _guard = cached_clone.write().unwrap();
			panic!("intentional panic to poison lock");
		}));

		// Act - try_resolve should recover from the poisoned lock
		let result = lazy_url.try_resolve();

		// Assert
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), "/");
	}

	#[rstest]
	fn test_is_resolved_recovers_from_poisoned_cache_lock() {
		// Arrange
		let resolver = setup_resolver();
		let lazy_url = LazyUrl::new("home", resolver.clone());

		// Poison the cached_url RwLock
		let cached_clone = lazy_url.cached_url.clone();
		let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			let _guard = cached_clone.write().unwrap();
			panic!("intentional panic to poison lock");
		}));

		// Act - is_resolved should not panic on poisoned lock
		let resolved = lazy_url.is_resolved();

		// Assert
		assert!(!resolved);
	}
}
