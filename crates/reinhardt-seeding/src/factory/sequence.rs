//! Sequence generator for auto-incrementing values.
//!
//! This module provides thread-safe sequence generators for creating
//! unique values in factories.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use once_cell::sync::Lazy;
use parking_lot::RwLock;

/// Global registry of named sequences.
static SEQUENCE_REGISTRY: Lazy<RwLock<HashMap<String, Arc<AtomicU64>>>> =
	Lazy::new(|| RwLock::new(HashMap::new()));

/// A thread-safe auto-incrementing sequence.
///
/// Sequences are useful for generating unique identifiers like usernames,
/// codes, or other fields that need sequential values.
///
/// # Example
///
/// ```
/// use reinhardt_seeding::factory::Sequence;
///
/// let seq = Sequence::new("user_code");
/// assert_eq!(seq.next(), 1);
/// assert_eq!(seq.next(), 2);
/// assert_eq!(seq.next(), 3);
/// ```
#[derive(Debug, Clone)]
pub struct Sequence {
	/// Name of this sequence.
	name: String,

	/// Shared counter for this sequence.
	counter: Arc<AtomicU64>,
}

impl Sequence {
	/// Creates a new sequence with the given name.
	///
	/// If a sequence with the same name already exists, the existing
	/// counter is shared.
	///
	/// # Arguments
	///
	/// * `name` - Name identifier for this sequence
	pub fn new(name: impl Into<String>) -> Self {
		let name = name.into();
		let counter = {
			let mut registry = SEQUENCE_REGISTRY.write();
			registry
				.entry(name.clone())
				.or_insert_with(|| Arc::new(AtomicU64::new(0)))
				.clone()
		};

		Self { name, counter }
	}

	/// Creates a new sequence starting from the specified value.
	///
	/// # Arguments
	///
	/// * `name` - Name identifier for this sequence
	/// * `start` - Starting value (first call to `next()` returns this + 1)
	pub fn starting_from(name: impl Into<String>, start: u64) -> Self {
		let name = name.into();
		let counter = {
			let mut registry = SEQUENCE_REGISTRY.write();
			let entry = registry
				.entry(name.clone())
				.or_insert_with(|| Arc::new(AtomicU64::new(0)));
			entry.store(start, Ordering::SeqCst);
			entry.clone()
		};

		Self { name, counter }
	}

	/// Returns the next value in the sequence.
	///
	/// This method is thread-safe and will always return unique values.
	pub fn next(&self) -> u64 {
		self.counter.fetch_add(1, Ordering::SeqCst) + 1
	}

	/// Returns the current value without incrementing.
	pub fn current(&self) -> u64 {
		self.counter.load(Ordering::SeqCst)
	}

	/// Resets the sequence to zero.
	pub fn reset(&self) {
		self.counter.store(0, Ordering::SeqCst);
	}

	/// Resets the sequence to a specific value.
	///
	/// # Arguments
	///
	/// * `value` - The value to reset to
	pub fn reset_to(&self, value: u64) {
		self.counter.store(value, Ordering::SeqCst);
	}

	/// Returns the name of this sequence.
	pub fn name(&self) -> &str {
		&self.name
	}

	/// Generates a formatted string using the sequence value.
	///
	/// The format string should contain `{n}` as a placeholder for the
	/// sequence number.
	///
	/// # Arguments
	///
	/// * `format` - Format string with `{n}` placeholder
	///
	/// # Example
	///
	/// ```
	/// use reinhardt_seeding::factory::Sequence;
	///
	/// let seq = Sequence::new("user");
	/// assert_eq!(seq.next_formatted("user_{n}"), "user_1");
	/// assert_eq!(seq.next_formatted("user_{n}@example.com"), "user_2@example.com");
	/// ```
	pub fn next_formatted(&self, format: &str) -> String {
		let n = self.next();
		format.replace("{n}", &n.to_string())
	}
}

/// Generates a formatted sequence value.
///
/// This is a convenience function that creates or retrieves a named sequence
/// and returns a formatted value.
///
/// # Arguments
///
/// * `name` - Name of the sequence
/// * `format` - Format string with `{n}` placeholder
///
/// # Example
///
/// ```
/// use reinhardt_seeding::factory::sequence;
///
/// let code1 = sequence("product_code", "PROD-{n}");
/// let code2 = sequence("product_code", "PROD-{n}");
/// assert_ne!(code1, code2); // Sequential values
/// ```
pub fn sequence(name: &str, format: &str) -> String {
	let seq = Sequence::new(name);
	seq.next_formatted(format)
}

/// Resets all sequences in the global registry.
///
/// This is primarily useful for test cleanup.
pub fn reset_all_sequences() {
	let registry = SEQUENCE_REGISTRY.read();
	for counter in registry.values() {
		counter.store(0, Ordering::SeqCst);
	}
}

/// Resets a specific named sequence.
///
/// # Arguments
///
/// * `name` - Name of the sequence to reset
///
/// # Returns
///
/// Returns `true` if the sequence existed and was reset, `false` otherwise.
pub fn reset_sequence(name: &str) -> bool {
	let registry = SEQUENCE_REGISTRY.read();
	if let Some(counter) = registry.get(name) {
		counter.store(0, Ordering::SeqCst);
		true
	} else {
		false
	}
}

