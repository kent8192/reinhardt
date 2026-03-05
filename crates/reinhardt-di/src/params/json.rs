//! JSON body extraction

use async_trait::async_trait;
use reinhardt_http::Request;
use serde::de::DeserializeOwned;
use std::fmt::{self, Debug};
use std::ops::Deref;

use super::{ParamContext, ParamError, ParamResult, extract::FromRequest};

/// Default maximum JSON body size: 2 MiB
const DEFAULT_MAX_JSON_BODY_SIZE: usize = 2 * 1024 * 1024;

/// Extract and deserialize JSON from request body
///
/// # Example
///
/// ```rust
/// use reinhardt_di::params::Json;
/// # use serde::Deserialize;
/// #[derive(Deserialize)]
/// struct CreateUser {
///     username: String,
///     email: String,
/// }
///
/// let user_data = CreateUser {
///     username: "alice".to_string(),
///     email: "alice@example.com".to_string(),
/// };
/// let user = Json(user_data);
/// let username = &user.username;
/// let email = &user.email;
/// ```
pub struct Json<T>(pub T);

impl<T> Json<T> {
	/// Unwrap the Json and return the inner value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::params::Json;
	/// use serde::Deserialize;
	///
	/// #[derive(Deserialize, Debug, PartialEq)]
	/// struct User {
	///     username: String,
	///     age: u32,
	/// }
	///
	/// let json = Json(User {
	///     username: "alice".to_string(),
	///     age: 30,
	/// });
	/// let inner = json.into_inner();
	/// assert_eq!(inner.username, "alice");
	/// assert_eq!(inner.age, 30);
	/// ```
	pub fn into_inner(self) -> T {
		self.0
	}
}

impl<T> Deref for Json<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T: Debug> Debug for Json<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.0.fmt(f)
	}
}

impl<T: Clone> Clone for Json<T> {
	fn clone(&self) -> Self {
		Json(self.0.clone())
	}
}

#[async_trait]
impl<T> FromRequest for Json<T>
where
	T: DeserializeOwned + Send,
{
	async fn from_request(req: &Request, _ctx: &ParamContext) -> ParamResult<Self> {
		// Read body bytes from request
		let body_bytes = req
			.read_body()
			.map_err(|e| ParamError::BodyError(format!("Failed to read body: {}", e)))?;

		// Enforce body size limit to prevent memory exhaustion
		if body_bytes.len() > DEFAULT_MAX_JSON_BODY_SIZE {
			return Err(ParamError::PayloadTooLarge(format!(
				"JSON body size {} bytes exceeds maximum allowed size of {} bytes",
				body_bytes.len(),
				DEFAULT_MAX_JSON_BODY_SIZE
			)));
		}

		// Deserialize JSON from body bytes with detailed error context
		serde_json::from_slice(&body_bytes).map(Json).map_err(|e| {
			let raw_value = String::from_utf8_lossy(&body_bytes).into_owned();
			ParamError::json_deserialization::<T>(e, Some(raw_value))
		})
	}
}
