//! Request tests from Django REST Framework
//!
//! These tests verify the request handling functionality, including:
//! - Request initialization
//! - Content parsing (form data, JSON, etc.)
//! - Method-specific behavior (GET, HEAD, POST, PUT)
//! - Security context (HTTPS detection)
//! - User/auth integration

use bytes::Bytes;
use hyper::{HeaderMap, Method, Uri, Version};
use reinhardt_http::{Request, TrustedProxies};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;

/// Test: Request type initialization
/// DRF: test_request_type
#[test]
fn test_request_initialization() {
	let method = Method::GET;
	let uri = Uri::from_static("/api/users");
	let version = Version::HTTP_11;
	let headers = HeaderMap::new();
	let body = Bytes::new();

	let request = Request::builder()
		.method(method)
		.uri(uri)
		.version(version)
		.headers(headers)
		.body(body)
		.build()
		.unwrap();

	assert_eq!(request.method, Method::GET);
	assert_eq!(request.path(), "/api/users");
}

/// Test: GET request has no content
/// DRF: test_standard_behaviour_determines_no_content_GET
#[test]
fn test_get_has_no_content() {
	let method = Method::GET;
	let uri = Uri::from_static("/api/resource");
	let version = Version::HTTP_11;
	let headers = HeaderMap::new();
	let body = Bytes::new();

	let request = Request::builder()
		.method(method)
		.uri(uri)
		.version(version)
		.headers(headers)
		.body(body)
		.build()
		.unwrap();

	// GET requests should not have body content
	assert_eq!(request.method, Method::GET);
	// Attempt to read body should return empty bytes
	let body_result = request.read_body();
	assert!(body_result.is_ok());
	assert_eq!(body_result.unwrap().len(), 0);
}

/// Test: HEAD request has no content
/// DRF: test_standard_behaviour_determines_no_content_HEAD
#[test]
fn test_head_has_no_content() {
	let method = Method::HEAD;
	let uri = Uri::from_static("/api/resource");
	let version = Version::HTTP_11;
	let headers = HeaderMap::new();
	let body = Bytes::new();

	let request = Request::builder()
		.method(method)
		.uri(uri)
		.version(version)
		.headers(headers)
		.body(body)
		.build()
		.unwrap();

	assert_eq!(request.method, Method::HEAD);

	// HEAD requests should not have body content
	let body_result = request.read_body();
	assert!(body_result.is_ok());
	assert_eq!(body_result.unwrap().len(), 0);
}

/// Test: POST with form content
/// DRF: test_request_POST_with_form_content
#[test]
fn test_post_with_form_content() {
	let method = Method::POST;
	let uri = Uri::from_static("/api/submit");
	let version = Version::HTTP_11;
	let mut headers = HeaderMap::new();
	headers.insert(
		hyper::header::CONTENT_TYPE,
		"application/x-www-form-urlencoded".parse().unwrap(),
	);
	let body = Bytes::from("name=John&age=30");

	let request = Request::builder()
		.method(method)
		.uri(uri)
		.version(version)
		.headers(headers)
		.body(body)
		.build()
		.unwrap();

	assert_eq!(request.method, Method::POST);

	// Body should be readable
	let body_result = request.read_body();
	assert!(body_result.is_ok());
	assert!(!body_result.unwrap().is_empty());
}

/// Test: PUT with form content
/// DRF: test_standard_behaviour_determines_form_content_PUT
#[test]
fn test_put_with_form_content() {
	let method = Method::PUT;
	let uri = Uri::from_static("/api/resource/1");
	let version = Version::HTTP_11;
	let mut headers = HeaderMap::new();
	headers.insert(
		hyper::header::CONTENT_TYPE,
		"application/x-www-form-urlencoded".parse().unwrap(),
	);
	let body = Bytes::from("name=Updated&status=active");

	let request = Request::builder()
		.method(method)
		.uri(uri)
		.version(version)
		.headers(headers)
		.body(body)
		.build()
		.unwrap();

	assert_eq!(request.method, Method::PUT);

	let body_result = request.read_body();
	assert!(body_result.is_ok());
	assert!(!body_result.unwrap().is_empty());
}

