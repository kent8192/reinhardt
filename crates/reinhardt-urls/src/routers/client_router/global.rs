//! Global registration for [`ClientUrlReverser`].
//!
//! Mirrors the pattern in `server_router/global.rs`. The reverser is registered
//! during [`UnifiedRouter::register_globally()`] and retrieved by
//! `ResolvedUrls::from_global()`.

use std::sync::{Arc, PoisonError, RwLock as StdRwLock};

use once_cell::sync::OnceCell;

use super::reverser::ClientUrlReverser;

static GLOBAL_CLIENT_REVERSER: OnceCell<StdRwLock<Option<Arc<ClientUrlReverser>>>> =
	OnceCell::new();

/// Register a [`ClientUrlReverser`] globally.
///
/// Called by `UnifiedRouter::register_globally()` after extracting the
/// reverser from the `ClientRouter`.
pub fn register_client_reverser(reverser: ClientUrlReverser) {
	let cell = GLOBAL_CLIENT_REVERSER.get_or_init(|| StdRwLock::new(None));
	let mut guard = cell.write().unwrap_or_else(PoisonError::into_inner);
	*guard = Some(Arc::new(reverser));
}

/// Retrieve the globally registered [`ClientUrlReverser`].
///
/// Returns `None` if no reverser has been registered.
pub fn get_client_reverser() -> Option<Arc<ClientUrlReverser>> {
	GLOBAL_CLIENT_REVERSER
		.get()
		.and_then(|cell| cell.read().unwrap_or_else(PoisonError::into_inner).clone())
}

/// Clear the registered client reverser.
///
/// Intended for test teardown to avoid cross-test interference.
pub fn clear_client_reverser() {
	if let Some(cell) = GLOBAL_CLIENT_REVERSER.get() {
		let mut guard = cell.write().unwrap_or_else(PoisonError::into_inner);
		*guard = None;
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;
	use serial_test::serial;
	use std::collections::HashMap;

	#[rstest]
	#[serial(client_reverser)]
	fn test_register_and_get_client_reverser() {
		// Arrange
		clear_client_reverser();
		let mut patterns = HashMap::new();
		patterns.insert("auth:login".to_string(), "/login/".to_string());
		let reverser = ClientUrlReverser::new(patterns);

		// Act
		register_client_reverser(reverser);
		let result = get_client_reverser();

		// Assert
		assert!(result.is_some());
		let r = result.unwrap();
		assert_eq!(r.reverse("auth:login", &[]), Some("/login/".to_string()));

		// Cleanup
		clear_client_reverser();
	}

	#[rstest]
	#[serial(client_reverser)]
	fn test_get_client_reverser_before_registration() {
		// Arrange
		clear_client_reverser();

		// Act
		let result = get_client_reverser();

		// Assert
		assert!(result.is_none());
	}
}
