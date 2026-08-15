//! Internal Fetch API wrapper used by generated WASM client code.

use crate::server_fn::ServerFnError;

/// Browser Fetch credentials mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FetchCredentials {
	/// Do not send credentials with the request.
	Omit,
	/// Send credentials for same-origin requests.
	#[default]
	SameOrigin,
	/// Send credentials for same-origin and cross-origin requests.
	Include,
}

#[cfg(wasm)]
impl FetchCredentials {
	fn into_request_credentials(self) -> web_sys::RequestCredentials {
		match self {
			Self::Omit => web_sys::RequestCredentials::Omit,
			Self::SameOrigin => web_sys::RequestCredentials::SameOrigin,
			Self::Include => web_sys::RequestCredentials::Include,
		}
	}
}

/// HTTP response body and status returned by the internal Fetch API wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchResponse {
	status: u16,
	body: String,
}

impl FetchResponse {
	/// Creates a response from a status code and text body.
	pub fn new(status: u16, body: String) -> Self {
		Self { status, body }
	}

	/// Returns the numeric HTTP status code.
	pub fn status(&self) -> u16 {
		self.status
	}

	/// Returns true for HTTP 2xx responses.
	pub fn is_success(&self) -> bool {
		(200..300).contains(&self.status)
	}

	/// Consumes the response and returns its text body.
	pub fn into_text(self) -> String {
		self.body
	}

	/// Deserializes the response text as JSON.
	pub fn json<T>(&self) -> Result<T, ServerFnError>
	where
		T: serde::de::DeserializeOwned,
	{
		serde_json::from_str(&self.body).map_err(|e| ServerFnError::deserialization(e.to_string()))
	}
}

/// Sends an HTTP request through the browser Fetch API.
///
/// Requests use the browser default-equivalent `same-origin` credentials mode.
/// Use [`request_with_credentials`] when a generated client must explicitly
/// include cross-origin credentials.
pub async fn request(
	method: &str,
	url: &str,
	body: Option<&str>,
	headers: Vec<(String, String)>,
) -> Result<FetchResponse, ServerFnError> {
	request_with_credentials(method, url, body, headers, FetchCredentials::default()).await
}

/// Sends an HTTP request through the browser Fetch API with an explicit credentials mode.
#[cfg(wasm)]
pub async fn request_with_credentials(
	method: &str,
	url: &str,
	body: Option<&str>,
	headers: Vec<(String, String)>,
	credentials: FetchCredentials,
) -> Result<FetchResponse, ServerFnError> {
	use wasm_bindgen::JsValue;

	let body = body.map(JsValue::from_str);
	request_with_credentials_and_cancellation(
		method,
		url,
		body.as_ref(),
		headers,
		credentials,
		crate::cancellation::active_cancellation(),
	)
	.await
}

/// Sends a browser FormData request with an explicit credentials mode.
#[cfg(wasm)]
pub(crate) async fn request_with_form_data(
	method: &str,
	url: &str,
	form_data: &web_sys::FormData,
	headers: Vec<(String, String)>,
	credentials: FetchCredentials,
) -> Result<FetchResponse, ServerFnError> {
	request_with_credentials_and_cancellation(
		method,
		url,
		Some(form_data.as_ref()),
		headers,
		credentials,
		crate::cancellation::active_cancellation(),
	)
	.await
}

#[cfg(wasm)]
pub(crate) async fn request_with_credentials_and_cancellation(
	method: &str,
	url: &str,
	body: Option<&wasm_bindgen::JsValue>,
	headers: Vec<(String, String)>,
	credentials: FetchCredentials,
	cancellation: Option<crate::cancellation::CancellationHandle>,
) -> Result<FetchResponse, ServerFnError> {
	use wasm_bindgen::JsCast;
	use wasm_bindgen_futures::JsFuture;
	use web_sys::{AbortController, Request, RequestInit, RequestMode, Response, window};

	let init = RequestInit::new();
	init.set_method(method);
	init.set_mode(RequestMode::Cors);
	init.set_credentials(credentials.into_request_credentials());
	let _cancellation_registration = if let Some(cancellation) = cancellation {
		let controller = AbortController::new()
			.map_err(|error| ServerFnError::network(js_error_message(error)))?;
		let signal = controller.signal();
		init.set_signal(Some(&signal));
		Some(cancellation.on_cancel(move || controller.abort()))
	} else {
		None
	};
	if let Some(body) = body {
		init.set_body(body);
	}

	let request = Request::new_with_str_and_init(url, &init)
		.map_err(|e| ServerFnError::network(js_error_message(e)))?;

	let request_headers = request.headers();
	for (name, value) in headers {
		request_headers
			.set(&name, &value)
			.map_err(|e| ServerFnError::network(js_error_message(e)))?;
	}

	let window = window().ok_or_else(|| ServerFnError::network("window is unavailable"))?;
	let response_value = JsFuture::from(window.fetch_with_request(&request))
		.await
		.map_err(|e| ServerFnError::network(js_error_message(e)))?;
	let response: Response = response_value
		.dyn_into()
		.map_err(|e| ServerFnError::network(js_error_message(e)))?;

	let status = response.status();
	let text_promise = response
		.text()
		.map_err(|e| ServerFnError::network(js_error_message(e)))?;
	let text_value = JsFuture::from(text_promise)
		.await
		.map_err(|e| ServerFnError::network(js_error_message(e)))?;
	let body = text_value.as_string().unwrap_or_default();

	Ok(FetchResponse::new(status, body))
}

