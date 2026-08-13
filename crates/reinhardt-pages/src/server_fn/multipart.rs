use hyper::header;
use reinhardt_core::parsers::multipart::MultipartPart;
use reinhardt_core::parsers::{MediaType, MultiPartParser, UploadedFile};
use reinhardt_http::Request;
use serde::de::DeserializeOwned;

use super::ServerFnError;

const INVALID_REQUEST_MESSAGE: &str = "Invalid server function request";

/// Ordered multipart arguments decoded for a generated native server function.
#[doc(hidden)]
pub struct MultipartArguments {
	parts: Vec<MultipartPart>,
}

impl MultipartArguments {
	/// Parses one multipart request using the core ordered-part parser.
	pub async fn from_request(request: &Request) -> Result<Self, ServerFnError> {
		let content_type = request
			.headers
			.get(header::CONTENT_TYPE)
			.and_then(|value| value.to_str().ok())
			.ok_or_else(|| invalid_request("missing_content_type", None))?;
		let media_type = MediaType::parse(content_type).map_err(|error| {
			tracing::warn!(error = %error, "Failed to parse multipart content type");
			invalid_request("malformed_content_type", None)
		})?;
		if !media_type.matches("multipart/form-data") {
			return Err(invalid_request("unexpected_content_type", None));
		}
		let boundary = media_type
			.parameters
			.get("boundary")
			.ok_or_else(|| invalid_request("missing_boundary", None))?;
		let body = request.read_body().map_err(|error| {
			tracing::warn!(error = %error, "Failed to read multipart request body");
			invalid_request("unavailable_body", None)
		})?;
		let parts = MultiPartParser::new()
			.parse_parts(boundary, body)
			.await
			.map_err(|error| {
				tracing::warn!(error = %error, "Failed to parse multipart request body");
				invalid_request("malformed_multipart", None)
			})?;
		if let Some(name) = duplicate_name(&parts) {
			return Err(invalid_request("duplicate_argument", Some(name)));
		}

		Ok(Self { parts })
	}

	/// Removes and decodes one required JSON scalar part.
	pub fn take_json<T: DeserializeOwned>(
		&mut self,
		name: &'static str,
	) -> Result<T, ServerFnError> {
		match take_part(&mut self.parts, name) {
			Some(MultipartPart::Field { data, .. }) => {
				serde_json::from_slice(&data).map_err(|error| {
					tracing::warn!(argument = name, error = %error, "Failed to decode multipart JSON argument");
					invalid_request("malformed_json", Some(name))
				})
			}
			Some(part) => Err(kind_mismatch(name, "json", &part)),
			None => Err(invalid_request("missing_argument", Some(name))),
		}
	}

	/// Removes one required file part.
	pub fn take_file(&mut self, name: &'static str) -> Result<UploadedFile, ServerFnError> {
		match take_part(&mut self.parts, name) {
			Some(MultipartPart::File(file)) if !is_empty_file_input(&file) => Ok(file),
			Some(MultipartPart::File(_)) => Err(invalid_request("empty_required_file", Some(name))),
			Some(part) => Err(kind_mismatch(name, "file", &part)),
			None => Err(invalid_request("missing_argument", Some(name))),
		}
	}

	/// Removes one optional file part, treating an empty browser file input as absent.
	pub fn take_optional_file(
		&mut self,
		name: &'static str,
	) -> Result<Option<UploadedFile>, ServerFnError> {
		match take_part(&mut self.parts, name) {
			Some(MultipartPart::File(file)) if is_empty_file_input(&file) => Ok(None),
			Some(MultipartPart::File(file)) => Ok(Some(file)),
			Some(part) => Err(kind_mismatch(name, "optional_file", &part)),
			None => Ok(None),
		}
	}

	/// Rejects any unconsumed multipart parts.
	pub fn finish(self) -> Result<(), ServerFnError> {
		match self.parts.first() {
			Some(part) => Err(invalid_request(
				"unexpected_argument",
				Some(part_name(part)),
			)),
			None => Ok(()),
		}
	}
}

fn duplicate_name(parts: &[MultipartPart]) -> Option<&str> {
	parts.iter().enumerate().find_map(|(index, part)| {
		let name = part_name(part);
		parts[..index]
			.iter()
			.any(|previous| part_name(previous) == name)
			.then_some(name)
	})
}

fn take_part(parts: &mut Vec<MultipartPart>, name: &str) -> Option<MultipartPart> {
	parts
		.iter()
		.position(|part| part_name(part) == name)
		.map(|index| parts.remove(index))
}

fn part_name(part: &MultipartPart) -> &str {
	match part {
		MultipartPart::Field { name, .. } => name,
		MultipartPart::File(file) => &file.name,
	}
}

fn part_kind(part: &MultipartPart) -> &'static str {
	match part {
		MultipartPart::Field { .. } => "json",
		MultipartPart::File(_) => "file",
	}
}

fn is_empty_file_input(file: &UploadedFile) -> bool {
	file.size == 0 && file.filename.as_deref().is_none_or(str::is_empty)
}

fn kind_mismatch(
	name: &'static str,
	expected: &'static str,
	part: &MultipartPart,
) -> ServerFnError {
	tracing::warn!(
		argument = name,
		expected_kind = expected,
		actual_kind = part_kind(part),
		"Multipart server function argument kind mismatch",
	);
	invalid_request("kind_mismatch", Some(name))
}

fn invalid_request(reason: &'static str, argument: Option<&str>) -> ServerFnError {
	tracing::warn!(
		reason,
		argument = argument.unwrap_or_default(),
		"Rejected multipart server function request",
	);
	ServerFnError::server(400, INVALID_REQUEST_MESSAGE)
}
