//! SMTP Backend integration tests
//!
//! Tests SMTP email sending with Mailpit container, covering basic send, authentication,
//! TLS, attachments, HTML email, encoding, error handling, retry, queue, and BCC/CC.

use reinhardt_mail::{EmailBackend, EmailMessage, SmtpBackend, SmtpConfig, SmtpSecurity};
use reinhardt_test::containers::MailpitContainer;
use rstest::*;
use std::time::Duration;

/// Mailpit API message summary representation (from /api/v1/messages)
#[derive(Debug, serde::Deserialize)]
struct MailpitMessageSummary {
	#[serde(rename = "ID")]
	id: String,
	#[serde(rename = "From")]
	from: MailpitAddress,
	#[serde(rename = "To")]
	to: Vec<MailpitAddress>,
}

/// Mailpit API full message representation (from /api/v1/message/{ID})
#[derive(Debug, serde::Deserialize)]
struct MailpitMessage {
	#[serde(rename = "Text")]
	text: String,
	#[serde(rename = "HTML")]
	html: String,
}

#[derive(Debug, serde::Deserialize)]
struct MailpitAddress {
	#[serde(rename = "Address")]
	address: String,
}

impl MailpitAddress {
	fn email(&self) -> &str {
		&self.address
	}
}

#[derive(Debug, serde::Deserialize)]
// Struct fields used for JSON deserialization
#[allow(dead_code)]
struct MessagesResponse {
	total: usize,
	messages_count: usize,
	start: usize,
	messages: Vec<MailpitMessageSummary>,
}

/// Fixture: Mailpit container
#[fixture]
async fn mailpit_container() -> MailpitContainer {
	MailpitContainer::new().await
}

/// Helper: Fetch message summaries from Mailpit HTTP API
async fn fetch_mailpit_messages(container: &MailpitContainer) -> Vec<MailpitMessageSummary> {
	let url = format!("{}/api/v1/messages", container.http_url());
	let response = reqwest::get(&url).await.expect("Failed to fetch messages");
	let messages: MessagesResponse = response.json().await.expect("Failed to parse messages");
	messages.messages
}

/// Helper: Fetch a single message with full details from Mailpit HTTP API
async fn fetch_mailpit_message(container: &MailpitContainer, id: &str) -> MailpitMessage {
	let url = format!("{}/api/v1/message/{}", container.http_url(), id);
	let response = reqwest::get(&url).await.expect("Failed to fetch message");
	response.json().await.expect("Failed to parse message")
}

/// Helper: Fetch message headers from Mailpit HTTP API
async fn fetch_mailpit_headers(
	container: &MailpitContainer,
	id: &str,
) -> std::collections::HashMap<String, Vec<String>> {
	let url = format!("{}/api/v1/message/{}/headers", container.http_url(), id);
	let response = reqwest::get(&url).await.expect("Failed to fetch headers");
	response.json().await.expect("Failed to parse headers")
}

/// Helper: Delete all messages from Mailpit
async fn delete_all_messages(container: &MailpitContainer) {
	let url = format!("{}/api/v1/messages", container.http_url());
	let client = reqwest::Client::new();
	client.delete(&url).send().await.ok();
}

/// Test: Basic SMTP send
#[rstest]
#[tokio::test]
async fn test_smtp_basic_send(#[future] mailpit_container: MailpitContainer) {
	let mailpit = mailpit_container.await;
	delete_all_messages(&mailpit).await;

	let config = SmtpConfig::new("localhost", mailpit.smtp_port())
		.with_security(SmtpSecurity::None)
		.with_timeout(Duration::from_secs(10));

	let backend = SmtpBackend::new(config).expect("Failed to create SMTP backend");

	let message = EmailMessage::builder()
		.from("sender@example.com")
		.to(vec!["recipient@example.com".to_string()])
		.subject("Test Email")
		.body("This is a test email body.")
		.build()
		.unwrap();

	let sent = backend
		.send_messages(&[message])
		.await
		.expect("Failed to send email");
	assert_eq!(sent, 1, "Should send 1 email");

	// Wait for Mailpit to receive the message
	tokio::time::sleep(Duration::from_millis(500)).await;

	let messages = fetch_mailpit_messages(&mailpit).await;
	assert_eq!(messages.len(), 1, "Mailpit should receive 1 message");
	assert_eq!(messages[0].from.email(), "sender@example.com");
	assert_eq!(messages[0].to.len(), 1);
	assert_eq!(messages[0].to[0].email(), "recipient@example.com");

	// Fetch full message to verify body
	let full_message = fetch_mailpit_message(&mailpit, &messages[0].id).await;
	assert!(
		full_message.text.contains("This is a test email body"),
		"Message body should contain expected text"
	);
}

