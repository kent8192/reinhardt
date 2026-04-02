//! Permission implementations for tuple combinators (`All`, `Any`) and `Not`.
//!
//! This module uses a macro to generate [`Permission`] implementations for
//! `All<(P1, P2, ...)>` and `Any<(P1, P2, ...)>` across tuple arities 2 through 8,
//! plus a `Not<P>` implementation that inverts a single permission.

use async_trait::async_trait;

use crate::core::{Permission, PermissionContext};
use crate::guard::{All, Any, Not};

/// Generates [`Permission`] implementations for [`All`] and [`Any`] with the given
/// tuple arities.
///
/// For `All`, all permissions must return `true` (short-circuit AND).
/// For `Any`, at least one permission must return `true` (short-circuit OR).
macro_rules! impl_combinator_tuples {
	($(($($T:ident),+)),+ $(,)?) => {
		$(
			#[async_trait]
			impl<$($T),+> Permission for All<($($T,)+)>
			where
				$($T: Permission + Default + Send + Sync,)+
			{
				async fn has_permission(&self, ctx: &PermissionContext<'_>) -> bool {
					$($T::default().has_permission(ctx).await &&)+ true
				}
			}

			#[async_trait]
			impl<$($T),+> Permission for Any<($($T,)+)>
			where
				$($T: Permission + Default + Send + Sync,)+
			{
				async fn has_permission(&self, ctx: &PermissionContext<'_>) -> bool {
					$($T::default().has_permission(ctx).await ||)+ false
				}
			}
		)+
	};
}

impl_combinator_tuples!(
	(A, B),
	(A, B, C),
	(A, B, C, D),
	(A, B, C, D, E),
	(A, B, C, D, E, F),
	(A, B, C, D, E, F, G),
	(A, B, C, D, E, F, G, H),
);

