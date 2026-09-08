//! Environment fixtures for serial-protected command tests.

use reinhardt_test::{TeardownGuard, TestResource};
use rstest::fixture;
use std::ffi::{OsStr, OsString};

pub(super) struct EnvVars {
	vars: Vec<(String, Option<OsString>)>,
}

impl EnvVars {
	pub(super) fn set(&mut self, key: &str, value: impl AsRef<OsStr>) {
		self.vars.push((key.to_owned(), std::env::var_os(key)));
		// SAFETY: environment-changing callers share a serial test group.
		unsafe {
			std::env::set_var(key, value);
		}
	}
}

impl TestResource for EnvVars {
	fn setup() -> Self {
		Self { vars: Vec::new() }
	}

	fn teardown(&mut self) {
		for (key, original) in self.vars.iter().rev() {
			// SAFETY: the caller retains its serial test guard during cleanup.
			unsafe {
				match original {
					Some(value) => std::env::set_var(key, value),
					None => std::env::remove_var(key),
				}
			}
		}
	}
}

#[fixture]
pub(super) fn env_vars() -> TeardownGuard<EnvVars> {
	TeardownGuard::new()
}
