//! Mail related commands

use crate::{BaseCommand, CommandContext, CommandError, CommandResult};
use async_trait::async_trait;
use reinhardt_mail::backends::EmailBackend;
use reinhardt_mail::message::EmailMessage;

fn test_email_message(recipients: &[String]) -> CommandResult<EmailMessage> {
	EmailMessage::builder()
		.subject("Test email from Reinhardt")
		.body("This is a test email sent from the sendtestemail command.")
		.from("noreply@example.com")
		.to(recipients.to_vec())
		.build()
		.map_err(|error| CommandError::ExecutionError(error.to_string()))
}

async fn send_test_email_with_backend<B: EmailBackend>(
	backend: &B,
	recipients: &[String],
) -> CommandResult<usize> {
	let message = test_email_message(recipients)?;
	backend
		.send_messages(&[message])
		.await
		.map_err(|error| CommandError::ExecutionError(error.to_string()))
}

/// Management command for sending a test email to verify mail configuration.
pub struct SendTestEmailCommand;

impl SendTestEmailCommand {
	/// Creates a new instance of the send test email command.
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
		use reinhardt_mail::backends::{ConsoleBackend, FileBackend, MemoryBackend};

		// Collect recipients from command arguments
		let mut recipients: Vec<String> = ctx.args.clone();

		// Check for --managers option
		let use_managers = ctx.has_option("managers");
		if use_managers {
			if let Some(settings) = &ctx.settings {
				for manager in &settings.contacts().managers {
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
				for admin in &settings.contacts().admins {
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

		// Select backend and send message
		let sent_count = match backend_name {
			"console" => {
				let backend = ConsoleBackend;
				send_test_email_with_backend(&backend, &recipients).await?
			}
			"memory" => {
				let backend = MemoryBackend::new();
				send_test_email_with_backend(&backend, &recipients).await?
			}
			"file" => {
				let backend = FileBackend::new("/tmp/reinhardt_emails");
				send_test_email_with_backend(&backend, &recipients).await?
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

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_mail::backends::MemoryBackend;

	#[tokio::test]
	async fn send_with_memory_backend_preserves_recipients_and_subject() {
		// Arrange
		let backend = MemoryBackend::new();
		let recipients = vec![
			"first@example.com".to_string(),
			"second@example.com".to_string(),
		];

		// Act
		let sent = send_test_email_with_backend(&backend, &recipients)
			.await
			.expect("memory backend accepts the test email");
		let messages = backend.get_messages().await;

		// Assert
		assert_eq!(sent, 1);
		assert_eq!(messages.len(), 1);
		assert_eq!(messages[0].to(), recipients);
		assert_eq!(messages[0].subject(), "Test email from Reinhardt");
		assert_eq!(messages[0].from_email(), "noreply@example.com");
	}

	#[tokio::test]
	async fn command_rejects_missing_recipients() {
		// Arrange
		let command = SendTestEmailCommand::new();
		let context = CommandContext::default();

		// Act
		let error = command
			.execute(&context)
			.await
			.expect_err("a recipient is required");

		// Assert
		assert!(matches!(
			error,
			CommandError::InvalidArguments(message)
				if message == "You must specify some email recipients, or pass the --managers or --admin options"
		));
	}

	#[tokio::test]
	async fn command_rejects_unknown_backend_before_sending() {
		// Arrange
		let command = SendTestEmailCommand::new();
		let mut context = CommandContext::new(vec!["recipient@example.com".to_string()]);
		context.set_option("backend".to_string(), "smtp".to_string());

		// Act
		let error = command
			.execute(&context)
			.await
			.expect_err("unsupported backends must be rejected");

		// Assert
		assert!(matches!(
			error,
			CommandError::InvalidArguments(message)
				if message == "Unknown backend: smtp. Valid options are: console, memory, file"
		));
	}
}