#[async_trait]
impl<P> Permission for Not<P>
where
	P: Permission + Default + Send + Sync,
{
	async fn has_permission(&self, ctx: &PermissionContext<'_>) -> bool {
		!P::default().has_permission(ctx).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, Version};
	use reinhardt_http::Request;
	use rstest::rstest;

	/// Test-only permission that always allows access.
	#[derive(Default)]
	struct Allow;

	#[async_trait]
	impl Permission for Allow {
		async fn has_permission(&self, _ctx: &PermissionContext<'_>) -> bool {
			true
		}
	}

	/// Test-only permission that always denies access.
	#[derive(Default)]
	struct Deny;

	#[async_trait]
	impl Permission for Deny {
		async fn has_permission(&self, _ctx: &PermissionContext<'_>) -> bool {
			false
		}
	}

	/// Helper to build a minimal request for testing.
	fn test_request() -> Request {
		Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	/// Helper to build a default permission context for testing.
	fn test_context(request: &Request) -> PermissionContext<'_> {
		PermissionContext {
			request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_all_both_allow() {
		// Arrange
		let request = test_request();
		let ctx = test_context(&request);
		let perm = All::<(Allow, Allow)>::default();

		// Act
		let result = perm.has_permission(&ctx).await;

		// Assert
		assert!(result);
	}

	#[rstest]
	#[tokio::test]
	async fn test_all_first_deny() {
		// Arrange
		let request = test_request();
		let ctx = test_context(&request);
		let perm = All::<(Deny, Allow)>::default();

		// Act
		let result = perm.has_permission(&ctx).await;

		// Assert
		assert!(!result);
	}

	#[rstest]
	#[tokio::test]
	async fn test_all_second_deny() {
		// Arrange
		let request = test_request();
		let ctx = test_context(&request);
		let perm = All::<(Allow, Deny)>::default();

		// Act
		let result = perm.has_permission(&ctx).await;

		// Assert
		assert!(!result);
	}

	#[rstest]
	#[tokio::test]
	async fn test_all_both_deny() {
		// Arrange
		let request = test_request();
		let ctx = test_context(&request);
		let perm = All::<(Deny, Deny)>::default();

		// Act
		let result = perm.has_permission(&ctx).await;

		// Assert
		assert!(!result);
	}

	#[rstest]
	#[tokio::test]
	async fn test_any_both_allow() {
		// Arrange
		let request = test_request();
		let ctx = test_context(&request);
		let perm = Any::<(Allow, Allow)>::default();

		// Act
		let result = perm.has_permission(&ctx).await;

		// Assert
		assert!(result);
	}

	#[rstest]
	#[tokio::test]
	async fn test_any_first_allow() {
		// Arrange
		let request = test_request();
		let ctx = test_context(&request);
		let perm = Any::<(Allow, Deny)>::default();

		// Act
		let result = perm.has_permission(&ctx).await;

		// Assert
		assert!(result);
	}

	#[rstest]
	#[tokio::test]
	async fn test_any_second_allow() {
		// Arrange
		let request = test_request();
		let ctx = test_context(&request);
		let perm = Any::<(Deny, Allow)>::default();

		// Act
		let result = perm.has_permission(&ctx).await;

		// Assert
		assert!(result);
	}

	#[rstest]
	#[tokio::test]
	async fn test_any_both_deny() {
		// Arrange
		let request = test_request();
		let ctx = test_context(&request);
		let perm = Any::<(Deny, Deny)>::default();

		// Act
		let result = perm.has_permission(&ctx).await;

		// Assert
		assert!(!result);
	}

	#[rstest]
	#[tokio::test]
	async fn test_not_allow_returns_false() {
		// Arrange
		let request = test_request();
		let ctx = test_context(&request);
		let perm = Not::<Allow>::default();

		// Act
		let result = perm.has_permission(&ctx).await;

		// Assert
		assert!(!result);
	}

	#[rstest]
	#[tokio::test]
	async fn test_not_deny_returns_true() {
		// Arrange
		let request = test_request();
		let ctx = test_context(&request);
		let perm = Not::<Deny>::default();

		// Act
		let result = perm.has_permission(&ctx).await;

		// Assert
		assert!(result);
	}

	#[rstest]
	#[tokio::test]
	async fn test_all_triple_all_allow() {
		// Arrange
		let request = test_request();
		let ctx = test_context(&request);
		let perm = All::<(Allow, Allow, Allow)>::default();

		// Act
		let result = perm.has_permission(&ctx).await;

		// Assert
		assert!(result);
	}

	#[rstest]
	#[tokio::test]
	async fn test_all_triple_one_deny() {
		// Arrange
		let request = test_request();
		let ctx = test_context(&request);
		let perm = All::<(Allow, Deny, Allow)>::default();

		// Act
		let result = perm.has_permission(&ctx).await;

		// Assert
		assert!(!result);
	}

	#[rstest]
	#[tokio::test]
	async fn test_any_triple_one_allow() {
		// Arrange
		let request = test_request();
		let ctx = test_context(&request);
		let perm = Any::<(Deny, Allow, Deny)>::default();

		// Act
		let result = perm.has_permission(&ctx).await;

		// Assert
		assert!(result);
	}

	#[rstest]
	#[tokio::test]
	async fn test_nested_not_in_all() {
		// Arrange: All<(Allow, Not<Deny>)> should pass since Not<Deny> == Allow
		let request = test_request();
		let ctx = test_context(&request);
		let perm = All::<(Allow, Not<Deny>)>::default();

		// Act
		let result = perm.has_permission(&ctx).await;

		// Assert
		assert!(result);
	}

	#[rstest]
	#[tokio::test]
	async fn test_nested_not_in_any() {
		// Arrange: Any<(Deny, Not<Allow>)> should fail since both are false
		let request = test_request();
		let ctx = test_context(&request);
		let perm = Any::<(Deny, Not<Allow>)>::default();

		// Act
		let result = perm.has_permission(&ctx).await;

		// Assert
		assert!(!result);
	}
}
