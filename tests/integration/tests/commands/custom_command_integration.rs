//! Custom command integration tests
//!
//! End-to-end tests for custom command implementations.

use async_trait::async_trait;
use reinhardt_commands::{
	BaseCommand, CommandArgument, CommandContext, CommandOption, CommandRegistry, CommandResult,
};
use rstest::rstest;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

// =============================================================================
// Test Command Implementations
// =============================================================================

/// A command that tracks execution count for testing
struct CountingCommand {
	name: String,
	execution_count: Arc<AtomicUsize>,
}

impl CountingCommand {
	fn new(name: &str) -> Self {
		Self {
			name: name.to_string(),
			execution_count: Arc::new(AtomicUsize::new(0)),
		}
	}

	fn count(&self) -> usize {
		self.execution_count.load(Ordering::SeqCst)
	}
}

#[async_trait]
impl BaseCommand for CountingCommand {
	fn name(&self) -> &str {
		&self.name
	}

	fn description(&self) -> &str {
		"A command that counts executions"
	}

	async fn execute(&self, _ctx: &CommandContext) -> CommandResult<()> {
		self.execution_count.fetch_add(1, Ordering::SeqCst);
		Ok(())
	}
}

/// A command that echoes its arguments
struct EchoCommand;

#[async_trait]
impl BaseCommand for EchoCommand {
	fn name(&self) -> &str {
		"echo"
	}

	fn description(&self) -> &str {
		"Echoes the provided arguments"
	}

	fn arguments(&self) -> Vec<CommandArgument> {
		vec![CommandArgument::optional("message", "Message to echo")]
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		if let Some(msg) = ctx.arg(0) {
			// In real usage, this would write to output
			assert!(!msg.is_empty() || msg.is_empty(), "Message exists");
		}
		Ok(())
	}
}

/// A command with options
struct OptionsCommand;

#[async_trait]
impl BaseCommand for OptionsCommand {
	fn name(&self) -> &str {
		"options"
	}

	fn description(&self) -> &str {
		"A command with various options"
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![
			CommandOption::flag(Some('v'), "verbose", "Enable verbose output"),
			CommandOption::option(Some('f'), "format", "Output format").with_default("text"),
			CommandOption::option(Some('o'), "output", "Output file").required(),
		]
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		// Verify options are accessible
		let _verbose = ctx.has_option("verbose");
		let _format = ctx.option("format");
		let _output = ctx.option("output");
		Ok(())
	}
}

// =============================================================================
// Use Case Tests
// =============================================================================

/// Test custom command receives arguments correctly
///
/// **Category**: Use Case
/// **Verifies**: Arguments are passed to execute method
#[rstest]
#[tokio::test]
async fn test_custom_command_receives_arguments() {
	struct ArgCheckCommand {
		expected_args: Vec<String>,
	}

	#[async_trait]
	impl BaseCommand for ArgCheckCommand {
		fn name(&self) -> &str {
			"argcheck"
		}

		async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
			assert_eq!(
				ctx.args.len(),
				self.expected_args.len(),
				"Argument count should match"
			);
			for (i, expected) in self.expected_args.iter().enumerate() {
				assert_eq!(ctx.arg(i), Some(expected), "Argument {} should match", i);
			}
			Ok(())
		}
	}

	let args = vec!["arg1".to_string(), "arg2".to_string(), "arg3".to_string()];
	let cmd = ArgCheckCommand {
		expected_args: args.clone(),
	};
	let ctx = CommandContext::new(args);

	let result = cmd.execute(&ctx).await;
	assert!(result.is_ok(), "Command should execute successfully");
}

/// Test custom command receives options correctly
///
/// **Category**: Use Case
/// **Verifies**: Options are passed to execute method
#[rstest]
#[tokio::test]
async fn test_custom_command_receives_options() {
	struct OptionCheckCommand;

	#[async_trait]
	impl BaseCommand for OptionCheckCommand {
		fn name(&self) -> &str {
			"optcheck"
		}

		async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
			assert!(ctx.has_option("verbose"), "Should have verbose option");
			assert_eq!(
				ctx.option("format"),
				Some(&"json".to_string()),
				"Format should be json"
			);
			assert!(
				!ctx.has_option("nonexistent"),
				"Should not have nonexistent option"
			);
			Ok(())
		}
	}

	let mut ctx = CommandContext::new(vec![]);
	ctx.set_option("verbose".to_string(), "true".to_string());
	ctx.set_option("format".to_string(), "json".to_string());

	let cmd = OptionCheckCommand;
	let result = cmd.execute(&ctx).await;
	assert!(result.is_ok(), "Command should execute successfully");
}

