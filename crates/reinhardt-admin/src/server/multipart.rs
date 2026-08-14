//! Dynamic multipart payloads used by admin model forms.

use std::collections::{HashMap, HashSet};

use reinhardt_core::parsers::UploadedFile;
#[cfg(server)]
use reinhardt_core::parsers::multipart::MultipartPart;
#[cfg(server)]
use reinhardt_pages::server_fn::{MultipartArguments, ServerFnError};

/// Reserved multipart part containing the registered model name.
pub(crate) const MODEL_PART: &str = "__reinhardt_model";
/// Reserved multipart part containing the edited record ID.
pub(crate) const ID_PART: &str = "__reinhardt_id";
/// Prefix for optional file-clear controls.
pub(crate) const CLEAR_PREFIX: &str = "__reinhardt_clear.";

/// Parsed dynamic admin form data.
#[derive(Debug)]
pub(crate) struct AdminMultipartPayload {
	/// Registered model name supplied by the form client.
	pub model_name: String,
	/// Edited record ID, or `None` for create requests.
	pub id: Option<String>,
	/// JSON scalar values keyed by logical field name.
	pub data: HashMap<String, serde_json::Value>,
	/// Uploaded files keyed by logical field name.
	pub uploads: HashMap<String, UploadedFile>,
	/// Empty browser file inputs keyed by logical field name.
	pub empty_uploads: HashSet<String>,
	/// Nullable file fields explicitly marked for clearing.
	pub clears: HashSet<String>,
}

/// Parse a dynamic admin multipart request while rejecting unconsumed input.
#[cfg(server)]
pub(crate) async fn parse_admin_multipart(
	request: &reinhardt_http::Request,
	update: bool,
) -> Result<AdminMultipartPayload, ServerFnError> {
	let mut arguments = MultipartArguments::from_request(request).await?;
	let mut model_name = None;
	let mut id = None;
	let mut data = HashMap::new();
	let mut uploads = HashMap::new();
	let mut empty_uploads = HashSet::new();
	let mut clears = HashSet::new();

	for part in arguments.take_parts() {
		match part {
			MultipartPart::Field { name, data: bytes } => {
				if bytes.is_empty() {
					return Err(invalid_request("empty JSON field"));
				}
				let value: serde_json::Value = serde_json::from_slice(&bytes)
					.map_err(|_| invalid_request("malformed JSON field"))?;
				if name == MODEL_PART {
					if model_name.is_some() {
						return Err(invalid_request("duplicate model part"));
					}
					model_name = Some(required_string(value, MODEL_PART)?);
				} else if name == ID_PART {
					if !update || id.is_some() {
						return Err(invalid_request("unexpected record ID part"));
					}
					id = Some(required_string(value, ID_PART)?);
				} else if let Some(field_name) = name.strip_prefix(CLEAR_PREFIX) {
					if field_name.is_empty() {
						return Err(invalid_request("empty clear field name"));
					}
					match value {
						serde_json::Value::Bool(true) => {
							clears.insert(field_name.to_owned());
						}
						serde_json::Value::Bool(false) => {}
						_ => return Err(invalid_request("clear marker must be boolean")),
					}
				} else if name.starts_with("__reinhardt_") {
					return Err(invalid_request("reserved multipart field name"));
				} else if data.insert(name, value).is_some() {
					return Err(invalid_request("duplicate form field"));
				}
			}
			MultipartPart::File(file) => {
				if file.name.is_empty() || file.name.starts_with("__reinhardt_") {
					return Err(invalid_request("invalid uploaded file field name"));
				}
				if is_empty_file_input(&file) {
					empty_uploads.insert(file.name.clone());
					continue;
				}
				if uploads.insert(file.name.clone(), file).is_some() {
					return Err(invalid_request("duplicate uploaded file"));
				}
			}
		}
	}
	arguments.finish()?;

	let model_name = model_name.ok_or_else(|| invalid_request("missing model part"))?;
	if update && id.is_none() {
		return Err(invalid_request("missing record ID part"));
	}

	Ok(AdminMultipartPayload {
		model_name,
		id,
		data,
		uploads,
		empty_uploads,
		clears,
	})
}

#[cfg(server)]
fn required_string(value: serde_json::Value, field: &str) -> Result<String, ServerFnError> {
	match value {
		serde_json::Value::String(value) if !value.trim().is_empty() => Ok(value),
		_ => Err(invalid_request(match field {
			MODEL_PART => "model part must be a non-empty string",
			ID_PART => "record ID part must be a non-empty string",
			_ => "reserved part must be a non-empty string",
		})),
	}
}

