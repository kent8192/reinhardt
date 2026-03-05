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
