//! Console and File Backend integration tests
//!
//! Tests Console and File backends for email output, covering console output,
//! file storage, directory creation, permissions, concurrent writes, filename collision,
//! and cleanup.

use reinhardt_mail::{
	Attachment, ConsoleBackend, EmailBackend, EmailMessage, FileBackend, MemoryBackend,
};
use rstest::rstest;
use std::fs;
use tempfile::TempDir;

/// Test: Console backend outputs to stdout (can't easily test, so we verify it doesn't panic)
#[tokio::test]
async fn test_console_backend_basic() {
	let backend = ConsoleBackend;

	let message = EmailMessage::builder()
		.from("sender@example.com")
		.to(vec!["console@example.com".to_string()])
		.subject("Console Test")
		.body("This should be printed to console")
		.build()
		.unwrap();

	let result = backend.send_messages(&[message]).await;
	assert!(result.is_ok(), "Console backend should send successfully");
	assert_eq!(result.unwrap(), 1, "Should send 1 email");
}

/// Test: Console backend with multiple messages
#[tokio::test]
async fn test_console_backend_multiple() {
	let backend = ConsoleBackend;

	let messages: Vec<_> = (1..=3)
		.map(|i| {
			EmailMessage::builder()
				.from("sender@example.com")
				.to(vec![format!("console{}@example.com", i)])
				.subject(format!("Console Test {}", i))
				.body(format!("Message {}", i))
				.build()
				.unwrap()
		})
		.collect();

	let sent = backend.send_messages(&messages).await.expect("Should send");
	assert_eq!(sent, 3, "Should send 3 messages");
}

