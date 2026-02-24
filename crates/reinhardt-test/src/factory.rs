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
	/// use reinhardt_test::factory::APIRequestFactory;
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
	pub fn with_format(mut self, format: impl Into<String>) -> Self {
		self.default_format = format.into();
		self
	}
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
	/// use reinhardt_test::factory::APIRequestFactory;
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
	/// use reinhardt_test::factory::APIRequestFactory;
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
	/// use reinhardt_test::factory::APIRequestFactory;
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
	/// use reinhardt_test::factory::APIRequestFactory;
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
	/// use reinhardt_test::factory::APIRequestFactory;
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
	/// use reinhardt_test::factory::APIRequestFactory;
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
	/// use reinhardt_test::factory::APIRequestFactory;
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
	/// use reinhardt_test::factory::APIRequestFactory;
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
	pub fn method(&self) -> Method {
		self.method.clone()
	}
	pub fn path(&self) -> &str {
		&self.path
	}
	pub fn with_format(mut self, format: &str) -> Self {
		self.format = format.to_string();
		self
	}
	// Fixes #865
	pub fn header(mut self, name: &str, value: &str) -> Result<Self, ClientError> {
		let header_name: http::header::HeaderName = name
			.parse()
			.map_err(|_| ClientError::RequestFailed(format!("Invalid header name: {}", name)))?;
		self.headers
			.insert(header_name, HeaderValue::from_str(value)?);
		Ok(self)
	}
	pub fn query(mut self, key: &str, value: &str) -> Self {
		self.query_params.insert(key.to_string(), value.to_string());
		self
	}
	pub fn query_param(self, key: &str, value: &str) -> Self {
		self.query(key, value)
	}
	/// Set request body as JSON
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::factory::APIRequestFactory;
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
	/// use reinhardt_test::factory::APIRequestFactory;
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
	/// use reinhardt_test::factory::APIRequestFactory;
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
	/// use reinhardt_test::factory::APIRequestFactory;
	/// use serde_json::json;
	///
	/// let factory = APIRequestFactory::new();
	/// let user = json!({"id": 1, "username": "testuser"});
	/// let request = factory.get("/api/profile/").force_authenticate(user).build().unwrap();
	/// ```
	pub fn force_authenticate(mut self, user: Value) -> Self {
		self.user = Some(user);
		self
	}
	/// Build the request
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::factory::APIRequestFactory;
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
