//! Tests for the settings-first storage configuration API.

#![allow(deprecated)] // Tests cover legacy compatibility conversion until removal.

use reinhardt_conf::settings::fragment::{SettingsFragment, SettingsValidation};
use reinhardt_conf::settings::profile::Profile;
use reinhardt_conf::settings::secret_types::SecretString;
use reinhardt_conf::settings::validation::ValidationError;
use reinhardt_storages::{BackendType, StorageConfig, StorageError, StorageSettings};

#[test]
fn storage_settings_section_is_storage() {
	assert_eq!(StorageSettings::section(), "storage");
}

#[test]
#[cfg(feature = "gcs")]
fn deserializes_gcs_settings_from_toml() {
	let raw = r#"
backend = "gcs"

[gcs]
bucket = "assets"
prefix = "uploads/"
endpoint = "http://127.0.0.1:4443"
service_account_json = { secret = "{\"client_email\":\"test@example.com\"}" }
"#;

	let settings: StorageSettings = toml::from_str(raw).unwrap();

	assert_eq!(settings.backend, BackendType::Gcs);
	let gcs = settings.gcs.as_ref().unwrap();
	assert_eq!(gcs.bucket, "assets");
	assert_eq!(gcs.prefix.as_deref(), Some("uploads/"));
	assert_eq!(gcs.endpoint.as_deref(), Some("http://127.0.0.1:4443"));
	assert_eq!(
		gcs.service_account_json.as_ref().unwrap().expose_secret(),
		r#"{"client_email":"test@example.com"}"#
	);
}

#[test]
#[cfg(feature = "azure")]
fn rejects_selected_backend_without_matching_nested_settings() {
	let settings: StorageSettings = toml::from_str(r#"backend = "azure""#).unwrap();

	let result = settings.to_config();

	match result {
		Err(StorageError::ConfigError(message)) => {
			assert_eq!(
				message,
				"Selected backend requires [storage.azure] settings"
			);
		}
		other => panic!("Expected ConfigError, got {other:?}"),
	}
}

#[test]
fn converts_local_settings_to_compat_config() {
	let settings: StorageSettings = toml::from_str(
		r#"
backend = "local"

[local]
base_path = "/tmp/reinhardt-storage"
"#,
	)
	.unwrap();

	let config = settings.to_config().unwrap();

	match config {
		StorageConfig::Local(local) => assert_eq!(local.base_path, "/tmp/reinhardt-storage"),
		other => panic!("expected local config, got {other:?}"),
	}
}

#[test]
#[cfg(feature = "s3")]
fn converts_s3_settings_to_compat_config() {
	let settings: StorageSettings = toml::from_str(
		r#"
backend = "s3"

[s3]
bucket = "assets"
region = "us-east-1"
endpoint = "http://127.0.0.1:4566"
prefix = "uploads/"
"#,
	)
	.unwrap();

	match settings.to_config().unwrap() {
		StorageConfig::S3(config) => {
			assert_eq!(config.bucket, "assets");
			assert_eq!(config.region.as_deref(), Some("us-east-1"));
			assert_eq!(config.endpoint.as_deref(), Some("http://127.0.0.1:4566"));
			assert_eq!(config.prefix.as_deref(), Some("uploads/"));
		}
		other => panic!("expected S3 config, got {other:?}"),
	}
}

#[test]
#[cfg(feature = "gcs")]
fn converts_gcs_settings_to_compat_config() {
	let settings: StorageSettings = toml::from_str(
		r#"
backend = "gcs"

[gcs]
bucket = "assets"
prefix = "uploads/"
endpoint = "http://127.0.0.1:4443"
service_account_json = { secret = "{\"client_email\":\"test@example.com\"}" }
"#,
	)
	.unwrap();

	match settings.to_config().unwrap() {
		StorageConfig::Gcs(config) => {
			assert_eq!(config.bucket, "assets");
			assert_eq!(config.prefix.as_deref(), Some("uploads/"));
			assert_eq!(config.endpoint.as_deref(), Some("http://127.0.0.1:4443"));
			assert_eq!(
				config
					.service_account_json
					.as_ref()
					.unwrap()
					.expose_secret(),
				"{\"client_email\":\"test@example.com\"}"
			);
		}
		other => panic!("expected GCS config, got {other:?}"),
	}
}

#[test]
#[cfg(feature = "azure")]
fn converts_azure_settings_to_compat_config() {
	let settings: StorageSettings = toml::from_str(
		r#"
backend = "azure"

[azure]
account = "storage-account"
container = "assets"
prefix = "uploads/"
endpoint = "http://127.0.0.1:10000/account"
access_key = { secret = "access-key" }
sas_token = { secret = "?sp=rl&sig=test" }
connection_string = { secret = "UseDevelopmentStorage=true" }
"#,
	)
	.unwrap();

	match settings.to_config().unwrap() {
		StorageConfig::Azure(config) => {
			assert_eq!(config.account, "storage-account");
			assert_eq!(config.container, "assets");
			assert_eq!(config.prefix.as_deref(), Some("uploads/"));
			assert_eq!(
				config.endpoint.as_deref(),
				Some("http://127.0.0.1:10000/account")
			);
			assert_eq!(
				config.access_key.as_ref().unwrap().expose_secret(),
				"access-key"
			);
			assert_eq!(
				config.sas_token.as_ref().unwrap().expose_secret(),
				"?sp=rl&sig=test"
			);
			assert_eq!(
				config.connection_string.as_ref().unwrap().expose_secret(),
				"UseDevelopmentStorage=true"
			);
		}
		other => panic!("expected Azure config, got {other:?}"),
	}
}

#[test]
fn reports_missing_backend_sections_and_validation_errors() {
	let missing_local: StorageSettings = toml::from_str(r#"backend = "local""#).unwrap();
	assert!(matches!(
		missing_local.to_config(),
		Err(StorageError::ConfigError(message)) if message == "Selected backend requires [storage.local] settings"
	));

	let validation = SettingsValidation::validate(&missing_local, &Profile::Development);
	assert!(matches!(
		validation,
		Err(ValidationError::InvalidValue { key, message })
			if key == "storage.backend" && message.contains("storage.local")
	));
}

#[test]
#[cfg(any(feature = "s3", feature = "gcs", feature = "azure", feature = "local"))]
fn default_settings_are_populated_for_the_selected_backend() {
	let settings = StorageSettings::default();
	assert!(settings.to_config().is_ok());
}

#[test]
fn secret_string_debug_redacts_credentials() {
	let secret = SecretString::new("super-secret-key");

	assert_eq!(format!("{secret:?}"), "SecretString([REDACTED])");
	assert_eq!(secret.expose_secret(), "super-secret-key");
}