/// Test: File backend basic file write
#[tokio::test]
async fn test_file_backend_basic() {
	let temp_dir = TempDir::with_prefix("mail_test_").expect("Failed to create temp dir");
	let file_path = temp_dir.path().to_path_buf();

	let backend = FileBackend::new(file_path.clone());

	let message = EmailMessage::builder()
		.from("file@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("File Test")
		.body("This should be saved to a file")
		.build()
		.unwrap();

	let sent = backend
		.send_messages(&[message])
		.await
		.expect("Should send to file");
	assert_eq!(sent, 1, "Should save 1 email");

	// Check file was created
	let files: Vec<_> = fs::read_dir(&file_path)
		.expect("Failed to read dir")
		.filter_map(|e| e.ok())
		.collect();
	assert_eq!(files.len(), 1, "Should create 1 email file");

	// Check file contains expected content
	let file_content = fs::read_to_string(files[0].path()).expect("Failed to read file");
	assert!(
		file_content.contains("File Test"),
		"File should contain subject"
	);
	assert!(
		file_content.contains("This should be saved to a file"),
		"File should contain body"
	);
}

/// Test: File backend with non-existent directory (should create it)
#[tokio::test]
async fn test_file_backend_directory_creation() {
	let temp_dir = TempDir::with_prefix("mail_test_").expect("Failed to create temp dir");
	let nested_path = temp_dir.path().join("nested/directory");

	let backend = FileBackend::new(nested_path.clone());

	let message = EmailMessage::builder()
		.from("dir@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("Directory Creation Test")
		.body("Testing directory creation")
		.build()
		.unwrap();

	let sent = backend
		.send_messages(&[message])
		.await
		.expect("Should create dir and save");
	assert_eq!(sent, 1);

	// Check directory was created
	assert!(nested_path.exists(), "Directory should be created");
	assert!(nested_path.is_dir(), "Path should be a directory");

	// Check file was created inside
	let files: Vec<_> = fs::read_dir(&nested_path)
		.expect("Failed to read dir")
		.filter_map(|e| e.ok())
		.collect();
	assert_eq!(files.len(), 1, "Should create 1 email file");
}

/// Test: File backend with multiple messages
#[tokio::test]
async fn test_file_backend_multiple_messages() {
	let temp_dir = TempDir::with_prefix("mail_test_").expect("Failed to create temp dir");
	let file_path = temp_dir.path().to_path_buf();

	let backend = FileBackend::new(file_path.clone());

	let messages: Vec<_> = (1..=5)
		.map(|i| {
			EmailMessage::builder()
				.from("multi@example.com")
				.to(vec![format!("user{}@example.com", i)])
				.subject(format!("Multi Test {}", i))
				.body(format!("Message number {}", i))
				.build()
				.unwrap()
		})
		.collect();

	let sent = backend
		.send_messages(&messages)
		.await
		.expect("Should send multiple");
	assert_eq!(sent, 5, "Should save 5 emails");

	// Check 5 files were created
	let files: Vec<_> = fs::read_dir(&file_path)
		.expect("Failed to read dir")
		.filter_map(|e| e.ok())
		.collect();
	assert_eq!(files.len(), 5, "Should create 5 email files");
}

/// Test: File backend concurrent writes
#[tokio::test]
async fn test_file_backend_concurrent_writes() {
	let temp_dir = TempDir::with_prefix("mail_test_").expect("Failed to create temp dir");
	let file_path = temp_dir.path().to_path_buf();

	let backend = std::sync::Arc::new(FileBackend::new(file_path.clone()));

	let mut tasks = vec![];

	for i in 1..=3 {
		let backend_clone = backend.clone();
		let task = tokio::spawn(async move {
			let message = EmailMessage::builder()
				.from("concurrent@example.com")
				.to(vec![format!("concurrent{}@example.com", i)])
				.subject(format!("Concurrent Test {}", i))
				.body(format!("Concurrent write {}", i))
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

	// Check all 3 files were created
	let files: Vec<_> = fs::read_dir(&file_path)
		.expect("Failed to read dir")
		.filter_map(|e| e.ok())
		.collect();
	assert_eq!(
		files.len(),
		3,
		"Should create 3 email files from concurrent writes"
	);
}

/// Test: File backend filename uniqueness (timestamp + random component)
#[tokio::test]
async fn test_file_backend_filename_uniqueness() {
	let temp_dir = TempDir::with_prefix("mail_test_").expect("Failed to create temp dir");
	let file_path = temp_dir.path().to_path_buf();

	let backend = FileBackend::new(file_path.clone());

	// Send 10 messages rapidly
	for i in 1..=10 {
		let message = EmailMessage::builder()
			.from("unique@example.com")
			.to(vec![format!("test{}@example.com", i)])
			.subject(format!("Unique Test {}", i))
			.body(format!("Testing uniqueness {}", i))
			.build()
			.unwrap();

		backend
			.send_messages(&[message])
			.await
			.expect("Should send");
	}

	// Check all 10 files have unique names
	let files: Vec<_> = fs::read_dir(&file_path)
		.expect("Failed to read dir")
		.filter_map(|e| e.ok())
		.map(|e| e.file_name())
		.collect();

	assert_eq!(files.len(), 10, "Should create 10 unique files");

	// Verify all filenames are unique (no duplicates)
	let unique_count = files.iter().collect::<std::collections::HashSet<_>>().len();
	assert_eq!(unique_count, 10, "All filenames should be unique");
}

/// Test: File backend empty message list
#[tokio::test]
async fn test_file_backend_empty_list() {
	let temp_dir = TempDir::with_prefix("mail_test_").expect("Failed to create temp dir");
	let file_path = temp_dir.path().to_path_buf();

	let backend = FileBackend::new(file_path.clone());

	let sent = backend
		.send_messages(&[])
		.await
		.expect("Should handle empty list");
	assert_eq!(sent, 0, "Should send 0 emails");

	// Check no files were created
	let files: Vec<_> = fs::read_dir(&file_path)
		.expect("Failed to read dir")
		.filter_map(|e| e.ok())
		.collect();
	assert_eq!(files.len(), 0, "Should not create any files");
}

/// Test: File backend with HTML content
#[tokio::test]
async fn test_file_backend_html_content() {
	let temp_dir = TempDir::with_prefix("mail_test_").expect("Failed to create temp dir");
	let file_path = temp_dir.path().to_path_buf();

	let backend = FileBackend::new(file_path.clone());

	let message = EmailMessage::builder()
		.from("html@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("HTML Email")
		.body("Plain text body")
		.html("<html><body><h1>HTML Content</h1></body></html>")
		.build()
		.unwrap();

	let sent = backend
		.send_messages(&[message])
		.await
		.expect("Should send HTML email");
	assert_eq!(sent, 1);

	// Check file contains HTML content
	let files: Vec<_> = fs::read_dir(&file_path)
		.expect("Failed to read dir")
		.filter_map(|e| e.ok())
		.collect();
	assert_eq!(files.len(), 1);

	let file_content = fs::read_to_string(files[0].path()).expect("Failed to read file");
	assert!(
		file_content.contains("HTML Content"),
		"File should contain HTML body"
	);
}

/// Test: File backend with UTF-8 content
#[tokio::test]
async fn test_file_backend_utf8_content() {
	let temp_dir = TempDir::with_prefix("mail_test_").expect("Failed to create temp dir");
	let file_path = temp_dir.path().to_path_buf();

	let backend = FileBackend::new(file_path.clone());

	let message = EmailMessage::builder()
		.from("utf8@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("日本語の件名")
		.body("メール本文に日本語が含まれています。")
		.build()
		.unwrap();

	let sent = backend
		.send_messages(&[message])
		.await
		.expect("Should send UTF-8 email");
	assert_eq!(sent, 1);

	// Check file contains UTF-8 content
	let files: Vec<_> = fs::read_dir(&file_path)
		.expect("Failed to read dir")
		.filter_map(|e| e.ok())
		.collect();
	assert_eq!(files.len(), 1);

	let file_content = fs::read_to_string(files[0].path()).expect("Failed to read file");
	assert!(
		file_content.contains("日本語"),
		"File should contain Japanese text"
	);
}

// ===== Attachment Content-Type Tests (Issue #530) =====

/// Test: Attachment auto-detects MIME type from filename extension
#[rstest]
fn test_attachment_mime_type_auto_detection() {
	// Arrange & Act
	let pdf = Attachment::new("report.pdf", b"pdf content".to_vec());
	let png = Attachment::new("logo.png", b"png content".to_vec());
	let txt = Attachment::new("readme.txt", b"text content".to_vec());
	let html = Attachment::new("page.html", b"<html>".to_vec());
	let unknown = Attachment::new("data.unknownext99", b"unknown".to_vec());

	// Assert
	assert_eq!(pdf.mime_type(), "application/pdf");
	assert_eq!(png.mime_type(), "image/png");
	assert_eq!(txt.mime_type(), "text/plain");
	assert_eq!(html.mime_type(), "text/html");
	assert_eq!(unknown.mime_type(), "application/octet-stream");
}

/// Test: Attachment custom MIME type override
#[rstest]
fn test_attachment_custom_mime_type() {
	// Arrange
	let mut attachment = Attachment::new("data.bin", b"binary content".to_vec());

	// Act
	attachment.with_mime_type("application/x-custom");

	// Assert
	assert_eq!(attachment.mime_type(), "application/x-custom");
}

/// Test: File backend includes attachment Content-Type in output
#[rstest]
#[tokio::test]
async fn test_file_backend_attachment_content_type() {
	// Arrange
	let temp_dir = TempDir::with_prefix("mail_test_").expect("Failed to create temp dir");
	let file_path = temp_dir.path().to_path_buf();
	let backend = FileBackend::new(file_path.clone());

	let attachment = Attachment::new("report.pdf", b"PDF content".to_vec());
	let message = EmailMessage::builder()
		.from("attach@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("Attachment Test")
		.body("Email with attachment")
		.attachment(attachment)
		.build()
		.unwrap();

	// Act
	backend.send_messages(&[message]).await.unwrap();

	// Assert
	let files: Vec<_> = fs::read_dir(&file_path)
		.unwrap()
		.filter_map(|e| e.ok())
		.collect();
	assert_eq!(files.len(), 1);

	let content = fs::read_to_string(files[0].path()).unwrap();
	assert!(
		content.contains("Content-Type: application/pdf"),
		"File output should include attachment Content-Type, got: {}",
		content
	);
	assert!(
		content.contains("report.pdf"),
		"File output should include attachment filename"
	);
}

/// Test: Memory backend preserves attachments with Content-Type
#[rstest]
#[tokio::test]
async fn test_memory_backend_preserves_attachment_content_type() {
	// Arrange
	let backend = MemoryBackend::new();
	let attachment = Attachment::new("image.jpg", b"JPEG data".to_vec());
	let message = EmailMessage::builder()
		.from("sender@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("Test")
		.body("Body")
		.attachment(attachment)
		.build()
		.unwrap();

	// Act
	backend.send_messages(&[message]).await.unwrap();

	// Assert
	let messages = backend.get_messages().await;
	assert_eq!(messages.len(), 1);
	assert_eq!(messages[0].attachments().len(), 1);
	assert_eq!(messages[0].attachments()[0].filename(), "image.jpg");
	assert_eq!(messages[0].attachments()[0].mime_type(), "image/jpeg");
}

// ===== Custom Header Propagation Tests (Issue #521) =====

/// Test: Custom headers are preserved in MemoryBackend
#[rstest]
#[tokio::test]
async fn test_memory_backend_preserves_custom_headers() {
	// Arrange
	let backend = MemoryBackend::new();
	let message = EmailMessage::builder()
		.from("sender@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("Header Test")
		.body("Body")
		.header("X-Custom-Auth", "signed-token-abc")
		.header("X-Tracking-ID", "msg-12345")
		.build()
		.unwrap();

	// Act
	backend.send_messages(&[message]).await.unwrap();

	// Assert
	let messages = backend.get_messages().await;
	assert_eq!(messages.len(), 1);
	let headers = messages[0].headers();
	assert_eq!(headers.len(), 2);
	assert!(headers.contains(&("X-Custom-Auth".to_string(), "signed-token-abc".to_string())));
	assert!(headers.contains(&("X-Tracking-ID".to_string(), "msg-12345".to_string())));
}

/// Test: Custom headers appear in ConsoleBackend output (does not panic)
#[rstest]
#[tokio::test]
async fn test_console_backend_outputs_custom_headers() {
	// Arrange
	let backend = ConsoleBackend;
	let message = EmailMessage::builder()
		.from("sender@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("Header Test")
		.body("Body")
		.header("List-Unsubscribe", "<https://example.com/unsub>")
		.header("X-Custom-Tag", "important")
		.build()
		.unwrap();

	// Act
	let result = backend.send_messages(&[message]).await;

	// Assert
	assert!(result.is_ok());
	assert_eq!(result.unwrap(), 1);
}

/// Test: Custom headers are written in FileBackend output
#[rstest]
#[tokio::test]
async fn test_file_backend_writes_custom_headers() {
	// Arrange
	let temp_dir = TempDir::with_prefix("mail_test_").expect("Failed to create temp dir");
	let file_path = temp_dir.path().to_path_buf();
	let backend = FileBackend::new(file_path.clone());

	let message = EmailMessage::builder()
		.from("sender@example.com")
		.to(vec!["test@example.com".to_string()])
		.subject("Header Test")
		.body("Body with headers")
		.header("X-Custom-Auth", "token-xyz")
		.header("X-Campaign-ID", "campaign-2024")
		.build()
		.unwrap();

	// Act
	backend.send_messages(&[message]).await.unwrap();

	// Assert
	let files: Vec<_> = fs::read_dir(&file_path)
		.unwrap()
		.filter_map(|e| e.ok())
		.collect();
	assert_eq!(files.len(), 1);

	let content = fs::read_to_string(files[0].path()).unwrap();
	assert!(
		content.contains("X-Custom-Auth: token-xyz"),
		"File should contain custom auth header, got: {}",
		content
	);
	assert!(
		content.contains("X-Campaign-ID: campaign-2024"),
		"File should contain campaign header, got: {}",
		content
	);
}
