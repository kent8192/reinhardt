use reinhardt_conf::settings::core_settings::CoreSettings;
use reinhardt_conf::settings::fragment::SettingsFragment;
use reinhardt_conf::settings::profile::Profile;
use reinhardt_conf::settings::security::SecuritySettings;
use reinhardt_conf::settings::validation::ValidationError;
use rstest::rstest;

#[rstest]
fn core_empty_secret_key_fails_in_development() {
	// Arrange
	let settings = CoreSettings {
		secret_key: String::new(),
		..Default::default()
	};

	// Act
	let result = settings.validate(&Profile::Development);

	// Assert
	match result {
		Err(ValidationError::MissingRequired(field)) => {
			assert_eq!(field, "secret_key");
		}
		other => panic!("expected MissingRequired(\"secret_key\"), got: {:?}", other),
	}
}

#[rstest]
fn core_empty_secret_key_fails_in_production() {
	// Arrange
	let settings = CoreSettings {
		secret_key: String::new(),
		debug: false,
		allowed_hosts: vec!["example.com".to_string()],
		security: SecuritySettings {
			secure_ssl_redirect: true,
			session_cookie_secure: true,
			csrf_cookie_secure: true,
			..Default::default()
		},
		..Default::default()
	};

	// Act
	let result = settings.validate(&Profile::Production);

	// Assert
	match result {
		Err(ValidationError::MissingRequired(field)) => {
			assert_eq!(field, "secret_key");
		}
		other => panic!("expected MissingRequired(\"secret_key\"), got: {:?}", other),
	}
}

#[rstest]
fn core_debug_true_fails_in_production() {
	// Arrange
	let settings = CoreSettings {
		secret_key: "production-secret-key".to_string(),
		debug: true,
		allowed_hosts: vec!["example.com".to_string()],
		security: SecuritySettings {
			secure_ssl_redirect: true,
			session_cookie_secure: true,
			csrf_cookie_secure: true,
			..Default::default()
		},
		..Default::default()
	};

	// Act
	let result = settings.validate(&Profile::Production);

	// Assert
	match result {
		Err(ValidationError::Security(msg)) => {
			assert!(
				msg.contains("debug"),
				"expected error message to contain \"debug\", got: {:?}",
				msg
			);
		}
		other => panic!(
			"expected Security error containing \"debug\", got: {:?}",
			other
		),
	}
}

#[rstest]
fn core_empty_allowed_hosts_fails_in_production() {
	// Arrange
	let settings = CoreSettings {
		secret_key: "production-secret-key".to_string(),
		debug: false,
		allowed_hosts: vec![],
		security: SecuritySettings {
			secure_ssl_redirect: true,
			session_cookie_secure: true,
			csrf_cookie_secure: true,
			..Default::default()
		},
		..Default::default()
	};

	// Act
	let result = settings.validate(&Profile::Production);

	// Assert
	match result {
		Err(ValidationError::MissingRequired(field)) => {
			assert_eq!(field, "allowed_hosts");
		}
		other => panic!(
			"expected MissingRequired(\"allowed_hosts\"), got: {:?}",
			other
		),
	}
}

#[rstest]
fn security_ssl_redirect_false_fails_in_production() {
	// Arrange
	let settings = SecuritySettings {
		secure_ssl_redirect: false,
		session_cookie_secure: true,
		csrf_cookie_secure: true,
		..Default::default()
	};

	// Act
	let result = settings.validate(&Profile::Production);

	// Assert
	match result {
		Err(ValidationError::Security(msg)) => {
			assert!(
				msg.to_lowercase().contains("ssl"),
				"expected error message to contain \"ssl\", got: {:?}",
				msg
			);
		}
		other => panic!(
			"expected Security error containing \"ssl\", got: {:?}",
			other
		),
	}
}

#[rstest]
fn security_session_cookie_insecure_fails_in_production() {
	// Arrange
	let settings = SecuritySettings {
		secure_ssl_redirect: true,
		session_cookie_secure: false,
		csrf_cookie_secure: true,
		..Default::default()
	};

	// Act
	let result = settings.validate(&Profile::Production);

	// Assert
	match result {
		Err(ValidationError::Security(msg)) => {
			assert!(
				msg.to_lowercase().contains("session"),
				"expected error message to contain \"session\", got: {:?}",
				msg
			);
		}
		other => panic!(
			"expected Security error containing \"session\", got: {:?}",
			other
		),
	}
}

#[rstest]
fn security_csrf_cookie_insecure_fails_in_production() {
	// Arrange
	let settings = SecuritySettings {
		secure_ssl_redirect: true,
		session_cookie_secure: true,
		csrf_cookie_secure: false,
		..Default::default()
	};

	// Act
	let result = settings.validate(&Profile::Production);

	// Assert
	match result {
		Err(ValidationError::Security(msg)) => {
			assert!(
				msg.to_lowercase().contains("csrf"),
				"expected error message to contain \"csrf\", got: {:?}",
				msg
			);
		}
		other => panic!(
			"expected Security error containing \"csrf\", got: {:?}",
			other
		),
	}
}

#[rstest]
fn validation_error_display_contains_field_info() {
	// Arrange
	let security_err = ValidationError::Security("weak key detected".to_string());
	let invalid_value_err = ValidationError::InvalidValue {
		key: "timeout".to_string(),
		message: "must be positive".to_string(),
	};
	let missing_required_err = ValidationError::MissingRequired("secret_key".to_string());
	let constraint_err = ValidationError::Constraint("value out of range".to_string());

	// Act
	let security_display = security_err.to_string();
	let invalid_value_display = invalid_value_err.to_string();
	let missing_required_display = missing_required_err.to_string();
	let constraint_display = constraint_err.to_string();

	// Assert - each Display output must be non-empty and contain key field info
	assert!(
		!security_display.is_empty(),
		"Security error display must not be empty"
	);
	assert!(
		security_display.contains("weak key detected"),
		"Security error display must contain the error message, got: {:?}",
		security_display
	);

	assert!(
		!invalid_value_display.is_empty(),
		"InvalidValue error display must not be empty"
	);
	assert!(
		invalid_value_display.contains("timeout"),
		"InvalidValue error display must contain the key name, got: {:?}",
		invalid_value_display
	);
	assert!(
		invalid_value_display.contains("must be positive"),
		"InvalidValue error display must contain the message, got: {:?}",
		invalid_value_display
	);

	assert!(
		!missing_required_display.is_empty(),
		"MissingRequired error display must not be empty"
	);
	assert!(
		missing_required_display.contains("secret_key"),
		"MissingRequired error display must contain the field name, got: {:?}",
		missing_required_display
	);

	assert!(
		!constraint_display.is_empty(),
		"Constraint error display must not be empty"
	);
	assert!(
		constraint_display.contains("value out of range"),
		"Constraint error display must contain the constraint message, got: {:?}",
		constraint_display
	);
}
