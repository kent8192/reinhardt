//! Ordered path parameter storage.
//!
//! `PathParams` preserves the order in which path parameters appear in the URL
//! pattern, which is essential for correct tuple-based extraction such as
//! `Path<(T1, T2)>`. Internally it is a `Vec<(String, String)>`, but it exposes
//! a small subset of the `HashMap`-like API (`get`, `iter`, `len`, `is_empty`,
//! `insert`, `values`) so existing callers can continue to look up parameters
//! by name without any code changes.
//!
//! # Why a `Vec` and not a `HashMap`?
//!
//! `HashMap` iteration order is non-deterministic. URL routers (matchit in
//! particular) yield parameters in URL declaration order, which is the order
//! users expect when destructuring `Path<(T1, T2)>`. Storing parameters in a
//! `Vec<(String, String)>` preserves that order all the way from the router to
//! the extractor.
//!
//! See issue #4013 for details.

use std::collections::HashMap;

/// Ordered collection of path parameters extracted from a URL pattern.
///
/// Preserves insertion order so that tuple extractors like `Path<(T1, T2)>`
/// can rely on URL pattern declaration order when populating tuple fields.
///
/// # Example
///
/// ```
/// use reinhardt_http::PathParams;
///
/// let mut params = PathParams::new();
/// params.insert("org", "myslug");
/// params.insert("cluster_id", "5");
///
/// // Insertion order is preserved.
/// let collected: Vec<_> = params.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
/// assert_eq!(collected, vec![("org", "myslug"), ("cluster_id", "5")]);
///
/// // Named lookup still works.
/// assert_eq!(params.get("org").map(String::as_str), Some("myslug"));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PathParams {
	inner: Vec<(String, String)>,
}

impl PathParams {
	/// Create a new, empty `PathParams`.
	pub fn new() -> Self {
		Self { inner: Vec::new() }
	}

	/// Number of stored parameters.
	pub fn len(&self) -> usize {
		self.inner.len()
	}

	/// `true` if no parameters are stored.
	pub fn is_empty(&self) -> bool {
		self.inner.is_empty()
	}

	/// Look up a parameter by name.
	///
	/// Returns the first match if multiple entries share the same name (which
	/// should not happen in practice because URL patterns require unique names).
	pub fn get(&self, key: &str) -> Option<&String> {
		self.inner.iter().find(|(k, _)| k == key).map(|(_, v)| v)
	}

	/// Insert or update a parameter.
	///
	/// If `key` already exists, its value is replaced and its position is kept.
	/// Otherwise the new entry is appended, preserving insertion order.
	pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
		let key = key.into();
		let value = value.into();
		if let Some(slot) = self.inner.iter_mut().find(|(k, _)| *k == key) {
			slot.1 = value;
		} else {
			self.inner.push((key, value));
		}
	}

	/// Iterate over `(key, value)` pairs in insertion order.
	pub fn iter(&self) -> std::slice::Iter<'_, (String, String)> {
		self.inner.iter()
	}

	/// Iterate over values in insertion order.
	pub fn values(&self) -> impl Iterator<Item = &String> {
		self.inner.iter().map(|(_, v)| v)
	}

	/// Borrow the underlying ordered slice of `(key, value)` pairs.
	pub fn as_slice(&self) -> &[(String, String)] {
		&self.inner
	}

	/// Consume the wrapper and return the inner ordered `Vec`.
	pub fn into_vec(self) -> Vec<(String, String)> {
		self.inner
	}
}

impl<K, V> FromIterator<(K, V)> for PathParams
where
	K: Into<String>,
	V: Into<String>,
{
	fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
		let mut params = PathParams::new();
		for (k, v) in iter {
			params.insert(k, v);
		}
		params
	}
}

impl IntoIterator for PathParams {
	type Item = (String, String);
	type IntoIter = std::vec::IntoIter<(String, String)>;

	fn into_iter(self) -> Self::IntoIter {
		self.inner.into_iter()
	}
}

impl<'a> IntoIterator for &'a PathParams {
	type Item = &'a (String, String);
	type IntoIter = std::slice::Iter<'a, (String, String)>;

	fn into_iter(self) -> Self::IntoIter {
		self.inner.iter()
	}
}

impl From<Vec<(String, String)>> for PathParams {
	fn from(inner: Vec<(String, String)>) -> Self {
		// Caller is responsible for the ordering of the supplied vector.
		Self { inner }
	}
}

impl From<HashMap<String, String>> for PathParams {
	/// Convert from a `HashMap`. Iteration order is **not** preserved because
	/// `HashMap` does not have a defined order. Prefer `From<Vec<_>>` when
	/// order matters.
	fn from(map: HashMap<String, String>) -> Self {
		Self {
			inner: map.into_iter().collect(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn insert_preserves_order() {
		// Arrange
		let mut params = PathParams::new();

		// Act
		params.insert("z", "first");
		params.insert("a", "second");
		params.insert("m", "third");

		// Assert
		let order: Vec<&str> = params.iter().map(|(k, _)| k.as_str()).collect();
		assert_eq!(order, vec!["z", "a", "m"]);
	}

	#[rstest]
	fn get_finds_by_name() {
		// Arrange
		let mut params = PathParams::new();
		params.insert("org", "myslug");
		params.insert("cluster_id", "5");

		// Act
		let org = params.get("org");
		let cluster_id = params.get("cluster_id");
		let missing = params.get("missing");

		// Assert
		assert_eq!(org.map(String::as_str), Some("myslug"));
		assert_eq!(cluster_id.map(String::as_str), Some("5"));
		assert_eq!(missing, None);
	}

	#[rstest]
	fn insert_replaces_existing_in_place() {
		// Arrange
		let mut params = PathParams::new();
		params.insert("a", "1");
		params.insert("b", "2");

		// Act
		params.insert("a", "updated");

		// Assert: order unchanged, value replaced
		let collected: Vec<_> = params
			.iter()
			.map(|(k, v)| (k.as_str(), v.as_str()))
			.collect();
		assert_eq!(collected, vec![("a", "updated"), ("b", "2")]);
	}

	#[rstest]
	fn from_vec_preserves_caller_order() {
		// Arrange
		let vec = vec![
			("org".to_string(), "myslug".to_string()),
			("cluster_id".to_string(), "5".to_string()),
		];

		// Act
		let params = PathParams::from(vec);

		// Assert
		let order: Vec<&str> = params.iter().map(|(k, _)| k.as_str()).collect();
		assert_eq!(order, vec!["org", "cluster_id"]);
	}

	#[rstest]
	fn from_iter_collects_in_order() {
		// Arrange
		let pairs = vec![("z", "1"), ("a", "2")];

		// Act
		let params: PathParams = pairs.into_iter().collect();

		// Assert
		let order: Vec<&str> = params.iter().map(|(k, _)| k.as_str()).collect();
		assert_eq!(order, vec!["z", "a"]);
	}
}
