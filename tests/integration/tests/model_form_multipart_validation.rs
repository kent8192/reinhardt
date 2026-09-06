//! Direct multipart requests must cross the generated model validation boundary.

use bytes::Bytes;
use reinhardt_core::macros::model;
use reinhardt_core::model_form::{
	ModelFormFileValue, ModelFormPolicy, ModelFormUpload, ModelFormValidatingPayload,
};
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
	if payload.avatar().is_some() && payload.note().and_then(Option::as_deref).is_none() {
		let mut errors = ValidationErrors::new();
		errors.add(
			"note",
			ValidationError::Custom("An avatar requires a caption".to_owned()),
		);
		return Err(errors);
	}
	if payload.title().map(String::as_str) == Some("require-file")
		&& !matches!(payload.document(), Some(ModelFormFileValue::Uploaded(upload))
			if upload.name == "document"
				&& upload.filename.as_deref() == Some("note.txt")
				&& upload.content_type.as_deref() == Some("text/plain")
				&& upload.size == 4)
	{
		let mut errors = ValidationErrors::new();
		errors.add(
			"document",
			ValidationError::Custom("The uploaded document must be present".to_owned()),
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
	multipart_request_with_avatar(fields, document, None)
}

fn multipart_request_with_avatar(
	fields: &[(&str, serde_json::Value)],
	document: bool,
	avatar: Option<(&str, &[u8])>,
) -> Request {
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
	let mut body = body.into_bytes();
	if let Some((filename, data)) = avatar {
		body.extend_from_slice(format!("--upload\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"{filename}\"\r\nContent-Type: image/png\r\n\r\n").as_bytes());
		body.extend_from_slice(data);
		body.extend_from_slice(b"\r\n");
	}
	body.extend_from_slice(b"--upload--\r\n");
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

#[rstest]
#[case::optional_image_requires_caption(
	"report", None, Some("avatar.png"), Some(("note", "An avatar requires a caption"))
)]
#[case::optional_image_accepts_caption("report", Some("  caption  "), Some("avatar.png"), None)]
#[case::named_empty_image_requires_caption(
	"report", None, Some("empty.png"), Some(("note", "An avatar requires a caption"))
)]
#[case::named_empty_image_accepts_caption("report", Some("caption"), Some("empty.png"), None)]
#[case::empty_browser_image_is_absent("report", None, Some(""), None)]
#[case::required_file_is_visible("require-file", None, None, None)]
#[tokio::test]
#[serial(model_form_multipart_validation)]
async fn multipart_cross_field_validation_observes_uploaded_files_before_handler(
	#[case] title: &str,
	#[case] caption: Option<&str>,
	#[case] avatar_filename: Option<&str>,
	#[case] expected_error: Option<(&str, &str)>,
) {
	// Arrange
	UPLOAD_CALLS.store(0, Ordering::SeqCst);
	let mut image = std::io::Cursor::new(Vec::new());
	if avatar_filename == Some("avatar.png") {
		image::DynamicImage::new_rgba8(1, 1)
			.write_to(&mut image, image::ImageFormat::Png)
			.expect("test image should encode");
	}
	let image = image.into_inner();
	let request = multipart_request_with_avatar(
		&[
			("title", serde_json::json!(title)),
			("category", serde_json::json!("documents")),
			("note", serde_json::json!(caption)),
		],
		true,
		avatar_filename.map(|filename| (filename, image.as_slice())),
	);

	// Act
	let result = upload::marker::handle(request).await;

	// Assert
	if let Some((field, message)) = expected_error {
		let body = result.expect_err("file-dependent validation must reject before dispatch");
		let error: ServerFnError = serde_json::from_slice(&body).unwrap();
		assert_eq!(error, ServerFnError::validation([(field, message)]));
		assert_eq!(UPLOAD_CALLS.load(Ordering::SeqCst), 0);
	} else {
		let body = result.expect("valid file-dependent input should reach the handler");
		assert_eq!(
			serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
			serde_json::json!([
				title,
				"documents",
				"draft",
				caption.map(str::trim),
				4,
				avatar_filename
					.filter(|filename| !filename.is_empty())
					.map(|_| image.len()),
			]),
		);
		assert_eq!(UPLOAD_CALLS.load(Ordering::SeqCst), 1);
	}
}

#[rstest]
fn uploaded_candidate_does_not_create_a_stored_file_reference() {
	// Arrange
	let mut payload = UploadModelFormData::<UploadPolicy>::empty();
	payload.set_title("report".to_owned()).unwrap();
	payload.set_category("documents".to_owned()).unwrap();
	let upload = ModelFormUpload {
		name: "document",
		filename: Some("note.txt".to_owned()),
		content_type: Some("text/plain".to_owned()),
		size: 4,
	};

	// Act
	let cleaned = payload
		.clean_and_validate_with_uploads(&["document"], std::slice::from_ref(&upload))
		.expect("pending upload should satisfy submission validation");
	let raw = cleaned.clone().into_raw();
	let model = cleaned
		.clone()
		.into_model(UploadModelFormServerContext::new().organization_id(42));
	let mut form =
		reinhardt_forms::model_form::ModelForm::<Upload, UploadPolicy>::from_payload(raw.clone());
	let form_model = form.build_instance();

	// Assert
	assert_eq!(
		cleaned.document(),
		Some(ModelFormFileValue::Uploaded(&upload))
	);
	assert_eq!(raw.document(), None);
	assert_eq!(raw.avatar(), None);
	assert_eq!(
		model.unwrap_err(),
		reinhardt_forms::model_form::ModelFormError::MissingModelField { field: "document" },
	);
	assert_eq!(
		form_model.unwrap_err(),
		reinhardt_forms::model_form::ModelFormError::FieldValidation {
			errors: std::collections::HashMap::from([(
				"document".to_owned(),
				vec!["This field is required.".to_owned()],
			)]),
		},
	);
}

#[rstest]
#[tokio::test]
#[serial(model_form_multipart_validation)]
async fn multipart_json_cannot_claim_uploaded_file_presence() {
	// Arrange
	UPLOAD_CALLS.store(0, Ordering::SeqCst);
	let request = multipart_request(
		&[
			("title", serde_json::json!("report")),
			("category", serde_json::json!("documents")),
			(
				"avatar",
				serde_json::json!({"path": "avatar.png", "storage": "default"}),
			),
		],
		true,
	);

	// Act
	let body = upload::marker::handle(request)
		.await
		.expect_err("JSON cannot represent a new upload");
	let error: ServerFnError = serde_json::from_slice(&body).unwrap();

	// Assert
	assert_eq!(error.status(), Some(400));
	assert_eq!(UPLOAD_CALLS.load(Ordering::SeqCst), 0);
}
