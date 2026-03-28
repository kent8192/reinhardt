//! Configuration and environment parsing tests.

use reinhardt_storages::{StorageConfig, StorageError};
use rstest::rstest;
use serial_test::serial;
use std::env;

/// Helper function to set environment variable and run closure.
///
/// # Safety
/// Tests using this function must be marked with `#[serial(env)]` to prevent
/// concurrent environment variable access.
async fn with_env<F, Fut>(key: &str, value: &str, f: F) -> Fut::Output
where
	F: FnOnce() -> Fut,
	Fut: std::future::Future,
{
	// SAFETY: Tests run sequentially with #[serial(env)], preventing concurrent env access
	unsafe { env::set_var(key, value) };
	let result = f().await;
	// SAFETY: Tests run sequentially with #[serial(env)], preventing concurrent env access
	unsafe { env::remove_var(key) };
	result
}

/// Helper function to set multiple environment variables.
///
/// # Safety
/// Tests using this function must be marked with `#[serial(env)]` to prevent
/// concurrent environment variable access.
async fn with_envs<F, Fut>(vars: &[(&str, &str)], f: F) -> Fut::Output
where
	F: FnOnce() -> Fut,
	Fut: std::future::Future,
{
	for (key, value) in vars {
		// SAFETY: Tests run sequentially with #[serial(env)], preventing concurrent env access
		unsafe { env::set_var(key, value) };
	}
	let result = f().await;
	for (key, _) in vars {
		// SAFETY: Tests run sequentially with #[serial(env)], preventing concurrent env access
		unsafe { env::remove_var(key) };
	}
	result
}

// ============================================================================
// Environment Parsing Tests
// ============================================================================

mod env_parsing_tests {
	use super::*;

	#[rstest]
	#[tokio::test]
	#[serial(env)]
	async fn test_s3_config_from_env() {
		with_envs(
			&[
				("STORAGE_BACKEND", "s3"),
				("S3_BUCKET", "test-bucket"),
				("S3_REGION", "us-east-1"),
				("S3_ENDPOINT", "http://localhost:4566"),
				("S3_PREFIX", "test-prefix"),
			],
			|| async {
				let config = StorageConfig::from_env().expect("Failed to load config");

				match config {
					StorageConfig::S3(s3_config) => {
						assert_eq!(s3_config.bucket, "test-bucket");
						assert_eq!(s3_config.region, Some("us-east-1".to_string()));
						assert_eq!(
							s3_config.endpoint,
							Some("http://localhost:4566".to_string())
						);
						assert_eq!(s3_config.prefix, Some("test-prefix".to_string()));
					}
					_ => panic!("Expected S3 config"),
				}
			},
		)
		.await;
	}

	#[rstest]
	#[tokio::test]
	#[serial(env)]
	async fn test_local_config_from_env() {
		with_envs(
			&[
				("STORAGE_BACKEND", "local"),
				("LOCAL_BASE_PATH", "/tmp/test-storage"),
			],
			|| async {
				let config = StorageConfig::from_env().expect("Failed to load config");

				match config {
					StorageConfig::Local(local_config) => {
						assert_eq!(local_config.base_path, "/tmp/test-storage");
					}
					_ => panic!("Expected Local config"),
				}
			},
		)
		.await;
	}

	#[rstest]
	#[tokio::test]
	#[serial(env)]
	async fn test_s3_config_with_minimal_options() {
		with_envs(
			&[("STORAGE_BACKEND", "s3"), ("S3_BUCKET", "test-bucket")],
			|| async {
				let config = StorageConfig::from_env().expect("Failed to load config");

				match config {
					StorageConfig::S3(s3_config) => {
						assert_eq!(s3_config.bucket, "test-bucket");
						assert_eq!(s3_config.region, None);
						assert_eq!(s3_config.endpoint, None);
						assert_eq!(s3_config.prefix, None);
					}
					_ => panic!("Expected S3 config"),
				}
			},
		)
		.await;
	}

	#[rstest]
	#[tokio::test]
	#[serial(env)]
	async fn test_s3_config_without_bucket() {
		with_env("STORAGE_BACKEND", "s3", || async {
			let result = StorageConfig::from_env();
			assert!(result.is_err());
			assert!(matches!(result, Err(StorageError::ConfigError(_))));
		})
		.await;
	}

	#[rstest]
	#[tokio::test]
	#[serial(env)]
	async fn test_local_config_without_base_path() {
		with_env("STORAGE_BACKEND", "local", || async {
			let result = StorageConfig::from_env();
			assert!(result.is_err());
			assert!(matches!(result, Err(StorageError::ConfigError(_))));
		})
		.await;
	}

	#[rstest]
	#[tokio::test]
	#[serial(env)]
	async fn test_backend_type_case_insensitive() {
		for backend_type in &["s3", "S3", "S3"] {
			with_envs(
				&[
					("STORAGE_BACKEND", backend_type),
					("S3_BUCKET", "test-bucket"),
				],
				|| async {
					let config = StorageConfig::from_env().expect("Failed to load config");
					match config {
						StorageConfig::S3(_) => {
							// Success
						}
						_ => panic!("Expected S3 config"),
					}
				},
			)
			.await;
		}
	}
}

// ============================================================================
// Config Validation Tests
// ============================================================================

mod validation_tests {
	use super::*;

	#[rstest]
	#[tokio::test]
	#[serial(env)]
	async fn test_missing_storage_backend() {
		// Ensure STORAGE_BACKEND is not set
		// SAFETY: Tests run sequentially with #[serial(env)], preventing concurrent env access
		unsafe { env::remove_var("STORAGE_BACKEND") };
		let result = StorageConfig::from_env();
		assert!(result.is_err());
		assert!(matches!(result, Err(StorageError::ConfigError(_))));
	}

