//! Test: Server function marker exposes declared response metadata.
//!
//! This test verifies that `#[server_fn]` implements
//! `ServerFnResponseMetadata` for the generated marker and that the associated
//! `Response` type is the declared `Ok` type from the server function result.

use reinhardt_pages::server_fn::ServerFnResponseMetadata;
use reinhardt_pages::server_fn::{ServerFnMetadata, ServerFnRegistration};
use reinhardt_pages_macros::server_fn;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerFnError(String);

impl std::fmt::Display for ServerFnError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl std::error::Error for ServerFnError {}

impl From<reinhardt_pages::server_fn::ServerFnError> for ServerFnError {
	fn from(err: reinhardt_pages::server_fn::ServerFnError) -> Self {
		ServerFnError(format!("Client error: {}", err))
	}
}

impl From<serde_json::Error> for ServerFnError {
	fn from(err: serde_json::Error) -> Self {
		ServerFnError(format!("Serialization error: {}", err))
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeclaredResponse {
	value: String,
}

#[server_fn]
pub async fn response_metadata_sample(value: String) -> Result<DeclaredResponse, ServerFnError> {
	Ok(DeclaredResponse { value })
}

type GeneratedResponse =
	<response_metadata_sample::marker as ServerFnResponseMetadata>::Response;
type GeneratedError = <response_metadata_sample::marker as ServerFnResponseMetadata>::Error;

fn assert_response_metadata<T>()
where
	T: ServerFnResponseMetadata<Response = DeclaredResponse, Error = ServerFnError>,
{
}

fn assert_declared_response(value: GeneratedResponse) -> DeclaredResponse {
	value
}

fn assert_declared_error(value: GeneratedError) -> ServerFnError {
	value
}

mod scoped {
	use super::*;

	pub(super) mod endpoint {
		use super::*;

		#[derive(Debug, Serialize)]
		struct ScopedResponse {
			value: String,
		}

		#[derive(Debug, Serialize, Deserialize)]
		pub(super) struct ScopedVisibleResponse {
			value: String,
		}

		#[derive(Debug, Serialize)]
		struct ScopedPrivateResponse {
			value: String,
		}

		#[server_fn]
		// This fixture intentionally returns a private type to verify metadata can be skipped.
		#[allow(private_interfaces)]
		pub(super) async fn scoped_response_metadata_sample(
			value: String,
		) -> Result<ScopedResponse, ServerFnError> {
			Ok(ScopedResponse { value })
		}

		#[server_fn]
		pub(super) async fn scoped_visible_response_metadata_sample(
			value: String,
		) -> Result<ScopedVisibleResponse, ServerFnError> {
			Ok(ScopedVisibleResponse { value })
		}

		#[server_fn]
		// This fixture intentionally keeps its response private to the endpoint module.
		#[allow(private_interfaces)]
		pub(super) async fn scoped_private_response_metadata_sample(
			value: String,
		) -> Result<ScopedPrivateResponse, ServerFnError> {
			Ok(ScopedPrivateResponse { value })
		}

		pub(super) fn assert_scoped_visible_marker_is_nameable() {
			let _marker = scoped_visible_response_metadata_sample::marker;
			let _handler =
				<scoped_visible_response_metadata_sample::marker as ServerFnRegistration>::handler();
		}

		pub(super) fn assert_scoped_private_marker_is_nameable() {
			let _marker = scoped_private_response_metadata_sample::marker;
			let _handler =
				<scoped_private_response_metadata_sample::marker as ServerFnRegistration>::handler();
		}
	}

	pub fn assert_scoped_marker_is_nameable() {
		endpoint::assert_scoped_visible_marker_is_nameable();
		endpoint::assert_scoped_private_marker_is_nameable();
		let _marker = endpoint::scoped_response_metadata_sample::marker;
		assert_eq!(
			<endpoint::scoped_response_metadata_sample::marker as ServerFnMetadata>::NAME,
			"scoped_response_metadata_sample"
		);
		let _handler = <endpoint::scoped_response_metadata_sample::marker as ServerFnRegistration>::handler();
	}
}

fn main() {
	assert_response_metadata::<response_metadata_sample::marker>();
	let _assert_declared_response: fn(GeneratedResponse) -> DeclaredResponse =
		assert_declared_response;
	let _assert_declared_error: fn(GeneratedError) -> ServerFnError = assert_declared_error;
	scoped::assert_scoped_marker_is_nameable();
}
