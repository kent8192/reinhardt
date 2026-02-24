//! Core signal types and traits

use super::error::SignalError;
use std::any::TypeId;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Internal storage for signal names, supporting both static and owned strings.
/// This avoids `Box::leak` for dynamic names while keeping zero-cost for static names.
#[derive(Debug, Clone)]
enum SignalNameInner {
	/// Compile-time constant string (zero allocation)
	Static(&'static str),
	/// Dynamically created name (reference-counted, properly freed)
	Owned(Arc<str>),
}

/// Type-safe signal name wrapper
///
/// This type provides compile-time safety for signal names while still allowing
/// custom signal names when needed.
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::SignalName;
///
/// // Use built-in signal names
/// let signal_name = SignalName::PRE_SAVE;
///
/// // Create custom signal names
/// let custom = SignalName::custom("my_custom_signal");
/// ```
#[derive(Debug, Clone)]
pub struct SignalName(SignalNameInner);

impl SignalName {
	// Model signals
	/// Signal sent before saving a model instance
	pub const PRE_SAVE: Self = Self(SignalNameInner::Static("pre_save"));
	/// Signal sent after saving a model instance
	pub const POST_SAVE: Self = Self(SignalNameInner::Static("post_save"));
	/// Signal sent before deleting a model instance
	pub const PRE_DELETE: Self = Self(SignalNameInner::Static("pre_delete"));
	/// Signal sent after deleting a model instance
	pub const POST_DELETE: Self = Self(SignalNameInner::Static("post_delete"));
	/// Signal sent at the beginning of a model's initialization
	pub const PRE_INIT: Self = Self(SignalNameInner::Static("pre_init"));
	/// Signal sent at the end of a model's initialization
	pub const POST_INIT: Self = Self(SignalNameInner::Static("post_init"));
	/// Signal sent when many-to-many relationships change
	pub const M2M_CHANGED: Self = Self(SignalNameInner::Static("m2m_changed"));
	/// Signal sent when a model class is prepared
	pub const CLASS_PREPARED: Self = Self(SignalNameInner::Static("class_prepared"));

	// Migration signals
	/// Signal sent before running migrations
	pub const PRE_MIGRATE: Self = Self(SignalNameInner::Static("pre_migrate"));
	/// Signal sent after running migrations
	pub const POST_MIGRATE: Self = Self(SignalNameInner::Static("post_migrate"));

	// Request signals
	/// Signal sent when an HTTP request starts
	pub const REQUEST_STARTED: Self = Self(SignalNameInner::Static("request_started"));
	/// Signal sent when an HTTP request finishes
	pub const REQUEST_FINISHED: Self = Self(SignalNameInner::Static("request_finished"));
	/// Signal sent when an exception occurs during request handling
	pub const GOT_REQUEST_EXCEPTION: Self = Self(SignalNameInner::Static("got_request_exception"));

	// Management signals
	/// Signal sent when a configuration setting is changed
	pub const SETTING_CHANGED: Self = Self(SignalNameInner::Static("setting_changed"));

	// Database signals
	/// Signal sent before a database insert operation
	pub const DB_BEFORE_INSERT: Self = Self(SignalNameInner::Static("db_before_insert"));
	/// Signal sent after a database insert operation
	pub const DB_AFTER_INSERT: Self = Self(SignalNameInner::Static("db_after_insert"));
	/// Signal sent before a database update operation
	pub const DB_BEFORE_UPDATE: Self = Self(SignalNameInner::Static("db_before_update"));
	/// Signal sent after a database update operation
	pub const DB_AFTER_UPDATE: Self = Self(SignalNameInner::Static("db_after_update"));
	/// Signal sent before a database delete operation
	pub const DB_BEFORE_DELETE: Self = Self(SignalNameInner::Static("db_before_delete"));
	/// Signal sent after a database delete operation
	pub const DB_AFTER_DELETE: Self = Self(SignalNameInner::Static("db_after_delete"));

	/// Create a custom signal name without validation
	///
	/// Note: This requires a `'static` string to ensure the name lives long enough.
	/// For dynamic names, use `from_string()` instead.
	///
	/// For validated custom signal names, use `custom_validated()` instead.
	pub const fn custom(name: &'static str) -> Self {
		Self(SignalNameInner::Static(name))
	}

