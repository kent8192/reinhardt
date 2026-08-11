use async_trait::async_trait;
use reinhardt_mail::{
	Alternative, Attachment, EmailBackend, EmailError, EmailMessage, EmailResult, MemoryBackend,
};
use rstest::rstest;

struct FailingBackend;

#[async_trait]
impl EmailBackend for FailingBackend {
	async fn send_messages(&self, _messages: &[EmailMessage]) -> EmailResult<usize> {
		Err(EmailError::BackendError(
			"deterministic send failure".to_string(),
		))
	}
}

fn assert_message_fields(expected: &EmailMessage, actual: &EmailMessage) {
	assert_eq!(actual.subject(), expected.subject());
	assert_eq!(actual.body(), expected.body());
	assert_eq!(actual.from_email(), expected.from_email());
	assert_eq!(actual.to(), expected.to());
	assert_eq!(actual.cc(), expected.cc());
	assert_eq!(actual.bcc(), expected.bcc());
	assert_eq!(actual.reply_to(), expected.reply_to());
	assert_eq!(actual.html_body(), expected.html_body());
	assert_eq!(actual.headers(), expected.headers());
	assert_eq!(actual.alternatives().len(), expected.alternatives().len());
	for (actual_alternative, expected_alternative) in
		actual.alternatives().iter().zip(expected.alternatives())
	{
		assert_eq!(
			actual_alternative.content_type(),
			expected_alternative.content_type()
		);
		assert_eq!(actual_alternative.content(), expected_alternative.content());
	}
	assert_eq!(actual.attachments().len(), expected.attachments().len());
	for (actual_attachment, expected_attachment) in
		actual.attachments().iter().zip(expected.attachments())
	{
		assert_eq!(actual_attachment.filename(), expected_attachment.filename());
		assert_eq!(actual_attachment.content(), expected_attachment.content());
		assert_eq!(
			actual_attachment.mime_type(),
			expected_attachment.mime_type()
		);
		assert_eq!(
			actual_attachment.content_id(),
			expected_attachment.content_id()
		);
		assert_eq!(
			actual_attachment.is_inline(),
			expected_attachment.is_inline()
		);
	}
}

#[rstest]
#[tokio::test]
async fn email_message_send_delegates_and_propagates_failure() {
	// Arrange
	let message = EmailMessage::builder()
		.subject("Invoice")
		.body("Attached")
		.from("billing@example.com")
		.to(vec![
			"customer@example.com".to_string(),
			"accounts@example.com".to_string(),
		])
		.cc(vec!["manager@example.com".to_string()])
		.bcc(vec!["audit@example.com".to_string()])
		.reply_to(vec!["support@example.com".to_string()])
		.html("<p>Attached</p>")
		.alternative(Alternative::plain("Attached"))
		.alternative(Alternative::html("<p>Attached</p>"))
		.attachment(Attachment::new("invoice.txt", b"invoice".to_vec()))
		.attachment(Attachment::inline("logo.png", vec![1, 2, 3], "logo-cid"))
		.header("X-Invoice-Id", "invoice-123")
		.build()
		.unwrap();
	let memory = MemoryBackend::new();

	// Act
	let send_result = message.send(&memory).await;
	let alias_result = message.send_with_backend(&memory).await;
	let failure = message.send(&FailingBackend).await;
	let alias_failure = message.send_with_backend(&FailingBackend).await;

	// Assert
	send_result.unwrap();
	alias_result.unwrap();
	let stored = memory.get_messages().await;
	assert_eq!(stored.len(), 2);
	assert_message_fields(&message, &stored[0]);
	assert_message_fields(&message, &stored[1]);
	assert_eq!(
		failure.unwrap_err().to_string(),
		"Backend error: deterministic send failure"
	);
	assert_eq!(
		alias_failure.unwrap_err().to_string(),
		"Backend error: deterministic send failure"
	);
}
