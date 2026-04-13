//! Request factory for creating test requests
//!
//! Similar to DRF's APIRequestFactory

use bytes::Bytes;
use http::{HeaderMap, HeaderValue, Method, Request};
use http_body_util::Full;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

use crate::client::ClientError;

/// Factory for creating test requests
pub struct APIRequestFactory {
	default_format: String,
	default_headers: HeaderMap,
}

impl APIRequestFactory {
	/// Create a new request factory
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::factory::APIRequestFactory;
	///
	/// let factory = APIRequestFactory::new();
	/// let request = factory.get("/api/users/").build();
	/// ```
	pub fn new() -> Self {
		Self {
			default_format: "json".to_string(),
			default_headers: HeaderMap::new(),
		}
	}
	/// Set the default content format (e.g., `"json"`, `"xml"`).
	pub fn with_format(mut self, format: impl Into<String>) -> Self {
		self.default_format = format.into();
		self
	}
	/// Add a default header to all requests created by this factory.
	pub fn with_header(
		mut self,
		name: impl AsRef<str>,
		value: impl AsRef<str>,
	) -> Result<Self, ClientError> {
		let header_name: http::header::HeaderName = name.as_ref().parse().map_err(|_| {
			ClientError::RequestFailed(format!("Invalid header name: {}", name.as_ref()))
		})?;
		self.default_headers
			.insert(header_name, HeaderValue::from_str(value.as_ref())?);
		Ok(self)
	}
	/// Create a GET request
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::factory::APIRequestFactory;
	///
	/// let factory = APIRequestFactory::new();
	/// let request = factory.get("/api/users/").build().unwrap();
	/// assert_eq!(request.method(), "GET");
	/// ```
	pub fn get(&self, path: &str) -> RequestBuilder {
		RequestBuilder::new(Method::GET, path, &self.default_headers)
	}
	/// Create a POST request
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::factory::APIRequestFactory;
	/// use serde_json::json;
	///
	/// let factory = APIRequestFactory::new();
	/// let data = json!({"name": "test"});
	/// let request = factory.post("/api/users/").json(&data).unwrap().build().unwrap();
	/// assert_eq!(request.method(), "POST");
	/// ```
	pub fn post(&self, path: &str) -> RequestBuilder {
		RequestBuilder::new(Method::POST, path, &self.default_headers)
			.with_format(&self.default_format)
	}
	/// Create a PUT request
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::factory::APIRequestFactory;
	/// use serde_json::json;
	///
	/// let factory = APIRequestFactory::new();
	/// let data = json!({"name": "updated"});
	/// let request = factory.put("/api/users/1/").json(&data).unwrap().build().unwrap();
	/// assert_eq!(request.method(), "PUT");
	/// ```
	pub fn put(&self, path: &str) -> RequestBuilder {
		RequestBuilder::new(Method::PUT, path, &self.default_headers)
			.with_format(&self.default_format)
	}
	/// Create a PATCH request
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::factory::APIRequestFactory;
	/// use serde_json::json;
	///
	/// let factory = APIRequestFactory::new();
	/// let data = json!({"name": "partial_update"});
	/// let request = factory.patch("/api/users/1/").json(&data).unwrap().build().unwrap();
	/// assert_eq!(request.method(), "PATCH");
	/// ```
	pub fn patch(&self, path: &str) -> RequestBuilder {
		RequestBuilder::new(Method::PATCH, path, &self.default_headers)
			.with_format(&self.default_format)
	}
	/// Create a DELETE request
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::factory::APIRequestFactory;
	///
	/// let factory = APIRequestFactory::new();
	/// let request = factory.delete("/api/users/1/").build().unwrap();
	/// assert_eq!(request.method(), "DELETE");
	/// ```
	pub fn delete(&self, path: &str) -> RequestBuilder {
		RequestBuilder::new(Method::DELETE, path, &self.default_headers)
	}
	/// Create a HEAD request
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::factory::APIRequestFactory;
	///
	/// let factory = APIRequestFactory::new();
	/// let request = factory.head("/api/users/").build().unwrap();
	/// assert_eq!(request.method(), "HEAD");
	/// ```
	pub fn head(&self, path: &str) -> RequestBuilder {
		RequestBuilder::new(Method::HEAD, path, &self.default_headers)
	}
	/// Create an OPTIONS request
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::factory::APIRequestFactory;
	///
	/// let factory = APIRequestFactory::new();
	/// let request = factory.options("/api/users/").build().unwrap();
	/// assert_eq!(request.method(), "OPTIONS");
	/// ```
	pub fn options(&self, path: &str) -> RequestBuilder {
		RequestBuilder::new(Method::OPTIONS, path, &self.default_headers)
	}
	/// Create a generic request with custom method
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::factory::APIRequestFactory;
	/// use http::Method;
	///
	/// let factory = APIRequestFactory::new();
	/// let request = factory.request(Method::TRACE, "/api/trace/").build().unwrap();
	/// assert_eq!(request.method(), "TRACE");
	/// ```
	pub fn request(&self, method: Method, path: &str) -> RequestBuilder {
		RequestBuilder::new(method, path, &self.default_headers)
	}
}

