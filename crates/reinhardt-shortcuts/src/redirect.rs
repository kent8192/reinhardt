//! Redirect shortcut functions
//!
//! Provides convenient functions for creating HTTP redirects.
//! All redirect functions validate URLs to prevent open redirect attacks.

use std::collections::HashSet;

use crate::url::Url;
use reinhardt_core::security::redirect::{RedirectValidationError, validate_redirect_url};
use reinhardt_http::Response;

/// Create a validated temporary redirect (HTTP 302) to the specified URL.
///
/// Validates the redirect URL against a set of allowed hosts to prevent
/// open redirect attacks. Relative URLs (starting with `/`) are always allowed.
///
/// # Examples
///
/// ```
/// use reinhardt_shortcuts::redirect;
/// use std::collections::HashSet;
///
/// let allowed: HashSet<String> = HashSet::new();
///
/// // Relative URLs are always allowed
/// let response = redirect("/users/profile/", &allowed).unwrap();
/// assert_eq!(response.status, 302);
///
/// // External URLs must be in allowed_hosts
/// assert!(redirect("https://evil.com/", &allowed).is_err());
/// ```
///
/// # Arguments
///
/// * `to` - The URL to redirect to. Can be a relative or absolute URL.
/// * `allowed_hosts` - Set of allowed hostnames for absolute URL redirects.
///
/// # Errors
///
/// Returns `RedirectValidationError` if the URL fails validation.
pub fn redirect(
	to: impl AsRef<str>,
	allowed_hosts: &HashSet<String>,
) -> Result<Response, RedirectValidationError> {
	let url = to.as_ref();
	validate_redirect_url(url, allowed_hosts)?;
	Ok(Response::temporary_redirect(url))
}

/// Create a validated permanent redirect (HTTP 301) to the specified URL.
///
/// Validates the redirect URL against a set of allowed hosts to prevent
/// open redirect attacks. Relative URLs (starting with `/`) are always allowed.
///
/// Use this when a resource has permanently moved to a new location.
/// Search engines will update their indexes to point to the new URL.
///
/// # Examples
///
/// ```
/// use reinhardt_shortcuts::redirect_permanent;
/// use std::collections::HashSet;
///
/// let allowed: HashSet<String> = HashSet::new();
///
/// // Relative URLs are always allowed
/// let response = redirect_permanent("/new-page/", &allowed).unwrap();
/// assert_eq!(response.status, 301);
///
/// // External URLs must be in allowed_hosts
/// assert!(redirect_permanent("https://evil.com/", &allowed).is_err());
/// ```
///
/// # Arguments
///
/// * `to` - The URL to redirect to. Can be a relative or absolute URL.
/// * `allowed_hosts` - Set of allowed hostnames for absolute URL redirects.
///
/// # Errors
///
/// Returns `RedirectValidationError` if the URL fails validation.
pub fn redirect_permanent(
	to: impl AsRef<str>,
	allowed_hosts: &HashSet<String>,
) -> Result<Response, RedirectValidationError> {
	let url = to.as_ref();
	validate_redirect_url(url, allowed_hosts)?;
	Ok(Response::permanent_redirect(url))
}

/// Redirects to the specified URL (using `Url` type) with validation.
///
/// This variant provides type safety for the redirect URL.
///
/// # Examples
///
/// ```
/// use reinhardt_shortcuts::{redirect_to, Url};
/// use std::collections::HashSet;
///
/// let allowed: HashSet<String> = HashSet::new();
/// let url = Url::new("/home")?;
/// let response = redirect_to(url, &allowed).unwrap();
/// assert_eq!(response.status, 302);
/// # Ok::<(), reinhardt_shortcuts::UrlError>(())
/// ```
///
/// # Errors
///
/// Returns `RedirectValidationError` if the URL fails validation.
pub fn redirect_to(
	to: Url,
	allowed_hosts: &HashSet<String>,
) -> Result<Response, RedirectValidationError> {
	redirect(to.as_str(), allowed_hosts)
}

/// Redirects permanently to the specified URL (using `Url` type) with validation.
///
/// This variant provides type safety for the redirect URL.
///
/// # Examples
///
/// ```
/// use reinhardt_shortcuts::{redirect_permanent_to, Url};
/// use std::collections::HashSet;
///
/// let allowed: HashSet<String> = HashSet::new();
/// let url = Url::new("/new-location")?;
/// let response = redirect_permanent_to(url, &allowed).unwrap();
/// assert_eq!(response.status, 301);
/// # Ok::<(), reinhardt_shortcuts::UrlError>(())
/// ```
///
/// # Errors
///
/// Returns `RedirectValidationError` if the URL fails validation.
pub fn redirect_permanent_to(
	to: Url,
	allowed_hosts: &HashSet<String>,
) -> Result<Response, RedirectValidationError> {
	redirect_permanent(to.as_str(), allowed_hosts)
}

#[cfg(test)]
mod tests {
	use super::*;
	use hyper::StatusCode;
	use rstest::rstest;

	fn empty_hosts() -> HashSet<String> {
		HashSet::new()
	}

	fn allowed_hosts() -> HashSet<String> {
		["example.com"].iter().map(|s| s.to_string()).collect()
	}

	#[rstest]
	fn test_redirect_temporary_relative() {
		// Arrange
		let hosts = empty_hosts();

		// Act
		let response = redirect("/users/", &hosts).unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::FOUND);
		assert_eq!(
			response.headers.get("location").unwrap().to_str().unwrap(),
			"/users/"
		);
	}

	#[rstest]
	fn test_redirect_permanent_relative() {
		// Arrange
		let hosts = empty_hosts();

		// Act
		let response = redirect_permanent("/new-location/", &hosts).unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::MOVED_PERMANENTLY);
		assert_eq!(
			response.headers.get("location").unwrap().to_str().unwrap(),
			"/new-location/"
		);
	}

	#[rstest]
	fn test_redirect_allowed_absolute_url() {
		// Arrange
		let hosts = allowed_hosts();

		// Act
		let response = redirect("https://example.com/page/", &hosts).unwrap();

		// Assert
		assert_eq!(
			response.headers.get("location").unwrap().to_str().unwrap(),
			"https://example.com/page/"
		);
	}

	#[rstest]
	fn test_redirect_rejects_untrusted_host() {
		// Arrange
		let hosts = allowed_hosts();

		// Act
		let result = redirect("https://evil.com/phish", &hosts);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_redirect_rejects_javascript_protocol() {
		// Arrange
		let hosts = empty_hosts();

		// Act
		let result = redirect("javascript:alert(1)", &hosts);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_redirect_rejects_protocol_relative() {
		// Arrange
		let hosts = empty_hosts();

		// Act
		let result = redirect("//evil.com/path", &hosts);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_redirect_with_query_params() {
		// Arrange
		let hosts = empty_hosts();

		// Act
		let response = redirect("/search/?q=test&page=2", &hosts).unwrap();

		// Assert
		assert_eq!(
			response.headers.get("location").unwrap().to_str().unwrap(),
			"/search/?q=test&page=2"
		);
	}

	#[rstest]
	fn test_redirect_permanent_rejects_untrusted() {
		// Arrange
		let hosts = empty_hosts();

		// Act
		let result = redirect_permanent("https://evil.com/steal", &hosts);

		// Assert
		assert!(result.is_err());
	}
}
