use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

/// Default maximum number of entries allowed in a `TemplateContext`.
const DEFAULT_MAX_ENTRIES: usize = 1_000;

/// Error returned when a `TemplateContext` operation fails.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ContextError {
	/// The context has reached its maximum entry capacity.
	#[error("context capacity exceeded: limit is {limit}, current size is {current}")]
	CapacityExceeded {
		/// The maximum allowed entries.
		limit: usize,
		/// The current number of entries.
		current: usize,
	},
}

/// A context for template rendering.
///
/// This structure provides a type-safe way to pass data to templates,
/// replacing the generic `HashMap<K, V>` pattern.
///
/// A configurable maximum entry count prevents unbounded memory growth
/// from user-controlled data. The default limit is 1,000 entries.
///
/// # Examples
///
/// ```
/// use reinhardt_shortcuts::TemplateContext;
///
/// let mut context = TemplateContext::new();
/// context.insert("user", "Alice");
/// context.insert("count", 42);
/// ```
///
/// ```
/// use reinhardt_shortcuts::TemplateContext;
///
/// // Create a context with a custom capacity limit
/// let mut context = TemplateContext::with_capacity_limit(50);
/// context.insert("key", "value");
/// ```
#[derive(Debug, Clone)]
pub struct TemplateContext {
	inner: HashMap<String, Value>,
	max_entries: usize,
}

impl Default for TemplateContext {
	fn default() -> Self {
		Self {
			inner: HashMap::new(),
			max_entries: DEFAULT_MAX_ENTRIES,
		}
	}
}

impl TemplateContext {
	/// Creates a new empty template context with the default capacity limit.
	pub fn new() -> Self {
		Self::default()
	}

	/// Creates a new empty template context with a custom capacity limit.
	///
	/// # Arguments
	///
	/// * `max_entries` - The maximum number of entries allowed in the context.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_shortcuts::TemplateContext;
	///
	/// let mut context = TemplateContext::with_capacity_limit(10);
	/// ```
	pub fn with_capacity_limit(max_entries: usize) -> Self {
		Self {
			inner: HashMap::new(),
			max_entries,
		}
	}

	/// Returns the maximum number of entries allowed in the context.
	pub fn max_entries(&self) -> usize {
		self.max_entries
	}

	/// Returns the current number of entries in the context.
	pub fn len(&self) -> usize {
		self.inner.len()
	}

	/// Returns `true` if the context contains no entries.
	pub fn is_empty(&self) -> bool {
		self.inner.is_empty()
	}

	/// Inserts a key-value pair into the context.
	///
	/// If the context is at capacity and the key does not already exist,
	/// the insertion is silently skipped and a warning is logged. Use
	/// [`try_insert`](Self::try_insert) for explicit error handling.
	///
	/// Replacing an existing key does not count against the capacity limit.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_shortcuts::TemplateContext;
	///
	/// let mut context = TemplateContext::new();
	/// context.insert("name", "Bob");
	/// context.insert("age", 30);
	/// ```
	pub fn insert<K, V>(&mut self, key: K, value: V)
	where
		K: Into<String>,
		V: Serialize,
	{
		if let Err(e) = self.try_insert(key, value) {
			tracing::warn!("{}", e);
		}
	}

	/// Tries to insert a key-value pair into the context.
	///
	/// Returns an error if the context is at capacity and the key does not
	/// already exist. Replacing an existing key always succeeds.
	///
	/// # Errors
	///
	/// Returns [`ContextError::CapacityExceeded`] when the insertion would
	/// exceed the configured maximum entry count.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_shortcuts::TemplateContext;
	///
	/// let mut context = TemplateContext::with_capacity_limit(2);
	/// assert!(context.try_insert("a", 1).is_ok());
	/// assert!(context.try_insert("b", 2).is_ok());
	/// assert!(context.try_insert("c", 3).is_err()); // capacity exceeded
	///
	/// // Overwriting an existing key always succeeds
	/// assert!(context.try_insert("a", 10).is_ok());
	/// ```
	pub fn try_insert<K, V>(&mut self, key: K, value: V) -> Result<(), ContextError>
	where
		K: Into<String>,
		V: Serialize,
	{
		let key = key.into();
		// Allow overwriting existing keys without counting against the limit
		if !self.inner.contains_key(&key) && self.inner.len() >= self.max_entries {
			return Err(ContextError::CapacityExceeded {
				limit: self.max_entries,
				current: self.inner.len(),
			});
		}
		let value = serde_json::to_value(value).unwrap_or(Value::Null);
		self.inner.insert(key, value);
		Ok(())
	}

	/// Creates a context from a HashMap.
	///
	/// If the map contains more entries than the default capacity limit,
	/// only the first entries up to the limit are included and a warning
	/// is logged.
	///
	/// # Examples
	///
	/// ```
	/// use std::collections::HashMap;
	/// use reinhardt_shortcuts::TemplateContext;
	///
	/// let mut map = HashMap::new();
	/// map.insert("key", "value");
	///
	/// let context = TemplateContext::from_map(map);
	/// ```
	pub fn from_map<K, V>(map: HashMap<K, V>) -> Self
	where
		K: AsRef<str>,
		V: Serialize,
	{
		let mut ctx = Self::new();
		for (k, v) in map {
			ctx.insert(k.as_ref(), v);
		}
		ctx
	}
}

impl<K, V> From<HashMap<K, V>> for TemplateContext
where
	K: AsRef<str>,
	V: Serialize,
{
	fn from(map: HashMap<K, V>) -> Self {
		Self::from_map(map)
	}
}
