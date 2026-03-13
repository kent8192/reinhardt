//! Endpoint URL validation for OAuth2/OIDC flows
//!
//! Ensures endpoint URLs use HTTPS to prevent cleartext transmission of
//! sensitive credentials (access tokens, authorization codes, client secrets).


use url::{Host, Url};

use super::core::SocialAuthError;

/// Returns a sanitized representation of a URL containing only
/// scheme, host, and port. Avoids leaking sensitive components
/// such as userinfo, path, or query parameters in error messages.
fn sanitize_url(parsed: &Url) -> String {
	let scheme = parsed.scheme();
	let host = parsed.host_str().unwrap_or("unknown");
	match parsed.port_or_known_default() {
		Some(port) => format!("{}://{}:{}", scheme, host, port),
		None => format!("{}://{}", scheme, host),
	}
}

/// Validates that an endpoint URL uses a secure transport scheme.
///
/// Requires HTTPS for all endpoints. HTTP is permitted only for loopback
/// addresses (`localhost`, `127.0.0.1`, `[::1]`) to support local development.
///
/// # Errors
///
/// Returns [`SocialAuthError::InsecureEndpoint`] if the URL uses an insecure scheme
/// or [`SocialAuthError::Configuration`] if the URL cannot be parsed.
pub(crate) fn validate_endpoint_url(url: &str) -> Result<(), SocialAuthError> {
	let parsed = Url::parse(url)
		.map_err(|e| SocialAuthError::Configuration(format!("invalid endpoint URL: {}", e)))?;

	match parsed.scheme() {
		"https" => Ok(()),
		"http" if is_loopback(&parsed) => Ok(()),
		scheme => Err(SocialAuthError::InsecureEndpoint(format!(
			"endpoint '{}' uses insecure scheme '{}': HTTPS is required",
			sanitize_url(&parsed),
			scheme
		))),
	}
}

/// Checks whether the URL's host is a loopback address using
/// network-level detection to catch all loopback variants
/// (e.g. `127.0.0.2`, `127.255.255.255`).
fn is_loopback(parsed: &Url) -> bool {
	match parsed.host() {
		Some(Host::Domain("localhost")) => true,
		Some(Host::Ipv4(addr)) => addr.is_loopback(),
		Some(Host::Ipv6(addr)) => addr.is_loopback(),
		_ => false,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[case("https://accounts.google.com/o/oauth2/token")]
	#[case("https://login.microsoftonline.com/common/oauth2/v2.0/token")]
	#[case("https://github.com/login/oauth/access_token")]
	fn test_https_urls_are_accepted(#[case] url: &str) {
		// Arrange
		// (URL provided via test parameter)

		// Act
		let result = validate_endpoint_url(url);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[case("http://localhost:8080/callback")]
	#[case("http://127.0.0.1:3000/auth")]
	#[case("http://[::1]:8080/token")]
	#[case("http://localhost/token")]
	#[case("http://127.0.0.2:8080/callback")]
	#[case("http://127.255.255.255/token")]
	fn test_http_loopback_is_accepted(#[case] url: &str) {
		// Arrange
		// (URL provided via test parameter)

		// Act
		let result = validate_endpoint_url(url);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[case("http://example.com/token")]
	#[case("http://evil.com/steal-tokens")]
	#[case("http://192.168.1.1/token")]
	fn test_http_non_loopback_is_rejected(#[case] url: &str) {
		// Arrange
		// (URL provided via test parameter)

		// Act
		let result = validate_endpoint_url(url);

		// Assert
		let err = result.unwrap_err();
		assert!(matches!(err, SocialAuthError::InsecureEndpoint(_)));
	}

	#[rstest]
	#[case("ftp://example.com/token")]
	#[case("ws://example.com/socket")]
	fn test_non_http_schemes_are_rejected(#[case] url: &str) {
		// Arrange
		// (URL provided via test parameter)

		// Act
		let result = validate_endpoint_url(url);

		// Assert
		let err = result.unwrap_err();
		assert!(matches!(err, SocialAuthError::InsecureEndpoint(_)));
	}

	#[rstest]
	fn test_error_messages_do_not_leak_sensitive_url_components() {
		// Arrange
		let url = "http://user:password@example.com/token?secret=abc123";

		// Act
		let result = validate_endpoint_url(url);

		// Assert
		let err = result.unwrap_err();
		let message = format!("{}", err);
		assert!(
			!message.contains("password"),
			"Error message should not contain userinfo: {}",
			message
		);
		assert!(
			!message.contains("secret=abc123"),
			"Error message should not contain query params: {}",
			message
		);
	}

	#[rstest]
	#[case("not a url")]
	#[case("")]
	fn test_invalid_urls_are_rejected(#[case] url: &str) {
		// Arrange
		// (URL provided via test parameter)

		// Act
		let result = validate_endpoint_url(url);

		// Assert
		let err = result.unwrap_err();
		assert!(matches!(err, SocialAuthError::Configuration(_)));
	}
}
