use reinhardt_conf::EmailSettings;
use reinhardt_mail::{EmailMessage, backend_from_settings};
use rstest::rstest;

#[rstest]
#[tokio::test]
async fn backend_from_settings_selects_backends_and_rejects_bad_configuration() {
	// Arrange
	let temp_dir = tempfile::tempdir().unwrap();
	let mut file_settings = EmailSettings::default();
	file_settings.backend = "file".to_string();
	file_settings.file_path = Some(temp_dir.path().to_path_buf());
	file_settings.from_email = "sender@example.com".to_string();
	let message = EmailMessage::builder()
		.subject("File backend")
		.body("Saved body")
		.from("sender@example.com")
		.to(vec!["recipient@example.com".to_string()])
		.build()
		.unwrap();

	// Act
	let file_backend = backend_from_settings(&file_settings).unwrap();
	let sent = file_backend
		.send_messages(std::slice::from_ref(&message))
		.await
		.unwrap();
	let files: Vec<_> = std::fs::read_dir(temp_dir.path())
		.unwrap()
		.map(|entry| entry.unwrap().path())
		.collect();

	let mut missing_path = EmailSettings::default();
	missing_path.backend = "file".to_string();
	let missing = backend_from_settings(&missing_path).err().unwrap();

	let mut unknown_settings = EmailSettings::default();
	unknown_settings.backend = "unknown".to_string();
	let unknown = backend_from_settings(&unknown_settings).err().unwrap();

	let mut invalid_settings = EmailSettings::default();
	invalid_settings.from_email = "invalid".to_string();
	let invalid_from = backend_from_settings(&invalid_settings).err().unwrap();

	let mut memory_settings = EmailSettings::default();
	memory_settings.backend = "memory".to_string();
	let memory_backend = backend_from_settings(&memory_settings).unwrap();
	let memory_sent = memory_backend
		.send_messages(std::slice::from_ref(&message))
		.await
		.unwrap();

	let mut console_settings = EmailSettings::default();
	console_settings.backend = "console".to_string();
	let console_backend = backend_from_settings(&console_settings).unwrap();
	let console_empty = console_backend.send_messages(&[]).await.unwrap();

	// Assert
	assert_eq!(sent, 1);
	assert_eq!(files.len(), 1);
	let saved = std::fs::read_to_string(&files[0]).unwrap();
	assert_eq!(
		saved,
		"From: sender@example.com\nTo: recipient@example.com\nSubject: File backend\n\nSaved body"
	);
	assert_eq!(missing.to_string(), "Missing required field: file_path");
	assert_eq!(
		unknown.to_string(),
		"Backend error: Unknown email backend type: 'unknown'. Valid options: smtp, console, file, memory"
	);
	assert_eq!(
		invalid_from.to_string(),
		"Invalid email address: Email must contain exactly one @ symbol, found 0"
	);
	assert_eq!(memory_sent, 1);
	assert_eq!(console_empty, 0);
}
