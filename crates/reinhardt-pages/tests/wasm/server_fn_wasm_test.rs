//! Server Function WASM Integration Tests
//!
//! These tests verify server function client-side behavior in a browser environment,
//! including CSRF token injection into HTTP requests.
//!
//! **Run with**: `wasm-pack test --headless --chrome`

#![cfg(wasm)]

use js_sys::{Function, Object, Reflect};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

use reinhardt_pages::csrf::{CSRF_HEADER_NAME, csrf_headers};
use reinhardt_pages::server_fn::resolve_endpoint;
use reinhardt_pages::testing::{cleanup_csrf_fixtures, setup_csrf_cookie, setup_csrf_meta_tag};
use reinhardt_pages_macros::server_fn;

#[derive(Debug)]
struct CustomClientError(String);

impl std::fmt::Display for CustomClientError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&self.0)
	}
}

impl std::error::Error for CustomClientError {}

impl From<reinhardt_pages::server_fn::ServerFnError> for CustomClientError {
	fn from(err: reinhardt_pages::server_fn::ServerFnError) -> Self {
		Self(err.to_string())
	}
}

#[server_fn]
async fn custom_error_server_fn(value: u32) -> Result<u32, CustomClientError> {
	Ok(value)
}

#[server_fn]
async fn server_fn_cancellation_probe(
	value: u32,
) -> Result<u32, reinhardt_pages::server_fn::ServerFnError> {
	Ok(value)
}

#[server_fn]
async fn save(
	name: String,
	age: u32,
	avatar: Option<reinhardt_core::parsers::UploadedFile>,
) -> Result<String, reinhardt_pages::server_fn::ServerFnError> {
	Ok(format!("{name}:{age}:{}", avatar.is_some()))
}

struct FetchStubGuard {
	window: web_sys::Window,
	previous_fetch: JsValue,
	previous_abort_controller: JsValue,
	probe: Object,
}

impl FetchStubGuard {
	fn install() -> Self {
		let window = web_sys::window().expect("browser window");
		let global = js_sys::global();
		let previous_fetch = Reflect::get(window.as_ref(), &JsValue::from_str("fetch"))
			.expect("window.fetch must be readable");
		let previous_abort_controller =
			Reflect::get(global.as_ref(), &JsValue::from_str("AbortController"))
				.expect("global AbortController must be readable");
		let probe = Object::new();
		Reflect::set(&probe, &JsValue::from_str("aborted"), &JsValue::FALSE)
			.expect("probe aborted property");
		Reflect::set(
			&probe,
			&JsValue::from_str("abortControllerCalls"),
			&JsValue::from_f64(0.0),
		)
		.expect("probe abortControllerCalls property");
		Reflect::set(
			global.as_ref(),
			&JsValue::from_str("__reinhardtServerFnFetchProbe"),
			&probe,
		)
		.expect("install server function fetch probe");
		let abort_controller_spy = Function::new_with_args(
			"OriginalAbortController, probe",
			r#"
				return class extends OriginalAbortController {
					constructor() {
						super();
						probe.abortControllerCalls += 1;
					}
				};
			"#,
		)
		.call2(&JsValue::NULL, &previous_abort_controller, &probe)
		.expect("create AbortController spy");
		Reflect::set(
			global.as_ref(),
			&JsValue::from_str("AbortController"),
			&abort_controller_spy,
		)
		.expect("install AbortController spy");
		let stub = Function::new_with_args(
			"request",
			r#"
			const probe = globalThis.__reinhardtServerFnFetchProbe;
			probe.aborted = request.signal ? request.signal.aborted : false;
			return Promise.resolve(new Response('42', { status: 200 }));
			"#,
		);
		Reflect::set(window.as_ref(), &JsValue::from_str("fetch"), stub.as_ref())
			.expect("install server function fetch stub");

		Self {
			window,
			previous_fetch,
			previous_abort_controller,
			probe,
		}
	}

	fn flag(&self, name: &str) -> bool {
		Reflect::get(&self.probe, &JsValue::from_str(name))
			.expect("probe flag must be readable")
			.as_bool()
			.unwrap_or(false)
	}

	fn abort_controller_calls(&self) -> u32 {
		Reflect::get(&self.probe, &JsValue::from_str("abortControllerCalls"))
			.expect("probe abortControllerCalls must be readable")
			.as_f64()
			.unwrap_or_default() as u32
	}
}

impl Drop for FetchStubGuard {
	fn drop(&mut self) {
		let _ = Reflect::set(
			self.window.as_ref(),
			&JsValue::from_str("fetch"),
			&self.previous_fetch,
		);
		let _ = Reflect::set(
			js_sys::global().as_ref(),
			&JsValue::from_str("AbortController"),
			&self.previous_abort_controller,
		);
		let _ = Reflect::delete_property(
			js_sys::global().as_ref(),
			&JsValue::from_str("__reinhardtServerFnFetchProbe"),
		);
	}
}

struct MultipartFetchStubGuard {
	window: web_sys::Window,
	previous_fetch: JsValue,
}

