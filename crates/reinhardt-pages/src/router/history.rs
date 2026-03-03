//! History API Integration for client-side routing.
//!
//! This module provides integration with the browser's History API
//! for navigation without full page reloads.

use std::collections::HashMap;

/// The type of navigation that occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationType {
	/// Navigation via `pushState` (new history entry).
	Push,
	/// Navigation via `replaceState` (replace current entry).
	Replace,
	/// Navigation via browser back/forward buttons.
	Pop,
	/// Initial page load.
	Initial,
}

/// State object stored in the history entry.
#[derive(Debug, Clone, Default)]
pub struct HistoryState {
	/// The path of this history entry.
	pub path: String,
	/// Route parameters extracted from the path.
	pub params: HashMap<String, String>,
	/// The name of the matched route (if named).
	pub route_name: Option<String>,
	/// Custom data associated with this entry.
	pub data: HashMap<String, String>,
	/// Scroll position to restore.
	pub scroll_position: Option<(i32, i32)>,
}

impl HistoryState {
	/// Creates a new history state.
	pub fn new(path: impl Into<String>) -> Self {
		Self {
			path: path.into(),
			params: HashMap::new(),
			route_name: None,
			data: HashMap::new(),
			scroll_position: None,
		}
	}

	/// Sets the route parameters.
	pub fn with_params(mut self, params: HashMap<String, String>) -> Self {
		self.params = params;
		self
	}

	/// Sets the route name.
	pub fn with_route_name(mut self, name: impl Into<String>) -> Self {
		self.route_name = Some(name.into());
		self
	}

	/// Adds custom data.
	pub fn with_data(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
		self.data.insert(key.into(), value.into());
		self
	}

	/// Sets the scroll position.
	pub fn with_scroll(mut self, x: i32, y: i32) -> Self {
		self.scroll_position = Some((x, y));
		self
	}

	/// Serializes the state to a JSON string.
	pub fn to_json(&self) -> Result<String, serde_json::Error> {
		serde_json::to_string(&HistoryStateJson {
			path: self.path.clone(),
			params: self.params.clone(),
			route_name: self.route_name.clone(),
			data: self.data.clone(),
			scroll_x: self.scroll_position.map(|(x, _)| x),
			scroll_y: self.scroll_position.map(|(_, y)| y),
		})
	}

	/// Deserializes the state from a JSON string.
	pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
		let parsed: HistoryStateJson = serde_json::from_str(json)?;
		Ok(Self {
			path: parsed.path,
			params: parsed.params,
			route_name: parsed.route_name,
			data: parsed.data,
			scroll_position: match (parsed.scroll_x, parsed.scroll_y) {
				(Some(x), Some(y)) => Some((x, y)),
				_ => None,
			},
		})
	}
}

#[derive(serde::Serialize, serde::Deserialize)]
struct HistoryStateJson {
	path: String,
	params: HashMap<String, String>,
	route_name: Option<String>,
	data: HashMap<String, String>,
	scroll_x: Option<i32>,
	scroll_y: Option<i32>,
}

/// Pushes a new state to the browser history.
#[cfg(target_arch = "wasm32")]
pub(super) fn push_state(state: &HistoryState) -> Result<(), String> {
	use wasm_bindgen::JsValue;

	let window = web_sys::window().ok_or("Window not available")?;
	let history = window.history().map_err(|_| "History not available")?;

	let state_json = state.to_json().map_err(|e| e.to_string())?;
	let js_state = JsValue::from_str(&state_json);

	history
		.push_state_with_url(&js_state, "", Some(&state.path))
		.map_err(|_| "Failed to push state".to_string())
}

/// Non-WASM version for testing.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn push_state(_state: &HistoryState) -> Result<(), String> {
	Ok(())
}

/// Replaces the current state in the browser history.
#[cfg(target_arch = "wasm32")]
pub(super) fn replace_state(state: &HistoryState) -> Result<(), String> {
	use wasm_bindgen::JsValue;

	let window = web_sys::window().ok_or("Window not available")?;
	let history = window.history().map_err(|_| "History not available")?;

	let state_json = state.to_json().map_err(|e| e.to_string())?;
	let js_state = JsValue::from_str(&state_json);

	history
		.replace_state_with_url(&js_state, "", Some(&state.path))
		.map_err(|_| "Failed to replace state".to_string())
}