	/// Create a signal name from an owned string
	///
	/// Uses `Arc<str>` internally so the name is properly freed when no longer
	/// referenced. This avoids the memory leak caused by `Box::leak`.
	pub fn from_string(name: impl Into<Arc<str>>) -> Self {
		Self(SignalNameInner::Owned(name.into()))
	}

	/// Create a validated custom signal name
	///
	/// This method validates that the signal name:
	/// - Is not empty
	/// - Uses snake_case format
	/// - Does not conflict with reserved signal names
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::SignalName;
	///
	/// // Valid custom signal names
	/// let valid = SignalName::custom_validated("my_custom_signal").unwrap();
	///
	/// // Invalid: not snake_case
	/// assert!(SignalName::custom_validated("MySignal").is_err());
	///
	/// // Invalid: reserved name
	/// assert!(SignalName::custom_validated("pre_save").is_err());
	/// ```
	///
	/// # Errors
	///
	/// Returns `SignalError` if validation fails.
	pub fn custom_validated(name: &'static str) -> Result<Self, SignalError> {
		validate_signal_name(name)?;
		Ok(Self(SignalNameInner::Static(name)))
	}

	/// Get all reserved signal names
	///
	/// Returns a list of all built-in signal names that cannot be used
	/// for custom signals.
	pub fn reserved_names() -> &'static [&'static str] {
		&[
			"pre_save",
			"post_save",
			"pre_delete",
			"post_delete",
			"pre_init",
			"post_init",
			"m2m_changed",
			"class_prepared",
			"pre_migrate",
			"post_migrate",
			"request_started",
			"request_finished",
			"got_request_exception",
			"setting_changed",
			"db_before_insert",
			"db_after_insert",
			"db_before_update",
			"db_after_update",
			"db_before_delete",
			"db_after_delete",
		]
	}

	/// Get the string representation of this signal name
	pub fn as_str(&self) -> &str {
		match &self.0 {
			SignalNameInner::Static(s) => s,
			SignalNameInner::Owned(s) => s,
		}
	}
}

impl PartialEq for SignalName {
	fn eq(&self, other: &Self) -> bool {
		self.as_str() == other.as_str()
	}
}

impl Eq for SignalName {}

impl std::hash::Hash for SignalName {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.as_str().hash(state);
	}
}

impl fmt::Display for SignalName {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

impl From<SignalName> for String {
	fn from(name: SignalName) -> String {
		name.as_str().to_string()
	}
}

impl AsRef<str> for SignalName {
	fn as_ref(&self) -> &str {
		self.as_str()
	}
}

/// Validate a custom signal name
///
/// Checks that the signal name:
/// - Is not empty
/// - Uses snake_case format (lowercase letters, numbers, and underscores only)
/// - Does not conflict with reserved signal names
///
/// # Errors
///
/// Returns `SignalError` if validation fails.
fn validate_signal_name(name: &str) -> Result<(), SignalError> {
	// Check if empty
	if name.is_empty() {
		return Err(SignalError::new("Signal name cannot be empty"));
	}

	// Check if reserved
	if SignalName::reserved_names().contains(&name) {
		return Err(SignalError::new(format!(
			"Signal name '{}' is reserved and cannot be used for custom signals",
			name
		)));
	}

	// Check snake_case format
	// Valid: lowercase letters, numbers, underscores
	// Must start with a letter or underscore
	// Cannot have consecutive underscores
	// Cannot end with underscore
	if !name
		.chars()
		.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
	{
		return Err(SignalError::new(format!(
			"Signal name '{}' must use snake_case format (lowercase letters, numbers, and underscores only)",
			name
		)));
	}

	// Check first character
	if let Some(first) = name.chars().next()
		&& !first.is_ascii_lowercase()
		&& first != '_'
	{
		return Err(SignalError::new(format!(
			"Signal name '{}' must start with a lowercase letter or underscore",
			name
		)));
	}

	// Check for consecutive underscores
	if name.contains("__") {
		return Err(SignalError::new(format!(
			"Signal name '{}' cannot contain consecutive underscores",
			name
		)));
	}

	// Check if ends with underscore
	if name.ends_with('_') {
		return Err(SignalError::new(format!(
			"Signal name '{}' cannot end with an underscore",
			name
		)));
	}

	Ok(())
}

/// Common trait for all signal dispatchers
///
/// This trait provides a unified interface for both async and sync signals,
/// enabling generic code and easier testing.
pub trait SignalDispatcher<T: Send + Sync + 'static> {
	/// Get the number of connected receivers
	fn receiver_count(&self) -> usize;

