//! Tests for the settings-first storage configuration API.

#![allow(deprecated)] // Tests cover legacy compatibility conversion until removal.

use reinhardt_conf::settings::fragment::SettingsFragment;
use reinhardt_conf::settings::schema::{SettingsNode, SettingsPathSegment};
use reinhardt_conf::settings::secret_types::SecretString;
use reinhardt_storages::{BackendType, StorageConfig, StorageError, StorageSettings};
use rstest::rstest;

#[derive(serde::Deserialize)]
struct SettingsDocument {
	storage: StorageSettings,
}

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
fn secret_string_debug_redacts_credentials() {
	let secret = SecretString::new("super-secret-key");

	assert_eq!(format!("{secret:?}"), "SecretString([REDACTED])");
	assert_eq!(secret.expose_secret(), "super-secret-key");
}

#[test]
#[cfg(feature = "gcs")]
fn named_gcs_credentials_are_described_as_secret_paths() {
	let schema = <StorageSettings as SettingsNode>::node_schema();
	let mut paths = Vec::new();
	schema.collect_secret_paths(&mut paths);

	assert!(paths.iter().any(|path| {
		path.segments()
			== [
				SettingsPathSegment::Key("named"),
				SettingsPathSegment::AnyKey,
				SettingsPathSegment::Key("gcs"),
				SettingsPathSegment::Key("service_account_json"),
			]
	}));
}

#[test]
#[cfg(feature = "azure")]
fn named_azure_credentials_are_described_as_secret_paths() {
	let schema = <StorageSettings as SettingsNode>::node_schema();
	let mut paths = Vec::new();
	schema.collect_secret_paths(&mut paths);

	for key in ["access_key", "sas_token", "connection_string"] {
		assert!(paths.iter().any(|path| {
			path.segments()
				== [
					SettingsPathSegment::Key("named"),
					SettingsPathSegment::AnyKey,
					SettingsPathSegment::Key("azure"),
					SettingsPathSegment::Key(key),
				]
		}));
	}
}

#[rstest]
fn named_local_storage_uses_its_own_default_url_expiry() {
	// A missing url_expiry_secs must receive the file-field URL default instead of
	// inheriting a value from the default storage entry.
	let document: SettingsDocument = toml::from_str(
		r#"
[storage]
backend = "local"
url_expiry_secs = 3600

[storage.local]
base_path = "media"

[storage.named.private_uploads]
backend = "local"

[storage.named.private_uploads.local]
base_path = "private-media"
"#,
	)
	.unwrap();

	assert_eq!(document.storage.url_expiry_secs, 3_600);
	assert_eq!(
		document.storage.named["private_uploads"].url_expiry_secs,
		3_600
	);
}

#[rstest]
fn named_storage_rejects_recursive_named_sections() {
	// A named entry must remain a backend entry, not another registry tree.
	let result = toml::from_str::<SettingsDocument>(
		r#"
		[storage]
		backend = "local"

		[storage.local]
		base_path = "media"

		[storage.named.private_uploads]
		backend = "local"

		[storage.named.private_uploads.local]
		base_path = "private-media"

		[storage.named.private_uploads.named.nested]
backend = "local"
"#,
	);

	assert!(result.is_err());
}

#[rstest]
fn named_storage_rejects_reserved_default_alias_while_deserializing() {
	let result = toml::from_str::<SettingsDocument>(
		r#"
[storage]
backend = "local"

[storage.local]
base_path = "media"

[storage.named.default]
backend = "local"

[storage.named.default.local]
base_path = "private-media"
"#,
	);

	assert!(result.is_err());
}

#[rstest]
#[case("PrivateUploads")]
#[case("private.uploads")]
#[case("-private")]
fn named_storage_rejects_invalid_alias_while_deserializing(#[case] alias: &str) {
	let result = toml::from_str::<SettingsDocument>(&format!(
		r#"
[storage]
backend = "local"

[storage.local]
base_path = "media"

[storage.named."{alias}"]
backend = "local"

[storage.named."{alias}".local]
base_path = "private-media"
"#,
	));

	assert!(result.is_err());
}