/// Removes a sequence from the registry.
///
/// # Arguments
///
/// * `name` - Name of the sequence to remove
///
/// # Returns
///
/// Returns `true` if the sequence existed and was removed.
pub fn remove_sequence(name: &str) -> bool {
	SEQUENCE_REGISTRY.write().remove(name).is_some()
}

/// Clears all sequences from the registry.
///
/// This removes all sequences, not just reset them.
pub fn clear_sequences() {
	SEQUENCE_REGISTRY.write().clear();
}

/// Returns the names of all registered sequences.
pub fn sequence_names() -> Vec<String> {
	SEQUENCE_REGISTRY.read().keys().cloned().collect()
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::thread;

	#[rstest]
	fn test_sequence_basic() {
		clear_sequences();

		let seq = Sequence::new("test_basic");
		assert_eq!(seq.next(), 1);
		assert_eq!(seq.next(), 2);
		assert_eq!(seq.next(), 3);
	}

	#[rstest]
	fn test_sequence_shared() {
		clear_sequences();

		let seq1 = Sequence::new("test_shared");
		let seq2 = Sequence::new("test_shared");

		assert_eq!(seq1.next(), 1);
		assert_eq!(seq2.next(), 2);
		assert_eq!(seq1.next(), 3);
	}

	#[rstest]
	fn test_sequence_starting_from() {
		clear_sequences();

		let seq = Sequence::starting_from("test_start", 100);
		assert_eq!(seq.next(), 101);
		assert_eq!(seq.next(), 102);
	}

	#[rstest]
	fn test_sequence_reset() {
		clear_sequences();

		let seq = Sequence::new("test_reset");
		seq.next();
		seq.next();
		seq.reset();
		assert_eq!(seq.next(), 1);
	}

	#[rstest]
	fn test_sequence_reset_to() {
		clear_sequences();

		let seq = Sequence::new("test_reset_to");
		seq.next();
		seq.reset_to(50);
		assert_eq!(seq.next(), 51);
	}

	#[rstest]
	fn test_sequence_formatted() {
		clear_sequences();

		let seq = Sequence::new("test_format");
		assert_eq!(seq.next_formatted("user_{n}"), "user_1");
		assert_eq!(seq.next_formatted("email_{n}@test.com"), "email_2@test.com");
	}

	#[rstest]
	fn test_sequence_function() {
		clear_sequences();

		let code1 = sequence("fn_test", "CODE-{n}");
		let code2 = sequence("fn_test", "CODE-{n}");
		assert_eq!(code1, "CODE-1");
		assert_eq!(code2, "CODE-2");
	}

	#[rstest]
	fn test_reset_all_sequences() {
		clear_sequences();

		let seq1 = Sequence::new("reset_all_1");
		let seq2 = Sequence::new("reset_all_2");
		seq1.next();
		seq2.next();

		reset_all_sequences();

		assert_eq!(seq1.next(), 1);
		assert_eq!(seq2.next(), 1);
	}

	#[rstest]
	fn test_reset_specific_sequence() {
		clear_sequences();

		let seq = Sequence::new("reset_specific");
		seq.next();

		assert!(reset_sequence("reset_specific"));
		assert_eq!(seq.next(), 1);

		assert!(!reset_sequence("nonexistent"));
	}

	#[rstest]
	fn test_remove_sequence() {
		clear_sequences();

		Sequence::new("to_remove").next();
		assert!(remove_sequence("to_remove"));
		assert!(!remove_sequence("to_remove")); // Already removed
	}

	#[rstest]
	fn test_sequence_names() {
		clear_sequences();

		Sequence::new("names_test_1");
		Sequence::new("names_test_2");

		let names = sequence_names();
		assert!(names.contains(&"names_test_1".to_string()));
		assert!(names.contains(&"names_test_2".to_string()));
	}

	#[rstest]
	fn test_sequence_thread_safety() {
		clear_sequences();

		let seq = Sequence::new("thread_test");
		let handles: Vec<_> = (0..10)
			.map(|_| {
				let seq_clone = seq.clone();
				thread::spawn(move || {
					let mut values = Vec::new();
					for _ in 0..100 {
						values.push(seq_clone.next());
					}
					values
				})
			})
			.collect();

		let mut all_values: Vec<u64> = handles
			.into_iter()
			.flat_map(|h| h.join().unwrap())
			.collect();
		all_values.sort();

		// Should have 1000 unique values from 1 to 1000
		assert_eq!(all_values.len(), 1000);
		for (i, &v) in all_values.iter().enumerate() {
			assert_eq!(v, i as u64 + 1);
		}
	}

	#[rstest]
	fn test_current_value() {
		clear_sequences();

		let seq = Sequence::new("current_test");
		assert_eq!(seq.current(), 0);
		seq.next();
		assert_eq!(seq.current(), 1);
		seq.next();
		assert_eq!(seq.current(), 2);
	}
}
