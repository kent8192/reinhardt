//! JS interop for `window.fetch` override and restoration.

#[cfg(wasm)]
mod wasm_impl {
	use std::cell::RefCell;
	use std::rc::Rc;

	use js_sys::{self, Promise, Reflect};
	use wasm_bindgen::JsCast;
	use wasm_bindgen::prelude::*;
	use wasm_bindgen_futures::JsFuture;
	use web_sys::{Headers, Request, Response, ResponseInit};

	use crate::msw::handler::{ErasedHandler, InterceptedRequest};
	use crate::msw::recorder::{RecordedRequest, RequestRecorder};
	use crate::msw::response::MockResponse;

	/// Behavior when a request matches no handler.
	#[derive(Debug, Clone)]
	pub(crate) enum UnhandledPolicy {
		Error,
		Passthrough,
		Warn,
	}

	/// Save the original `window.fetch` and return it.
	pub(crate) fn save_original_fetch() -> JsValue {
		let window = web_sys::window().expect("MSW: no window object available");
		Reflect::get(&window, &"fetch".into()).expect("MSW: window.fetch not found")
	}

	/// Install a Rust-backed fetch replacement on `window`.
	pub(crate) fn install_fetch_override(
		handlers: Rc<RefCell<Vec<Box<dyn ErasedHandler>>>>,
		recorder: Rc<RefCell<RequestRecorder>>,
		policy: UnhandledPolicy,
		original_fetch: JsValue,
	) -> Closure<dyn FnMut(JsValue, JsValue) -> Promise> {
		let original_for_passthrough = original_fetch.clone();

		let closure = Closure::wrap(Box::new(move |input: JsValue, init: JsValue| {
			let handlers = handlers.clone();
			let recorder = recorder.clone();
			let policy = policy.clone();
			let original = original_for_passthrough.clone();

			wasm_bindgen_futures::future_to_promise(async move {
				let intercepted = extract_request_info(&input, &init)?;

				// Record the request
				recorder.borrow_mut().record(RecordedRequest {
					url: intercepted.url.clone(),
					method: intercepted.method.clone(),
					headers: intercepted.headers.clone(),
					body: intercepted.body.clone(),
					timestamp: js_sys::Date::now(),
				});

				// Find matching handler
				let handler_result = {
					let handlers_ref = handlers.borrow();
					handlers_ref
						.iter()
						.find(|h| h.matches(&intercepted))
						.map(|h| {
							let is_network_error = h.is_network_error();
							let delay = h.delay();
							let response = h.respond(&intercepted);
							(is_network_error, delay, response)
						})
				};

				match handler_result {
					Some((true, _, _)) => Err(JsValue::from(js_sys::TypeError::new(&format!(
						"MSW: Simulated network error for {} {}",
						intercepted.method, intercepted.url
					)))),
					Some((false, delay, Some(mock_response))) => {
						if let Some(duration) = delay {
							gloo_timers::future::sleep(duration).await;
						}
						build_js_response(&mock_response)
					}
					Some((false, _, None)) => {
						let error_response = MockResponse {
							status: 400,
							headers: {
								let mut h = std::collections::HashMap::new();
								h.insert(
									"content-type".to_string(),
									"application/json".to_string(),
								);
								h
							},
							body: format!(
								r#"{{"error":"MSW: Failed to process request for {} {}"}}"#,
								intercepted.method, intercepted.url
							),
						};
						build_js_response(&error_response)
					}
					None => match policy {
						UnhandledPolicy::Error => {
							Err(JsValue::from(js_sys::TypeError::new(&format!(
								"MSW: No handler for {} {} (UnhandledPolicy::Error)",
								intercepted.method, intercepted.url
							))))
						}
						UnhandledPolicy::Warn => {
							web_sys::console::warn_1(
								&format!(
									"MSW: No handler for {} {}, passing through",
									intercepted.method, intercepted.url
								)
								.into(),
							);
							call_original_fetch(&original, &input, &init).await
						}
						UnhandledPolicy::Passthrough => {
							call_original_fetch(&original, &input, &init).await
						}
					},
				}
			})
		}) as Box<dyn FnMut(JsValue, JsValue) -> Promise>);

		let window = web_sys::window().expect("MSW: no window");
		Reflect::set(&window, &"fetch".into(), closure.as_ref())
			.expect("MSW: failed to set window.fetch");

		closure
	}

