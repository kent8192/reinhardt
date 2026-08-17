//! Provider-to-storage error mapping integration tests.

use reinhardt_providers::ProviderError;
use reinhardt_storages::StorageError;

#[test]
fn maps_provider_errors_to_storage_contract() {
	let cases = [
		(
			ProviderError::Config("invalid credentials".to_string()),
			StorageError::ConfigError("invalid credentials".to_string()),
		),
		(
			ProviderError::NotFound("missing".to_string()),
			StorageError::Other("provider resource not found".to_string()),
		),
		(
			ProviderError::PermissionDenied("denied".to_string()),
			StorageError::PermissionDenied("denied".to_string()),
		),
		(
			ProviderError::Service {
				status: 404,
				message: "missing".to_string(),
			},
			StorageError::NotFound("missing".to_string()),
		),
		(
			ProviderError::Service {
				status: 503,
				message: "unavailable".to_string(),
			},
			StorageError::NetworkError("unavailable".to_string()),
		),
		(
			ProviderError::Header("bad header".to_string()),
			StorageError::NetworkError("bad header".to_string()),
		),
	];

	for (provider_error, expected) in cases {
		assert_eq!(
			StorageError::from(provider_error).to_string(),
			expected.to_string()
		);
	}

	let url_error = reqwest::Url::parse("://invalid").expect_err("URL should be invalid");
	let expected_message = url_error.to_string();
	let converted = StorageError::from(ProviderError::Url(url_error));
	assert!(matches!(
		converted,
		StorageError::ConfigError(message) if message == expected_message
	));
}

#[tokio::test]
async fn maps_provider_http_errors_to_network_errors() {
	let http_error = reqwest::Client::new()
		.get("http://[::1")
		.send()
		.await
		.expect_err("malformed URL should fail before a request is sent");
	let expected_message = http_error.to_string();
	let converted = StorageError::from(ProviderError::Http(http_error));

	assert!(matches!(
		converted,
		StorageError::NetworkError(message) if message == expected_message
	));
}
