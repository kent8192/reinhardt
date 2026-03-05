#[cfg(feature = "graphql")]
use async_graphql::{EmptySubscription, Schema};
#[cfg(feature = "graphql")]
use http::{Method, StatusCode};
#[cfg(feature = "graphql")]
use reinhardt_http::Handler;
#[cfg(feature = "graphql")]
use reinhardt_http::{Request, Response};
#[cfg(feature = "graphql")]
use std::sync::Arc;

/// GraphQL server handler that wraps an async-graphql schema
#[cfg(feature = "graphql")]
pub struct GraphQLHandler<Query, Mutation> {
	schema: Schema<Query, Mutation, EmptySubscription>,
}

#[cfg(feature = "graphql")]
impl<Query, Mutation> GraphQLHandler<Query, Mutation>
where
	Query: async_graphql::ObjectType + 'static,
	Mutation: async_graphql::ObjectType + 'static,
{
	/// Create a new GraphQL handler with the given schema
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_server::server::GraphQLHandler;
	/// use async_graphql::{Schema, EmptySubscription, EmptyMutation, Object};
	///
	/// struct QueryRoot;
	///
	/// #[Object]
	/// impl QueryRoot {
	///     async fn hello(&self) -> &str {
	///         "Hello, world!"
	///     }
	/// }
	///
	/// let schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription).finish();
	/// let handler = GraphQLHandler::new(schema);
	/// ```
	pub fn new(schema: Schema<Query, Mutation, EmptySubscription>) -> Self {
		Self { schema }
	}
	/// Create a new GraphQL handler from query and mutation roots
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_server::server::GraphQLHandler;
	/// use async_graphql::Object;
	///
	/// struct QueryRoot;
	///
	/// #[Object]
	/// impl QueryRoot {
	///     async fn version(&self) -> &str {
	///         "1.0.0"
	///     }
	/// }
	///
	/// struct MutationRoot;
	///
	/// #[Object]
	/// impl MutationRoot {
	///     async fn noop(&self) -> bool {
	///         true
	///     }
	/// }
	///
	/// let handler = GraphQLHandler::build(QueryRoot, MutationRoot);
	/// ```
	pub fn build(query: Query, mutation: Mutation) -> Self {
		let schema = Schema::build(query, mutation, EmptySubscription).finish();
		Self { schema }
	}
}

#[cfg(feature = "graphql")]
#[async_trait::async_trait]
impl<Query, Mutation> Handler for GraphQLHandler<Query, Mutation>
where
	Query: async_graphql::ObjectType + Send + Sync + 'static,
	Mutation: async_graphql::ObjectType + Send + Sync + 'static,
{
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		// Only accept POST requests for GraphQL queries
		if request.method != Method::POST {
			return Ok(Response::new(StatusCode::METHOD_NOT_ALLOWED)
				.with_body("Only POST requests are allowed"));
		}

		// Parse the GraphQL request
		let body = request
			.read_body()
			.map_err(|e| reinhardt_core::exception::Error::ParseError(e.to_string()))?;
		let body_str = String::from_utf8(body.to_vec())
			.map_err(|e| reinhardt_core::exception::Error::ParseError(e.to_string()))?;

		let graphql_request: async_graphql::Request = serde_json::from_str(&body_str)
			.map_err(|e| reinhardt_core::exception::Error::Serialization(e.to_string()))?;

		// Execute the query
		let response = self.schema.execute(graphql_request).await;

		// Serialize the response
		let response_json = serde_json::to_string(&response)
			.map_err(|e| reinhardt_core::exception::Error::Serialization(e.to_string()))?;

		Ok(Response::ok()
			.with_header("content-type", "application/json")
			.with_body(response_json))
	}
}

/// Helper function to create a GraphQL handler
///
/// This is a convenience function that creates a `GraphQLHandler` wrapped in an `Arc`.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use reinhardt_server::server::graphql_handler;
/// use async_graphql::Object;
///
/// struct QueryRoot;
///
/// #[Object]
/// impl QueryRoot {
///     async fn greeting(&self, name: String) -> String {
///         format!("Hello, {}!", name)
///     }
/// }
///
/// struct MutationRoot;
///
/// #[Object]
/// impl MutationRoot {
///     async fn increment(&self, value: i32) -> i32 {
///         value + 1
///     }
/// }
///
/// let handler = graphql_handler(QueryRoot, MutationRoot);
/// // handler can now be used with HttpServer or other handlers
/// ```
#[cfg(feature = "graphql")]
pub fn graphql_handler<Query, Mutation>(
	query: Query,
	mutation: Mutation,
) -> Arc<GraphQLHandler<Query, Mutation>>
where
	Query: async_graphql::ObjectType + Send + Sync + 'static,
	Mutation: async_graphql::ObjectType + Send + Sync + 'static,
{
	Arc::new(GraphQLHandler::build(query, mutation))
}

#[cfg(all(test, feature = "graphql"))]
mod tests {
	use super::*;
	use async_graphql::Object;
	use bytes::Bytes;
	use http::{HeaderMap, Method, StatusCode, Version};

	struct QueryRoot;

	#[Object]
	impl QueryRoot {
		async fn hello(&self) -> &str {
			"Hello, GraphQL!"
		}

		async fn add(&self, a: i32, b: i32) -> i32 {
			a + b
		}
	}

	struct MutationRoot;

	#[Object]
	impl MutationRoot {
		async fn noop(&self) -> bool {
			true
		}
	}

	#[tokio::test]
	async fn test_graphql_handler_creation() {
		let _handler = GraphQLHandler::build(QueryRoot, MutationRoot);
	}

	#[tokio::test]
	async fn test_graphql_query() {
		let handler = GraphQLHandler::build(QueryRoot, MutationRoot);

		let query = r#"{"query": "{ hello }"}"#;
		let request = Request::builder()
			.method(Method::POST)
			.uri("/graphql")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::from(query))
			.build()
			.unwrap();

		let response = handler.handle(request).await.unwrap();
		assert_eq!(response.status, StatusCode::OK);

		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		assert!(body_str.contains("Hello, GraphQL!"));
	}

	#[tokio::test]
	async fn test_graphql_method_not_allowed() {
		let handler = GraphQLHandler::build(QueryRoot, MutationRoot);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/graphql")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = handler.handle(request).await.unwrap();
		assert_eq!(response.status, StatusCode::METHOD_NOT_ALLOWED);
	}
}