/// Test: PUT with non-form content (JSON)
/// DRF: test_standard_behaviour_determines_non_form_content_PUT
#[test]
fn test_put_with_json_content() {
	let method = Method::PUT;
	let uri = Uri::from_static("/api/resource/1");
	let version = Version::HTTP_11;
	let mut headers = HeaderMap::new();
	headers.insert(
		hyper::header::CONTENT_TYPE,
		"application/json".parse().unwrap(),
	);
	let body = Bytes::from(r#"{"name":"Updated","status":"active"}"#);

	let request = Request::builder()
		.method(method)
		.uri(uri)
		.version(version)
		.headers(headers)
		.body(body)
		.build()
		.unwrap();

	assert_eq!(request.method, Method::PUT);

	// Should be able to parse as JSON
	#[derive(serde::Deserialize)]
	struct TestData {
		name: String,
		status: String,
	}

	let json_result: Result<TestData, _> = request.json();
	let data = json_result.unwrap();
	assert_eq!(data.name, "Updated");
	assert_eq!(data.status, "active");
}

/// Test: Secure flag default false
/// DRF: test_default_secure_false
#[test]
fn test_secure_default_false() {
	let method = Method::GET;
	let uri = Uri::from_static("http://example.com/api");
	let version = Version::HTTP_11;
	let headers = HeaderMap::new();
	let body = Bytes::new();

	let request = Request::builder()
		.method(method)
		.uri(uri)
		.version(version)
		.headers(headers)
		.body(body)
		.build()
		.unwrap();

	// Default should be insecure (HTTP)
	assert!(!request.is_secure());
	assert_eq!(request.scheme(), "http");
}

/// Test: Secure flag can be set true
/// DRF: test_default_secure_true
#[test]
fn test_secure_can_be_true() {
	let method = Method::GET;
	let uri = Uri::from_static("https://example.com/api");
	let version = Version::HTTP_11;
	let headers = HeaderMap::new();
	let body = Bytes::new();

	let request = Request::builder()
		.method(method)
		.uri(uri)
		.version(version)
		.headers(headers)
		.body(body)
		.secure(true)
		.build()
		.unwrap();

	assert!(request.is_secure());
	assert_eq!(request.scheme(), "https");
}

/// Test: X-Forwarded-Proto header detection (only from trusted proxies)
#[test]
fn test_secure_via_forwarded_proto() {
	let method = Method::GET;
	let uri = Uri::from_static("/api");
	let version = Version::HTTP_11;
	let mut headers = HeaderMap::new();
	headers.insert("x-forwarded-proto", "https".parse().unwrap());
	let body = Bytes::new();

	let proxy_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
	let request = Request::builder()
		.method(method)
		.uri(uri)
		.version(version)
		.headers(headers)
		.body(body)
		.remote_addr(SocketAddr::new(proxy_ip, 8080))
		.build()
		.unwrap();

	// Configure trusted proxies so X-Forwarded-Proto is respected
	request.set_trusted_proxies(TrustedProxies::new(vec![proxy_ip]));

	// Should detect HTTPS via X-Forwarded-Proto header from trusted proxy
	assert!(request.is_secure());
	assert_eq!(request.scheme(), "https");
}

/// Test: Request representation
/// DRF: test_repr
#[test]
fn test_request_properties() {
	let method = Method::POST;
	let uri = Uri::from_static("/api/users?page=1&limit=10");
	let version = Version::HTTP_11;
	let headers = HeaderMap::new();
	let body = Bytes::from("test body");

	let request = Request::builder()
		.method(method)
		.uri(uri)
		.version(version)
		.headers(headers)
		.body(body)
		.build()
		.unwrap();

	// Verify basic properties
	assert_eq!(request.method, Method::POST);
	assert_eq!(request.path(), "/api/users");
	assert_eq!(request.query_params.get("page"), Some(&"1".to_string()));
	assert_eq!(request.query_params.get("limit"), Some(&"10".to_string()));
}

/// Test: Query parameters parsing
#[test]
fn test_query_parameters() {
	let uri = Uri::from_str("/api/search?q=test&category=books&page=2").unwrap();
	let request = Request::builder()
		.method(Method::GET)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	assert_eq!(request.query_params.get("q"), Some(&"test".to_string()));
	assert_eq!(
		request.query_params.get("category"),
		Some(&"books".to_string())
	);
	assert_eq!(request.query_params.get("page"), Some(&"2".to_string()));
}

/// Test: Path extraction
#[test]
fn test_path_extraction() {
	let uri = Uri::from_str("/api/users/123?details=true").unwrap();
	let request = Request::builder()
		.method(Method::GET)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	assert_eq!(request.path(), "/api/users/123");
}

/// Test: Body can only be consumed once
/// DRF: test_duplicate_request_stream_parsing_exception
#[test]
fn test_body_consumed_once() {
	let request = Request::builder()
		.method(Method::POST)
		.uri("/api/data")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::from("test data"))
		.build()
		.unwrap();

	// First read should succeed
	let first_read = request.read_body();
	assert!(first_read.is_ok());

	// Second read should fail
	let second_read = request.read_body();
	assert!(second_read.is_err());
}

/// Test: Build absolute URI
#[test]
fn test_build_absolute_uri() {
	let uri = Uri::from_str("/api/users").unwrap();
	let mut headers = HeaderMap::new();
	headers.insert(hyper::header::HOST, "example.com".parse().unwrap());

	let request = Request::builder()
		.method(Method::GET)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(headers)
		.body(Bytes::new())
		.build()
		.unwrap();

	let absolute_uri = request.build_absolute_uri(None);
	assert_eq!(absolute_uri, "http://example.com/api/users");
}

