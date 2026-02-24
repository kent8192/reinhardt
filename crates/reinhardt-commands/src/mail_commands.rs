//! Mail related commands

use crate::{BaseCommand, CommandContext, CommandError, CommandResult};
use async_trait::async_trait;

pub struct SendTestEmailCommand;

impl SendTestEmailCommand {
	pub fn new() -> Self {
		Self
	}
}

impl Default for SendTestEmailCommand {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl BaseCommand for SendTestEmailCommand {
	fn name(&self) -> &str {
		"sendtestemail"
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		use reinhardt_mail::backends::{ConsoleBackend, EmailBackend, FileBackend, MemoryBackend};
		use reinhardt_mail::message::EmailMessage;

		// Collect recipients from command arguments
		let mut recipients: Vec<String> = ctx.args.clone();

		// Check for --managers option
		let use_managers = ctx.has_option("managers");
		if use_managers {
			if let Some(settings) = &ctx.settings {
				for manager in &settings.managers {
					recipients.push(manager.email.clone());
				}
			} else {
				return Err(CommandError::ExecutionError(
					"Settings not available in command context. Cannot load MANAGERS.".to_string(),
				));
			}
		}

		// Check for --admins option
		let use_admins = ctx.has_option("admins");
		if use_admins {
			if let Some(settings) = &ctx.settings {
				for admin in &settings.admins {
					recipients.push(admin.email.clone());
				}
			} else {
				return Err(CommandError::ExecutionError(
					"Settings not available in command context. Cannot load ADMINS.".to_string(),
				));
			}
		}

		// Validate that we have at least one recipient
		if recipients.is_empty() {
			return Err(CommandError::InvalidArguments(
				"You must specify some email recipients, or pass the --managers or --admin options"
					.to_string(),
			));
		}

		// Get backend option (defaults to console)
		let backend_name = ctx
			.option("backend")
			.map(|s| s.as_str())
			.unwrap_or("console");

		// Check verbose option
		let verbose = ctx.has_option("verbose");

		// Create email message
		let message = EmailMessage::builder()
			.subject("Test email from Reinhardt")
			.body("This is a test email sent from the sendtestemail command.")
			.from("noreply@example.com")
			.to(recipients.clone())
			.build()
			.map_err(|e| CommandError::ExecutionError(e.to_string()))?;

		// Select backend and send message
		let sent_count = match backend_name {
			"console" => {
				let backend = ConsoleBackend;
				backend
					.send_messages(&[message])
					.await
					.map_err(|e| CommandError::ExecutionError(e.to_string()))?
			}
			"memory" => {
				let backend = MemoryBackend::new();
				backend
					.send_messages(&[message])
					.await
					.map_err(|e| CommandError::ExecutionError(e.to_string()))?
			}
			"file" => {
				let backend = FileBackend::new("/tmp/reinhardt_emails");
				backend
					.send_messages(&[message])
					.await
					.map_err(|e| CommandError::ExecutionError(e.to_string()))?
			}
			_ => {
				return Err(CommandError::InvalidArguments(format!(
					"Unknown backend: {}. Valid options are: console, memory, file",
					backend_name
				)));
			}
		};

		// Output results
		if verbose {
			ctx.verbose(&format!(
				"Successfully sent {} test email(s) to {} recipient(s) using {} backend",
				sent_count,
				recipients.len(),
				backend_name
			));
			for recipient in &recipients {
				ctx.verbose(&format!("  - {}", recipient));
			}
		} else {
			ctx.success(&format!(
				"Successfully sent test email to {} recipient(s)",
				recipients.len()
			));
		}

		Ok(())
	}
}
