//! Form data extraction

use async_trait::async_trait;
use reinhardt_http::Request;
use serde::de::DeserializeOwned;
use std::fmt::{self, Debug};
use std::ops::Deref;

use super::{
	ParamContext, ParamError, ParamErrorContext, ParamResult, ParamType, extract::FromRequest,
};

#[cfg(feature = "multipart")]
use futures_util::{future::ready, stream::once};
#[cfg(feature = "multipart")]
use serde_json::Value;

/// Default maximum form body size: 2 MiB
const DEFAULT_MAX_FORM_BODY_SIZE: usize = 2 * 1024 * 1024;

/// Extract form data from request body
pub struct Form<T>(pub T);

impl<T> Form<T> {
	/// Unwrap the Form and return the inner value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::params::Form;
	/// use serde::Deserialize;
	///
	/// #[derive(Deserialize, Debug, PartialEq)]
	/// struct LoginForm {
	///     username: String,
	///     password: String,
	/// }
	///
	/// let form = Form(LoginForm {
	///     username: "alice".to_string(),
	///     password: "secret123".to_string(),
	/// });
	/// let inner = form.into_inner();
	/// assert_eq!(inner.username, "alice");
	/// assert_eq!(inner.password, "secret123");
	/// ```
	pub fn into_inner(self) -> T {
		self.0
	}

	/// Parse multipart/form-data from request
	///
	/// This method handles `multipart/form-data` content type, which is commonly
	/// used for file uploads. Only text fields are extracted; file fields are ignored.
	///
	/// Note: This is an internal method. Use `Form<T>` with `FromRequest` trait instead.
	#[cfg(feature = "multipart")]
	async fn from_multipart_internal(req: &Request) -> ParamResult<Form<T>>
	where
		T: DeserializeOwned,
	{
		// Extract boundary from Content-Type header
		let content_type = req
			.headers
			.get(http::header::CONTENT_TYPE)
			.and_then(|v| v.to_str().ok())
			.ok_or_else(|| {
				ParamError::InvalidParameter(Box::new(
					ParamErrorContext::new(ParamType::Form, "Missing Content-Type header")
						.with_field("Content-Type"),
				))
			})?;

		// Parse boundary
		let boundary = multer::parse_boundary(content_type).map_err(|e| {
			ParamError::InvalidParameter(Box::new(
				ParamErrorContext::new(ParamType::Form, format!("Failed to parse boundary: {}", e))
					.with_field("Content-Type")
					.with_raw_value(content_type),
			))
		})?;

		// Read body
		let body = req
			.read_body()
			.map_err(|e| ParamError::BodyError(format!("Failed to read body: {}", e)))?;

		// Enforce body size limit to prevent memory exhaustion
		if body.len() > DEFAULT_MAX_FORM_BODY_SIZE {
			return Err(ParamError::PayloadTooLarge(format!(
				"Multipart form body size {} bytes exceeds maximum allowed size of {} bytes",
				body.len(),
				DEFAULT_MAX_FORM_BODY_SIZE
			)));
		}

		// Convert Bytes to Stream
		let stream = once(ready(Ok::<_, std::io::Error>(body)));

		// Create multipart parser
		let mut multipart = multer::Multipart::new(stream, boundary);

		// Extract text fields into a map
		let mut fields = serde_json::Map::new();

		while let Some(field) = multipart
			.next_field()
			.await
			.map_err(|e| ParamError::BodyError(format!("Failed to read multipart field: {}", e)))?
		{
			let name = field
				.name()
				.ok_or_else(|| ParamError::BodyError("Field name missing".to_string()))?
				.to_string();

			// Only extract text fields, skip file fields
			if field.file_name().is_none() {
				let text = field.text().await.map_err(|e| {
					ParamError::BodyError(format!("Failed to read text field: {}", e))
				})?;

				fields.insert(name, Value::String(text));
			}
		}

		// Deserialize the fields map into T
		let json_str = serde_json::to_string(&Value::Object(fields.clone())).ok();
		let data: T = serde_json::from_value(Value::Object(fields)).map_err(|e| {
			let mut ctx = ParamErrorContext::new(ParamType::Form, e.to_string())
				.with_expected_type::<T>()
				.with_source(Box::new(e));
			if let Some(raw) = json_str {
				ctx = ctx.with_raw_value(raw);
			}
			ParamError::DeserializationError(Box::new(ctx))
		})?;

		Ok(Form(data))
	}
}

impl<T> Deref for Form<T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T: Debug> Debug for Form<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.0.fmt(f)
	}
}

#[async_trait]
impl<T> FromRequest for Form<T>
where
	T: DeserializeOwned + Send,
{
	async fn from_request(req: &Request, _ctx: &ParamContext) -> ParamResult<Self> {
		// Extract form data from request body
		// Form data is typically sent as application/x-www-form-urlencoded

		// Check Content-Type header
		let content_type = req
			.headers
			.get(http::header::CONTENT_TYPE)
			.and_then(|h| h.to_str().ok())
			.unwrap_or("");

		if !content_type.contains("application/x-www-form-urlencoded")
			&& !content_type.contains("multipart/form-data")
		{
			return Err(ParamError::InvalidParameter(Box::new(
				ParamErrorContext::new(
					ParamType::Form,
					format!(
						"Expected application/x-www-form-urlencoded or multipart/form-data, got {}",
						content_type
					),
				)
				.with_field("Content-Type")
				.with_expected_type::<T>(),
			)));
		}

		// Parse the body as form data
		if content_type.contains("application/x-www-form-urlencoded") {
			let body_bytes = req
				.read_body()
				.map_err(|e| ParamError::BodyError(format!("Failed to read body: {}", e)))?;

			// Enforce body size limit to prevent memory exhaustion
			if body_bytes.len() > DEFAULT_MAX_FORM_BODY_SIZE {
				return Err(ParamError::PayloadTooLarge(format!(
					"Form body size {} bytes exceeds maximum allowed size of {} bytes",
					body_bytes.len(),
					DEFAULT_MAX_FORM_BODY_SIZE
				)));
			}

			let body_str = std::str::from_utf8(&body_bytes)
				.map_err(|e| ParamError::BodyError(format!("Invalid UTF-8 in body: {}", e)))?;

			let raw_value = if body_str.is_empty() {
				None
			} else {
				Some(body_str.to_string())
			};
			serde_urlencoded::from_str(body_str)
				.map(Form)
				.map_err(|e| ParamError::url_encoding::<T>(ParamType::Form, e, raw_value))
		} else if content_type.contains("multipart/form-data") {
			#[cfg(feature = "multipart")]
			{
				Self::from_multipart_internal(req).await
			}
			#[cfg(not(feature = "multipart"))]
			{
				Err(ParamError::BodyError(
					"multipart/form-data parsing requires 'multipart' feature".to_string(),
				))
			}
		} else {
			Err(ParamError::InvalidParameter(Box::new(
				ParamErrorContext::new(
					ParamType::Form,
					format!("Unsupported content type: {}", content_type),
				)
				.with_field("Content-Type")
				.with_expected_type::<T>(),
			)))
		}
	}
}
