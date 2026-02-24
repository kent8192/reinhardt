//! Values wrapper struct.

use super::Value;

/// Wrapper struct for collected query parameters.
///
/// This struct holds the values collected during SQL generation,
/// which will be used as bind parameters when executing the query.
///
/// # Example
///
/// ```rust
/// use reinhardt_query::{Value, Values};
///
/// let values = Values(vec![
///     Value::Int(Some(42)),
///     Value::String(Some(Box::new("hello".to_string()))),
/// ]);
///
/// assert_eq!(values.len(), 2);
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Values(pub Vec<Value>);

impl Values {
	/// Creates a new empty `Values` collection.
	#[must_use]
	pub fn new() -> Self {
		Self(Vec::new())
	}

	/// Creates a `Values` collection with the specified capacity.
	#[must_use]
	pub fn with_capacity(capacity: usize) -> Self {
		Self(Vec::with_capacity(capacity))
	}

	/// Returns the number of values in this collection.
	#[must_use]
	pub fn len(&self) -> usize {
		self.0.len()
	}

	/// Returns `true` if this collection is empty.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
	}

	/// Adds a value to this collection and returns its 1-based index.
	///
	/// This is useful for generating placeholder indices like `$1`, `$2`, etc.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::{Value, Values};
	///
	/// let mut values = Values::new();
	/// let idx1 = values.push(Value::Int(Some(1)));
	/// let idx2 = values.push(Value::Int(Some(2)));
	///
	/// assert_eq!(idx1, 1);
	/// assert_eq!(idx2, 2);
	/// ```
	pub fn push(&mut self, value: Value) -> usize {
		self.0.push(value);
		self.0.len()
	}

	/// Returns an iterator over references to the values.
	pub fn iter(&self) -> impl Iterator<Item = &Value> {
		self.0.iter()
	}

	/// Consumes this collection and returns the underlying vector.
	#[must_use]
	pub fn into_inner(self) -> Vec<Value> {
		self.0
	}
}

impl IntoIterator for Values {
	type Item = Value;
	type IntoIter = std::vec::IntoIter<Value>;

	fn into_iter(self) -> Self::IntoIter {
		self.0.into_iter()
	}
}

impl<'a> IntoIterator for &'a Values {
	type Item = &'a Value;
	type IntoIter = std::slice::Iter<'a, Value>;

	fn into_iter(self) -> Self::IntoIter {
		self.0.iter()
	}
}

impl From<Vec<Value>> for Values {
	fn from(values: Vec<Value>) -> Self {
		Self(values)
	}
}

impl From<Values> for Vec<Value> {
	fn from(values: Values) -> Self {
		values.0
	}
}

impl std::ops::Deref for Values {
	type Target = [Value];

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl std::ops::Index<usize> for Values {
	type Output = Value;

	fn index(&self, index: usize) -> &Self::Output {
		&self.0[index]
	}
}
