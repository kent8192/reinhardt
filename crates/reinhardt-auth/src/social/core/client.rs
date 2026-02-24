//! HTTP client wrapper for OAuth2 requests

use reqwest::Client;
use std::time::Duration;

/// OAuth2 HTTP client
///
/// Wrapper around `reqwest::Client` with OAuth2-specific configuration.
/// Cloning is cheap since `reqwest::Client` uses an `Arc` internally.
#[derive(Clone)]
pub struct OAuth2Client {
	client: Client,
}

impl OAuth2Client {
	/// Create a new OAuth2 client
	pub fn new() -> Self {
		let client = Client::builder()
			.timeout(Duration::from_secs(30))
			.connect_timeout(Duration::from_secs(10))
			.build()
			.expect("Failed to build HTTP client");

		Self { client }
	}

	/// Get the underlying reqwest client
	pub fn client(&self) -> &Client {
		&self.client
	}
}

impl Default for OAuth2Client {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_client_creation() {
		let client = OAuth2Client::new();
		// Verify that client() returns a valid reference
		let _client_ref = client.client();
	}

	#[test]
	fn test_client_default() {
		let client = OAuth2Client::default();
		// Verify that client() returns a valid reference
		let _client_ref = client.client();
	}
}
