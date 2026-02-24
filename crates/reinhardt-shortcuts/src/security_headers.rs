//! Security headers shortcut functions
//!
//! Provides a convenient function for applying common security-related HTTP response headers.
//! These headers help protect against common web vulnerabilities such as MIME sniffing,
//! clickjacking, and cross-site scripting.

use reinhardt_http::Response;

/// Apply common security headers to an HTTP response.
///
/// Sets the following headers on the response:
///
/// - `X-Content-Type-Options: nosniff` — Prevents MIME type sniffing, ensuring the browser
///   respects the declared `Content-Type`.
/// - `X-Frame-Options: DENY` — Blocks the response from being embedded in `<frame>`,
///   `<iframe>`, or `<object>` elements, protecting against clickjacking attacks.
/// - `Referrer-Policy: strict-origin-when-cross-origin` — Sends the full URL as the
///   `Referer` header for same-origin requests, but only the origin for cross-origin
///   requests. No `Referer` is sent for downgrades (HTTPS -> HTTP).
/// - `X-XSS-Protection: 1; mode=block` — Enables the legacy XSS filter in older browsers
///   and instructs them to block rather than sanitize detected attacks.
///
/// # Examples
///
/// ```
/// use reinhardt_shortcuts::{render_html, security_headers};
///
/// let response = render_html("<h1>Hello</h1>");
/// let response = security_headers(response);
///
/// assert_eq!(
///     response.headers.get("x-content-type-options").unwrap().to_str().unwrap(),
///     "nosniff"
/// );
/// assert_eq!(
///     response.headers.get("x-frame-options").unwrap().to_str().unwrap(),
///     "DENY"
/// );
/// assert_eq!(
///     response.headers.get("referrer-policy").unwrap().to_str().unwrap(),
///     "strict-origin-when-cross-origin"
/// );
/// assert_eq!(
///     response.headers.get("x-xss-protection").unwrap().to_str().unwrap(),
///     "1; mode=block"
/// );
/// ```
///
/// # Arguments
///
/// * `response` - The HTTP response to add security headers to.
///
/// # Returns
///
/// The same `Response` with security headers applied.
pub fn security_headers(mut response: Response) -> Response {
	response
		.headers
		.insert("x-content-type-options", "nosniff".parse().unwrap());
	response
		.headers
		.insert("x-frame-options", "DENY".parse().unwrap());
	response.headers.insert(
		"referrer-policy",
		"strict-origin-when-cross-origin".parse().unwrap(),
	);
	response
		.headers
		.insert("x-xss-protection", "1; mode=block".parse().unwrap());
	response
}

#[cfg(test)]
mod tests {
	use super::*;
	use hyper::StatusCode;
	use rstest::rstest;

	#[rstest]
	fn test_security_headers_x_content_type_options() {
		// Arrange
		let response = Response::ok();

		// Act
		let response = security_headers(response);

		// Assert
		assert_eq!(
			response
				.headers
				.get("x-content-type-options")
				.unwrap()
				.to_str()
				.unwrap(),
			"nosniff"
		);
	}

	#[rstest]
	fn test_security_headers_x_frame_options() {
		// Arrange
		let response = Response::ok();

		// Act
		let response = security_headers(response);

		// Assert
		assert_eq!(
			response
				.headers
				.get("x-frame-options")
				.unwrap()
				.to_str()
				.unwrap(),
			"DENY"
		);
	}

	#[rstest]
	fn test_security_headers_referrer_policy() {
		// Arrange
		let response = Response::ok();

		// Act
		let response = security_headers(response);

		// Assert
		assert_eq!(
			response
				.headers
				.get("referrer-policy")
				.unwrap()
				.to_str()
				.unwrap(),
			"strict-origin-when-cross-origin"
		);
	}

	#[rstest]
	fn test_security_headers_x_xss_protection() {
		// Arrange
		let response = Response::ok();

		// Act
		let response = security_headers(response);

		// Assert
		assert_eq!(
			response
				.headers
				.get("x-xss-protection")
				.unwrap()
				.to_str()
				.unwrap(),
			"1; mode=block"
		);
	}

	#[rstest]
	fn test_security_headers_preserves_status_code() {
		// Arrange
		let response = Response::not_found();

		// Act
		let response = security_headers(response);

		// Assert
		assert_eq!(response.status, StatusCode::NOT_FOUND);
	}

	#[rstest]
	fn test_security_headers_preserves_existing_headers() {
		// Arrange
		let mut response = Response::ok();
		response
			.headers
			.insert("content-type", "text/html; charset=utf-8".parse().unwrap());

		// Act
		let response = security_headers(response);

		// Assert
		assert_eq!(
			response
				.headers
				.get("content-type")
				.unwrap()
				.to_str()
				.unwrap(),
			"text/html; charset=utf-8"
		);
		// Security headers must also be present
		assert!(response.headers.get("x-content-type-options").is_some());
		assert!(response.headers.get("x-frame-options").is_some());
		assert!(response.headers.get("referrer-policy").is_some());
		assert!(response.headers.get("x-xss-protection").is_some());
	}

	#[rstest]
	fn test_security_headers_all_four_headers_present() {
		// Arrange
		let response = Response::ok();

		// Act
		let response = security_headers(response);

		// Assert - verify all four security headers are set
		assert_eq!(
			response
				.headers
				.get("x-content-type-options")
				.unwrap()
				.to_str()
				.unwrap(),
			"nosniff"
		);
		assert_eq!(
			response
				.headers
				.get("x-frame-options")
				.unwrap()
				.to_str()
				.unwrap(),
			"DENY"
		);
		assert_eq!(
			response
				.headers
				.get("referrer-policy")
				.unwrap()
				.to_str()
				.unwrap(),
			"strict-origin-when-cross-origin"
		);
		assert_eq!(
			response
				.headers
				.get("x-xss-protection")
				.unwrap()
				.to_str()
				.unwrap(),
			"1; mode=block"
		);
	}
}
