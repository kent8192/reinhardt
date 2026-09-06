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
	if P::allows("category") && payload.category().is_none() {
		let mut errors = ValidationErrors::new();
		errors.add(
			"_all",
			ValidationError::Custom("Selected category must be present".to_owned()),
		);
		return Err(errors);
	}
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

#[server_fn(model_form_payload = "UploadModelFormData<UploadPolicy>")]
async fn upload_selected(
	title: String,
	note: Option<String>,
	document: UploadedFile,
) -> Result<(String, Option<String>, usize), ServerFnError> {
	UPLOAD_CALLS.fetch_add(1, Ordering::SeqCst);
	Ok((title, note, document.size))
}

fn multipart_request(fields: &[(&str, serde_json::Value)], document: bool) -> Request {
	let mut body = String::new();
	for (name, value) in fields {
		body.push_str(&format!(
			"--upload\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{}\r\n",
			serde_json::to_string(value).expect("text scalar encodes as JSON"),
		));
	}
	if document {
		body.push_str("--upload\r\nContent-Disposition: form-data; name=\"document\"; filename=\"note.txt\"\r\nContent-Type: text/plain\r\n\r\nfile\r\n");
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

fn request(title: &str, category: &str, document: bool, forbidden: bool) -> Request {
	let mut fields = vec![
		("title", serde_json::json!(title)),
		("category", serde_json::json!(category)),
		("note", serde_json::json!("   ")),
	];
	if forbidden {
		fields.push(("organization_id", serde_json::json!(42)));
	}
	multipart_request(&fields, document)
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

#[rstest]
#[tokio::test]
#[serial(model_form_multipart_validation)]
async fn selected_multipart_ignores_unselected_required_fields_and_narrows_validator_policy() {
	// Arrange
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let form = reinhardt_pages::form! {
			name: SelectedUploadForm,
			model: Upload,
			policy: UploadPolicy,
			fields: [title, note, document],
			server_fn: upload_selected,
		};
		assert_eq!(form.loading().get(), false);
	});
	UPLOAD_CALLS.store(0, Ordering::SeqCst);
	let request = multipart_request(
		&[
			("title", serde_json::json!("  report  ")),
			("note", serde_json::json!("   ")),
		],
		true,
	);

	// Act
	let body = upload_selected::marker::handle(request)
		.await
		.expect("unselected category must not block field or application validation");

	// Assert
	assert_eq!(
		serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
		serde_json::json!(["report", null, 4]),
	);
	assert_eq!(UPLOAD_CALLS.load(Ordering::SeqCst), 1);
}

#[rstest]
#[case::missing_scalar(false, true, false, 422)]
#[case::missing_file(true, false, false, 400)]
#[case::unselected_scalar(true, true, true, 400)]
#[tokio::test]
#[serial(model_form_multipart_validation)]
async fn selected_multipart_enforces_selected_required_fields_and_argument_allowlist(
	#[case] title: bool,
	#[case] document: bool,
	#[case] category: bool,
	#[case] status: u16,
) {
	// Arrange
	UPLOAD_CALLS.store(0, Ordering::SeqCst);
	let mut fields = Vec::new();
	if title {
		fields.push(("title", serde_json::json!("report")));
	}
	if category {
		fields.push(("category", serde_json::json!("documents")));
	}
	let request = multipart_request(&fields, document);

	// Act
	let body = upload_selected::marker::handle(request)
		.await
		.expect_err("invalid multipart input must not reach the handler");
	let error: ServerFnError = serde_json::from_slice(&body).unwrap();

	// Assert
	assert_eq!(error.status(), Some(status));
	assert_eq!(UPLOAD_CALLS.load(Ordering::SeqCst), 0);
}
