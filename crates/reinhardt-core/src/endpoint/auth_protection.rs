#![cfg(native)]

//! Authentication protection level for endpoints
//!
//! This module defines the [`AuthProtection`] enum that records the
//! authentication contract declared by a route.

use super::EndpointMetadata;

/// Authentication protection level declared by an endpoint handler.
///
/// Each variant indicates the auth requirement declared by route metadata.
/// Endpoints without an explicit declaration use [`AuthProtection::None`],
/// which signals a potential security gap detectable at startup via
/// [`validate_endpoint_security`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthProtection {
	/// Endpoint requires authentication.
	Protected,
	/// Authentication is optional.
	Optional,
	/// Endpoint is explicitly marked public (no auth required by design).
	Public,
	/// No auth parameter detected -- potential security gap.
	None,
}

impl AuthProtection {
	/// Returns `true` if this protection level represents a security violation.
	///
	/// Only [`AuthProtection::None`] is considered a violation, meaning the
	/// endpoint has no auth-related parameter and has not been explicitly
	/// marked as public.
	pub fn is_violation(&self) -> bool {
		matches!(self, AuthProtection::None)
	}
}

/// Validates that all registered endpoints have explicit auth protection.
///
/// Iterates over all [`EndpointMetadata`] entries collected via `inventory`
/// and panics if any endpoint has [`AuthProtection::None`]. This function
/// is intended to be called at application startup to catch unguarded
/// endpoints early.
///
/// # Panics
///
/// Panics with a descriptive message listing the endpoint path, method,
/// and function name if a violation is found.
pub fn validate_endpoint_security() {
	for metadata in inventory::iter::<EndpointMetadata>() {
		if metadata.auth_protection.is_violation() {
			panic!(
				"Endpoint security violation: {} {} (fn {}) has no auth protection. \
					 Declare `auth = \"protected\"`, `auth = \"optional\"`, or \
					 `auth = \"public\"` in the route macro.",
				metadata.method, metadata.path, metadata.function_name,
			);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[case::protected(AuthProtection::Protected, false)]
	#[case::optional(AuthProtection::Optional, false)]
	#[case::public(AuthProtection::Public, false)]
	#[case::none(AuthProtection::None, true)]
	fn test_is_violation(#[case] protection: AuthProtection, #[case] expected: bool) {
		// Arrange
		// (provided via rstest parameters)

		// Act
		let result = protection.is_violation();

		// Assert
		assert_eq!(result, expected);
	}
}
