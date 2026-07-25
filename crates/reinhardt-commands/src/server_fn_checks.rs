//! System checks for native server function inventory configuration.

use reinhardt_pages::server_fn::{ServerFnInventoryError, validate_server_fn_inventory};
use reinhardt_utils::utils_core::checks::{Check, CheckMessage, CheckRegistry};
use std::sync::Once;

/// Validates native server function inventory before commands execute.
pub struct ServerFnInventoryCheck;

impl Check for ServerFnInventoryCheck {
	fn tags(&self) -> Vec<String> {
		vec!["pages".to_string(), "routing".to_string()]
	}

	fn check(&self) -> Vec<CheckMessage> {
		messages_for_errors(validate_server_fn_inventory())
	}
}

/// Registers command-provided system checks once per process.
pub fn ensure_builtin_checks_registered() {
	static REGISTER_BUILTIN_CHECKS: Once = Once::new();

	REGISTER_BUILTIN_CHECKS.call_once(|| {
		let registry = CheckRegistry::global();
		let mut registry_guard = registry.lock().unwrap_or_else(|poisoned| {
			// Recover the inner value from a poisoned mutex.
			// The check registry data is still usable even after a panic in another thread.
			poisoned.into_inner()
		});
		registry_guard.register(Box::new(ServerFnInventoryCheck));
	});
}

/// Converts deterministic inventory diagnostics into error-level system-check messages.
pub fn messages_for_errors(
	errors: impl IntoIterator<Item = ServerFnInventoryError>,
) -> Vec<CheckMessage> {
	let mut errors = errors.into_iter().collect::<Vec<_>>();
	errors.sort_unstable_by_key(ToString::to_string);

	errors
		.into_iter()
		.map(|error| {
			let (id, hint) = error_id_and_hint(&error);
			let message = error
				.to_string()
				.strip_prefix(&format!("{id}: "))
				.expect("inventory errors include their check identifier")
				.to_string();
			CheckMessage::error(id, message).with_hint(hint)
		})
		.collect()
}

fn error_id_and_hint(error: &ServerFnInventoryError) -> (&'static str, &'static str) {
	match error {
		ServerFnInventoryError::OrphanCaller { .. } => (
			"pages.server_fn.E001",
			"Move the router construction under an #[app_config] module.",
		),
		ServerFnInventoryError::OrphanFunction { .. } => (
			"pages.server_fn.E002",
			"Move the function under an #[app_config] module or set #[server_fn(auto_register = false)].",
		),
		ServerFnInventoryError::AmbiguousOwner { .. } => (
			"pages.server_fn.E003",
			"Ensure exactly one #[app_config] module owns this server function.",
		),
		ServerFnInventoryError::DuplicatePath { .. } => (
			"pages.server_fn.E004",
			"Use a unique endpoint path for each server function in the application.",
		),
		ServerFnInventoryError::DuplicateName { .. } => (
			"pages.server_fn.E005",
			"Use a unique route name for each server function in the application.",
		),
	}
}