#[cfg(server)]
fn invalid_request(message: &'static str) -> ServerFnError {
	tracing::warn!(message, "Rejected dynamic admin multipart request");
	ServerFnError::server(400, "Invalid admin multipart request")
}

fn is_empty_file_input(file: &UploadedFile) -> bool {
	file.size == 0 && file.filename.as_deref().is_none_or(str::is_empty)
}

#[cfg(all(test, server))]
mod tests {
	use super::*;

	fn multipart_request(parts: &str) -> reinhardt_http::Request {
		reinhardt_http::Request::builder()
			.uri("/api/server_fn/create_record_multipart")
			.header(
				hyper::header::CONTENT_TYPE,
				"multipart/form-data; boundary=boundary",
			)
			.body(parts.as_bytes().to_vec().into())
			.build()
			.expect("multipart request should build")
	}

	#[tokio::test]
	async fn parse_admin_multipart_extracts_fields_uploads_and_clears() {
		let request = multipart_request(
			"--boundary\r\nContent-Disposition: form-data; name=\"__reinhardt_model\"\r\n\r\n\"Article\"\r\n--boundary\r\nContent-Disposition: form-data; name=\"__reinhardt_id\"\r\n\r\n\"42\"\r\n--boundary\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\n\"Hello\"\r\n--boundary\r\nContent-Disposition: form-data; name=\"__reinhardt_clear.thumbnail\"\r\n\r\ntrue\r\n--boundary\r\nContent-Disposition: form-data; name=\"image\"; filename=\"cover.png\"\r\nContent-Type: image/png\r\n\r\npng\r\n--boundary\r\nContent-Disposition: form-data; name=\"attachment\"; filename=\"\"\r\nContent-Type: application/octet-stream\r\n\r\n\r\n--boundary--\r\n",
		);

		let payload = parse_admin_multipart(&request, true)
			.await
			.expect("multipart payload should parse");

		assert_eq!(payload.model_name, "Article");
		assert_eq!(payload.id.as_deref(), Some("42"));
		assert_eq!(payload.data.get("title"), Some(&serde_json::json!("Hello")));
		assert_eq!(
			payload
				.uploads
				.get("image")
				.and_then(|file| file.filename.as_deref()),
			Some("cover.png")
		);
		assert!(payload.empty_uploads.contains("attachment"));
		assert!(payload.clears.contains("thumbnail"));
	}

	#[tokio::test]
	async fn parse_admin_multipart_rejects_duplicate_names() {
		let request = multipart_request(
			"--boundary\r\nContent-Disposition: form-data; name=\"__reinhardt_model\"\r\n\r\n\"Article\"\r\n--boundary\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\n\"one\"\r\n--boundary\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\n\"two\"\r\n--boundary--\r\n",
		);

		let error = parse_admin_multipart(&request, false)
			.await
			.expect_err("duplicate multipart names must fail");

		assert_eq!(error.status(), Some(400));
	}

	#[tokio::test]
	async fn parse_admin_multipart_rejects_malformed_json() {
		let request = multipart_request(
			"--boundary\r\nContent-Disposition: form-data; name=\"__reinhardt_model\"\r\n\r\n\"Article\"\r\n--boundary\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\nnot-json\r\n--boundary--\r\n",
		);

		let error = parse_admin_multipart(&request, false)
			.await
			.expect_err("malformed JSON must fail");

		assert_eq!(error.status(), Some(400));
	}

	#[tokio::test]
	async fn parse_admin_multipart_requires_an_id_for_updates() {
		let request = multipart_request(
			"--boundary\r\nContent-Disposition: form-data; name=\"__reinhardt_model\"\r\n\r\n\"Article\"\r\n--boundary--\r\n",
		);

		let error = parse_admin_multipart(&request, true)
			.await
			.expect_err("update multipart payloads require an ID");

		assert_eq!(error.status(), Some(400));
	}

	#[tokio::test]
	async fn parse_admin_multipart_rejects_update_parts_on_creates() {
		let request = multipart_request(
			"--boundary\r\nContent-Disposition: form-data; name=\"__reinhardt_model\"\r\n\r\n\"Article\"\r\n--boundary\r\nContent-Disposition: form-data; name=\"__reinhardt_id\"\r\n\r\n\"42\"\r\n--boundary--\r\n",
		);

		let error = parse_admin_multipart(&request, false)
			.await
			.expect_err("create multipart payloads must reject update-only parts");

		assert_eq!(error.status(), Some(400));
	}
}
