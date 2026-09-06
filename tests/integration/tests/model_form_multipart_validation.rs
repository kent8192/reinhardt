//! Direct multipart requests must cross the generated model validation boundary.

use bytes::Bytes;
use reinhardt_core::macros::model;
use reinhardt_core::model_form::ModelFormPolicy;
use reinhardt_core::parsers::UploadedFile;
use reinhardt_core::validators::{ValidationError, ValidationErrors};
use reinhardt_db::orm::{FileField, ImageField};
use reinhardt_http::Request;
use reinhardt_pages::server_fn::{
	ServerFnError, ServerFnErrorKind, ServerFnRegistration, server_fn,
};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use serial_test::serial;
use std::sync::atomic::{AtomicUsize, Ordering};

static UPLOAD_CALLS: AtomicUsize = AtomicUsize::new(0);

#[model(app_label = "multipart_validation", form = true, info = false)]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[form(validate = validate_upload)]
struct Upload {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(min_length = 3, max_length = 12)]
	#[form(trim)]
	title: String,
	#[field(max_length = 32)]
	#[form(trim)]
	category: String,
	#[field(max_length = 12, default = "draft")]
	status: String,
	#[field(max_length = 12, blank = true)]
	#[form(trim)]
	note: Option<String>,
	#[field(upload_to = "documents")]
	document: FileField,
	#[field(upload_to = "avatars")]
	avatar: Option<ImageField>,
	#[field(editable = false)]
	organization_id: i64,
}

struct UploadPolicy;

impl ModelFormPolicy for UploadPolicy {
	fn allows(field: &str) -> bool {
		matches!(
			field,
			"title" | "category" | "status" | "note" | "document" | "avatar"
		)
	}
}

fn validate_upload<P: ModelFormPolicy>(
	payload: &CleanedUploadModelFormData<P>,
) -> Result<(), ValidationErrors> {
	if payload.title() == payload.category() {
		let mut errors = ValidationErrors::new();
		errors.add(
			"_all",
			ValidationError::Custom("Title and category must differ".to_owned()),
		);
		return Err(errors);
	}
	Ok(())
}

#[server_fn(model_form_payload = "UploadModelFormData<UploadPolicy>")]
async fn upload(
	title: String,
	category: String,
	status: Option<String>,
	note: Option<String>,
	document: UploadedFile,
	avatar: Option<UploadedFile>,
) -> Result<
	(
		String,
		String,
		Option<String>,
		Option<String>,
		usize,
		Option<usize>,
	),
	ServerFnError,
> {
	UPLOAD_CALLS.fetch_add(1, Ordering::SeqCst);
	Ok((
		title,
		category,
		status,
		note,
		document.size,
		avatar.map(|file| file.size),
	))
}

fn request(title: &str, category: &str, document: bool, forbidden: bool) -> Request {
	let mut body = String::new();
	for (name, value) in [("title", title), ("category", category), ("note", "   ")] {
		body.push_str(&format!(
			"--upload\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{}\r\n",
			serde_json::to_string(value).expect("text scalar encodes as JSON"),
		));
	}
	if document {
		body.push_str("--upload\r\nContent-Disposition: form-data; name=\"document\"; filename=\"note.txt\"\r\nContent-Type: text/plain\r\n\r\nfile\r\n");
	}
	if forbidden {
		body.push_str(
			"--upload\r\nContent-Disposition: form-data; name=\"organization_id\"\r\n\r\n42\r\n",
		);
	}
	body.push_str("--upload--\r\n");
	Request::builder()
		.method(hyper::Method::POST)
		.uri("/api/server_fn/upload")
		.header(
			hyper::header::CONTENT_TYPE,
			"multipart/form-data; boundary=upload",
		)
		.body(Bytes::from(body))
		.build()
		.expect("direct multipart request should build")
}

#[rstest]
#[case::too_short(" ab ", "documents", false, "title")]
#[case::too_long(" thirteenchars ", "documents", false, "title")]
#[case::cross_field(" same ", "same", false, "_all")]
#[case::server_owned(" valid ", "documents", true, "organization_id")]
#[tokio::test]
#[serial(model_form_multipart_validation)]
async fn direct_multipart_rejects_invalid_scalars_before_user_handler(
	#[case] title: &str,
	#[case] category: &str,
	#[case] forbidden: bool,
	#[case] field: &str,
) {
	// Arrange
	UPLOAD_CALLS.store(0, Ordering::SeqCst);
	let request = request(title, category, true, forbidden);

	// Act
	let error_body = upload::marker::handle(request)
		.await
		.expect_err("direct requests must run generated scalar validation");
	let error: ServerFnError =
		serde_json::from_slice(&error_body).expect("validation uses the structured error envelope");

	// Assert
	assert_eq!(error.kind(), ServerFnErrorKind::Validation);
	assert_eq!(
		error
			.field_errors()
			.iter()
			.map(|error| error.field())
			.collect::<Vec<_>>(),
		[field],
	);
	assert_eq!(UPLOAD_CALLS.load(Ordering::SeqCst), 0);
}

#[rstest]
#[tokio::test]
#[serial(model_form_multipart_validation)]
async fn direct_multipart_passes_normalized_scalars_and_uploaded_file() {
	// Arrange
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let form = reinhardt_pages::form! {
			name: UploadForm,
			model: Upload,
			policy: UploadPolicy,
			fields: [title, category, status, note, document, avatar],
			server_fn: upload,
		};
		assert_eq!(form.loading().get(), false);
	});
	UPLOAD_CALLS.store(0, Ordering::SeqCst);
	let request = request("  report  ", " documents ", true, false);

	// Act
	let body = upload::marker::handle(request)
		.await
		.expect("normalized scalars and a required upload should reach the handler");

	// Assert
	assert_eq!(
		serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
		serde_json::json!(["report", "documents", "draft", null, 4, null]),
	);
	assert_eq!(UPLOAD_CALLS.load(Ordering::SeqCst), 1);
}

#[rstest]
#[tokio::test]
#[serial(model_form_multipart_validation)]
async fn direct_multipart_still_requires_the_uploaded_file() {
	// Arrange
	UPLOAD_CALLS.store(0, Ordering::SeqCst);
	let request = request("report", "documents", false, false);

	// Act
	let error_body = upload::marker::handle(request)
		.await
		.expect_err("scalar deferral must not allow missing required uploads");
	let error: ServerFnError = serde_json::from_slice(&error_body).unwrap();

	// Assert
	assert_eq!(error.status(), Some(400));
	assert_eq!(UPLOAD_CALLS.load(Ordering::SeqCst), 0);
}
