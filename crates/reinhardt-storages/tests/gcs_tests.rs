//! Integration tests for Google Cloud Storage using fake-gcs-server.
//!
//! Gated on the `gcs` feature: the fixtures and assertions used here are only
//! compiled when `gcs` is enabled, so the whole test binary is conditional.
#![cfg(feature = "gcs")]

mod fixtures;
mod utils;

use fixtures::gcs_fixture;
use reinhardt_storages::config::GcsConfig;
use reinhardt_storages::{StorageBackend, StorageConfig, StorageError, create_storage};
use serial_test::serial;
use utils::{assert_file_size, assert_gcs_signed_url, assert_storage_not_exists};
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
#[serial(gcs)]
async fn gcs_save_open_delete_roundtrip() {
	let fixture = gcs_fixture().await;
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
#[serial(gcs)]
async fn gcs_overwrites_and_handles_empty_files() {
	let fixture = gcs_fixture().await;
	let name = "empty.txt";

	fixture.backend.save(name, b"first").await.unwrap();
	fixture.backend.save(name, b"").await.unwrap();

	assert_eq!(fixture.backend.open(name).await.unwrap(), Vec::<u8>::new());
	assert_eq!(fixture.backend.size(name).await.unwrap(), 0);

	// Cleanup
	fixture.backend.delete(name).await.unwrap();
}

#[tokio::test]
#[serial(gcs)]
async fn gcs_missing_object_errors_are_not_found() {
	let fixture = gcs_fixture().await;
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
#[serial(gcs)]
async fn gcs_generates_signed_url_shape() {
	let fixture = gcs_fixture().await;
	let name = "signed.txt";
	fixture.backend.save(name, b"signed content").await.unwrap();

	let url = fixture.backend.url(name, 300).await.unwrap();

	assert_gcs_signed_url(&url).unwrap();
	let response = reqwest::get(url).await.unwrap();
	assert!(
		response.status().is_success(),
		"fake-gcs-server accepts the generated emulator URL: {}",
		response.status()
	);
	assert_eq!(response.bytes().await.unwrap().as_ref(), b"signed content");

	// Cleanup
	fixture.backend.delete(name).await.unwrap();
}

#[tokio::test]
async fn gcs_exclusive_save_uses_zero_generation_and_returns_the_logical_name() {
	let server = MockServer::start().await;
	Mock::given(any())
		.respond_with(ResponseTemplate::new(200))
		.mount(&server)
		.await;
	let backend = create_storage(StorageConfig::Gcs(GcsConfig {
		bucket: "test-bucket".to_string(),
		prefix: Some("configured-prefix".to_string()),
		endpoint: Some(server.uri()),
		service_account_json: None,
	}))
	.await
	.expect("custom endpoint should create a GCS backend");

	assert!(backend.capabilities().exclusive_create);
	assert_eq!(
		backend
			.save_if_absent("avatars/a.png", b"content")
			.await
			.expect("conditional GCS save should succeed"),
		"avatars/a.png"
	);

	let requests = server
		.received_requests()
		.await
		.expect("mock server should retain the upload request");
	let request = requests
		.iter()
		.find(|request| request.method == wiremock::http::Method::POST)
		.expect("conditional GCS save should upload an object");
	assert_eq!(
		request.url.query_pairs().collect::<Vec<_>>(),
		vec![
			("uploadType".into(), "media".into()),
			("name".into(), "configured-prefix/avatars/a.png".into()),
			("ifGenerationMatch".into(), "0".into()),
		]
	);
}

#[tokio::test]
async fn gcs_exclusive_save_maps_precondition_conflicts_without_changing_save_errors() {
	for status in [412] {
		let server = MockServer::start().await;
		Mock::given(any())
			.respond_with(ResponseTemplate::new(status))
			.mount(&server)
			.await;
		let backend = create_storage(StorageConfig::Gcs(GcsConfig {
			bucket: "test-bucket".to_string(),
			prefix: None,
			endpoint: Some(server.uri()),
			service_account_json: None,
		}))
		.await
		.expect("custom endpoint should create a GCS backend");

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
