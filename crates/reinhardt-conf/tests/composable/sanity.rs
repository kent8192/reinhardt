use reinhardt_conf::settings::core_settings::{CoreSettings, HasCoreSettings};
use reinhardt_conf::settings::profile::Profile;
use reinhardt_conf::settings::validation::{ValidationError, ValidationResult};
use rstest::rstest;

// ---------------------------------------------------------------------------
// Helper struct for HasCoreSettings trait-bound test
// ---------------------------------------------------------------------------

/// Minimal struct that manually implements [`HasCoreSettings`].
struct MinimalSettings {
	core: CoreSettings,
}

impl HasCoreSettings for MinimalSettings {
	fn core(&self) -> &CoreSettings {
		&self.core
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Constructing `ValidationError::Security` must hold the provided message.
#[rstest]
fn validation_error_security_variant() {
	// Arrange
	let msg = "ssl must be enabled in production";

	// Act
	let error = ValidationError::Security(msg.to_string());

	// Assert
	match error {
		ValidationError::Security(stored) => {
			assert_eq!(stored, msg, "Security variant must preserve its message");
		}
		other => panic!("expected Security variant, got: {:?}", other),
	}
}

/// Constructing `ValidationError::InvalidValue` must store both key and message.
#[rstest]
fn validation_error_invalid_value_fields() {
	// Arrange
	let key = "timeout";
	let message = "must be a positive integer";

	// Act
	let error = ValidationError::InvalidValue {
		key: key.to_string(),
		message: message.to_string(),
	};

	// Assert
	match error {
		ValidationError::InvalidValue {
			key: stored_key,
			message: stored_msg,
		} => {
			assert_eq!(stored_key, key, "InvalidValue must preserve the key field");
			assert_eq!(
				stored_msg, message,
				"InvalidValue must preserve the message field"
			);
		}
		other => panic!("expected InvalidValue variant, got: {:?}", other),
	}
}

/// Constructing `ValidationError::MissingRequired` must hold the field name.
#[rstest]
fn validation_error_missing_required_holds_name() {
	// Arrange
	let field = "secret_key";

	// Act
	let error = ValidationError::MissingRequired(field.to_string());

	// Assert
	match error {
		ValidationError::MissingRequired(stored) => {
			assert_eq!(
				stored, field,
				"MissingRequired must preserve the field name"
			);
		}
		other => panic!("expected MissingRequired variant, got: {:?}", other),
	}
}

/// Constructing `ValidationError::Constraint` must hold the constraint message.
#[rstest]
fn validation_error_constraint_holds_message() {
	// Arrange
	let msg = "allowed_hosts must not be empty in production";

	// Act
	let error = ValidationError::Constraint(msg.to_string());

	// Assert
	match error {
		ValidationError::Constraint(stored) => {
			assert_eq!(stored, msg, "Constraint must preserve its message");
		}
		other => panic!("expected Constraint variant, got: {:?}", other),
	}
}

/// Constructing `ValidationError::Multiple` must hold all inner errors and
/// preserve the count.
#[rstest]
fn validation_error_multiple_holds_inner_errors() {
	// Arrange
	let inner = vec![
		ValidationError::Security("a".to_string()),
		ValidationError::Constraint("b".to_string()),
	];

	// Act
	let error = ValidationError::Multiple(inner);

	// Assert
	match error {
		ValidationError::Multiple(errs) => {
			assert_eq!(errs.len(), 2, "Multiple must hold exactly 2 inner errors");
		}
		other => panic!("expected Multiple variant, got: {:?}", other),
	}
}

/// A `ValidationResult` wrapping `Ok(())` must satisfy `is_ok()`.
#[rstest]
fn validation_result_ok_variant() {
	// Arrange / Act
	let result: ValidationResult = Ok(());

	// Assert
	assert!(
		result.is_ok(),
		"ValidationResult Ok(()) must satisfy is_ok()"
	);
}

/// A `ValidationResult` wrapping an error must satisfy `is_err()` and carry
/// the correct error variant.
#[rstest]
fn validation_result_err_variant() {
	// Arrange / Act
	let result: ValidationResult = Err(ValidationError::Security("x".to_string()));

	// Assert
	assert!(
		result.is_err(),
		"ValidationResult Err(...) must satisfy is_err()"
	);
	match result.unwrap_err() {
		ValidationError::Security(msg) => {
			assert_eq!(msg, "x", "Security message must be preserved in the result");
		}
		other => panic!("expected Security variant, got: {:?}", other),
	}
}

/// `Profile::default()` must equal `Profile::Development`.
#[rstest]
fn profile_default_is_development() {
	// Arrange / Act
	let profile = Profile::default();

	// Assert
	assert_eq!(
		profile,
		Profile::Development,
		"Profile::default() must be Profile::Development"
	);
}

/// A function bounded on `impl HasCoreSettings` must be callable with a struct
/// that manually implements the trait, and must return the correct `debug` flag.
#[rstest]
fn has_core_settings_usable_as_trait_bound() {
	// Helper function that exercises the trait bound
	fn get_debug(s: &impl HasCoreSettings) -> bool {
		s.core().debug
	}

	// Arrange
	let settings = MinimalSettings {
		core: CoreSettings {
			secret_key: "test-secret".to_string(),
			debug: true,
			..Default::default()
		},
	};

	// Act
	let debug_flag = get_debug(&settings);

	// Assert
	assert!(
		debug_flag,
		"get_debug must return the debug flag from the core settings"
	);
	assert_eq!(
		settings.core().secret_key,
		"test-secret",
		"core().secret_key must match the value set during construction"
	);
}