/// Non-WASM version for testing.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn replace_state(_state: &HistoryState) -> Result<(), String> {
	Ok(())
}

/// Navigates back in the browser history.
#[cfg(target_arch = "wasm32")]
// Allow dead_code: pub(super) API reserved for future router navigation features
#[allow(dead_code)]
pub(super) fn go_back() -> Result<(), String> {
	let window = web_sys::window().ok_or("Window not available")?;
	let history = window.history().map_err(|_| "History not available")?;

	history.back().map_err(|_| "Failed to go back".to_string())
}

/// Non-WASM version for testing.
#[cfg(not(target_arch = "wasm32"))]
// Allow dead_code: pub(super) API reserved for future router navigation features
#[allow(dead_code)]
pub(super) fn go_back() -> Result<(), String> {
	Ok(())
}

/// Navigates forward in the browser history.
#[cfg(target_arch = "wasm32")]
// Allow dead_code: pub(super) API reserved for future router navigation features
#[allow(dead_code)]
pub(super) fn go_forward() -> Result<(), String> {
	let window = web_sys::window().ok_or("Window not available")?;
	let history = window.history().map_err(|_| "History not available")?;

	history
		.forward()
		.map_err(|_| "Failed to go forward".to_string())
}

/// Non-WASM version for testing.
#[cfg(not(target_arch = "wasm32"))]
// Allow dead_code: pub(super) API reserved for future router navigation features
#[allow(dead_code)]
pub(super) fn go_forward() -> Result<(), String> {
	Ok(())
}

/// Navigates to a specific position in the history.
#[cfg(target_arch = "wasm32")]
// Allow dead_code: pub(super) API reserved for future router navigation features
#[allow(dead_code)]
pub(super) fn go(delta: i32) -> Result<(), String> {
	let window = web_sys::window().ok_or("Window not available")?;
	let history = window.history().map_err(|_| "History not available")?;

	history
		.go_with_delta(delta)
		.map_err(|_| "Failed to navigate".to_string())
}

/// Non-WASM version for testing.
#[cfg(not(target_arch = "wasm32"))]
// Allow dead_code: pub(super) API reserved for future router navigation features
#[allow(dead_code)]
pub(super) fn go(_delta: i32) -> Result<(), String> {
	Ok(())
}

/// Gets the current pathname from the browser.
#[cfg(target_arch = "wasm32")]
pub(super) fn current_path() -> Result<String, String> {
	let window = web_sys::window().ok_or("Window not available")?;
	let location = window.location();
	location
		.pathname()
		.map_err(|_| "Failed to get pathname".to_string())
}

/// Non-WASM version for testing.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn current_path() -> Result<String, String> {
	Ok("/".to_string())
}

/// Gets the current search query from the browser.
#[cfg(target_arch = "wasm32")]
// Allow dead_code: pub(super) API reserved for future router navigation features
#[allow(dead_code)]
pub(super) fn current_search() -> Result<String, String> {
	let window = web_sys::window().ok_or("Window not available")?;
	let location = window.location();
	location
		.search()
		.map_err(|_| "Failed to get search".to_string())
}

/// Non-WASM version for testing.
#[cfg(not(target_arch = "wasm32"))]
// Allow dead_code: pub(super) API reserved for future router navigation features
#[allow(dead_code)]
pub(super) fn current_search() -> Result<String, String> {
	Ok(String::new())
}

/// Gets the current hash from the browser.
#[cfg(target_arch = "wasm32")]
// Allow dead_code: pub(super) API reserved for future router navigation features
#[allow(dead_code)]
pub(super) fn current_hash() -> Result<String, String> {
	let window = web_sys::window().ok_or("Window not available")?;
	let location = window.location();
	location
		.hash()
		.map_err(|_| "Failed to get hash".to_string())
}

/// Non-WASM version for testing.
#[cfg(not(target_arch = "wasm32"))]
// Allow dead_code: pub(super) API reserved for future router navigation features
#[allow(dead_code)]
pub(super) fn current_hash() -> Result<String, String> {
	Ok(String::new())
}

