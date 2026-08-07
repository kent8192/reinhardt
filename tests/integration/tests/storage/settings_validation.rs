//! Cross-crate storage settings validation tests.

use reinhardt_conf::settings::fragment::SettingsValidation;
use reinhardt_conf::settings::profile::Profile;
use reinhardt_conf::settings::validation::ValidationError;
use reinhardt_storages::{StorageError, StorageSettings};

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
			if key == "storage.backend"
				&& message == "Configuration error: Selected backend requires [storage.local] settings"
	));
}
