//! EmailMessage Builder API integration tests
//!
//! Tests EmailMessage fluent API, covering builder pattern, validation,
//! headers, attachments, alternatives, encoding, and message construction.

use reinhardt_mail::{Alternative, Attachment, EmailMessage};
use rstest::rstest;

/// Test: Builder pattern basic construction
#[rstest]
fn test_builder_basic_construction() {
	// Arrange
	let builder = EmailMessage::builder()
		.from("sender@example.com")
		.to(vec!["recipient@example.com".to_string()])
		.subject("Test Subject")
		.body("Test Body");

	// Act
	let message = builder.build().unwrap();

	// Assert
	assert_eq!(message.from_email(), "sender@example.com");
	assert_eq!(message.to(), vec!["recipient@example.com"]);
	assert_eq!(message.subject(), "Test Subject");
	assert_eq!(message.body(), "Test Body");
}

/// Test: Builder method chaining
#[rstest]
fn test_builder_method_chaining() {
	// Arrange & Act
	let message = EmailMessage::builder()
		.from("chain@example.com")
		.to(vec!["to@example.com".to_string()])
		.cc(vec!["cc@example.com".to_string()])
		.bcc(vec!["bcc@example.com".to_string()])
		.reply_to(vec!["reply@example.com".to_string()])
		.subject("Chained")
		.body("Body")
		.build()
		.unwrap();

	// Assert
	assert_eq!(message.from_email(), "chain@example.com");
	assert_eq!(message.to(), vec!["to@example.com"]);
	assert_eq!(message.cc(), vec!["cc@example.com"]);
	assert_eq!(message.bcc(), vec!["bcc@example.com"]);
	assert_eq!(message.reply_to(), vec!["reply@example.com"]);
	assert_eq!(message.subject(), "Chained");
	assert_eq!(message.body(), "Body");
}

