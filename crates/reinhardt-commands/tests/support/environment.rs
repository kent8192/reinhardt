//! Environment restoration for serial-protected command tests.

use std::ffi::OsString;

pub(super) struct EnvVarGuard {
	vars: Vec<(String, Option<OsString>)>,
}

impl EnvVarGuard {
	pub(super) fn new() -> Self {
		Self { vars: Vec::new() }
	}

	pub(super) fn set(&mut self, key: &str, value: &str) {
		self.vars.push((key.to_owned(), std::env::var_os(key)));
		// SAFETY: environment-changing callers share a serial test group.
		unsafe {
			std::env::set_var(key, value);
		}
	}
}

impl Drop for EnvVarGuard {
	fn drop(&mut self) {
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