/// Test: Build absolute URI with HTTPS
#[test]
fn test_build_absolute_uri_https() {
	let uri = Uri::from_str("/api/users").unwrap();
	let mut headers = HeaderMap::new();
	headers.insert(hyper::header::HOST, "example.com".parse().unwrap());

	let request = Request::builder()
		.method(Method::GET)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(headers)
		.body(Bytes::new())
		.secure(true)
		.build()
		.unwrap();

	let absolute_uri = request.build_absolute_uri(None);
	assert_eq!(absolute_uri, "https://example.com/api/users");
}

/// Test: Accept-Language header parsing
#[test]
fn test_accept_language_parsing() {
	let uri = Uri::from_static("/api");
	let mut headers = HeaderMap::new();
	headers.insert(
		hyper::header::ACCEPT_LANGUAGE,
		"en-US,en;q=0.9,ja;q=0.8".parse().unwrap(),
	);

	let request = Request::builder()
		.method(Method::GET)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(headers)
		.body(Bytes::new())
		.build()
		.unwrap();

	let languages = request.get_accepted_languages();
	assert_eq!(languages.len(), 3);

	// Should be sorted by quality
	assert_eq!(languages[0].0, "en-US");
	assert_eq!(languages[0].1, 1.0);
	assert_eq!(languages[1].0, "en");
	assert_eq!(languages[1].1, 0.9);
	assert_eq!(languages[2].0, "ja");
	assert_eq!(languages[2].1, 0.8);
}

/// Test: Preferred language extraction
#[test]
fn test_preferred_language() {
	let uri = Uri::from_static("/api");
	let mut headers = HeaderMap::new();
	headers.insert(
		hyper::header::ACCEPT_LANGUAGE,
		"ja;q=0.8,en-US,en;q=0.9".parse().unwrap(),
	);

	let request = Request::builder()
		.method(Method::GET)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(headers)
		.body(Bytes::new())
		.build()
		.unwrap();

	let preferred = request.get_preferred_language();
	assert_eq!(preferred, Some("en-US".to_string())); // Highest quality
}

/// Test: Language from cookie
#[test]
fn test_language_from_cookie() {
	let uri = Uri::from_static("/api");
	let mut headers = HeaderMap::new();
	headers.insert(
		hyper::header::COOKIE,
		"session=abc123; reinhardt_language=fr; other=value"
			.parse()
			.unwrap(),
	);

	let request = Request::builder()
		.method(Method::GET)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(headers)
		.body(Bytes::new())
		.build()
		.unwrap();

	let language = request.get_language_from_cookie("reinhardt_language");
	assert_eq!(language, Some("fr".to_string()));
}

/// Test: JSON parsing
#[test]
fn test_json_parsing() {
	#[derive(serde::Deserialize, serde::Serialize)]
	struct User {
		name: String,
		email: String,
	}

	let body = serde_json::to_vec(&User {
		name: "John Doe".to_string(),
		email: "john@example.com".to_string(),
	})
	.unwrap();

	let mut headers = HeaderMap::new();
	headers.insert(
		hyper::header::CONTENT_TYPE,
		"application/json".parse().unwrap(),
	);

	let request = Request::builder()
		.method(Method::POST)
		.uri("/api/users")
		.version(Version::HTTP_11)
		.headers(headers)
		.body(Bytes::from(body))
		.build()
		.unwrap();

	let user: Result<User, _> = request.json();
	let user = user.unwrap();
	assert_eq!(user.name, "John Doe");
	assert_eq!(user.email, "john@example.com");
}

/// Test: Path parameters
#[test]
fn test_path_parameters() {
	let mut request = Request::builder()
		.method(Method::GET)
		.uri("/api/users/123")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	// Manually set path params (normally done by router)
	request
		.path_params
		.insert("id".to_string(), "123".to_string());

	assert_eq!(request.path_params.get("id"), Some(&"123".to_string()));
}

/// Test: Empty query string
#[test]
fn test_empty_query_string() {
	let request = Request::builder()
		.method(Method::GET)
		.uri("/api/users")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	assert!(request.query_params.is_empty());
}

/// Test: Multiple values in query (last wins in simple HashMap)
#[test]
fn test_query_param_single_value() {
	let uri = Uri::from_str("/api/search?tag=rust").unwrap();
	let request = Request::builder()
		.method(Method::GET)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	assert_eq!(request.query_params.get("tag"), Some(&"rust".to_string()));
}

/// Test: Invalid language codes are rejected
#[test]
fn test_invalid_language_codes_rejected() {
	let uri = Uri::from_static("/api");
	let mut headers = HeaderMap::new();
	// Invalid: starts with hyphen
	headers.insert(
		hyper::header::ACCEPT_LANGUAGE,
		"-invalid,en".parse().unwrap(),
	);

	let request = Request::builder()
		.method(Method::GET)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(headers)
		.body(Bytes::new())
		.build()
		.unwrap();

	let languages = request.get_accepted_languages();
	// Should only have valid 'en'
	assert_eq!(languages.len(), 1);
	assert_eq!(languages[0].0, "en");
}