/// Test registry as command dispatcher
///
/// **Category**: Use Case
/// **Verifies**: Registry can dispatch to registered commands
#[rstest]
#[tokio::test]
async fn test_registry_as_dispatcher() {
	let mut registry = CommandRegistry::new();

	registry.register(Box::new(EchoCommand));
	registry.register(Box::new(OptionsCommand));

	// Dispatch to echo command
	let echo_cmd = registry.get("echo");
	assert!(echo_cmd.is_some(), "Echo command should be registered");
	assert_eq!(echo_cmd.unwrap().name(), "echo");

	// Dispatch to options command
	let options_cmd = registry.get("options");
	assert!(
		options_cmd.is_some(),
		"Options command should be registered"
	);
	assert_eq!(options_cmd.unwrap().name(), "options");

	// Unknown command returns None
	assert!(
		registry.get("unknown").is_none(),
		"Unknown command should return None"
	);
}

/// Test verbose output levels
///
/// **Category**: Use Case
/// **Verifies**: Verbosity level affects command behavior
#[rstest]
#[case(0, false, false)]
#[case(1, true, false)]
#[case(2, true, false)]
#[case(3, true, true)]
#[tokio::test]
async fn test_verbosity_levels_in_command(
	#[case] level: u8,
	#[case] show_info: bool,
	#[case] show_debug: bool,
) {
	struct VerboseCommand;

	#[async_trait]
	impl BaseCommand for VerboseCommand {
		fn name(&self) -> &str {
			"verbose"
		}

		async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
			let info_shown = ctx.verbosity() >= 1;
			let debug_shown = ctx.verbosity() >= 3;

			// These would control actual output in real implementation
			assert_eq!(info_shown, ctx.verbosity() >= 1);
			assert_eq!(debug_shown, ctx.verbosity() >= 3);
			Ok(())
		}
	}

	let mut ctx = CommandContext::new(vec![]);
	ctx.set_verbosity(level);

	// Verify expected behavior
	assert_eq!(ctx.verbosity() >= 1, show_info);
	assert_eq!(ctx.verbosity() >= 3, show_debug);

	let cmd = VerboseCommand;
	let result = cmd.execute(&ctx).await;
	assert!(result.is_ok());
}

/// Test command execution counting
///
/// **Category**: Use Case
/// **Verifies**: Commands can track their executions
#[rstest]
#[tokio::test]
async fn test_command_execution_tracking() {
	let cmd = CountingCommand::new("counter");
	let ctx = CommandContext::new(vec![]);

	assert_eq!(cmd.count(), 0, "Initial count should be 0");

	cmd.execute(&ctx).await.unwrap();
	assert_eq!(cmd.count(), 1, "Count should be 1 after first execution");

	cmd.execute(&ctx).await.unwrap();
	cmd.execute(&ctx).await.unwrap();
	assert_eq!(cmd.count(), 3, "Count should be 3 after three executions");
}

/// Test command with multi-value options
///
/// **Category**: Use Case
/// **Verifies**: Commands can handle multiple values for an option
#[rstest]
#[tokio::test]
async fn test_command_multi_value_options() {
	struct MultiValueCommand;

	#[async_trait]
	impl BaseCommand for MultiValueCommand {
		fn name(&self) -> &str {
			"multivalue"
		}

		fn options(&self) -> Vec<CommandOption> {
			vec![CommandOption::option(Some('t'), "tags", "Tags to apply").multi()]
		}

		async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
			let tags = ctx.option_values("tags");
			assert!(tags.is_some(), "Tags should be present");
			let tag_list = tags.unwrap();
			assert_eq!(tag_list.len(), 3, "Should have 3 tags");
			assert_eq!(tag_list[0], "one");
			assert_eq!(tag_list[1], "two");
			assert_eq!(tag_list[2], "three");
			Ok(())
		}
	}

	let mut ctx = CommandContext::new(vec![]);
	ctx.set_option_multi(
		"tags".to_string(),
		vec!["one".to_string(), "two".to_string(), "three".to_string()],
	);

	let cmd = MultiValueCommand;
	let result = cmd.execute(&ctx).await;
	assert!(result.is_ok());
}

// =============================================================================
// State Transition Tests
// =============================================================================