/// Sets up a popstate event listener that triggers when browser back/forward is used.
///
/// The callback receives the current path and an optional `HistoryState` if one was
/// stored in the history entry.
///
/// # Example
///
/// ```ignore
/// setup_popstate_listener(|path, state| {
///     println!("Navigated to: {}", path);
///     if let Some(s) = state {
///         println!("Route: {:?}", s.route_name);
///     }
/// })?;
/// ```
///
/// # Returns
///
/// On WASM, returns a `Closure` that must be kept alive for the listener to work.
/// Call `.forget()` on the closure to keep it active for the lifetime of the page.
///
/// # Errors
///
/// Returns an error if the window object is not available.
#[cfg(target_arch = "wasm32")]
pub fn setup_popstate_listener<F>(
	callback: F,
) -> Result<wasm_bindgen::closure::Closure<dyn FnMut(web_sys::PopStateEvent)>, wasm_bindgen::JsValue>
where
	F: Fn(String, Option<HistoryState>) + 'static,
{
	use wasm_bindgen::JsCast;
	use wasm_bindgen::prelude::*;

	let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window object"))?;

	let closure =
		wasm_bindgen::closure::Closure::wrap(Box::new(move |event: web_sys::PopStateEvent| {
			// Get current path from location
			let path = web_sys::window()
				.and_then(|w| w.location().pathname().ok())
				.unwrap_or_else(|| "/".to_string());

			// Try to restore state from history
			let state = event
				.state()
				.as_string()
				.and_then(|s: String| HistoryState::from_json(&s).ok());

			callback(path, state);
		}) as Box<dyn FnMut(_)>);

	window.add_event_listener_with_callback("popstate", closure.as_ref().unchecked_ref())?;

	Ok(closure)
}

/// Non-WASM version for testing.
#[cfg(not(target_arch = "wasm32"))]
pub fn setup_popstate_listener<F>(_callback: F) -> Result<(), String>
where
	F: Fn(String, Option<HistoryState>) + 'static,
{
	// No-op on non-WASM targets
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_navigation_type() {
		assert_ne!(NavigationType::Push, NavigationType::Replace);
		assert_eq!(NavigationType::Pop, NavigationType::Pop);
	}

	#[test]
	fn test_history_state_new() {
		let state = HistoryState::new("/users/42/");
		assert_eq!(state.path, "/users/42/");
		assert!(state.params.is_empty());
		assert!(state.route_name.is_none());
	}

	#[test]
	fn test_history_state_builder() {
		let mut params = HashMap::new();
		params.insert("id".to_string(), "42".to_string());

		let state = HistoryState::new("/users/42/")
			.with_params(params)
			.with_route_name("user_detail")
			.with_data("ref", "home")
			.with_scroll(0, 500);

		assert_eq!(state.path, "/users/42/");
		assert_eq!(state.params.get("id"), Some(&"42".to_string()));
		assert_eq!(state.route_name, Some("user_detail".to_string()));
		assert_eq!(state.data.get("ref"), Some(&"home".to_string()));
		assert_eq!(state.scroll_position, Some((0, 500)));
	}

	#[test]
	fn test_history_state_json_roundtrip() {
		let mut params = HashMap::new();
		params.insert("id".to_string(), "42".to_string());

		let state = HistoryState::new("/users/42/")
			.with_params(params)
			.with_route_name("user_detail");

		let json = state.to_json().unwrap();
		let restored = HistoryState::from_json(&json).unwrap();

		assert_eq!(restored.path, state.path);
		assert_eq!(restored.params, state.params);
		assert_eq!(restored.route_name, state.route_name);
	}

	#[test]
	fn test_push_state_non_wasm() {
		let state = HistoryState::new("/test/");
		assert!(push_state(&state).is_ok());
	}

	#[test]
	fn test_replace_state_non_wasm() {
		let state = HistoryState::new("/test/");
		assert!(replace_state(&state).is_ok());
	}

	#[test]
	fn test_navigation_functions_non_wasm() {
		assert!(go_back().is_ok());
		assert!(go_forward().is_ok());
		assert!(go(-1).is_ok());
	}

	#[test]
	fn test_current_location_non_wasm() {
		assert_eq!(current_path().unwrap(), "/");
		assert_eq!(current_search().unwrap(), "");
		assert_eq!(current_hash().unwrap(), "");
	}
}
