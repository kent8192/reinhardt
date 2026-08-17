use async_trait::async_trait;
use reinhardt_conf::EmailSettings;
use reinhardt_mail::{
	EmailBackend, EmailError, EmailMessage, EmailResult, MemoryBackend, mail_admins, mail_managers,
	send_mail_with_backend,
};
use rstest::rstest;

struct TransientFailureBackend;

#[async_trait]
impl EmailBackend for TransientFailureBackend {
	async fn send_messages(&self, _messages: &[EmailMessage]) -> EmailResult<usize> {
		Err(EmailError::IoError(std::io::Error::other(
			"transient failure",
		)))
	}
}

struct PermanentFailureBackend;

#[async_trait]
impl EmailBackend for PermanentFailureBackend {
	async fn send_messages(&self, _messages: &[EmailMessage]) -> EmailResult<usize> {
		Err(EmailError::BackendError("permanent failure".to_string()))
	}
}

#[rstest]
#[tokio::test]
async fn role_mail_helpers_apply_recipient_and_failure_policy() {
	// Arrange
	let mut settings = EmailSettings::default();
	settings.admins = vec![("Admin".to_string(), "admin@example.com".to_string())];
	settings.managers = vec![("Manager".to_string(), "manager@example.com".to_string())];
	settings.server_email = "server@example.com".to_string();
	settings.from_email = "fallback@example.com".to_string();
	settings.subject_prefix = "[Reinhardt]".to_string();
	let backend = MemoryBackend::new();
	let empty_settings = EmailSettings::default();
	let transient = TransientFailureBackend;
	let permanent = PermanentFailureBackend;

	// Act
	mail_admins(
		&settings,
		"Database Error",
		"Connection timeout",
		false,
		&backend,
	)
	.await
	.unwrap();
	mail_managers(&settings, "Weekly Report", "New signups", false, &backend)
		.await
		.unwrap();
	send_mail_with_backend(
		"Welcome",
		"Plain body",
		"sender@example.com",
		vec!["recipient@example.com"],
		Some("<p>HTML body</p>".to_string()),
		&backend,
	)
	.await
	.unwrap();
	let messages = backend.get_messages().await;

	let silent_admins = mail_admins(&empty_settings, "No admins", "Ignored", true, &backend).await;
	let count_after_silent = backend.count().await;
	let missing_admins =
		mail_admins(&empty_settings, "No admins", "Rejected", false, &backend).await;
	let missing_managers =
		mail_managers(&empty_settings, "No managers", "Rejected", false, &backend).await;

	let transient_suppressed =
		mail_admins(&settings, "Transient", "Suppressed", true, &transient).await;
	let transient_propagated =
		mail_admins(&settings, "Transient", "Propagated", false, &transient).await;
	let permanent_suppressed =
		mail_admins(&settings, "Permanent", "Must propagate", true, &permanent).await;

	// Assert
	assert_eq!(messages.len(), 3);
	assert_eq!(messages[0].subject(), "[Reinhardt] Database Error");
	assert_eq!(messages[0].from_email(), "server@example.com");
	assert_eq!(messages[0].to(), &["admin@example.com".to_string()]);
	assert_eq!(messages[0].body(), "Connection timeout");
	assert_eq!(messages[1].subject(), "[Reinhardt] Weekly Report");
	assert_eq!(messages[1].to(), &["manager@example.com".to_string()]);
	assert_eq!(messages[2].html_body(), Some("<p>HTML body</p>"));
	assert_eq!(messages[2].to(), &["recipient@example.com".to_string()]);
	silent_admins.unwrap();
	assert_eq!(count_after_silent, 3);
	assert_eq!(
		missing_admins.unwrap_err().to_string(),
		"Missing required field: admins"
	);
	assert_eq!(
		missing_managers.unwrap_err().to_string(),
		"Missing required field: managers"
	);
	transient_suppressed.unwrap();
	assert_eq!(
		transient_propagated.unwrap_err().to_string(),
		"IO error: transient failure"
	);
	assert_eq!(
		permanent_suppressed.unwrap_err().to_string(),
		"Backend error: permanent failure"
	);
}