/// Native placeholder for accidental non-WASM use.
#[cfg(native)]
pub async fn request_with_credentials(
	_method: &str,
	_url: &str,
	_body: Option<&str>,
	_headers: Vec<(String, String)>,
	_credentials: FetchCredentials,
) -> Result<FetchResponse, ServerFnError> {
	Err(ServerFnError::network(
		"Fetch API is not available outside browser WASM",
	))
}

#[cfg(wasm)]
fn js_error_message(value: wasm_bindgen::JsValue) -> String {
	value.as_string().unwrap_or_else(|| format!("{value:?}"))
}

#[cfg(test)]
mod tests {
	use super::FetchCredentials;

	#[test]
	fn default_credentials_mode_is_same_origin() {
		assert_eq!(FetchCredentials::default(), FetchCredentials::SameOrigin);
	}
}

#[cfg(all(test, wasm))]
mod wasm_tests {
	use std::cell::RefCell;
	use std::rc::Rc;

	use js_sys::{Function, Object, Reflect};
	use wasm_bindgen::JsValue;
	use wasm_bindgen_test::*;

	use super::*;
	use crate::cancellation::{CancellationSource, scope_cancellation};

	wasm_bindgen_test_configure!(run_in_browser);

	struct FetchStubGuard {
		window: web_sys::Window,
		previous_fetch: JsValue,
		probe: Object,
	}

	impl FetchStubGuard {
		fn install(script: &str) -> Self {
			let window = web_sys::window().expect("browser window");
			let previous_fetch = Reflect::get(window.as_ref(), &JsValue::from_str("fetch"))
				.expect("window.fetch must be readable");
			let probe = Object::new();
			Reflect::set(&probe, &JsValue::from_str("signalSeen"), &JsValue::FALSE)
				.expect("probe signalSeen property");
			Reflect::set(&probe, &JsValue::from_str("aborted"), &JsValue::FALSE)
				.expect("probe aborted property");
			Reflect::set(
				js_sys::global().as_ref(),
				&JsValue::from_str("__reinhardtFetchProbe"),
				&probe,
			)
			.expect("install fetch probe");
			let stub = Function::new_with_args("request", script);
			Reflect::set(window.as_ref(), &JsValue::from_str("fetch"), stub.as_ref())
				.expect("install fetch stub");

			Self {
				window,
				previous_fetch,
				probe,
			}
		}

		fn flag(&self, name: &str) -> bool {
			Reflect::get(&self.probe, &JsValue::from_str(name))
				.expect("probe flag must be readable")
				.as_bool()
				.unwrap_or(false)
		}
	}

	impl Drop for FetchStubGuard {
		fn drop(&mut self) {
			let _ = Reflect::set(
				self.window.as_ref(),
				&JsValue::from_str("fetch"),
				&self.previous_fetch,
			);
			let _ = Reflect::delete_property(
				js_sys::global().as_ref(),
				&JsValue::from_str("__reinhardtFetchProbe"),
			);
		}
	}

	#[wasm_bindgen_test]
	async fn active_cancellation_attaches_abort_signal() {
		// Arrange
		let _stub = FetchStubGuard::install(
			r#"
			const probe = globalThis.__reinhardtFetchProbe;
			probe.signalSeen = !!request.signal;
			probe.aborted = request.signal ? request.signal.aborted : false;
			return Promise.resolve(new Response('{}', { status: 200 }));
			"#,
		);
		let source = CancellationSource::new();

		// Act
		let response = scope_cancellation(
			source.handle(),
			request("GET", "/__reinhardt_cancellation_probe", None, vec![]),
		)
		.await
		.expect("stubbed fetch should resolve");

		// Assert
		assert_eq!(response.status(), 200);
		// The request was polled inside the scoped cancellation future.
		// `source` is still live, so the signal must not have been aborted.
		assert!(!_stub.flag("aborted"));
		assert!(_stub.flag("signalSeen"));
	}

	#[wasm_bindgen_test]
	async fn cancelling_active_scope_aborts_browser_fetch() {
		// Arrange
		let _stub = FetchStubGuard::install(
			r#"
			const probe = globalThis.__reinhardtFetchProbe;
			probe.signalSeen = !!request.signal;
			return new Promise((resolve, reject) => {
				request.signal.addEventListener('abort', () => {
					probe.aborted = true;
					reject(new Error('aborted'));
				});
			});
			"#,
		);
		let source = CancellationSource::new();
		let result_slot: Rc<RefCell<Option<Result<FetchResponse, ServerFnError>>>> =
			Rc::new(RefCell::new(None));
		let result_slot_for_task = Rc::clone(&result_slot);
		let token = source.handle();

		// Act
		wasm_bindgen_futures::spawn_local(async move {
			let result = scope_cancellation(
				token,
				request("GET", "/__reinhardt_cancellation_probe", None, vec![]),
			)
			.await;
			*result_slot_for_task.borrow_mut() = Some(result);
		});
		for _ in 0..8 {
			crate::platform::defer_yield().await;
			if _stub.flag("signalSeen") {
				break;
			}
		}
		assert!(_stub.flag("signalSeen"));
		source.cancel();
		for _ in 0..8 {
			crate::platform::defer_yield().await;
			if result_slot.borrow().is_some() {
				break;
			}
		}

		// Assert
		assert!(_stub.flag("aborted"));
		assert!(result_slot.borrow().is_some());
	}
}
