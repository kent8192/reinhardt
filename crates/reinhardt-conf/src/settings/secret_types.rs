//! Core secret types that prevent accidental exposure
//!
//! These types wrap sensitive data and ensure it's not accidentally logged
//! or displayed in debug output. This module is always available regardless
//! of feature flags.

use serde::{Deserialize, Serialize};
use std::fmt;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// A secret string that won't be exposed in logs or debug output
#[derive(Clone, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct SecretString {
	#[serde(rename = "secret")]
	inner: String,
}

impl Serialize for SecretString {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		// Always serialize as [REDACTED] to prevent accidental exposure
		serializer.serialize_str("[REDACTED]")
	}
}

impl SecretString {
	/// Create a new secret string
	pub fn new(secret: impl Into<String>) -> Self {
		Self {
			inner: secret.into(),
		}
	}
	/// Access the secret value (use with caution)
	///
	pub fn expose_secret(&self) -> &str {
		&self.inner
	}
	/// Convert to owned String (consumes self)
	///
	/// # Safety
	/// This method moves the inner value out without cloning, bypassing
	/// the `ZeroizeOnDrop` protection. The caller is responsible for
	/// ensuring the returned String is properly handled.
	pub fn into_inner(self) -> String {
		// Use ManuallyDrop to prevent the Drop handler from zeroizing
		// the inner value after we've moved it out.
		let this = std::mem::ManuallyDrop::new(self);
		// SAFETY: We're reading the inner field before ManuallyDrop drops,
		// and ManuallyDrop prevents the Drop impl from running.
		unsafe { std::ptr::read(&this.inner) }
	}
	/// Get the length of the secret
	///
	pub fn len(&self) -> usize {
		self.inner.len()
	}
	/// Check if the secret is empty
	///
	pub fn is_empty(&self) -> bool {
		self.inner.is_empty()
	}
}

impl fmt::Debug for SecretString {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str("SecretString([REDACTED])")
	}
}

impl fmt::Display for SecretString {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str("[REDACTED]")
	}
}

impl From<String> for SecretString {
	fn from(s: String) -> Self {
		Self::new(s)
	}
}

impl From<&str> for SecretString {
	fn from(s: &str) -> Self {
		Self::new(s.to_string())
	}
}

impl PartialEq for SecretString {
	fn eq(&self, other: &Self) -> bool {
		// Use constant-time comparison to prevent timing attacks
		use subtle::ConstantTimeEq;
		self.inner.as_bytes().ct_eq(other.inner.as_bytes()).into()
	}
}

impl Eq for SecretString {}

/// A generic secret value that can hold any serializable type
#[derive(Clone, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct SecretValue<T: Zeroize> {
	#[serde(bound(deserialize = "T: Deserialize<'de>"))]
	inner: T,
}

impl<T: Zeroize> Serialize for SecretValue<T> {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		// Always serialize as [REDACTED] to prevent accidental exposure
		serializer.serialize_str("[REDACTED]")
	}
}

impl<T: Zeroize> SecretValue<T> {
	/// Create a new secret value
	pub fn new(value: T) -> Self {
		Self { inner: value }
	}
	/// Access the secret value (use with caution)
	///
	pub fn expose_secret(&self) -> &T {
		&self.inner
	}
	/// Convert to owned value (consumes self)
	///
	/// # Safety
	/// This method moves the inner value out without cloning, bypassing
	/// the `ZeroizeOnDrop` protection. The caller is responsible for
	/// ensuring the returned value is properly handled.
	pub fn into_inner(self) -> T {
		// Use ManuallyDrop to prevent the Drop handler from zeroizing
		// the inner value after we've moved it out.
		let this = std::mem::ManuallyDrop::new(self);
		// SAFETY: We're reading the inner field before ManuallyDrop drops,
		// and ManuallyDrop prevents the Drop impl from running.
		unsafe { std::ptr::read(&this.inner) }
	}
}

impl<T: Zeroize> fmt::Debug for SecretValue<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str("SecretValue([REDACTED])")
	}
}

impl<T: Zeroize> fmt::Display for SecretValue<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str("[REDACTED]")
	}
}

impl<T: Zeroize> From<T> for SecretValue<T> {
	fn from(value: T) -> Self {
		Self::new(value)
	}
}

impl<T: Zeroize + AsRef<[u8]>> PartialEq for SecretValue<T> {
	fn eq(&self, other: &Self) -> bool {
		// Use constant-time comparison to prevent timing attacks
		use subtle::ConstantTimeEq;
		self.inner.as_ref().ct_eq(other.inner.as_ref()).into()
	}
}

impl<T: Zeroize + AsRef<[u8]> + Eq> Eq for SecretValue<T> {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_secret_string_debug() {
		let secret = SecretString::new("my-secret-password");
		let debug_output = format!("{:?}", secret);
		assert!(!debug_output.contains("my-secret-password"));
		assert!(debug_output.contains("REDACTED"));
	}

