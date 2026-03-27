//! Test logging utilities for Reinhardt framework
//!
//! Provides utilities for initializing logging in test environments.

use std::sync::Once;

static INIT: Once = Once::new();

/// Initialize logging for tests (call once)
///
/// This function ensures that logging is initialized only once across all tests.
/// It uses `env_logger` with test mode enabled.
///
/// # Examples
///
/// ```
/// use reinhardt_testkit::logging::init_test_logging;
///
/// // In your test:
/// init_test_logging();
/// // Your test code
/// ```
pub fn init_test_logging() {
	INIT.call_once(|| {
		let _ = env_logger::builder().is_test(true).try_init();
	});
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_init_test_logging_succeeds() {
		// Arrange / Act (should not panic)
		init_test_logging();

		// Assert - if we reach here, initialization succeeded
		assert!(true);
	}

	#[rstest]
	fn test_init_test_logging_idempotent() {
		// Arrange / Act - calling multiple times should not panic
		init_test_logging();
		init_test_logging();
		init_test_logging();

		// Assert - multiple calls are safe due to Once guard
		assert!(true);
	}
}
