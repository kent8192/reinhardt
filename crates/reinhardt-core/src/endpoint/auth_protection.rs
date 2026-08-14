#![cfg(native)]

//! Authentication protection level for endpoints
//!
//! This module defines the [`AuthProtection`] enum that records the
//! authentication contract declared by a route.
//!
//! ## Contract Verification
//!
//! [`collect_endpoint_security_violations`] is the side-effect-free contract
//! collector. It accepts resolved mounted endpoints and reports only entries
//! whose authentication decision is absent, using the stable finding code
//! `authorization.missing_declaration`. It does not execute route factories,
//! initialize routers or dependency injection, open a database, or inspect
//! permission semantics. The startup-facing [`validate_endpoint_security`]
//! wrapper retains its existing panic behavior.

use super::{EndpointMetadata, ResolvedEndpoint};

/// An endpoint whose route declaration lacks an authentication decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndpointSecurityViolation {
	/// HTTP method dispatched by the endpoint.
	pub method: String,
	/// Fully resolved mounted path.
	pub path: String,
	/// Module containing the handler function.
	pub module_path: String,
	/// Handler function name.
	pub function_name: String,
}

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
	let endpoints: Vec<_> = inventory::iter::<EndpointMetadata>()
		.map(|metadata| ResolvedEndpoint {
			handler_identity: format!("{}::{}", metadata.module_path, metadata.function_name),
			method: metadata.method.to_string(),
			resolved_path: metadata.path.to_string(),
			metadata: metadata.clone(),
		})
		.collect();

	panic_for_endpoint_security_violations(&endpoints);
}

fn panic_for_endpoint_security_violations(endpoints: &[ResolvedEndpoint]) {
	if let Some(violation) = collect_endpoint_security_violations(endpoints).into_iter().next() {
		panic!(
			"Endpoint security violation: {} {} (fn {}) has no auth protection. \
				 Declare `auth = \"protected\"`, `auth = \"optional\"`, or \
				 `auth = \"public\"` in the route macro.",
			violation.method, violation.path, violation.function_name,
		);
	}
}

/// Collect endpoints whose route declaration lacks an authentication decision.
pub fn collect_endpoint_security_violations(
	endpoints: &[ResolvedEndpoint],
) -> Vec<EndpointSecurityViolation> {
	endpoints
		.iter()
		.filter(|endpoint| endpoint.metadata.auth_protection.is_violation())
		.map(|endpoint| EndpointSecurityViolation {
			method: endpoint.method.clone(),
			path: endpoint.resolved_path.clone(),
			module_path: endpoint.metadata.module_path.to_string(),
			function_name: endpoint.metadata.function_name.to_string(),
		})
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	fn endpoint(auth_protection: AuthProtection) -> ResolvedEndpoint {
		ResolvedEndpoint {
			handler_identity: "fixture::admin::export".to_string(),
			method: "POST".to_string(),
			resolved_path: "/admin/export".to_string(),
			metadata: EndpointMetadata {
				path: "/ignored",
				method: "GET",
				name: None,
				function_name: "export",
				module_path: "fixture::admin",
				request_body_type: None,
				request_content_type: None,
				responses: &[],
				headers: &[],
				security: &[],
				auth_protection,
				guard_description: None,
			},
		}
	}

	#[test]
	fn collector_reports_only_endpoints_without_authentication_declaration() {
		let endpoints = [
			endpoint(AuthProtection::Protected),
			endpoint(AuthProtection::Optional),
			endpoint(AuthProtection::Public),
			endpoint(AuthProtection::None),
		];

		let violations = collect_endpoint_security_violations(&endpoints);

		assert_eq!(
			violations,
			vec![EndpointSecurityViolation {
				method: "POST".to_string(),
				path: "/admin/export".to_string(),
				module_path: "fixture::admin".to_string(),
				function_name: "export".to_string(),
			}]
		);
	}

	#[test]
	#[should_panic(expected = "Endpoint security violation: POST /admin/export (fn export)")]
	fn panic_wrapper_uses_collector_classification() {
		panic_for_endpoint_security_violations(&[endpoint(AuthProtection::None)]);
	}

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
