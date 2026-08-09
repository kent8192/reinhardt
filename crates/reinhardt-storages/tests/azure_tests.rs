//! Integration tests for Azure Blob Storage using Azurite.
//!
//! Gated on the `azure` feature: the fixtures and assertions used here are only
//! compiled when `azure` is enabled, so the whole test binary is conditional.
#![cfg(feature = "azure")]

mod fixtures;
mod utils;

use fixtures::azure_fixture;
use reinhardt_storages::config::AzureConfig;
use reinhardt_storages::{StorageBackend, StorageConfig, StorageError, create_storage};
use serial_test::serial;
use utils::{assert_azure_signed_url, assert_file_size, assert_storage_not_exists};
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
#[serial(azure)]
async fn azure_save_open_delete_roundtrip() {
	let fixture = azure_fixture().await;
	let name = "path/to/file.bin";
	let content = vec![0, 1, 2, 3, 254, 255];

	assert_eq!(fixture.backend.save(name, &content).await.unwrap(), name);
	assert!(fixture.backend.exists(name).await.unwrap());
	assert_eq!(fixture.backend.open(name).await.unwrap(), content);
	assert_file_size(&*fixture.backend, name, 6).await.unwrap();

	let modified = fixture.backend.get_modified_time(name).await.unwrap();
	assert!(modified.timestamp() > 0);

	fixture.backend.delete(name).await.unwrap();
	assert_storage_not_exists(&*fixture.backend, name)
		.await
		.unwrap();
}

#[tokio::test]
#[serial(azure)]
async fn azure_overwrites_and_handles_empty_files() {
	let fixture = azure_fixture().await;
	let name = "empty.txt";

	fixture.backend.save(name, b"first").await.unwrap();
	fixture.backend.save(name, b"").await.unwrap();

	assert_eq!(fixture.backend.open(name).await.unwrap(), Vec::<u8>::new());
	assert_eq!(fixture.backend.size(name).await.unwrap(), 0);

	// Cleanup
	fixture.backend.delete(name).await.unwrap();
}

#[tokio::test]
#[serial(azure)]
async fn azure_missing_object_errors_are_not_found() {
	let fixture = azure_fixture().await;
	let name = "missing.txt";

	assert!(!fixture.backend.exists(name).await.unwrap());
	assert!(matches!(
		fixture.backend.open(name).await,
		Err(StorageError::NotFound(_))
	));
	assert!(matches!(
		fixture.backend.delete(name).await,
		Err(StorageError::NotFound(_))
	));
	assert!(matches!(
		fixture.backend.url(name, 60).await,
		Err(StorageError::NotFound(_))
	));
	assert!(matches!(
		fixture.backend.size(name).await,
		Err(StorageError::NotFound(_))
	));
	assert!(matches!(
		fixture.backend.get_modified_time(name).await,
		Err(StorageError::NotFound(_))
	));
}

#[tokio::test]
#[serial(azure)]
async fn azure_generates_sas_url_shape() {
	let fixture = azure_fixture().await;
	let name = "signed.txt";
	fixture.backend.save(name, b"signed content").await.unwrap();

	let url = fixture.backend.url(name, 300).await.unwrap();

	assert_azure_signed_url(&url).unwrap();
	let response = reqwest::get(url).await.unwrap();
	assert!(
		response.status().is_success(),
		"Azurite accepts the generated SAS URL: {}",
		response.status()
	);
	assert_eq!(response.bytes().await.unwrap().as_ref(), b"signed content");

	// Cleanup
	fixture.backend.delete(name).await.unwrap();
}

#[tokio::test]
async fn azure_exclusive_save_sends_if_none_match_and_returns_the_logical_name() {
	let server = MockServer::start().await;
	Mock::given(any())
		.respond_with(ResponseTemplate::new(201))
		.mount(&server)
		.await;
	let backend = create_storage(StorageConfig::Azure(AzureConfig {
		account: "testaccount".to_string(),
		container: "testcontainer".to_string(),
		prefix: Some("configured-prefix".to_string()),
		endpoint: Some(format!("{}/testaccount", server.uri())),
		access_key: Some(fixtures::AZURITE_KEY.into()),
		sas_token: None,
		connection_string: None,
	}))
	.await
	.expect("custom endpoint should create an Azure backend");

	assert!(backend.capabilities().exclusive_create);
	assert_eq!(
		backend
			.save_if_absent("avatars/a.png", b"content")
			.await
			.expect("conditional Azure save should succeed"),
		"avatars/a.png"
	);

	let requests = server
		.received_requests()
		.await
		.expect("mock server should retain the blob upload request");
	let request = requests
		.iter()
		.find(|request| {
			request
				.url
				.as_str()
				.contains("configured%2Dprefix%2Favatars%2Fa%2Epng")
		})
		.unwrap_or_else(|| {
			panic!(
				"conditional Azure save should upload a blob; received URLs: {:?}",
				requests
					.iter()
					.map(|request| request.url.as_str())
					.collect::<Vec<_>>()
			)
		});
	assert_eq!(request.headers["if-none-match"], "*");
}

#[tokio::test]
async fn azure_exclusive_save_maps_precondition_conflicts_without_changing_save_errors() {
	for status in [409, 412] {
		let server = MockServer::start().await;
		Mock::given(any())
			.respond_with(move |request: &wiremock::Request| {
				if request
					.url
					.query_pairs()
					.any(|(key, value)| key == "restype" && value == "container")
				{
					ResponseTemplate::new(201)
				} else {
					ResponseTemplate::new(status)
				}
			})
			.mount(&server)
			.await;
		let backend = create_storage(StorageConfig::Azure(AzureConfig {
			account: "testaccount".to_string(),
			container: "testcontainer".to_string(),
			prefix: None,
			endpoint: Some(format!("{}/testaccount", server.uri())),
			access_key: Some(fixtures::AZURITE_KEY.into()),
			sas_token: None,
			connection_string: None,
		}))
		.await
		.expect("custom endpoint should create an Azure backend");

		assert!(matches!(
			backend.save_if_absent("existing.txt", b"content").await,
			Err(StorageError::AlreadyExists(name)) if name == "existing.txt"
		));
		assert!(matches!(
			backend.save("existing.txt", b"content").await,
			Err(StorageError::Other(_))
		));
	}
}