	/// Restore the original `window.fetch`.
	pub(crate) fn restore_fetch(original: &JsValue) {
		let window = web_sys::window().expect("MSW: no window");
		Reflect::set(&window, &"fetch".into(), original)
			.expect("MSW: failed to restore window.fetch");
	}

	fn extract_request_info(
		input: &JsValue,
		init: &JsValue,
	) -> Result<InterceptedRequest, JsValue> {
		let (url, method, headers, body) = if input.is_instance_of::<Request>() {
			let request: Request = input.clone().unchecked_into();
			let url = request.url();
			let method = request.method();
			let headers = extract_headers(&request.headers());
			let body = if !init.is_undefined() && !init.is_null() {
				Reflect::get(init, &"body".into())
					.ok()
					.and_then(|b| b.as_string())
			} else {
				None
			};
			(url, method, headers, body)
		} else {
			let url = input
				.as_string()
				.ok_or_else(|| JsValue::from_str("MSW: fetch input must be a string or Request"))?;
			let method = if !init.is_undefined() && !init.is_null() {
				Reflect::get(init, &"method".into())
					.ok()
					.and_then(|m| m.as_string())
					.unwrap_or_else(|| "GET".to_string())
			} else {
				"GET".to_string()
			};
			let headers = if !init.is_undefined() && !init.is_null() {
				Reflect::get(init, &"headers".into())
					.ok()
					.and_then(|h| {
						if h.is_undefined() || h.is_null() {
							None
						} else if h.is_instance_of::<Headers>() {
							Some(extract_headers(&h.unchecked_into()))
						} else {
							// Normalize plain objects and arrays by constructing a Headers
							// instance, which accepts both `Record<string, string>` and
							// `[string, string][]` forms per the Fetch spec.
							Headers::new_with_str_sequence_sequence(&h)
								.or_else(|_| Headers::new_with_str_mapping(&h))
								.ok()
								.map(|normalized| extract_headers(&normalized))
						}
					})
					.unwrap_or_default()
			} else {
				std::collections::HashMap::new()
			};
			let body = if !init.is_undefined() && !init.is_null() {
				Reflect::get(init, &"body".into())
					.ok()
					.and_then(|b| b.as_string())
			} else {
				None
			};
			(url, method, headers, body)
		};

		Ok(InterceptedRequest {
			url,
			method,
			headers,
			body,
		})
	}

	fn extract_headers(headers: &Headers) -> std::collections::HashMap<String, String> {
		let mut map = std::collections::HashMap::new();
		for name in [
			"content-type",
			"authorization",
			"accept",
			"x-csrftoken",
			"x-requested-with",
		] {
			if let Ok(Some(value)) = headers.get(name) {
				map.insert(name.to_string(), value);
			}
		}
		map
	}

	fn build_js_response(mock: &MockResponse) -> Result<JsValue, JsValue> {
		let init = ResponseInit::new();
		init.set_status(mock.status);

		let headers = Headers::new()?;
		for (key, value) in &mock.headers {
			headers.set(key, value)?;
		}
		init.set_headers(&headers);

		let response = Response::new_with_opt_str_and_init(
			if mock.body.is_empty() {
				None
			} else {
				Some(&mock.body)
			},
			&init,
		)?;

		Ok(response.into())
	}

	async fn call_original_fetch(
		original: &JsValue,
		input: &JsValue,
		init: &JsValue,
	) -> Result<JsValue, JsValue> {
		let func: &js_sys::Function = original.unchecked_ref();
		// Use the window (or globalThis) as the receiver to avoid "Illegal invocation"
		// errors that occur when calling `window.fetch` with `this = null`.
		let receiver = web_sys::window()
			.map(JsValue::from)
			.unwrap_or_else(js_sys::global);
		let result = if !init.is_undefined() && !init.is_null() {
			func.call2(&receiver, input, init)?
		} else {
			func.call1(&receiver, input)?
		};
		let promise: Promise = result.unchecked_into();
		JsFuture::from(promise).await
	}
}

#[cfg(wasm)]
pub(crate) use wasm_impl::*;
