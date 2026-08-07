//! Observable system-check behavior that does not require infrastructure services.

use reinhardt_commands::{BaseCommand, CheckCommand, CommandContext};

struct EnvVarGuard {
	key: &'static str,
	original: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
	fn capture(key: &'static str) -> Self {
		Self {
			key,
			original: std::env::var_os(key),
		}
	}
}

impl Drop for EnvVarGuard {
	fn drop(&mut self) {
		// SAFETY: each environment-changing test is serial-protected.
		unsafe {
			match &self.original {
				Some(value) => std::env::set_var(self.key, value),
				None => std::env::remove_var(self.key),
			}
		}
	}
}

fn isolate_check_environment() -> Vec<EnvVarGuard> {
	let guards = [
		"DATABASE_URL",
		"STATIC_ROOT",
		"SECRET_KEY",
		"DEBUG",
		"ALLOWED_HOSTS",
		"SECURE_SSL_REDIRECT",
	]
	.into_iter()
	.map(EnvVarGuard::capture)
	.collect::<Vec<_>>();

	// SAFETY: each caller is serial-protected and guards restore every value.
	unsafe {
		for key in [
			"DATABASE_URL",
			"STATIC_ROOT",
			"SECRET_KEY",
			"DEBUG",
			"ALLOWED_HOSTS",
			"SECURE_SSL_REDIRECT",
		] {
			std::env::remove_var(key);
		}
	}

	guards
}

#[tokio::test]
#[serial_test::serial(check_command_environment)]
async fn development_check_skips_unconfigured_external_services() {
	// Arrange
	let _environment = isolate_check_environment();

	// Act
	let result = CheckCommand.execute(&CommandContext::default()).await;

	// Assert
	assert!(
		result.is_ok(),
		"development check should not require services: {result:?}"
	);
}

#[tokio::test]
#[serial_test::serial(check_command_environment)]
async fn deployment_check_accepts_complete_local_security_configuration() {
	// Arrange
	let _environment = isolate_check_environment();
	unsafe {
		std::env::set_var("STATIC_ROOT", "public-assets");
		std::env::set_var("SECRET_KEY", "a".repeat(32));
		std::env::set_var("DEBUG", "false");
		std::env::set_var("ALLOWED_HOSTS", "example.test");
		std::env::set_var("SECURE_SSL_REDIRECT", "true");
	}
	let mut context = CommandContext::default();
	context.set_option("deploy".to_string(), "true".to_string());

	// Act
	let result = CheckCommand.execute(&context).await;

	// Assert
	assert!(
		result.is_ok(),
		"complete deployment configuration should pass: {result:?}"
	);
}