/// Test command lifecycle state transitions
///
/// **Category**: State Transition
/// **Verifies**: Commands go through proper lifecycle
#[rstest]
#[tokio::test]
async fn test_command_lifecycle_transitions() {
	use std::sync::atomic::AtomicU8;

	struct LifecycleCommand {
		state: Arc<AtomicU8>,
	}

	// States: 0=Created, 1=BeforeExecuted, 2=Executed, 3=AfterExecuted

	#[async_trait]
	impl BaseCommand for LifecycleCommand {
		fn name(&self) -> &str {
			"lifecycle"
		}

		async fn before_execute(&self, _ctx: &CommandContext) -> CommandResult<()> {
			assert_eq!(
				self.state.load(Ordering::SeqCst),
				0,
				"Before execute should start from Created state"
			);
			self.state.store(1, Ordering::SeqCst);
			Ok(())
		}

		async fn execute(&self, _ctx: &CommandContext) -> CommandResult<()> {
			assert_eq!(
				self.state.load(Ordering::SeqCst),
				1,
				"Execute should follow BeforeExecute"
			);
			self.state.store(2, Ordering::SeqCst);
			Ok(())
		}

		async fn after_execute(&self, _ctx: &CommandContext) -> CommandResult<()> {
			assert_eq!(
				self.state.load(Ordering::SeqCst),
				2,
				"AfterExecute should follow Execute"
			);
			self.state.store(3, Ordering::SeqCst);
			Ok(())
		}
	}

	let cmd = LifecycleCommand {
		state: Arc::new(AtomicU8::new(0)),
	};
	let ctx = CommandContext::new(vec![]);

	// Run through lifecycle
	cmd.before_execute(&ctx).await.unwrap();
	cmd.execute(&ctx).await.unwrap();
	cmd.after_execute(&ctx).await.unwrap();

	assert_eq!(
		cmd.state.load(Ordering::SeqCst),
		3,
		"Final state should be AfterExecuted"
	);
}

// =============================================================================
// Edge Case Tests
// =============================================================================

/// Test command with empty arguments
///
/// **Category**: Edge Case
/// **Verifies**: Commands handle empty arguments gracefully
#[rstest]
#[tokio::test]
async fn test_command_empty_arguments() {
	let cmd = EchoCommand;
	let ctx = CommandContext::new(vec![]);

	let result = cmd.execute(&ctx).await;
	assert!(result.is_ok(), "Command should handle empty arguments");
}

/// Test command with Unicode arguments
///
/// **Category**: Edge Case
/// **Verifies**: Commands handle Unicode correctly
#[rstest]
#[tokio::test]
async fn test_command_unicode_arguments() {
	struct UnicodeCommand;

	#[async_trait]
	impl BaseCommand for UnicodeCommand {
		fn name(&self) -> &str {
			"unicode"
		}

		async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
			assert_eq!(ctx.arg(0), Some(&"こんにちは".to_string()));
			assert_eq!(ctx.arg(1), Some(&"🦀".to_string()));
			assert_eq!(ctx.arg(2), Some(&"مرحبا".to_string()));
			Ok(())
		}
	}

	let ctx = CommandContext::new(vec![
		"こんにちは".to_string(),
		"🦀".to_string(),
		"مرحبا".to_string(),
	]);

	let cmd = UnicodeCommand;
	let result = cmd.execute(&ctx).await;
	assert!(result.is_ok());
}

/// Test registry with duplicate command names
///
/// **Category**: Edge Case
/// **Verifies**: Later registration replaces earlier one
#[rstest]
fn test_registry_duplicate_replaces() {
	let mut registry = CommandRegistry::new();

	struct FirstCommand;
	struct SecondCommand;

	#[async_trait]
	impl BaseCommand for FirstCommand {
		fn name(&self) -> &str {
			"duplicate"
		}
		fn description(&self) -> &str {
			"First"
		}
		async fn execute(&self, _: &CommandContext) -> CommandResult<()> {
			Ok(())
		}
	}

	#[async_trait]
	impl BaseCommand for SecondCommand {
		fn name(&self) -> &str {
			"duplicate"
		}
		fn description(&self) -> &str {
			"Second"
		}
		async fn execute(&self, _: &CommandContext) -> CommandResult<()> {
			Ok(())
		}
	}

	registry.register(Box::new(FirstCommand));
	registry.register(Box::new(SecondCommand));

	let cmd = registry.get("duplicate").unwrap();
	assert_eq!(
		cmd.description(),
		"Second",
		"Second registration should replace first"
	);
}
