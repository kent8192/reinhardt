use reinhardt_conf::EmailSettings;
use reinhardt_mail::{EmailMessage, backend_from_settings};
use rstest::rstest;
use std::process::Command;

fn test_message() -> EmailMessage {
	EmailMessage::builder()
		.subject("File backend")
		.body("Saved body")
		.from("sender@example.com")
		.to(vec!["recipient@example.com".to_string()])
		.build()
		.unwrap()
}

fn run_backend_probe(backend: &str) -> String {
	let output = Command::new(std::env::current_exe().unwrap())
		.args([
			"--ignored",
			"--exact",
			"backend_settings_integration::backend_selection_probe",
			"--nocapture",
		])
		.env("REINHARDT_MAIL_BACKEND_PROBE", backend)
		.output()
		.unwrap();
	assert!(
		output.status.success(),
		"backend probe failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	String::from_utf8(output.stdout).unwrap()
}

fn backend_message_block(output: &str) -> String {
	const START: &str = "========== Email 1 ==========\n";
	const END: &str = "==============================\n\n";
	let Some(start) = output.find(START) else {
		return String::new();
	};
	let remaining = &output[start..];
	let end = remaining
		.find(END)
		.expect("console backend output was not terminated");
	remaining[..end + END.len()].to_string()
}

#[rstest]
#[ignore = "invoked by the backend settings integration test"]
fn backend_selection_probe() {
	// Arrange
	let Ok(backend_name) = std::env::var("REINHARDT_MAIL_BACKEND_PROBE") else {
		return;
	};
	let mut settings = EmailSettings::default();
	settings.backend = backend_name;
	let backend = backend_from_settings(&settings).unwrap();
	let message = test_message();

	// Act
	let sent = tokio::runtime::Runtime::new()
		.unwrap()
		.block_on(async move {
			backend
				.send_messages(std::slice::from_ref(&message))
				.await
				.unwrap()
		});

	// Assert
	assert_eq!(sent, 1);
}

#[rstest]
#[tokio::test]
async fn backend_from_settings_selects_backends_and_rejects_bad_configuration() {
	// Arrange
	let temp_dir = tempfile::tempdir().unwrap();
	let mut file_settings = EmailSettings::default();
	file_settings.backend = "file".to_string();
	file_settings.file_path = Some(temp_dir.path().to_path_buf());
	file_settings.from_email = "sender@example.com".to_string();
	let message = test_message();
	let mut missing_path = EmailSettings::default();
	missing_path.backend = "file".to_string();
	let mut unknown_settings = EmailSettings::default();
	unknown_settings.backend = "unknown".to_string();
	let mut invalid_settings = EmailSettings::default();
	invalid_settings.from_email = "invalid".to_string();
	let mut memory_settings = EmailSettings::default();
	memory_settings.backend = "memory".to_string();
	let mut console_settings = EmailSettings::default();
	console_settings.backend = "console".to_string();

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
	let missing = backend_from_settings(&missing_path).err().unwrap();
	let unknown = backend_from_settings(&unknown_settings).err().unwrap();
	let invalid_from = backend_from_settings(&invalid_settings).err().unwrap();
	let memory_output = run_backend_probe(&memory_settings.backend);
	let console_output = run_backend_probe(&console_settings.backend);

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
	assert_eq!(backend_message_block(&memory_output), "");
	assert_eq!(
		backend_message_block(&console_output),
		"========== Email 1 ==========\nFrom: sender@example.com\nTo: recipient@example.com\nSubject: File backend\n\nSaved body\n==============================\n\n"
	);
}
