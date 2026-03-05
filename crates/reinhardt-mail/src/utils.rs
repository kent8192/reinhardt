//! Email utility functions
//!
//! Helper functions for sending emails quickly.

use crate::{EmailMessage, EmailResult, backends::EmailBackend};
use reinhardt_conf::settings::EmailSettings;

/// Send a simple email using the configured backend from settings.
///
/// This is a convenience function for sending simple emails without
/// constructing an EmailMessage manually.
///
/// # Arguments
/// * `settings` - Email configuration settings (determines backend, from_email, etc.)
/// * `subject` - Email subject line
/// * `message` - Plain text email body
/// * `recipient_list` - List of recipient email addresses
/// * `html_message` - Optional HTML body for multipart emails
///
/// # Example
///
/// ```rust,no_run
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use reinhardt_mail::send_mail;
/// use reinhardt_conf::settings::EmailSettings;
///
/// let mut settings = EmailSettings::default();
/// settings.backend = "smtp".to_string();
/// settings.host = "smtp.example.com".to_string();
/// settings.from_email = "noreply@example.com".to_string();
///
/// send_mail(
///     &settings,
///     "Welcome!",
///     "Welcome to our service",
///     vec!["user@example.com"],
///     None,
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn send_mail(
	settings: &reinhardt_conf::settings::EmailSettings,
	subject: impl Into<String>,
	message: impl Into<String>,
	recipient_list: Vec<impl Into<String>>,
	html_message: Option<String>,
) -> EmailResult<()> {
	let backend = crate::backends::backend_from_settings(settings)?;

	let recipients: Vec<String> = recipient_list.into_iter().map(|r| r.into()).collect();

	let mut email_builder = EmailMessage::builder()
		.subject(subject)
		.body(message)
		.from(&settings.from_email)
		.to(recipients);

	if let Some(html) = html_message {
		email_builder = email_builder.html(html);
	}

	let email = email_builder.build()?;
	backend.send_messages(&[email]).await?;
	Ok(())
}
/// Send a simple email with a specific backend
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_mail::{send_mail_with_backend, MemoryBackend};
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let backend = MemoryBackend::new();
///
/// send_mail_with_backend(
///     "Order Confirmation",
///     "Thank you for your order!",
///     "orders@example.com",
///     vec!["customer@example.com"],
///     Some("<h1>Thank you for your order!</h1>".to_string()),
///     &backend,
/// ).await?;
///
/// assert_eq!(backend.count().await, 1);
/// # Ok(())
/// # }
/// ```
pub async fn send_mail_with_backend(
	subject: impl Into<String>,
	message: impl Into<String>,
	from_email: impl Into<String>,
	recipient_list: Vec<impl Into<String>>,
	html_message: Option<String>,
	backend: &dyn EmailBackend,
) -> EmailResult<()> {
	let recipients: Vec<String> = recipient_list.into_iter().map(|r| r.into()).collect();

	let mut email_builder = EmailMessage::builder()
		.subject(subject)
		.body(message)
		.from(from_email)
		.to(recipients);

	if let Some(html) = html_message {
		email_builder = email_builder.html(html);
	}

	let email = email_builder.build()?;
	backend.send_messages(&[email]).await?;
	Ok(())
}
/// Send multiple emails using the same connection (bulk send)
///
/// This is more efficient than sending emails individually when you
/// have many emails to send.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_mail::{send_mass_mail, EmailMessage, MemoryBackend};
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let backend = MemoryBackend::new();
///
/// let messages = vec![
///     EmailMessage::builder()
///         .subject("Newsletter #1")
///         .body("This month's updates")
///         .from("newsletter@example.com")
///         .to(vec!["user1@example.com".to_string()])
///         .build()?,
///     EmailMessage::builder()
///         .subject("Newsletter #1")
///         .body("This month's updates")
///         .from("newsletter@example.com")
///         .to(vec!["user2@example.com".to_string()])
///         .build()?,
/// ];
///
/// let results = send_mass_mail(messages, &backend).await?;
/// assert_eq!(results, 2);
/// assert_eq!(backend.count().await, 2);
/// # Ok(())
/// # }
/// ```
pub async fn send_mass_mail(
	messages: Vec<EmailMessage>,
	backend: &dyn EmailBackend,
) -> EmailResult<usize> {
	backend.send_messages(&messages).await
}
/// Send an email to administrators
///
/// Sends an email to all administrators defined in settings.admins.
/// The subject will be prefixed with settings.subject_prefix if set.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_mail::{mail_admins, MemoryBackend};
/// use reinhardt_conf::settings::EmailSettings;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut settings = EmailSettings::default();
/// settings.admins = vec![
///     ("Admin".to_string(), "admin@example.com".to_string()),
/// ];
/// settings.from_email = "system@example.com".to_string();
/// settings.subject_prefix = "[ALERT]".to_string();
///
/// let backend = MemoryBackend::new();
///
/// mail_admins(
///     &settings,
///     "Database Error",
///     "Connection timeout occurred",
///     false,
///     &backend,
/// ).await?;
///
/// assert_eq!(backend.count().await, 1);
/// let messages = backend.get_messages().await;
/// assert!(messages[0].subject().starts_with("[ALERT]"));
/// # Ok(())
/// # }
/// ```
pub async fn mail_admins(
	settings: &EmailSettings,
	subject: impl Into<String>,
	message: impl Into<String>,
	fail_silently: bool,
	backend: &dyn EmailBackend,
) -> EmailResult<()> {
	if settings.admins.is_empty() {
		if fail_silently {
			return Ok(());
		} else {
			return Err(crate::EmailError::MissingField("admins".to_string()));
		}
	}

	let admin_emails: Vec<String> = settings
		.admins
		.iter()
		.map(|(_, email)| email.clone())
		.collect();

	let subject_str = subject.into();
	let final_subject = if !settings.subject_prefix.is_empty() {
		format!("{} {}", settings.subject_prefix, subject_str)
	} else {
		subject_str
	};

	let from_email = if !settings.server_email.is_empty() {
		settings.server_email.clone()
	} else {
		settings.from_email.clone()
	};

	let result = send_mail_with_backend(
		final_subject,
		message,
		from_email,
		admin_emails,
		None,
		backend,
	)
	.await;

	match result {
		Ok(()) => Ok(()),
		Err(e) if fail_silently && e.is_transient() => {
			eprintln!("Email to admins failed (fail_silently=true): {}", e);
			Ok(())
		}
		Err(e) => Err(e),
	}
}
/// Send an email to managers
///
/// Sends an email to all managers defined in settings.managers.
/// The subject will be prefixed with settings.subject_prefix if set.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_mail::{mail_managers, MemoryBackend};
/// use reinhardt_conf::settings::EmailSettings;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut settings = EmailSettings::default();
/// settings.managers = vec![
///     ("Manager".to_string(), "manager@example.com".to_string()),
/// ];
/// settings.from_email = "system@example.com".to_string();
/// settings.subject_prefix = "[INFO]".to_string();
///
/// let backend = MemoryBackend::new();
///
/// mail_managers(
///     &settings,
///     "Weekly Report",
///     "User signups increased by 20%",
///     false,
///     &backend,
/// ).await?;
///
/// assert_eq!(backend.count().await, 1);
/// let messages = backend.get_messages().await;
/// assert!(messages[0].subject().starts_with("[INFO]"));
/// # Ok(())
/// # }
/// ```
pub async fn mail_managers(
	settings: &EmailSettings,
	subject: impl Into<String>,
	message: impl Into<String>,
	fail_silently: bool,
	backend: &dyn EmailBackend,
) -> EmailResult<()> {
	if settings.managers.is_empty() {
		if fail_silently {
			return Ok(());
		} else {
			return Err(crate::EmailError::MissingField("managers".to_string()));
		}
	}

	let manager_emails: Vec<String> = settings
		.managers
		.iter()
		.map(|(_, email)| email.clone())
		.collect();

	let subject_str = subject.into();
	let final_subject = if !settings.subject_prefix.is_empty() {
		format!("{} {}", settings.subject_prefix, subject_str)
	} else {
		subject_str
	};

	let from_email = if !settings.server_email.is_empty() {
		settings.server_email.clone()
	} else {
		settings.from_email.clone()
	};

	let result = send_mail_with_backend(
		final_subject,
		message,
		from_email,
		manager_emails,
		None,
		backend,
	)
	.await;

	match result {
		Ok(()) => Ok(()),
		Err(e) if fail_silently && e.is_transient() => {
			eprintln!("Email to managers failed (fail_silently=true): {}", e);
			Ok(())
		}
		Err(e) => Err(e),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::backends::MemoryBackend;

	#[tokio::test]
	async fn test_send_mail() {
		let backend = MemoryBackend::new();

		let result = send_mail_with_backend(
			"Test Subject",
			"Test Message",
			"from@example.com",
			vec!["to@example.com"],
			None,
			&backend,
		)
		.await;

		assert!(result.is_ok());
		assert_eq!(backend.count().await, 1);
	}

	#[tokio::test]
	async fn test_send_mail_with_html() {
		let backend = MemoryBackend::new();

		let result = send_mail_with_backend(
			"Test Subject",
			"Test Message",
			"from@example.com",
			vec!["to@example.com"],
			Some("<h1>Test HTML</h1>".to_string()),
			&backend,
		)
		.await;

		assert!(result.is_ok());

		let messages = backend.get_messages().await;
		assert!(messages[0].html_body().is_some());
	}

	#[tokio::test]
	async fn test_send_mass_mail() {
		let backend = MemoryBackend::new();

		let messages = vec![
			EmailMessage::builder()
				.subject("Test 1")
				.body("Body 1")
				.from("from@example.com")
				.to(vec!["to1@example.com".to_string()])
				.build()
				.unwrap(),
			EmailMessage::builder()
				.subject("Test 2")
				.body("Body 2")
				.from("from@example.com")
				.to(vec!["to2@example.com".to_string()])
				.build()
				.unwrap(),
		];

		let results = send_mass_mail(messages, &backend).await.unwrap();

		assert_eq!(results, 2);
		assert_eq!(backend.count().await, 2);
	}
}