/// Test: Builder with HTML body
#[rstest]
fn test_builder_html_body() {
	// Arrange & Act
	let message = EmailMessage::builder()
		.from("html@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("HTML Email")
		.body("Plain text body")
		.html("<html><body><h1>HTML Body</h1></body></html>")
		.build()
		.unwrap();

	// Assert
	assert_eq!(message.body(), "Plain text body");
	assert_eq!(
		message.html_body(),
		Some("<html><body><h1>HTML Body</h1></body></html>")
	);
}

/// Test: Builder with multiple recipients
#[rstest]
fn test_builder_multiple_recipients() {
	// Arrange & Act
	let message = EmailMessage::builder()
		.from("sender@example.com")
		.to(vec![
			"to1@example.com".to_string(),
			"to2@example.com".to_string(),
			"to3@example.com".to_string(),
		])
		.cc(vec![
			"cc1@example.com".to_string(),
			"cc2@example.com".to_string(),
		])
		.subject("Multiple Recipients")
		.body("Test")
		.build()
		.unwrap();

	// Assert
	assert_eq!(message.to().len(), 3);
	assert_eq!(message.cc().len(), 2);
	assert!(message.to().contains(&"to1@example.com".to_string()));
	assert!(message.to().contains(&"to2@example.com".to_string()));
	assert!(message.to().contains(&"to3@example.com".to_string()));
}

/// Test: Builder with custom headers
#[rstest]
fn test_builder_custom_headers() {
	// Arrange & Act
	let message = EmailMessage::builder()
		.from("headers@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("Custom Headers")
		.body("Body")
		.header("X-Custom-Header", "CustomValue")
		.header("X-Priority", "1")
		.header("X-Mailer", "Reinhardt Mail")
		.build()
		.unwrap();

	// Assert
	assert_eq!(message.headers().len(), 3);
	assert!(
		message
			.headers()
			.contains(&("X-Custom-Header".to_string(), "CustomValue".to_string()))
	);
	assert!(
		message
			.headers()
			.contains(&("X-Priority".to_string(), "1".to_string()))
	);
	assert!(
		message
			.headers()
			.contains(&("X-Mailer".to_string(), "Reinhardt Mail".to_string()))
	);
}

/// Test: Builder with single attachment
#[rstest]
fn test_builder_single_attachment() {
	// Arrange
	let attachment = Attachment::new("document.txt", b"File content".to_vec());

	// Act
	let message = EmailMessage::builder()
		.from("attach@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("Attachment Test")
		.body("Email with attachment")
		.attachment(attachment)
		.build()
		.unwrap();

	// Assert
	assert_eq!(message.attachments().len(), 1);
	assert_eq!(message.attachments()[0].filename(), "document.txt");
}

/// Test: Builder with multiple attachments
#[rstest]
fn test_builder_multiple_attachments() {
	// Arrange
	let attachment1 = Attachment::new("file1.txt", b"Content 1".to_vec());
	let attachment2 = Attachment::new("file2.pdf", b"Content 2".to_vec());
	let attachment3 = Attachment::new("image.png", b"Content 3".to_vec());

	// Act
	let message = EmailMessage::builder()
		.from("multi@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("Multiple Attachments")
		.body("Email with 3 attachments")
		.attachment(attachment1)
		.attachment(attachment2)
		.attachment(attachment3)
		.build()
		.unwrap();

	// Assert
	assert_eq!(message.attachments().len(), 3);
	assert_eq!(message.attachments()[0].filename(), "file1.txt");
	assert_eq!(message.attachments()[1].filename(), "file2.pdf");
	assert_eq!(message.attachments()[2].filename(), "image.png");
}

/// Test: Builder with alternative content
#[rstest]
fn test_builder_alternative_content() {
	// Arrange
	let alt = Alternative::new("text/plain", "Alternative plain text".as_bytes().to_vec());

	// Act
	let message = EmailMessage::builder()
		.from("alt@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("Alternative Content")
		.body("Main body")
		.alternative(alt)
		.build()
		.unwrap();

	// Assert
	assert_eq!(message.alternatives().len(), 1);
	assert_eq!(message.alternatives()[0].content_type(), "text/plain");
}

/// Test: Builder default values (empty builder builds successfully)
#[rstest]
fn test_builder_default_values() {
	// Arrange & Act
	let message = EmailMessage::builder().build().unwrap();

	// Assert
	assert_eq!(message.subject(), "");
	assert_eq!(message.body(), "");
	assert_eq!(message.from_email(), "");
	assert_eq!(message.to().len(), 0);
	assert_eq!(message.cc().len(), 0);
	assert_eq!(message.bcc().len(), 0);
	assert_eq!(message.reply_to().len(), 0);
	assert_eq!(message.html_body(), None);
	assert_eq!(message.alternatives().len(), 0);
	assert_eq!(message.attachments().len(), 0);
	assert_eq!(message.headers().len(), 0);
}

/// Test: Builder UTF-8 content
#[rstest]
fn test_builder_utf8_content() {
	// Arrange & Act
	let message = EmailMessage::builder()
		.from("utf8@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("日本語の件名")
		.body("本文に日本語が含まれています。\nこれはテストメールです。")
		.build()
		.unwrap();

	// Assert
	assert_eq!(message.subject(), "日本語の件名");
	assert!(message.body().contains("日本語"));
	assert!(message.body().contains("テストメール"));
}

/// Test: Builder from_email alias method
#[rstest]
fn test_builder_from_email_alias() {
	// Arrange & Act
	let message1 = EmailMessage::builder()
		.from("sender1@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("Test")
		.body("Body")
		.build()
		.unwrap();

	let message2 = EmailMessage::builder()
		.from_email("sender2@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("Test")
		.body("Body")
		.build()
		.unwrap();

	// Assert
	assert_eq!(message1.from_email(), "sender1@example.com");
	assert_eq!(message2.from_email(), "sender2@example.com");
}

/// Test: Builder `Into<String>` conversion
#[rstest]
fn test_builder_into_string_conversion() {
	// Arrange
	let subject_owned = String::from("Owned String");
	let body_str = "String slice";

	// Act
	let message = EmailMessage::builder()
		.from("convert@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject(subject_owned)
		.body(body_str)
		.build()
		.unwrap();

	// Assert
	assert_eq!(message.subject(), "Owned String");
	assert_eq!(message.body(), "String slice");
}

/// Test: Builder large email construction
#[rstest]
fn test_builder_large_email() {
	// Arrange
	let large_body = "Lorem ipsum dolor sit amet. ".repeat(1000);
	let large_html =
		"<html><body>".to_string() + &"<p>Paragraph</p>".repeat(1000) + "</body></html>";

	// Act
	let message = EmailMessage::builder()
		.from("large@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("Large Email")
		.body(&large_body)
		.html(&large_html)
		.build()
		.unwrap();

	// Assert
	assert!(message.body().len() > 20000);
	assert!(message.html_body().unwrap().len() > 10000);
}

/// Test: Builder with empty collections
#[rstest]
fn test_builder_empty_collections() {
	// Arrange & Act
	let message = EmailMessage::builder()
		.from("empty@example.com")
		.to(vec![])
		.cc(vec![])
		.bcc(vec![])
		.reply_to(vec![])
		.subject("Empty Collections")
		.body("Test")
		.build()
		.unwrap();

	// Assert
	assert!(message.to().is_empty());
	assert!(message.cc().is_empty());
	assert!(message.bcc().is_empty());
	assert!(message.reply_to().is_empty());
}

/// Test: Builder header ordering
#[rstest]
fn test_builder_header_ordering() {
	// Arrange & Act
	let message = EmailMessage::builder()
		.from("order@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("Header Order")
		.body("Body")
		.header("X-First", "1")
		.header("X-Second", "2")
		.header("X-Third", "3")
		.build()
		.unwrap();

	// Assert
	assert_eq!(message.headers().len(), 3);
	assert_eq!(
		message.headers()[0],
		("X-First".to_string(), "1".to_string())
	);
	assert_eq!(
		message.headers()[1],
		("X-Second".to_string(), "2".to_string())
	);
	assert_eq!(
		message.headers()[2],
		("X-Third".to_string(), "3".to_string())
	);
}

/// Test: Builder with long recipient list
#[rstest]
fn test_builder_long_recipient_list() {
	// Arrange
	let recipients: Vec<String> = (1..=100)
		.map(|i| format!("user{}@example.com", i))
		.collect();

	// Act
	let message = EmailMessage::builder()
		.from("sender@example.com")
		.to(recipients.clone())
		.subject("Long Recipient List")
		.body("Mass email")
		.build()
		.unwrap();

	// Assert
	assert_eq!(message.to().len(), 100);
	assert_eq!(message.to()[0], "user1@example.com");
	assert_eq!(message.to()[99], "user100@example.com");
}

// ===== Email Validation Tests (Issue #515) =====

/// Test: Builder rejects invalid from_email address
#[rstest]
fn test_builder_rejects_invalid_from_email() {
	// Arrange & Act
	let result = EmailMessage::builder()
		.from("invalid-email")
		.to(vec!["test@example.com".to_string()])
		.subject("Test")
		.body("Body")
		.build();

	// Assert
	assert!(result.is_err());
}

/// Test: Builder rejects invalid to address
#[rstest]
fn test_builder_rejects_invalid_to_address() {
	// Arrange & Act
	let result = EmailMessage::builder()
		.from("sender@example.com")
		.to(vec!["user@.com".to_string()])
		.subject("Test")
		.body("Body")
		.build();

	// Assert
	assert!(result.is_err());
}

/// Test: Builder rejects invalid cc address
#[rstest]
fn test_builder_rejects_invalid_cc_address() {
	// Arrange & Act
	let result = EmailMessage::builder()
		.from("sender@example.com")
		.to(vec!["valid@example.com".to_string()])
		.cc(vec!["@missing-local.com".to_string()])
		.subject("Test")
		.body("Body")
		.build();

	// Assert
	assert!(result.is_err());
}

/// Test: Builder rejects invalid bcc address
#[rstest]
fn test_builder_rejects_invalid_bcc_address() {
	// Arrange & Act
	let result = EmailMessage::builder()
		.from("sender@example.com")
		.to(vec!["valid@example.com".to_string()])
		.bcc(vec!["no-at-sign".to_string()])
		.subject("Test")
		.body("Body")
		.build();

	// Assert
	assert!(result.is_err());
}

/// Test: Builder rejects invalid reply_to address
#[rstest]
fn test_builder_rejects_invalid_reply_to_address() {
	// Arrange & Act
	let result = EmailMessage::builder()
		.from("sender@example.com")
		.to(vec!["valid@example.com".to_string()])
		.reply_to(vec!["double@@at.com".to_string()])
		.subject("Test")
		.body("Body")
		.build();

	// Assert
	assert!(result.is_err());
}

/// Test: Builder rejects email with header injection in from
#[rstest]
fn test_builder_rejects_header_injection_in_from() {
	// Arrange & Act
	let result = EmailMessage::builder()
		.from("user@example.com\nBcc: attacker@evil.com")
		.to(vec!["test@example.com".to_string()])
		.subject("Test")
		.body("Body")
		.build();

	// Assert
	assert!(result.is_err());
}

/// Test: Builder rejects email with header injection in to
#[rstest]
fn test_builder_rejects_header_injection_in_to() {
	// Arrange & Act
	let result = EmailMessage::builder()
		.from("sender@example.com")
		.to(vec!["test@example.com\rCc: attacker@evil.com".to_string()])
		.subject("Test")
		.body("Body")
		.build();

	// Assert
	assert!(result.is_err());
}

/// Test: Builder accepts valid email addresses
#[rstest]
fn test_builder_accepts_valid_emails() {
	// Arrange & Act
	let result = EmailMessage::builder()
		.from("user@example.com")
		.to(vec!["recipient@example.com".to_string()])
		.cc(vec!["cc+tag@example.com".to_string()])
		.bcc(vec!["bcc.user@example.co.uk".to_string()])
		.reply_to(vec!["reply_to@example.com".to_string()])
		.subject("Valid Emails")
		.body("Body")
		.build();

	// Assert
	assert!(result.is_ok());
}

/// Test: Builder allows empty from_email (not validated when empty)
#[rstest]
fn test_builder_allows_empty_from_email() {
	// Arrange & Act
	let result = EmailMessage::builder().subject("Test").body("Body").build();

	// Assert
	assert!(result.is_ok());
}

// ===== Field Access Protection Tests (Issue #512) =====

/// Test: EmailMessage fields are accessible only through getters
#[rstest]
fn test_email_message_getter_methods() {
	// Arrange
	let message = EmailMessage::builder()
		.from("test@example.com")
		.to(vec!["to@example.com".to_string()])
		.cc(vec!["cc@example.com".to_string()])
		.bcc(vec!["bcc@example.com".to_string()])
		.reply_to(vec!["reply@example.com".to_string()])
		.subject("Subject")
		.body("Body")
		.html("<p>HTML</p>")
		.header("X-Test", "value")
		.build()
		.unwrap();

	// Assert - all getters return expected values
	assert_eq!(message.from_email(), "test@example.com");
	assert_eq!(message.to(), &["to@example.com".to_string()]);
	assert_eq!(message.cc(), &["cc@example.com".to_string()]);
	assert_eq!(message.bcc(), &["bcc@example.com".to_string()]);
	assert_eq!(message.reply_to(), &["reply@example.com".to_string()]);
	assert_eq!(message.subject(), "Subject");
	assert_eq!(message.body(), "Body");
	assert_eq!(message.html_body(), Some("<p>HTML</p>"));
	assert_eq!(message.headers().len(), 1);
}

// ===== Header Injection Protection Tests (Issue #515) =====

/// Test: Builder rejects subject with CRLF injection
#[rstest]
fn test_builder_rejects_subject_header_injection() {
	// Arrange & Act
	let result = EmailMessage::builder()
		.from("sender@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("Normal\r\nBcc: attacker@evil.com")
		.body("Body")
		.build();

	// Assert
	assert!(result.is_err());
}

/// Test: Builder rejects subject with newline injection
#[rstest]
fn test_builder_rejects_subject_newline_injection() {
	// Arrange & Act
	let result = EmailMessage::builder()
		.from("sender@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("Subject\nX-Injected: malicious")
		.body("Body")
		.build();

	// Assert
	assert!(result.is_err());
}

/// Test: Builder rejects custom header name with CRLF
#[rstest]
fn test_builder_rejects_header_name_injection() {
	// Arrange & Act
	let result = EmailMessage::builder()
		.from("sender@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("Test")
		.body("Body")
		.header("X-Header\r\nBcc: evil@attacker.com", "value")
		.build();

	// Assert
	assert!(result.is_err());
}

/// Test: Builder rejects custom header value with CRLF
#[rstest]
fn test_builder_rejects_header_value_injection() {
	// Arrange & Act
	let result = EmailMessage::builder()
		.from("sender@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("Test")
		.body("Body")
		.header("X-Custom", "value\r\nBcc: evil@attacker.com")
		.build();

	// Assert
	assert!(result.is_err());
}

// ===== Email Sanitization Tests (Issue #517) =====

/// Test: sanitize_email preserves local part case
#[rstest]
fn test_sanitize_email_preserves_local_part_case() {
	// Arrange & Act & Assert
	// RFC 5321: local part is case-sensitive, only domain is lowercased
	assert_eq!(
		reinhardt_mail::validation::sanitize_email("John.Smith@Example.COM").unwrap(),
		"John.Smith@example.com"
	);
}

/// Test: sanitize_email lowercases only domain
#[rstest]
fn test_sanitize_email_lowercases_only_domain() {
	// Arrange & Act & Assert
	assert_eq!(
		reinhardt_mail::validation::sanitize_email("MixedCase+Tag@DOMAIN.COM").unwrap(),
		"MixedCase+Tag@domain.com"
	);
}

/// Test: sanitize_email trims whitespace without altering case
#[rstest]
fn test_sanitize_email_trims_whitespace() {
	// Arrange & Act & Assert
	assert_eq!(
		reinhardt_mail::validation::sanitize_email("  User@Example.com  ").unwrap(),
		"User@example.com"
	);
}

/// Test: sanitize_email_list preserves local part case for all entries
#[rstest]
fn test_sanitize_email_list_preserves_local_case() {
	// Arrange
	let emails = vec!["Alice@Example.COM", "BOB@Domain.ORG"];

	// Act
	let result = reinhardt_mail::validation::sanitize_email_list(&emails).unwrap();

	// Assert
	assert_eq!(result, vec!["Alice@example.com", "BOB@domain.org"]);
}
