//! Server function inventory system-check integration tests.

use async_trait::async_trait;
use reinhardt_commands::{
	BaseCommand, CommandContext, CommandError, CommandResult,
	server_fn_checks::{
		ServerFnInventoryCheck, ensure_builtin_checks_registered, messages_for_errors,
	},
};
use reinhardt_pages::server_fn::{ServerFnInventoryEntry, ServerFnInventoryError};
use reinhardt_urls::routers::ServerRouter;
use reinhardt_utils::utils_core::checks::{Check, CheckLevel, CheckRegistry};
use serial_test::serial;
use std::sync::{
	Arc,
	atomic::{AtomicBool, Ordering},
};

inventory::submit! {
	ServerFnInventoryEntry::new(
		module_path!(),
		"/api/server_fn/orphan",
		"orphan",
		register_orphan_server_fn,
	)
}

fn register_orphan_server_fn(router: ServerRouter) -> ServerRouter {
	router
}

struct DefaultChecksCommand {
	executed: Arc<AtomicBool>,
}

impl DefaultChecksCommand {
	fn new() -> Self {
		Self {
			executed: Arc::new(AtomicBool::new(false)),
		}
	}
}

#[async_trait]
impl BaseCommand for DefaultChecksCommand {
	fn name(&self) -> &str {
		"default-checks"
	}

	async fn execute(&self, _ctx: &CommandContext) -> CommandResult<()> {
		self.executed.store(true, Ordering::SeqCst);
		Ok(())
	}
}

#[test]
fn orphan_function_becomes_a_pages_system_check_error() {
	// Arrange
	let messages = messages_for_errors([ServerFnInventoryError::OrphanFunction {
		module_path: "demo::shared::save".to_string(),
		path: "/api/server_fn/save".to_string(),
	}]);

	// Act
	let message = &messages[0];

	// Assert
	assert_eq!(messages.len(), 1);
	assert_eq!(message.id, "pages.server_fn.E002");
	assert_eq!(message.level, CheckLevel::Error);
	assert_eq!(
		message.hint.as_deref(),
		Some(
			"Move the function under an #[app_config] module or set \
			 #[server_fn(auto_register = false)]."
		)
	);
}

#[test]
fn inventory_error_messages_are_sorted_by_identifier() {
	// Arrange
	let errors = [
		ServerFnInventoryError::DuplicateName {
			app_label: "demo".to_string(),
			name: "save".to_string(),
			modules: vec!["demo::save_a".to_string(), "demo::save_b".to_string()],
		},
		ServerFnInventoryError::OrphanCaller {
			module_path: "demo::urls".to_string(),
		},
		ServerFnInventoryError::DuplicatePath {
			app_label: "demo".to_string(),
			path: "/api/server_fn/save".to_string(),
			modules: vec!["demo::save_a".to_string(), "demo::save_b".to_string()],
		},
		ServerFnInventoryError::AmbiguousOwner {
			module_path: "demo::shared::save".to_string(),
			labels: vec!["alpha".to_string(), "zeta".to_string()],
		},
		ServerFnInventoryError::OrphanFunction {
			module_path: "demo::shared::load".to_string(),
			path: "/api/server_fn/load".to_string(),
		},
	];

	// Act
	let messages = messages_for_errors(errors);

	// Assert
	assert_eq!(
		messages
			.into_iter()
			.map(|message| message.id)
			.collect::<Vec<_>>(),
		[
			"pages.server_fn.E001",
			"pages.server_fn.E002",
			"pages.server_fn.E003",
			"pages.server_fn.E004",
			"pages.server_fn.E005",
		]
	);
}

#[test]
#[serial(server_fn_inventory)]
fn builtin_inventory_check_is_registered_once_with_pages_and_routing_tags() {
	// Arrange
	ensure_builtin_checks_registered();
	ensure_builtin_checks_registered();

	// Act
	let messages = CheckRegistry::global()
		.lock()
		.expect("test check registry mutex should not be poisoned")
		.run_checks(&["pages".to_string()]);

	// Assert
	assert_eq!(
		ServerFnInventoryCheck.tags(),
		vec!["pages".to_string(), "routing".to_string()]
	);
	assert_eq!(
		messages
			.into_iter()
			.filter(|message| message.id == "pages.server_fn.E002")
			.count(),
		1,
		"the process-global check must be registered exactly once"
	);
}

#[tokio::test]
#[serial(server_fn_inventory)]
async fn inventory_error_stops_command_before_execute() {
	// Arrange
	let command = DefaultChecksCommand::new();
	let context = CommandContext::new(vec![]);

	// Act
	let result = command.run(&context).await;

	// Assert
	assert!(
		matches!(
			result,
			Err(CommandError::ExecutionError(message))
				if message.contains("pages.server_fn.E002")
		),
		"an orphan server function must fail system checks"
	);
	assert!(
		!command.executed.load(Ordering::SeqCst),
		"execute must not run after an error-level inventory check"
	);
}
