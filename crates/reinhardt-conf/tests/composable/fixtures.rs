use reinhardt_conf::settings::core_settings::CoreSettings;
use reinhardt_conf::settings::security::SecuritySettings;
use rstest::fixture;

/// CoreSettings valid for Production profile.
#[fixture]
pub fn production_core_settings() -> CoreSettings {
	CoreSettings {
		secret_key: "a-very-long-secure-random-key-that-is-at-least-32-chars".to_string(),
		debug: false,
		allowed_hosts: vec!["example.com".to_string()],
		security: production_security_settings(),
		..Default::default()
	}
}

/// CoreSettings with Development defaults + valid secret_key.
#[fixture]
pub fn development_core_settings() -> CoreSettings {
	CoreSettings {
		secret_key: "dev-secret-key".to_string(),
		..Default::default()
	}
}

/// SecuritySettings valid for Production profile.
#[fixture]
pub fn production_security_settings() -> SecuritySettings {
	SecuritySettings {
		secure_ssl_redirect: true,
		session_cookie_secure: true,
		csrf_cookie_secure: true,
		..Default::default()
	}
}