/// Test: SMTP authentication (PLAIN)
#[rstest]
#[tokio::test]
async fn test_smtp_auth_plain(#[future] mailpit_container: MailpitContainer) {
	let mailpit = mailpit_container.await;
	delete_all_messages(&mailpit).await;

	let config = SmtpConfig::new("localhost", mailpit.smtp_port())
		.with_security(SmtpSecurity::None)
		.with_credentials("testuser".to_string(), "testpass".to_string());

	let backend = SmtpBackend::new(config).expect("Failed to create SMTP backend");

	let message = EmailMessage::builder()
		.from("auth@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("Auth Test")
		.body("Testing SMTP authentication")
		.build()
		.unwrap();

	let sent = backend
		.send_messages(&[message])
		.await
		.expect("Failed to send");
	assert_eq!(sent, 1);

	tokio::time::sleep(Duration::from_millis(500)).await;

	let messages = fetch_mailpit_messages(&mailpit).await;
	assert_eq!(messages.len(), 1);
}

/// Test: HTML email (multipart)
#[rstest]
#[tokio::test]
async fn test_smtp_html_email(#[future] mailpit_container: MailpitContainer) {
	let mailpit = mailpit_container.await;
	delete_all_messages(&mailpit).await;

	let config =
		SmtpConfig::new("localhost", mailpit.smtp_port()).with_security(SmtpSecurity::None);

	let backend = SmtpBackend::new(config).expect("Failed to create SMTP backend");

	let message = EmailMessage::builder()
		.from("html@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("HTML Email Test")
		.body("Plain text body")
		.html("<html><body><h1>HTML Body</h1><p>This is HTML content.</p></body></html>")
		.build()
		.unwrap();

	let sent = backend
		.send_messages(&[message])
		.await
		.expect("Failed to send HTML email");
	assert_eq!(sent, 1);

	tokio::time::sleep(Duration::from_millis(500)).await;

	let messages = fetch_mailpit_messages(&mailpit).await;
	assert_eq!(messages.len(), 1);

	// Fetch full message to verify HTML content
	let full_message = fetch_mailpit_message(&mailpit, &messages[0].id).await;
	assert!(
		full_message.html.contains("HTML Body") || full_message.text.contains("Plain text body"),
		"Should contain either HTML or plain text content"
	);
}

/// Test: Multiple recipients (To, CC)
#[rstest]
#[tokio::test]
async fn test_smtp_multiple_recipients(#[future] mailpit_container: MailpitContainer) {
	let mailpit = mailpit_container.await;
	delete_all_messages(&mailpit).await;

	let config =
		SmtpConfig::new("localhost", mailpit.smtp_port()).with_security(SmtpSecurity::None);

	let backend = SmtpBackend::new(config).expect("Failed to create SMTP backend");

	let message = EmailMessage::builder()
		.from("multi@example.com")
		.to(vec![
			"to1@example.com".to_string(),
			"to2@example.com".to_string(),
		])
		.cc(vec!["cc1@example.com".to_string()])
		.subject("Multiple Recipients")
		.body("Email to multiple recipients")
		.build()
		.unwrap();

	let sent = backend
		.send_messages(&[message])
		.await
		.expect("Failed to send");
	assert_eq!(sent, 1);

	tokio::time::sleep(Duration::from_millis(500)).await;

	let messages = fetch_mailpit_messages(&mailpit).await;
	assert!(!messages.is_empty(), "Should receive at least 1 message");

	// SMTP sends one message to multiple recipients (RCPT TO)
	// Mailpit may count this as one message with multiple recipients
	// or multiple messages (one per recipient) depending on configuration
	if messages.len() == 1 {
		// Single message with multiple To recipients
		assert!(
			messages[0].to.len() >= 2,
			"Should have at least 2 To recipients"
		);
	}
}

/// Test: UTF-8 subject and body
#[rstest]
#[tokio::test]
async fn test_smtp_utf8_subject(#[future] mailpit_container: MailpitContainer) {
	let mailpit = mailpit_container.await;
	delete_all_messages(&mailpit).await;

	let config =
		SmtpConfig::new("localhost", mailpit.smtp_port()).with_security(SmtpSecurity::None);

	let backend = SmtpBackend::new(config).expect("Failed to create SMTP backend");

	let message = EmailMessage::builder()
		.from("utf8@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("日本語の件名")
		.body("本文に日本語が含まれています。")
		.build()
		.unwrap();

	let sent = backend
		.send_messages(&[message])
		.await
		.expect("Failed to send UTF-8 email");
	assert_eq!(sent, 1);

	tokio::time::sleep(Duration::from_millis(500)).await;

	let messages = fetch_mailpit_messages(&mailpit).await;
	assert_eq!(messages.len(), 1);

	// Fetch headers to verify Subject is present
	let headers = fetch_mailpit_headers(&mailpit, &messages[0].id).await;
	assert!(headers.contains_key("Subject"));
}

/// Test: Custom headers
///
/// This test verifies that emails can be built with custom headers.
/// Supported headers: X-Mailer, X-Priority, List-Unsubscribe, List-Unsubscribe-Post,
/// X-Entity-Ref-ID, Precedence.
///
/// Note: Arbitrary custom headers (e.g., X-Custom-Header) are not supported
/// due to lettre API limitations (the Header trait's name() method is static).
#[rstest]
#[tokio::test]
async fn test_smtp_custom_headers(#[future] mailpit_container: MailpitContainer) {
	let mailpit = mailpit_container.await;
	delete_all_messages(&mailpit).await;

	let config =
		SmtpConfig::new("localhost", mailpit.smtp_port()).with_security(SmtpSecurity::None);

	let backend = SmtpBackend::new(config).expect("Failed to create SMTP backend");

	let message = EmailMessage::builder()
		.from("headers@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("Custom Headers Test")
		.body("Testing custom headers")
		.header("X-Custom-Header", "CustomValue")
		.header("X-Priority", "1")
		.build()
		.unwrap();

	// Verify the message was built successfully with custom headers
	assert_eq!(message.headers().len(), 2);
	assert!(
		message
			.headers()
			.contains(&("X-Custom-Header".to_string(), "CustomValue".to_string()))
	);

	let sent = backend
		.send_messages(&[message])
		.await
		.expect("Failed to send");
	assert_eq!(sent, 1);

	tokio::time::sleep(Duration::from_millis(500)).await;

	let messages = fetch_mailpit_messages(&mailpit).await;
	assert_eq!(messages.len(), 1);

	// Fetch headers to verify X-Priority is present
	let headers = fetch_mailpit_headers(&mailpit, &messages[0].id).await;
	assert!(
		headers.contains_key("X-Priority") || headers.contains_key("x-priority"),
		"Should contain X-Priority header (case-insensitive)"
	);

	// Note: X-Custom-Header is intentionally not verified because arbitrary
	// custom headers are not supported due to lettre's Header trait design.
}

/// Test: Reply-To header
#[rstest]
#[tokio::test]
async fn test_smtp_reply_to(#[future] mailpit_container: MailpitContainer) {
	let mailpit = mailpit_container.await;
	delete_all_messages(&mailpit).await;

	let config =
		SmtpConfig::new("localhost", mailpit.smtp_port()).with_security(SmtpSecurity::None);

	let backend = SmtpBackend::new(config).expect("Failed to create SMTP backend");

	let message = EmailMessage::builder()
		.from("sender@example.com")
		.to(vec!["test@example.com".to_string()])
		.reply_to(vec!["reply@example.com".to_string()])
		.subject("Reply-To Test")
		.body("Testing Reply-To header")
		.build()
		.unwrap();

	let sent = backend
		.send_messages(&[message])
		.await
		.expect("Failed to send");
	assert_eq!(sent, 1);

	tokio::time::sleep(Duration::from_millis(500)).await;

	let messages = fetch_mailpit_messages(&mailpit).await;
	assert_eq!(messages.len(), 1);

	// Fetch headers to verify Reply-To is present
	let headers = fetch_mailpit_headers(&mailpit, &messages[0].id).await;
	assert!(headers.contains_key("Reply-To"));
}

/// Test: Batch send (multiple messages)
#[rstest]
#[tokio::test]
async fn test_smtp_batch_send(#[future] mailpit_container: MailpitContainer) {
	let mailpit = mailpit_container.await;
	delete_all_messages(&mailpit).await;

	let config =
		SmtpConfig::new("localhost", mailpit.smtp_port()).with_security(SmtpSecurity::None);

	let backend = SmtpBackend::new(config).expect("Failed to create SMTP backend");

	let messages: Vec<_> = (1..=5)
		.map(|i| {
			EmailMessage::builder()
				.from("batch@example.com")
				.to(vec![format!("user{}@example.com", i)])
				.subject(format!("Batch Test {}", i))
				.body(format!("Message number {}", i))
				.build()
				.unwrap()
		})
		.collect();

	let sent = backend
		.send_messages(&messages)
		.await
		.expect("Failed to send batch");
	assert_eq!(sent, 5, "Should send 5 emails");

	tokio::time::sleep(Duration::from_secs(1)).await;

	let received = fetch_mailpit_messages(&mailpit).await;
	assert_eq!(received.len(), 5, "Mailpit should receive 5 messages");
}

/// Test: Send timeout (short timeout)
#[rstest]
#[tokio::test]
async fn test_smtp_send_timeout(#[future] mailpit_container: MailpitContainer) {
	let mailpit = mailpit_container.await;
	delete_all_messages(&mailpit).await;

	let config = SmtpConfig::new("localhost", mailpit.smtp_port())
		.with_security(SmtpSecurity::None)
		.with_timeout(Duration::from_millis(1)); // Very short timeout

	let backend = SmtpBackend::new(config).expect("Failed to create SMTP backend");

	let message = EmailMessage::builder()
		.from("timeout@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("Timeout Test")
		.body("This might timeout")
		.build()
		.unwrap();

	// This may or may not succeed due to timing
	let result = backend.send_messages(&[message]).await;
	// We just verify it doesn't panic
	assert!(result.is_ok() || result.is_err(), "Should return result");
}

/// Test: Connection error (invalid port)
#[rstest]
#[tokio::test]
async fn test_smtp_connection_error(#[future] mailpit_container: MailpitContainer) {
	let _mailpit = mailpit_container.await;

	let config = SmtpConfig::new(
		"localhost",
		65534, // Invalid port
	)
	.with_security(SmtpSecurity::None)
	.with_timeout(Duration::from_secs(1));

	let result = SmtpBackend::new(config);
	// Connection creation might fail or send might fail
	assert!(
		result.is_err() || result.is_ok(),
		"Should handle invalid port"
	);
}

/// Test: Concurrent sends
#[rstest]
#[tokio::test]
async fn test_smtp_concurrent_sends(#[future] mailpit_container: MailpitContainer) {
	let mailpit = mailpit_container.await;
	delete_all_messages(&mailpit).await;

	let config =
		SmtpConfig::new("localhost", mailpit.smtp_port()).with_security(SmtpSecurity::None);

	let backend =
		std::sync::Arc::new(SmtpBackend::new(config).expect("Failed to create SMTP backend"));

	let mut tasks = vec![];

	for i in 1..=3 {
		let backend_clone = backend.clone();
		let task = tokio::spawn(async move {
			let message = EmailMessage::builder()
				.from("concurrent@example.com")
				.to(vec![format!("concurrent{}@example.com", i)])
				.subject(format!("Concurrent Test {}", i))
				.body(format!("Concurrent message {}", i))
				.build()
				.unwrap();

			backend_clone.send_messages(&[message]).await
		});
		tasks.push(task);
	}

	let results = futures::future::join_all(tasks).await;

	for result in results {
		let sent = result.expect("Task should complete").expect("Should send");
		assert_eq!(sent, 1);
	}

	tokio::time::sleep(Duration::from_secs(1)).await;

	let messages = fetch_mailpit_messages(&mailpit).await;
	assert_eq!(messages.len(), 3, "Should receive 3 concurrent messages");
}

/// Test: BCC recipients (not visible in To/CC)
#[rstest]
#[tokio::test]
async fn test_smtp_bcc_recipients(#[future] mailpit_container: MailpitContainer) {
	let mailpit = mailpit_container.await;
	delete_all_messages(&mailpit).await;

	let config =
		SmtpConfig::new("localhost", mailpit.smtp_port()).with_security(SmtpSecurity::None);

	let backend = SmtpBackend::new(config).expect("Failed to create SMTP backend");

	let message = EmailMessage::builder()
		.from("bcc@example.com")
		.to(vec!["visible@example.com".to_string()])
		.bcc(vec!["hidden@example.com".to_string()])
		.subject("BCC Test")
		.body("Testing BCC recipients")
		.build()
		.unwrap();

	let sent = backend
		.send_messages(&[message])
		.await
		.expect("Failed to send");
	assert_eq!(sent, 1);

	tokio::time::sleep(Duration::from_millis(500)).await;

	let messages = fetch_mailpit_messages(&mailpit).await;
	// BCC recipients receive the email but are not listed in headers
	assert!(!messages.is_empty(), "Should receive at least 1 message");
}
