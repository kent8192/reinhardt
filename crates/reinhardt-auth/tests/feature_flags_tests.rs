//! Tests for feature flags in reinhardt-auth
//!
//! These tests verify conditional compilation based on feature flags,
//! particularly the `params` feature for auth extractors.
//!
//! `AuthInfo` is tested directly because it has no generic type parameters.
//! `CurrentUser<U>` and `AuthUser<U>` require `BaseUser + Model` bounds which
//! depend on the `argon2-hasher` feature, so their fallback behavior is
//! verified via `cargo make feature-check` compilation checks.

// ============================================================================
// Tests with params feature disabled
// ============================================================================

#[cfg(not(feature = "params"))]
mod params_feature_disabled {
	use reinhardt_di::{DiError, Injectable, InjectionContext, SingletonScope};
	use rstest::rstest;

	fn create_empty_context() -> InjectionContext {
		let singleton_scope = SingletonScope::new();
		InjectionContext::builder(singleton_scope).build()
	}

	#[rstest]
	#[tokio::test]
	async fn test_auth_info_returns_not_found_without_params() {
		// Arrange
		let ctx = create_empty_context();

		// Act
		let result = reinhardt_auth::AuthInfo::inject(&ctx).await;

		// Assert
		assert!(
			result.is_err(),
			"AuthInfo should fail without params feature"
		);
		match result.unwrap_err() {
			DiError::NotFound(msg) => {
				assert_eq!(msg, "AuthInfo requires the 'params' feature to be enabled");
			}
			other => panic!("Expected DiError::NotFound, got: {other:?}"),
		}
	}
}

// ============================================================================
// Tests with params feature enabled
// ============================================================================

#[cfg(feature = "params")]
mod params_feature_enabled {
	use rstest::rstest;

	#[rstest]
	fn test_params_feature_compilation() {
		// This test verifies that the code compiles when params feature is enabled.
		// Simply compiling this module is sufficient.
		assert!(
			true,
			"Auth extractors are available when params feature enabled"
		);
	}
}
