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

/// Rejects startup when linked server function inventory is invalid.
///
/// This is separate from the command check registry because non-CLI callers
/// can invoke [`crate::RunServerCommand::execute`] directly.
pub fn validate_server_fn_inventory_for_startup() -> Result<(), String> {
	let errors = validate_server_fn_inventory();
	if errors.is_empty() {
		Ok(())
	} else {
		Err(format_inventory_errors(errors))
	}
}

fn format_inventory_errors(errors: impl IntoIterator<Item = ServerFnInventoryError>) -> String {
	let mut errors = errors
		.into_iter()
		.map(|error| error.to_string())
		.collect::<Vec<_>>();
	errors.sort_unstable();
	errors.join("\n")
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

#[cfg(test)]
mod tests {
	use super::format_inventory_errors;
	use reinhardt_pages::server_fn::ServerFnInventoryError;

	#[test]
	fn startup_inventory_errors_are_sorted_deterministically() {
		// Arrange
		let errors = [
			ServerFnInventoryError::OrphanFunction {
				module_path: "demo::outside::server_fn".to_string(),
				path: "/api/outside".to_string(),
			},
			ServerFnInventoryError::OrphanCaller {
				module_path: "demo::outside::urls".to_string(),
			},
		];

		// Act
		let actual = format_inventory_errors(errors);

		// Assert
		assert_eq!(
			actual,
			"pages.server_fn.E001: no application owns caller module `demo::outside::urls`\n\
			 pages.server_fn.E002: no application owns server function `demo::outside::server_fn` at `/api/outside`"
		);
	}
}
