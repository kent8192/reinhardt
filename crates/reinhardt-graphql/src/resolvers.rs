//! GraphQL resolvers
//!
//! Provides traits for building GraphQL resolvers with proper error handling.
//! Use [`ContextResolver`] for resolvers that need to access context data,
//! as it returns GraphQL errors instead of panicking when context data is missing.

use crate::context::GraphQLContext;
use async_graphql::Result as GqlResult;
use async_trait::async_trait;

/// Base resolver trait
#[async_trait]
pub trait Resolver: Send + Sync {
	type Output;

	async fn resolve(&self) -> GqlResult<Self::Output>;
}

/// Resolver trait with context access and proper error handling
///
/// Unlike using `unwrap()` on context lookups, this trait ensures that missing
/// context data is reported as a GraphQL error rather than causing a panic.
///
/// # Examples
///
/// ```rust
/// # use async_trait::async_trait;
/// # use async_graphql::Result as GqlResult;
/// # use reinhardt_graphql::context::GraphQLContext;
/// # use reinhardt_graphql::resolvers::ContextResolver;
/// # use serde_json::json;
/// struct UserResolver;
///
/// #[async_trait]
/// impl ContextResolver for UserResolver {
///     type Output = String;
///
///     async fn resolve_with_context(&self, ctx: &GraphQLContext) -> GqlResult<Self::Output> {
///         let user_id = ctx.require_data("user_id")?;
///         Ok(format!("User: {}", user_id))
///     }
/// }
/// ```
#[async_trait]
pub trait ContextResolver: Send + Sync {
	type Output;

	/// Resolve using the provided GraphQL context
	///
	/// Returns a GraphQL error if required context data is missing,
	/// instead of panicking via `unwrap()`.
	async fn resolve_with_context(&self, ctx: &GraphQLContext) -> GqlResult<Self::Output>;
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use serde_json::json;

	struct TestResolver {
		value: i32,
	}

	#[async_trait]
	impl Resolver for TestResolver {
		type Output = i32;

		async fn resolve(&self) -> GqlResult<Self::Output> {
			Ok(self.value * 2)
		}
	}

	struct StringResolver {
		message: String,
	}

	#[async_trait]
	impl Resolver for StringResolver {
		type Output = String;

		async fn resolve(&self) -> GqlResult<Self::Output> {
			Ok(format!("Resolved: {}", self.message))
		}
	}

	struct ContextDataResolver {
		key: String,
	}

	#[async_trait]
	impl ContextResolver for ContextDataResolver {
		type Output = String;

		async fn resolve_with_context(&self, ctx: &GraphQLContext) -> GqlResult<Self::Output> {
			let value = ctx.require_data(&self.key)?;
			Ok(format!("Value: {}", value))
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_resolver_trait_implementation() {
		// Arrange
		let resolver = TestResolver { value: 21 };

		// Act
		let result = resolver.resolve().await;

		// Assert
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), 42);
	}

	#[rstest]
	#[tokio::test]
	async fn test_string_resolver() {
		// Arrange
		let resolver = StringResolver {
			message: "Hello GraphQL".to_string(),
		};

		// Act
		let result = resolver.resolve().await;

		// Assert
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), "Resolved: Hello GraphQL");
	}

	#[rstest]
	#[tokio::test]
	async fn test_resolver_multiple_calls() {
		// Arrange
		let resolver = TestResolver { value: 10 };

		// Act
		let result1 = resolver.resolve().await.unwrap();
		let result2 = resolver.resolve().await.unwrap();

		// Assert
		assert_eq!(result1, result2);
		assert_eq!(result1, 20);
	}

	#[rstest]
	#[tokio::test]
	async fn test_context_resolver_returns_data_when_present() {
		// Arrange
		let ctx = GraphQLContext::new();
		ctx.set_data("user_id".to_string(), json!("user-42"));
		let resolver = ContextDataResolver {
			key: "user_id".to_string(),
		};

		// Act
		let result = resolver.resolve_with_context(&ctx).await;

		// Assert
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), "Value: \"user-42\"");
	}

	#[rstest]
	#[tokio::test]
	async fn test_context_resolver_returns_error_when_data_missing() {
		// Arrange
		let ctx = GraphQLContext::new();
		let resolver = ContextDataResolver {
			key: "missing_key".to_string(),
		};

		// Act
		let result = resolver.resolve_with_context(&ctx).await;

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(
			err.message.contains("missing_key"),
			"Error should mention the missing key, got: {}",
			err.message
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_context_resolver_does_not_panic_on_missing_data() {
		// Arrange
		let ctx = GraphQLContext::new();
		let resolver = ContextDataResolver {
			key: "nonexistent".to_string(),
		};

		// Act
		// This should NOT panic, unlike unwrap() on get_data()
		let result = resolver.resolve_with_context(&ctx).await;

		// Assert
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.message
				.contains("Required context data not found")
		);
	}
}
