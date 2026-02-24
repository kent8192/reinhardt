//! Base command trait
//!
//! Defines the interface for all management commands.

use crate::{CommandContext, CommandError, CommandResult};
use async_trait::async_trait;
use reinhardt_utils::utils_core::checks::{CheckLevel, CheckRegistry};

/// Base command trait
///
/// All management commands must implement this trait.
#[async_trait]
pub trait BaseCommand: Send + Sync {
	/// Get the command name
	fn name(&self) -> &str;

	/// Get the command description
	fn description(&self) -> &str {
		"No description available"
	}

	/// Get the command help text
	fn help(&self) -> &str {
		self.description()
	}

	/// Define command arguments
	fn arguments(&self) -> Vec<CommandArgument> {
		Vec::new()
	}

	/// Define command options/flags
	fn options(&self) -> Vec<CommandOption> {
		Vec::new()
	}

	/// Execute the command
	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()>;

	/// Called before execute
	async fn before_execute(&self, _ctx: &CommandContext) -> CommandResult<()> {
		Ok(())
	}

	/// Called after execute
	async fn after_execute(&self, _ctx: &CommandContext) -> CommandResult<()> {
		Ok(())
	}

	/// Whether this command requires system checks to run
	///
	/// By default, commands require system checks. Override this to disable checks.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_commands::BaseCommand;
	/// # use reinhardt_commands::{CommandContext, CommandResult};
	/// # use async_trait::async_trait;
	///
	/// struct MyCommand;
	///
	/// #[async_trait]
	/// impl BaseCommand for MyCommand {
	///     fn name(&self) -> &str {
	///         "mycommand"
	///     }
	///
	///     fn requires_system_checks(&self) -> bool {
	///         false  // Disable system checks for this command
	///     }
	///
	///     async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
	///         Ok(())
	///     }
	/// }
	/// ```
	fn requires_system_checks(&self) -> bool {
		true
	}

	/// Tags for system checks to run
	///
	/// If empty, all checks are run. If specified, only checks with matching tags are run.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_commands::BaseCommand;
	/// # use reinhardt_commands::{CommandContext, CommandResult};
	/// # use async_trait::async_trait;
	///
	/// struct MyCommand;
	///
	/// #[async_trait]
	/// impl BaseCommand for MyCommand {
	///     fn name(&self) -> &str {
	///         "mycommand"
	///     }
	///
	///     fn check_tags(&self) -> Vec<String> {
	///         vec!["staticfiles".to_string(), "models".to_string()]
	///     }
	///
	///     async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
	///         Ok(())
	///     }
	/// }
	/// ```
	fn check_tags(&self) -> Vec<String> {
		vec![]
	}

	/// Run the full command lifecycle
	///
	/// This method runs system checks (if required), then executes the command lifecycle:
	/// before_execute -> execute -> after_execute
	async fn run(&self, ctx: &CommandContext) -> CommandResult<()> {
		// Run system checks if required and not skipped
		if self.requires_system_checks() && !ctx.should_skip_checks() {
			let registry = CheckRegistry::global();
			let registry_guard = registry.lock().unwrap_or_else(|poisoned| {
				// Recover the inner value from a poisoned mutex.
				// The check registry data is still usable even after a panic in another thread.
				poisoned.into_inner()
			});

			let tags = self.check_tags();
			let messages = registry_guard.run_checks(&tags);

			// Check for errors or critical issues
			for msg in &messages {
				match msg.level {
					CheckLevel::Critical | CheckLevel::Error => {
						let hint_text = msg
							.hint
							.as_ref()
							.map(|h| format!("\nHint: {}", h))
							.unwrap_or_default();
						return Err(CommandError::ExecutionError(format!(
							"System check failed [{}]: {}{}",
							msg.id, msg.message, hint_text
						)));
					}
					CheckLevel::Warning => {
						ctx.warning(&format!("[{}] {}", msg.id, msg.message));
					}
					CheckLevel::Info => {
						ctx.info(&format!("[{}] {}", msg.id, msg.message));
					}
					CheckLevel::Debug => {
						// Debug messages are typically not shown unless verbose mode
					}
				}
			}
		}

		self.before_execute(ctx).await?;
		self.execute(ctx).await?;
		self.after_execute(ctx).await?;
		Ok(())
	}
}

/// Command argument definition
#[derive(Debug, Clone)]
pub struct CommandArgument {
	pub name: String,
	pub description: String,
	pub required: bool,
	pub default: Option<String>,
}

impl CommandArgument {
	/// Create a new required argument
	///
	pub fn required(name: impl Into<String>, description: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			description: description.into(),
			required: true,
			default: None,
		}
	}
	/// Create a new optional argument
	///
	pub fn optional(name: impl Into<String>, description: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			description: description.into(),
			required: false,
			default: None,
		}
	}
	/// Set a default value
	pub fn with_default(mut self, default: impl Into<String>) -> Self {
		self.default = Some(default.into());
		self
	}
}

/// Command option/flag definition
#[derive(Debug, Clone)]
pub struct CommandOption {
	pub short: Option<char>,
	pub long: String,
	pub description: String,
	pub takes_value: bool,
	pub required: bool,
	pub default: Option<String>,
	pub multiple: bool,
}

impl CommandOption {
	/// Create a new flag (boolean option)
	///
	pub fn flag(
		short: Option<char>,
		long: impl Into<String>,
		description: impl Into<String>,
	) -> Self {
		Self {
			short,
			long: long.into(),
			description: description.into(),
			takes_value: false,
			required: false,
			default: None,
			multiple: false,
		}
	}
	/// Create a new option that takes a value
	///
	pub fn option(
		short: Option<char>,
		long: impl Into<String>,
		description: impl Into<String>,
	) -> Self {
		Self {
			short,
			long: long.into(),
			description: description.into(),
			takes_value: true,
			required: false,
			default: None,
			multiple: false,
		}
	}
	/// Make this option required
	///
	pub fn required(mut self) -> Self {
		self.required = true;
		self
	}
	/// Set a default value
	pub fn with_default(mut self, default: impl Into<String>) -> Self {
		self.default = Some(default.into());
		self
	}
	/// Allow this option to accept multiple values
	///
	pub fn multi(mut self) -> Self {
		self.multiple = true;
		self
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	struct TestCommand;

	#[async_trait]
	impl BaseCommand for TestCommand {
		fn name(&self) -> &str {
			"test"
		}

		fn description(&self) -> &str {
			"A test command"
		}

		async fn execute(&self, _ctx: &CommandContext) -> CommandResult<()> {
			Ok(())
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_command_basic() {
		let cmd = TestCommand;
		assert_eq!(cmd.name(), "test");
		assert_eq!(cmd.description(), "A test command");
	}
}