impl MultipartFetchStubGuard {
	fn install(expected_avatar: JsValue) -> Self {
		let window = web_sys::window().expect("browser window");
		let previous_fetch = Reflect::get(window.as_ref(), &JsValue::from_str("fetch"))
			.expect("window.fetch must be readable");
		Reflect::set(
			js_sys::global().as_ref(),
			&JsValue::from_str("__reinhardtExpectedMultipartAvatar"),
			&expected_avatar,
		)
		.expect("install expected multipart avatar");
		let stub = Function::new_with_args(
			"request",
			r#"
				return request.formData().then((formData) => {
					const expectedAvatar = globalThis.__reinhardtExpectedMultipartAvatar;
					if (!(formData instanceof FormData)) throw new Error('request body was not FormData');
					if (formData.get('name') !== '"Ada"') throw new Error('name was not JSON encoded');
					if (formData.get('age') !== '42') throw new Error('age was not JSON encoded');
					if (!request.headers.get('Content-Type')?.startsWith('multipart/form-data; boundary=')) {
						throw new Error('Content-Type must be generated by the browser');
					}
					if (expectedAvatar === null) {
						if (formData.has('avatar')) throw new Error('None avatar must be omitted');
					} else {
						const avatar = formData.get('avatar');
						if (!(avatar instanceof File)
							|| avatar.name !== expectedAvatar.name
							|| avatar.type !== expectedAvatar.type
							|| avatar.size !== expectedAvatar.size) {
							throw new Error('avatar was not appended as the expected File');
						}
					}
					return new Response('"saved"', { status: 200 });
				});
			"#,
		);
		Reflect::set(window.as_ref(), &JsValue::from_str("fetch"), stub.as_ref())
			.expect("install multipart fetch stub");

		Self {
			window,
			previous_fetch,
		}
	}
}

impl Drop for MultipartFetchStubGuard {
	fn drop(&mut self) {
		let _ = Reflect::set(
			self.window.as_ref(),
			&JsValue::from_str("fetch"),
			&self.previous_fetch,
		);
		let _ = Reflect::delete_property(
			js_sys::global().as_ref(),
			&JsValue::from_str("__reinhardtExpectedMultipartAvatar"),
		);
	}
}

#[wasm_bindgen_test]
async fn server_fn_call_without_active_cancellation_does_not_construct_abort_controller() {
	// Arrange
	let fetch_stub = FetchStubGuard::install();

	// Act
	let result = server_fn_cancellation_probe(7)
		.await
		.expect("the stubbed server function request should resolve");

	// Assert
	assert_eq!(result, 42);
	assert_eq!(
		fetch_stub.abort_controller_calls(),
		0,
		"a normal server function call must not construct an AbortController"
	);
	assert!(!fetch_stub.flag("aborted"));
}

#[wasm_bindgen_test]
async fn multipart_server_fn_sends_json_scalars_and_an_optional_file_as_form_data() {
	// Arrange
	let avatar =
		Function::new_no_args("return new File(['abc'], 'avatar.txt', { type: 'text/plain' });")
			.call0(&JsValue::NULL)
			.expect("create browser file")
			.dyn_into::<web_sys::File>()
			.expect("created value must be a File");
	let _fetch_stub = MultipartFetchStubGuard::install(avatar.clone().into());

	// Act
	let result = save("Ada".to_string(), 42, Some(avatar))
		.await
		.expect("multipart server function request should resolve");

	// Assert
	assert_eq!(result, "saved");
}

#[wasm_bindgen_test]
async fn multipart_server_fn_omits_none_file_parts() {
	// Arrange
	let _fetch_stub = MultipartFetchStubGuard::install(JsValue::NULL);

	// Act
	let result = save("Ada".to_string(), 42, None)
		.await
		.expect("multipart server function request should resolve");

	// Assert
	assert_eq!(result, "saved");
}

#[wasm_bindgen_test]
fn test_custom_client_error_display_contract() {
	let error: CustomClientError =
		reinhardt_pages::server_fn::ServerFnError::network("client error").into();
	assert_eq!(error.to_string(), "client error");
}

// ============================================================================
// csrf_headers() Integration Tests
// ============================================================================

/// Test csrf_headers() returns correct header tuple from cookie
#[wasm_bindgen_test]
fn test_csrf_headers_from_cookie() {
	cleanup_csrf_fixtures();

	setup_csrf_cookie("header_test_token");

	let headers = csrf_headers();
	assert!(headers.is_some());

	let (name, value) = headers.unwrap();
	assert_eq!(name, CSRF_HEADER_NAME);
	assert_eq!(name, "X-CSRFToken");
	assert_eq!(value, "header_test_token");

	cleanup_csrf_fixtures();
}

/// Test csrf_headers() returns correct header tuple from meta tag
#[wasm_bindgen_test]
fn test_csrf_headers_from_meta() {
	cleanup_csrf_fixtures();

	setup_csrf_meta_tag("meta_header_token");

	let headers = csrf_headers();
	assert!(headers.is_some());

	let (name, value) = headers.unwrap();
	assert_eq!(name, CSRF_HEADER_NAME);
	assert_eq!(value, "meta_header_token");

	cleanup_csrf_fixtures();
}

