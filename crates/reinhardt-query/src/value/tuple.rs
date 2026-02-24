//! Value tuple definitions.

use super::Value;

/// Represents a tuple of values.
///
/// This enum is used for operations that require multiple values,
/// such as IN clauses or multi-column comparisons. It provides
/// variants optimized for common tuple sizes (1-3 elements) and
/// a general variant for larger tuples.
///
/// # Example
///
/// ```rust
/// use reinhardt_query::{Value, ValueTuple};
///
/// // Single value tuple
/// let single = ValueTuple::One(Value::Int(Some(1)));
///
/// // Two value tuple
/// let pair = ValueTuple::Two(
///     Value::Int(Some(1)),
///     Value::String(Some(Box::new("hello".to_string()))),
/// );
///
/// // Many values
/// let many = ValueTuple::Many(vec![
///     Value::Int(Some(1)),
///     Value::Int(Some(2)),
///     Value::Int(Some(3)),
///     Value::Int(Some(4)),
/// ]);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub enum ValueTuple {
	/// Single value tuple
	One(Value),
	/// Two value tuple
	Two(Value, Value),
	/// Three value tuple
	Three(Value, Value, Value),
	/// Tuple with more than three values
	Many(Vec<Value>),
}

impl ValueTuple {
	/// Returns the number of values in this tuple.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::{Value, ValueTuple};
	///
	/// assert_eq!(ValueTuple::One(Value::Int(Some(1))).len(), 1);
	/// assert_eq!(ValueTuple::Two(Value::Int(Some(1)), Value::Int(Some(2))).len(), 2);
	/// ```
	#[must_use]
	pub fn len(&self) -> usize {
		match self {
			Self::One(_) => 1,
			Self::Two(_, _) => 2,
			Self::Three(_, _, _) => 3,
			Self::Many(v) => v.len(),
		}
	}

	/// Returns `true` if this tuple is empty.
	///
	/// Note: This is only possible with `Many(vec![])`.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		match self {
			Self::Many(v) => v.is_empty(),
			_ => false,
		}
	}

	/// Converts this tuple into a vector of values.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::{Value, ValueTuple};
	///
	/// let tuple = ValueTuple::Two(Value::Int(Some(1)), Value::Int(Some(2)));
	/// let values = tuple.into_vec();
	/// assert_eq!(values.len(), 2);
	/// ```
	#[must_use]
	pub fn into_vec(self) -> Vec<Value> {
		match self {
			Self::One(v) => vec![v],
			Self::Two(v1, v2) => vec![v1, v2],
			Self::Three(v1, v2, v3) => vec![v1, v2, v3],
			Self::Many(v) => v,
		}
	}

	/// Returns an iterator over references to the values in this tuple.
	pub fn iter(&self) -> ValueTupleIter<'_> {
		ValueTupleIter {
			tuple: self,
			index: 0,
		}
	}
}

impl IntoIterator for ValueTuple {
	type Item = Value;
	type IntoIter = std::vec::IntoIter<Value>;

	fn into_iter(self) -> Self::IntoIter {
		self.into_vec().into_iter()
	}
}

/// Iterator over references to values in a `ValueTuple`.
pub struct ValueTupleIter<'a> {
	tuple: &'a ValueTuple,
	index: usize,
}

impl<'a> Iterator for ValueTupleIter<'a> {
	type Item = &'a Value;

	fn next(&mut self) -> Option<Self::Item> {
		let result = match self.tuple {
			ValueTuple::One(v) if self.index == 0 => Some(v),
			ValueTuple::Two(v1, v2) => match self.index {
				0 => Some(v1),
				1 => Some(v2),
				_ => None,
			},
			ValueTuple::Three(v1, v2, v3) => match self.index {
				0 => Some(v1),
				1 => Some(v2),
				2 => Some(v3),
				_ => None,
			},
			ValueTuple::Many(v) => v.get(self.index),
			_ => None,
		};
		if result.is_some() {
			self.index += 1;
		}
		result
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let remaining = self.tuple.len().saturating_sub(self.index);
		(remaining, Some(remaining))
	}
}

impl ExactSizeIterator for ValueTupleIter<'_> {}

// Conversion traits for creating ValueTuple from Rust tuples
impl<V: Into<Value>> From<V> for ValueTuple {
	fn from(v: V) -> Self {
		Self::One(v.into())
	}
}

impl<V1, V2> From<(V1, V2)> for ValueTuple
where
	V1: Into<Value>,
	V2: Into<Value>,
{
	fn from((v1, v2): (V1, V2)) -> Self {
		Self::Two(v1.into(), v2.into())
	}
}

impl<V1, V2, V3> From<(V1, V2, V3)> for ValueTuple
where
	V1: Into<Value>,
	V2: Into<Value>,
	V3: Into<Value>,
{
	fn from((v1, v2, v3): (V1, V2, V3)) -> Self {
		Self::Three(v1.into(), v2.into(), v3.into())
	}
}
