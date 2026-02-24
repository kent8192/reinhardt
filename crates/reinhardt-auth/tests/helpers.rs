//! Test helpers for social authentication tests

#![cfg(feature = "social")]

#[path = "helpers/assertions.rs"]
pub mod assertions;
#[path = "helpers/mock_server.rs"]
pub mod mock_server;
#[path = "helpers/test_fixtures.rs"]
pub mod test_fixtures;

// Re-export commonly used helpers
pub use assertions::{
	assert_authorization_url_valid, assert_claims_has_email, assert_id_token_valid,
	assert_pkce_challenge_valid, assert_state_not_expired, assert_token_response_valid,
};
pub use mock_server::{ErrorMode, MockConfig, MockOAuth2Server};
pub use test_fixtures::TestFixtures;