/// Test csrf_headers() returns None when no token available
#[wasm_bindgen_test]
fn test_csrf_headers_none_when_no_token() {
	cleanup_csrf_fixtures();

	let headers = csrf_headers();
	assert!(headers.is_none());

	cleanup_csrf_fixtures();
}

// ============================================================================
// HTTP Request Header Injection Tests
// ============================================================================

/// Test that CSRF header name matches Django convention
#[wasm_bindgen_test]
fn test_csrf_header_name_django_compatible() {
	// Django expects X-CSRFToken header for AJAX requests
	assert_eq!(CSRF_HEADER_NAME, "X-CSRFToken");
}

/// Test headers can be used with the browser Fetch API request path.
#[wasm_bindgen_test]
fn test_csrf_headers_usable_with_request() {
	cleanup_csrf_fixtures();

	setup_csrf_cookie("request_test_token");

	// Verify headers can be destructured and used
	if let Some((header_name, header_value)) = csrf_headers() {
		assert_eq!(header_name, "X-CSRFToken");
		assert!(!header_value.is_empty());

		// Verify header value type is compatible with generated request headers
		let _: &str = header_name; // Static str
		let _: &str = &header_value; // String reference
	} else {
		panic!("Expected csrf_headers to return Some");
	}

	cleanup_csrf_fixtures();
}

// ============================================================================
// Token Priority in Headers Tests
// ============================================================================

/// Test headers use cookie token when available (highest priority)
#[wasm_bindgen_test]
fn test_csrf_headers_prefer_cookie() {
	cleanup_csrf_fixtures();

	setup_csrf_cookie("cookie_priority_header");
	setup_csrf_meta_tag("meta_priority_header");

	let headers = csrf_headers();
	let (_, value) = headers.unwrap();
	assert_eq!(value, "cookie_priority_header");

	cleanup_csrf_fixtures();
}

/// Test headers fall back to meta when no cookie
#[wasm_bindgen_test]
fn test_csrf_headers_fallback_to_meta() {
	cleanup_csrf_fixtures();

	setup_csrf_meta_tag("meta_fallback_header");

	let headers = csrf_headers();
	let (_, value) = headers.unwrap();
	assert_eq!(value, "meta_fallback_header");

	cleanup_csrf_fixtures();
}

// ============================================================================
// Server Function Client Stub Verification
// ============================================================================

#[wasm_bindgen_test]
fn test_resolve_endpoint_absolutizes_wasm_path() {
	let endpoint = resolve_endpoint("/api/server_fn/example");
	assert!(
		web_sys::Url::new(&endpoint).is_ok(),
		"endpoint should be absolute for browser fetch: {endpoint}"
	);
	assert!(
		endpoint.starts_with("http://") || endpoint.starts_with("https://"),
		"endpoint should include a browser HTTP origin: {endpoint}"
	);
	assert!(
		endpoint.ends_with("/api/server_fn/example"),
		"endpoint should preserve the server_fn path: {endpoint}"
	);
}

/// Test that automatic CSRF injection produces valid header format
///
/// This test verifies the contract that the server_fn macro relies on:
/// csrf_headers() returns `Option<(&'static str, String)>` where the first
/// element is the header name and second is the value.
#[wasm_bindgen_test]
fn test_csrf_headers_contract() {
	cleanup_csrf_fixtures();

	setup_csrf_cookie("contract_test");

	let result = csrf_headers();

	// Contract: returns Option<(&'static str, String)>
	assert!(result.is_some(), "Should return Some when token exists");

	let (name, value) = result.unwrap();

	// Contract: name is a static str (header name constant)
	assert!(!name.is_empty(), "Header name should not be empty");

	// Contract: value is the token string
	assert_eq!(value, "contract_test", "Value should match set token");

	// Contract: name matches Django's expected header
	assert_eq!(
		name, "X-CSRFToken",
		"Header name should be X-CSRFToken for Django compatibility"
	);

	cleanup_csrf_fixtures();
}

/// Test that no CSRF header is added when token unavailable
#[wasm_bindgen_test]
fn test_no_csrf_header_when_unavailable() {
	cleanup_csrf_fixtures();

	let result = csrf_headers();

	// When no token, csrf_headers returns None
	// Server functions with no_csrf=false will not add any CSRF header
	assert!(result.is_none(), "Should return None when no token exists");

	cleanup_csrf_fixtures();
}

// ============================================================================
// Cleanup Verification Tests
// ============================================================================

/// Test that cleanup properly removes all CSRF fixtures
#[wasm_bindgen_test]
fn test_cleanup_removes_all_fixtures() {
	// Set up all fixture types
	setup_csrf_cookie("cleanup_test_cookie");
	setup_csrf_meta_tag("cleanup_test_meta");

	// Verify they exist
	assert!(csrf_headers().is_some());

	// Clean up
	cleanup_csrf_fixtures();

	// Verify all removed
	assert!(csrf_headers().is_none());
}