	#[rstest]
	fn test_secret_string_display() {
		let secret = SecretString::new("my-secret-password");
		let display_output = format!("{}", secret);
		assert!(!display_output.contains("my-secret-password"));
		assert!(display_output.contains("REDACTED"));
	}

	#[rstest]
	fn test_secret_string_expose() {
		let secret = SecretString::new("my-secret-password");
		assert_eq!(secret.expose_secret(), "my-secret-password");
	}

	#[rstest]
	fn test_secret_string_len() {
		let secret = SecretString::new("password");
		assert_eq!(secret.len(), 8);
		assert!(!secret.is_empty());

		let empty = SecretString::new("");
		assert_eq!(empty.len(), 0);
		assert!(empty.is_empty());
	}

	#[rstest]
	fn test_secret_string_equality() {
		// Arrange
		let secret1 = SecretString::new("password");
		let secret2 = SecretString::new("password");
		let secret3 = SecretString::new("different");

		// Assert - uses constant-time comparison
		assert_eq!(secret1, secret2);
		assert_ne!(secret1, secret3);
	}

	#[rstest]
	fn test_secret_value_constant_time_equality() {
		// Arrange - SecretValue with AsRef<[u8]> types use constant-time comparison
		let val1 = SecretValue::new(vec![1u8, 2, 3, 4]);
		let val2 = SecretValue::new(vec![1u8, 2, 3, 4]);
		let val3 = SecretValue::new(vec![5u8, 6, 7, 8]);

		// Assert
		assert_eq!(val1, val2);
		assert_ne!(val1, val3);
	}

	#[rstest]
	fn test_secret_value_constant_time_equality_strings() {
		// Arrange - String implements AsRef<[u8]>
		let val1 = SecretValue::new("secret_token".to_string());
		let val2 = SecretValue::new("secret_token".to_string());
		let val3 = SecretValue::new("different_token".to_string());

		// Assert
		assert_eq!(val1, val2);
		assert_ne!(val1, val3);
	}

	#[rstest]
	fn test_secret_value_debug() {
		let secret = SecretValue::new(12345);
		let debug_output = format!("{:?}", secret);
		assert!(!debug_output.contains("12345"));
		assert!(debug_output.contains("REDACTED"));
	}

	#[rstest]
	fn test_secret_value_expose() {
		let secret = SecretValue::new(vec![1, 2, 3, 4, 5]);
		assert_eq!(secret.expose_secret(), &vec![1, 2, 3, 4, 5]);
	}

	#[rstest]
	fn test_secret_string_serialization_redacts_value() {
		let secret = SecretString::new("my-super-secret-password");
		let json = serde_json::to_string(&secret).unwrap();
		// Serialization should always output [REDACTED], not the actual secret
		assert!(!json.contains("my-super-secret-password"));
		assert!(json.contains("[REDACTED]"));
		assert_eq!(json, "\"[REDACTED]\"");
	}

	#[rstest]
	fn test_secret_string_deserialization() {
		// Deserialization should still work for config loading
		let json = r#"{"secret":"test-secret"}"#;
		let deserialized: SecretString = serde_json::from_str(json).unwrap();
		assert_eq!(deserialized.expose_secret(), "test-secret");
	}

	#[rstest]
	fn test_secret_value_serialization_redacts_value() {
		let secret = SecretValue::new(42);
		let json = serde_json::to_string(&secret).unwrap();
		// Serialization should always output [REDACTED], not the actual value
		assert!(!json.contains("42"));
		assert!(json.contains("[REDACTED]"));
		assert_eq!(json, "\"[REDACTED]\"");
	}

	#[rstest]
	fn test_secret_value_deserialization() {
		// Deserialization should still work for config loading
		let json = r#"{"inner":42}"#;
		let deserialized: SecretValue<i32> = serde_json::from_str(json).unwrap();
		assert_eq!(*deserialized.expose_secret(), 42);
	}

	#[rstest]
	fn test_secret_string_into_inner() {
		let secret = SecretString::new("my-secret-value");
		let inner = secret.into_inner();
		assert_eq!(inner, "my-secret-value");
	}

	#[rstest]
	fn test_secret_value_into_inner() {
		let secret = SecretValue::new(vec![1, 2, 3, 4, 5]);
		let inner = secret.into_inner();
		assert_eq!(inner, vec![1, 2, 3, 4, 5]);
	}

	#[rstest]
	fn test_secret_value_into_inner_non_clone() {
		// Test with a type that does NOT implement Clone
		// This verifies the fix: T: Clone bound was removed
		struct NonClone {
			inner: String,
		}
		impl Zeroize for NonClone {
			fn zeroize(&mut self) {
				self.inner.zeroize();
			}
		}
		let secret = SecretValue::new(NonClone {
			inner: "secret".to_string(),
		});
		let inner = secret.into_inner();
		assert_eq!(inner.inner, "secret");
	}
}
