//! Startup validation for auth extractor DI configuration.
//!
//! Provides [`validate_auth_extractors`] to check that required
//! dependencies are registered in the DI context at startup time.

use reinhardt_db::orm::DatabaseConnection;
use reinhardt_di::InjectionContext;

/// Validates that the DI context is properly configured for auth extractors.
///
/// Call this during application startup to detect missing dependencies
/// early, rather than discovering them at request time.
///
/// # Checks
///
/// - `DatabaseConnection` is registered as a singleton (required for `AuthUser<U>`)
/// - `AuthInfo` does not require additional DI setup (only needs auth middleware)
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_auth::validate_auth_extractors;
///
/// let di_context = build_di_context();
/// validate_auth_extractors(&di_context);
/// ```
pub fn validate_auth_extractors(ctx: &InjectionContext) {
	if ctx.get_singleton::<DatabaseConnection>().is_some() {
		::tracing::info!(
			"AuthExtractors: DatabaseConnection registered — AuthUser<U> injection available"
		);
	} else {
		::tracing::warn!(
			"AuthExtractors: DatabaseConnection not registered as singleton. \
			 AuthUser<U> injection will fail at request time. \
			 AuthInfo will still work (no DB required)."
		);
	}
}