	#[rstest]
	#[tokio::test]
	#[serial(env)]
	async fn test_invalid_backend_type() {
		with_env("STORAGE_BACKEND", "invalid", || async {
			let result = StorageConfig::from_env();
			assert!(result.is_err());
			assert!(matches!(result, Err(StorageError::ConfigError(_))));
		})
		.await;
	}

	#[rstest]
	#[tokio::test]
	#[serial(env)]
	async fn test_missing_required_s3_vars() {
		with_env("STORAGE_BACKEND", "s3", || async {
			let result = StorageConfig::from_env();
			assert!(result.is_err());
			assert!(matches!(result, Err(StorageError::ConfigError(_))));
		})
		.await;
	}

	#[rstest]
	#[tokio::test]
	#[serial(env)]
	async fn test_missing_required_local_vars() {
		with_env("STORAGE_BACKEND", "local", || async {
			let result = StorageConfig::from_env();
			assert!(result.is_err());
			assert!(matches!(result, Err(StorageError::ConfigError(_))));
		})
		.await;
	}
}

// ============================================================================
// BackendType Tests
// ============================================================================

mod backend_type_tests {
	use super::*;
	use reinhardt_storages::config::BackendType;
	use std::str::FromStr;

	#[rstest]
	fn test_parse_s3_lowercase() {
		let backend_type = BackendType::from_str("s3");
		assert!(matches!(backend_type, Ok(BackendType::S3)));
	}

	#[rstest]
	fn test_parse_s3_uppercase() {
		let backend_type = BackendType::from_str("S3");
		assert!(matches!(backend_type, Ok(BackendType::S3)));
	}

	#[rstest]
	fn test_parse_local_lowercase() {
		let backend_type = BackendType::from_str("local");
		assert!(matches!(backend_type, Ok(BackendType::Local)));
	}

	#[rstest]
	fn test_parse_local_uppercase() {
		let backend_type = BackendType::from_str("LOCAL");
		assert!(matches!(backend_type, Ok(BackendType::Local)));
	}

	#[rstest]
	fn test_parse_gcs_lowercase() {
		let backend_type = BackendType::from_str("gcs");
		assert!(matches!(backend_type, Ok(BackendType::Gcs)));
	}

	#[rstest]
	fn test_parse_azure_lowercase() {
		let backend_type = BackendType::from_str("azure");
		assert!(matches!(backend_type, Ok(BackendType::Azure)));
	}

	#[rstest]
	fn test_parse_invalid_type() {
		let backend_type = BackendType::from_str("invalid");
		assert!(backend_type.is_err());
	}

	#[rstest]
	fn test_parse_mixed_case() {
		let backend_type = BackendType::from_str("S3");
		assert!(matches!(backend_type, Ok(BackendType::S3)));

		let backend_type2 = BackendType::from_str("Local");
		assert!(matches!(backend_type2, Ok(BackendType::Local)));
	}

	#[rstest]
	fn test_backend_type_display() {
		assert_eq!(format!("{}", BackendType::S3), "S3");
		assert_eq!(format!("{}", BackendType::Local), "Local");
	}
}

// ============================================================================
// Config Clone Tests
// ============================================================================

mod config_clone_tests {
	use super::*;

	#[rstest]
	#[tokio::test]
	#[serial(env)]
	async fn test_s3_config_clone() {
		with_envs(
			&[
				("STORAGE_BACKEND", "s3"),
				("S3_BUCKET", "test-bucket"),
				("S3_REGION", "us-west-2"),
			],
			|| async {
				let config1 = StorageConfig::from_env().expect("Failed to load");
				let config2 = config1.clone();

				match (&config1, &config2) {
					(StorageConfig::S3(s3_1), StorageConfig::S3(s3_2)) => {
						assert_eq!(s3_1.bucket, s3_2.bucket);
						assert_eq!(s3_1.region, s3_2.region);
					}
					_ => panic!("Expected S3 configs"),
				}
			},
		)
		.await;
	}

	#[rstest]
	#[tokio::test]
	#[serial(env)]
	async fn test_local_config_clone() {
		with_envs(
			&[
				("STORAGE_BACKEND", "local"),
				("LOCAL_BASE_PATH", "/tmp/test"),
			],
			|| async {
				let config1 = StorageConfig::from_env().expect("Failed to load");
				let config2 = config1.clone();

				match (&config1, &config2) {
					(StorageConfig::Local(local_1), StorageConfig::Local(local_2)) => {
						assert_eq!(local_1.base_path, local_2.base_path);
					}
					_ => panic!("Expected Local configs"),
				}
			},
		)
		.await;
	}

	#[rstest]
	#[tokio::test]
	#[serial(env)]
	async fn test_s3_config_debug() {
		with_envs(
			&[("STORAGE_BACKEND", "s3"), ("S3_BUCKET", "test-bucket")],
			|| async {
				let config = StorageConfig::from_env().expect("Failed to load");
				let debug_str = format!("{:?}", config);
				assert!(debug_str.contains("S3"));
			},
		)
		.await;
	}
}

// ============================================================================
// Factory Integration Tests
// ============================================================================

mod factory_integration_tests {
	use super::*;

	#[rstest]
	#[tokio::test]
	#[serial(env)]
	async fn test_create_local_from_env_config() {
		with_envs(
			&[("STORAGE_BACKEND", "local"), ("LOCAL_BASE_PATH", "/tmp")],
			|| async {
				let config = StorageConfig::from_env().expect("Failed to load config");
				let backend = reinhardt_storages::create_storage(config)
					.await
					.expect("Failed to create backend");

				// Just verify it was created successfully
				drop(backend);
			},
		)
		.await;
	}
}
