//! Smoke tests for the public non-CLI server entrypoint surface (#4055).
//!
//! These tests verify that `auto_register_router` and `start_server` are
//! reachable from outside the crate and surface the expected error when no
//! `#[routes]` function is reachable in the test binary.

use rstest::*;

#[cfg(feature = "routers")]
#[rstest]
#[tokio::test]
async fn auto_register_router_is_public_and_reports_missing_routes() {
	// Arrange: integration-test binaries do not link any consumer-side
	// `#[routes]` registration, so the inventory walk must be empty.

	// Act
	let result = reinhardt_commands::auto_register_router().await;

	// Assert: function is reachable (public) and returns the documented
	// "no routes registered" diagnostic with the lib+bin hint.
	let err = result.expect_err("auto_register_router must error without #[routes]");
	let message = err.to_string();
	assert!(
		message.contains("No URL patterns registered"),
		"unexpected error message: {message}"
	);
	assert!(
		message.contains("library/binary split"),
		"missing lib+bin hint in error message: {message}"
	);
}

#[cfg(feature = "server")]
#[rstest]
#[tokio::test]
async fn start_server_is_public_and_propagates_route_registration_error() {
	// Arrange: no #[routes] registered in the integration-test binary, so
	// auto_register_router (called inside start_server) must fail before
	// the server is bound. Using port 0 would otherwise bind a real socket.
	let addr = "127.0.0.1:0";

	// Act
	let result = reinhardt_commands::start_server(addr).await;

	// Assert: failure must come from route registration, not from binding,
	// proving the helper performs auto-registration before delegating to
	// RunServerCommand.
	let err = result.expect_err("start_server must error without #[routes]");
	let message = err.to_string();
	assert!(
		message.contains("No URL patterns registered"),
		"start_server should surface auto_register_router error, got: {message}"
	);
}