	/// Clear all receivers
	fn disconnect_all(&self);

	/// Disconnect a receiver by dispatch_uid
	fn disconnect(&self, dispatch_uid: &str) -> bool;
}

/// Trait for asynchronous signal dispatchers
///
/// Extends SignalDispatcher with async-specific methods
#[async_trait::async_trait]
pub trait AsyncSignalDispatcher<T: Send + Sync + 'static>: SignalDispatcher<T> {
	/// Send signal to all connected receivers
	async fn send(&self, instance: T) -> Result<(), SignalError>;

	/// Send signal with sender type filtering
	async fn send_with_sender(
		&self,
		instance: T,
		sender_type_id: Option<TypeId>,
	) -> Result<(), SignalError>;

	/// Send signal robustly, catching errors
	async fn send_robust(
		&self,
		instance: T,
		sender_type_id: Option<TypeId>,
	) -> Vec<Result<(), SignalError>>;
}

/// Signal receiver function type
pub type ReceiverFn<T> = Arc<
	dyn Fn(Arc<T>) -> Pin<Box<dyn Future<Output = Result<(), SignalError>> + Send>> + Send + Sync,
>;

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_signal_name_static_constant() {
		// Arrange
		let name = SignalName::PRE_SAVE;

		// Act
		let str_repr = name.as_str();

		// Assert
		assert_eq!(str_repr, "pre_save");
	}

	#[rstest]
	fn test_signal_name_custom_static() {
		// Arrange
		let name = SignalName::custom("my_custom_signal");

		// Act
		let str_repr = name.as_str();

		// Assert
		assert_eq!(str_repr, "my_custom_signal");
	}

	#[rstest]
	fn test_signal_name_from_string_owned() {
		// Arrange
		let dynamic_name = format!("dynamic_signal_{}", 42);

		// Act
		let name = SignalName::from_string(dynamic_name.clone());

		// Assert
		assert_eq!(name.as_str(), "dynamic_signal_42");
	}

	#[rstest]
	fn test_signal_name_from_string_equality_with_static() {
		// Arrange
		let static_name = SignalName::custom("test_signal");
		let owned_name = SignalName::from_string("test_signal");

		// Act & Assert
		assert_eq!(static_name, owned_name);
	}

	#[rstest]
	fn test_signal_name_from_string_hash_consistency() {
		// Arrange
		use std::collections::HashSet;
		let static_name = SignalName::custom("test_signal");
		let owned_name = SignalName::from_string("test_signal");

		// Act
		let mut set = HashSet::new();
		set.insert(static_name);

		// Assert
		assert!(set.contains(&owned_name));
	}

	#[rstest]
	fn test_signal_name_from_string_clone() {
		// Arrange
		let name = SignalName::from_string("cloneable_signal");

		// Act
		let cloned = name.clone();

		// Assert
		assert_eq!(name, cloned);
		assert_eq!(cloned.as_str(), "cloneable_signal");
	}

	#[rstest]
	fn test_signal_name_display() {
		// Arrange
		let static_name = SignalName::PRE_SAVE;
		let owned_name = SignalName::from_string("dynamic_signal");

		// Act & Assert
		assert_eq!(format!("{}", static_name), "pre_save");
		assert_eq!(format!("{}", owned_name), "dynamic_signal");
	}

	#[rstest]
	fn test_signal_name_into_string() {
		// Arrange
		let name = SignalName::from_string("convertible");

		// Act
		let s: String = name.into();

		// Assert
		assert_eq!(s, "convertible");
	}

	#[rstest]
	fn test_signal_name_as_ref() {
		// Arrange
		let name = SignalName::from_string("referenceable");

		// Act
		let s: &str = name.as_ref();

		// Assert
		assert_eq!(s, "referenceable");
	}
}