impl Default for APIRequestFactory {
	fn default() -> Self {
		Self::new()
	}
}

/// Builder for constructing test requests
pub struct RequestBuilder {
	method: Method,
	path: String,
	headers: HeaderMap,
	query_params: HashMap<String, String>,
	body: Option<Bytes>,
	format: String,
	user: Option<Value>,
}

impl RequestBuilder {
	/// Create a new request builder with the given HTTP method, path, and default headers.
	pub fn new(method: Method, path: &str, default_headers: &HeaderMap) -> Self {
		Self {
			method,
			path: path.to_string(),
			headers: default_headers.clone(),
			query_params: HashMap::new(),
			body: None,
			format: "json".to_string(),
			user: None,
		}
	}
	/// Get the HTTP method of this request.
	pub fn method(&self) -> Method {
		self.method.clone()
	}
	/// Get the path of this request.
	pub fn path(&self) -> &str {
		&self.path
	}
	/// Set the content format for this request.
	pub fn with_format(mut self, format: &str) -> Self {
		self.format = format.to_string();
		self
	}
	// Fixes #865
	/// Add a custom HTTP header to this request builder.
	pub fn header(mut self, name: &str, value: &str) -> Result<Self, ClientError> {
		let header_name: http::header::HeaderName = name
			.parse()
			.map_err(|_| ClientError::RequestFailed(format!("Invalid header name: {}", name)))?;
		self.headers
			.insert(header_name, HeaderValue::from_str(value)?);
		Ok(self)
	}
	/// Add a query parameter to this request.
	pub fn query(mut self, key: &str, value: &str) -> Self {
		self.query_params.insert(key.to_string(), value.to_string());
		self
	}
	/// Add a query parameter (alias for `query`).
	pub fn query_param(self, key: &str, value: &str) -> Self {
		self.query(key, value)
	}
	/// Set request body as JSON
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::factory::APIRequestFactory;
	/// use serde_json::json;
	///
	/// let factory = APIRequestFactory::new();
	/// let data = json!({"name": "test"});
	/// let request = factory.post("/api/users/").json(&data).unwrap().build();
	/// ```
	pub fn json<T: Serialize>(mut self, data: &T) -> Result<Self, ClientError> {
		let json = serde_json::to_vec(data)?;
		self.body = Some(Bytes::from(json));
		self.format = "json".to_string();
		Ok(self)
	}
	/// Set request body as form data
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::factory::APIRequestFactory;
	/// use serde_json::json;
	///
	/// let factory = APIRequestFactory::new();
	/// let data = json!({"name": "test", "age": 30});
	/// let request = factory.post("/api/users/").form(&data).unwrap().build();
	/// ```
	pub fn form<T: Serialize>(mut self, data: &T) -> Result<Self, ClientError> {
		let json_value = serde_json::to_value(data)?;
		if let Value::Object(map) = json_value {
			let form_data = map
				.iter()
				.map(|(k, v)| {
					let value_str = match v {
						Value::String(s) => s.clone(),
						_ => v.to_string(),
					};
					format!(
						"{}={}",
						url::form_urlencoded::byte_serialize(k.as_bytes()).collect::<String>(),
						url::form_urlencoded::byte_serialize(value_str.as_bytes())
							.collect::<String>()
					)
				})
				.collect::<Vec<_>>()
				.join("&");
			self.body = Some(Bytes::from(form_data));
			self.format = "form".to_string();
			Ok(self)
		} else {
			Err(ClientError::RequestFailed(
				"Expected object for form data".to_string(),
			))
		}
	}
	/// Set raw body
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::factory::APIRequestFactory;
	///
	/// let factory = APIRequestFactory::new();
	/// let request = factory.post("/api/upload/").body("raw data").build().unwrap();
	/// ```
	pub fn body(mut self, body: impl Into<Bytes>) -> Self {
		self.body = Some(body.into());
		self
	}
	/// Force authenticate as user (for testing)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::factory::APIRequestFactory;
	/// use serde_json::json;
	///
	/// let factory = APIRequestFactory::new();
	/// let user = json!({"id": 1, "username": "testuser"});
	/// let request = factory.get("/api/profile/").force_authenticate(user).build().unwrap();
	/// ```
	#[deprecated(
		since = "0.1.0-rc.16",
		note = "use `client.auth().session()` or `client.auth().jwt()` instead"
	)]
	pub fn force_authenticate(mut self, user: Value) -> Self {
		self.user = Some(user);
		self
	}
	/// Build the request
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::factory::APIRequestFactory;
	///
	/// let factory = APIRequestFactory::new();
	/// let request = factory.get("/api/users/").build().unwrap();
	/// assert_eq!(request.method(), "GET");
	/// ```
	pub fn build(self) -> Result<Request<Full<Bytes>>, ClientError> {
		let mut url = self.path.clone();

		// Add query parameters
		if !self.query_params.is_empty() {
			let query_string = self
				.query_params
				.iter()
				.map(|(k, v)| {
					format!(
						"{}={}",
						url::form_urlencoded::byte_serialize(k.as_bytes()).collect::<String>(),
						url::form_urlencoded::byte_serialize(v.as_bytes()).collect::<String>()
					)
				})
				.collect::<Vec<_>>()
				.join("&");
			url = format!("{}?{}", url, query_string);
		}

		let mut request = Request::builder().method(self.method).uri(url);

		// Add headers
		for (name, value) in self.headers.iter() {
			request = request.header(name, value);
		}

		// Add content type based on format
		if self.body.is_some() {
			let content_type = match self.format.as_str() {
				"json" => "application/json",
				"form" => "application/x-www-form-urlencoded",
				_ => "application/octet-stream",
			};
			request = request.header("Content-Type", content_type);
		}

		// Add authentication marker if user is set
		if self.user.is_some() {
			request = request.header("X-Test-User", "authenticated");
		}

		// Build request with body
		let body = self.body.unwrap_or_default();
		let req = request.body(Full::new(body))?;

		Ok(req)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use serde_json::json;

	// ========================================================================
	// Normal: APIRequestFactory
	// ========================================================================

	#[rstest]
	fn test_factory_new() {
		// Arrange
		let factory = APIRequestFactory::new();

		// Act
		let request = factory.get("/api/users/").build().unwrap();

		// Assert
		assert_eq!(request.method(), Method::GET);
	}

	#[rstest]
	fn test_factory_default() {
		// Arrange
		let factory_new = APIRequestFactory::new();
		let factory_default = APIRequestFactory::default();

		// Act
		let req_new = factory_new.get("/test").build().unwrap();
		let req_default = factory_default.get("/test").build().unwrap();

		// Assert
		assert_eq!(req_new.method(), req_default.method());
		assert_eq!(req_new.uri(), req_default.uri());
	}

	#[rstest]
	fn test_factory_with_format() {
		// Arrange
		let factory = APIRequestFactory::new().with_format("xml");

		// Act
		let request = factory.post("/api/data/").body("payload").build().unwrap();

		// Assert
		assert_eq!(
			request.headers().get("Content-Type").unwrap(),
			"application/octet-stream"
		);
	}

	#[rstest]
	fn test_factory_with_header() {
		// Arrange
		let factory = APIRequestFactory::new()
			.with_header("X-Custom", "value123")
			.unwrap();

		// Act
		let request = factory.get("/api/items/").build().unwrap();

		// Assert
		assert_eq!(request.headers().get("x-custom").unwrap(), "value123");
	}

	#[rstest]
	fn test_factory_get() {
		// Arrange
		let factory = APIRequestFactory::new();

		// Act
		let request = factory.get("/api/users/").build().unwrap();

		// Assert
		assert_eq!(request.method(), Method::GET);
	}

	#[rstest]
	fn test_factory_post() {
		// Arrange
		let factory = APIRequestFactory::new();

		// Act
		let request = factory.post("/api/users/").build().unwrap();

		// Assert
		assert_eq!(request.method(), Method::POST);
	}

	#[rstest]
	fn test_factory_put() {
		// Arrange
		let factory = APIRequestFactory::new();

		// Act
		let request = factory.put("/api/users/1/").build().unwrap();

		// Assert
		assert_eq!(request.method(), Method::PUT);
	}

	#[rstest]
	fn test_factory_patch() {
		// Arrange
		let factory = APIRequestFactory::new();

		// Act
		let request = factory.patch("/api/users/1/").build().unwrap();

		// Assert
		assert_eq!(request.method(), Method::PATCH);
	}

	#[rstest]
	fn test_factory_delete() {
		// Arrange
		let factory = APIRequestFactory::new();

		// Act
		let request = factory.delete("/api/users/1/").build().unwrap();

		// Assert
		assert_eq!(request.method(), Method::DELETE);
	}

	#[rstest]
	fn test_factory_head() {
		// Arrange
		let factory = APIRequestFactory::new();

		// Act
		let request = factory.head("/api/users/").build().unwrap();

		// Assert
		assert_eq!(request.method(), Method::HEAD);
	}

	#[rstest]
	fn test_factory_options() {
		// Arrange
		let factory = APIRequestFactory::new();

		// Act
		let request = factory.options("/api/users/").build().unwrap();

		// Assert
		assert_eq!(request.method(), Method::OPTIONS);
	}

	#[rstest]
	fn test_factory_request_custom() {
		// Arrange
		let factory = APIRequestFactory::new();

		// Act
		let request = factory
			.request(Method::TRACE, "/api/trace/")
			.build()
			.unwrap();

		// Assert
		assert_eq!(request.method(), Method::TRACE);
	}

	// ========================================================================
	// Normal: RequestBuilder
	// ========================================================================

	#[rstest]
	fn test_builder_json() {
		// Arrange
		let factory = APIRequestFactory::new();
		let data = json!({"name": "test"});

		// Act
		let request = factory
			.post("/api/users/")
			.json(&data)
			.unwrap()
			.build()
			.unwrap();

		// Assert
		assert_eq!(
			request.headers().get("Content-Type").unwrap(),
			"application/json"
		);
		assert_eq!(request.method(), Method::POST);
	}

	#[rstest]
	fn test_builder_form() {
		// Arrange
		let factory = APIRequestFactory::new();
		let data = json!({"name": "test", "age": 30});

		// Act
		let request = factory
			.post("/api/users/")
			.form(&data)
			.unwrap()
			.build()
			.unwrap();

		// Assert
		assert_eq!(
			request.headers().get("Content-Type").unwrap(),
			"application/x-www-form-urlencoded"
		);
	}

	#[rstest]
	fn test_builder_raw_body() {
		// Arrange
		let factory = APIRequestFactory::new();

		// Act
		let request = factory
			.post("/api/upload/")
			.body("raw data")
			.build()
			.unwrap();

		// Assert
		assert_eq!(request.method(), Method::POST);
		assert_eq!(
			request.headers().get("Content-Type").unwrap(),
			"application/json"
		);
	}

	#[rstest]
	fn test_builder_query_single() {
		// Arrange
		let factory = APIRequestFactory::new();

		// Act
		let request = factory
			.get("/api/users/")
			.query("page", "1")
			.build()
			.unwrap();

		// Assert
		assert_eq!(request.uri().to_string(), "/api/users/?page=1");
	}

	#[rstest]
	fn test_builder_query_multiple() {
		// Arrange
		let factory = APIRequestFactory::new();

		// Act
		let request = factory
			.get("/api/users/")
			.query("page", "1")
			.query_param("limit", "10")
			.build()
			.unwrap();

		// Assert
		let uri = request.uri().to_string();
		assert!(uri.contains("page=1"));
		assert!(uri.contains("limit=10"));
		assert!(uri.contains('&'));
	}

	#[rstest]
	#[allow(deprecated)]
	fn test_builder_force_authenticate() {
		// Arrange
		let factory = APIRequestFactory::new();
		let user = json!({"id": 1, "username": "testuser"});

		// Act
		let request = factory
			.get("/api/profile/")
			.force_authenticate(user)
			.build()
			.unwrap();

		// Assert
		assert_eq!(
			request.headers().get("X-Test-User").unwrap(),
			"authenticated"
		);
	}

	#[rstest]
	fn test_builder_method_getter() {
		// Arrange
		let factory = APIRequestFactory::new();

		// Act
		let builder = factory.get("/test");

		// Assert
		assert_eq!(builder.method(), Method::GET);
	}

	#[rstest]
	fn test_builder_path_getter() {
		// Arrange
		let factory = APIRequestFactory::new();

		// Act
		let builder = factory.get("/api/items/");

		// Assert
		assert_eq!(builder.path(), "/api/items/");
	}

	#[rstest]
	fn test_builder_with_format() {
		// Arrange
		let factory = APIRequestFactory::new();

		// Act
		let request = factory
			.post("/api/data/")
			.with_format("form")
			.body("key=val")
			.build()
			.unwrap();

		// Assert
		assert_eq!(
			request.headers().get("Content-Type").unwrap(),
			"application/x-www-form-urlencoded"
		);
	}

	// ========================================================================
	// Error cases
	// ========================================================================

	#[rstest]
	fn test_factory_with_header_invalid_name() {
		// Arrange / Act
		let result = APIRequestFactory::new().with_header("invalid header!", "value");

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_builder_form_non_object() {
		// Arrange
		let factory = APIRequestFactory::new();
		let data = json!([1, 2, 3]);

		// Act
		let result = factory.post("/api/users/").form(&data);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_builder_header_invalid_name() {
		// Arrange
		let factory = APIRequestFactory::new();

		// Act
		let result = factory.get("/test").header("bad header!", "value");

		// Assert
		assert!(result.is_err());
	}

	// ========================================================================
	// Edge cases
	// ========================================================================

	#[rstest]
	fn test_builder_no_body_no_content_type() {
		// Arrange
		let factory = APIRequestFactory::new();

		// Act
		let request = factory.get("/api/users/").build().unwrap();

		// Assert
		assert!(request.headers().get("Content-Type").is_none());
	}

	#[rstest]
	fn test_builder_json_empty_object() {
		// Arrange
		let factory = APIRequestFactory::new();
		let data = json!({});

		// Act
		let request = factory
			.post("/api/data/")
			.json(&data)
			.unwrap()
			.build()
			.unwrap();

		// Assert
		assert_eq!(
			request.headers().get("Content-Type").unwrap(),
			"application/json"
		);
	}

	#[rstest]
	fn test_builder_query_special_chars() {
		// Arrange
		let factory = APIRequestFactory::new();

		// Act
		let request = factory
			.get("/api/search/")
			.query("q", "hello world&foo=bar")
			.build()
			.unwrap();

		// Assert
		let uri = request.uri().to_string();
		assert!(uri.contains("hello+world"));
		assert!(!uri.contains("hello world&foo=bar"));
	}

	#[rstest]
	fn test_builder_unknown_format() {
		// Arrange
		let factory = APIRequestFactory::new().with_format("xml");

		// Act
		let request = factory.post("/api/data/").body("<xml/>").build().unwrap();

		// Assert
		assert_eq!(
			request.headers().get("Content-Type").unwrap(),
			"application/octet-stream"
		);
	}
}
