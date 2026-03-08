//! Request lifecycle events

use super::core::SignalName;
use super::registry::get_signal;
use super::signal::Signal;
use std::collections::HashMap;

/// Request started signal - sent when an HTTP request starts
#[derive(Debug, Clone)]
pub struct RequestStartedEvent {
	/// Request environment variables and metadata.
	pub environ: HashMap<String, String>,
}

impl RequestStartedEvent {
	/// Creates a new request-started event with an empty environment.
	pub fn new() -> Self {
		Self {
			environ: HashMap::new(),
		}
	}

	/// Sets the request environment variables.
	pub fn with_environ(mut self, environ: HashMap<String, String>) -> Self {
		self.environ = environ;
		self
	}
}

impl Default for RequestStartedEvent {
	fn default() -> Self {
		Self::new()
	}
}

/// Returns the request-started signal.
pub fn request_started() -> Signal<RequestStartedEvent> {
	get_signal::<RequestStartedEvent>(SignalName::REQUEST_STARTED)
}

/// Request finished signal - sent when an HTTP request finishes
#[derive(Debug, Clone)]
pub struct RequestFinishedEvent {
	/// Request environment variables and metadata.
	pub environ: HashMap<String, String>,
}

impl RequestFinishedEvent {
	/// Creates a new request-finished event with an empty environment.
	pub fn new() -> Self {
		Self {
			environ: HashMap::new(),
		}
	}

	/// Sets the request environment variables.
	pub fn with_environ(mut self, environ: HashMap<String, String>) -> Self {
		self.environ = environ;
		self
	}
}

impl Default for RequestFinishedEvent {
	fn default() -> Self {
		Self::new()
	}
}

/// Returns the request-finished signal.
pub fn request_finished() -> Signal<RequestFinishedEvent> {
	get_signal::<RequestFinishedEvent>(SignalName::REQUEST_FINISHED)
}

/// Got request exception signal - sent when an exception occurs during request handling
#[derive(Debug, Clone)]
pub struct GotRequestExceptionEvent {
	/// The error message describing the exception.
	pub error_message: String,
	/// Additional request information at the time of the exception.
	pub request_info: HashMap<String, String>,
}

impl GotRequestExceptionEvent {
	/// Creates a new exception event with the given error message.
	pub fn new(error_message: impl Into<String>) -> Self {
		Self {
			error_message: error_message.into(),
			request_info: HashMap::new(),
		}
	}

	/// Sets additional request information for the exception event.
	pub fn with_request_info(mut self, request_info: HashMap<String, String>) -> Self {
		self.request_info = request_info;
		self
	}
}

/// Returns the got-request-exception signal.
pub fn got_request_exception() -> Signal<GotRequestExceptionEvent> {
	get_signal::<GotRequestExceptionEvent>(SignalName::GOT_REQUEST_EXCEPTION)
}

/// Setting changed signal - sent when a setting is changed
#[derive(Debug, Clone)]
pub struct SettingChangedEvent {
	/// Name of the setting that changed.
	pub setting_name: String,
	/// Previous value of the setting, if any.
	pub old_value: Option<String>,
	/// New value of the setting.
	pub new_value: String,
}

impl SettingChangedEvent {
	/// Creates a new setting-changed event.
	pub fn new(
		setting_name: impl Into<String>,
		old_value: Option<String>,
		new_value: impl Into<String>,
	) -> Self {
		Self {
			setting_name: setting_name.into(),
			old_value,
			new_value: new_value.into(),
		}
	}
}

/// Returns the setting-changed signal.
pub fn setting_changed() -> Signal<SettingChangedEvent> {
	get_signal::<SettingChangedEvent>(SignalName::SETTING_CHANGED)
}
