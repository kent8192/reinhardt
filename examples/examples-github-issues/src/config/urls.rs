//! URL configuration for examples-github-issues project
//!
//! This module configures the unified GraphQL schema and URL patterns.

use async_graphql::{
	MergedObject, MergedSubscription, Schema,
	http::{GraphQLPlaygroundConfig, playground_source},
};
use reinhardt::middleware::CorsMiddleware;
use reinhardt::middleware::cors::CorsConfig;
use reinhardt::routes;
use reinhardt::{JwtAuth, Request, Response, StatusCode, UnifiedRouter, ViewResult};

use crate::apps::auth::views::{AuthMutation, AuthQuery};
use crate::apps::issues::views::{IssueMutation, IssueQuery, IssueSubscription};
use crate::apps::projects::views::{ProjectMutation, ProjectQuery};

use super::schema::get_schema;
use super::views;

/// Merged Query type combining all app queries
#[derive(MergedObject, Default)]
pub struct Query(AuthQuery, IssueQuery, ProjectQuery);

/// Merged Mutation type combining all app mutations
#[derive(MergedObject, Default)]
pub struct Mutation(AuthMutation, IssueMutation, ProjectMutation);

/// Merged Subscription type for real-time updates
#[derive(MergedSubscription, Default)]
pub struct Subscription(IssueSubscription);

/// GraphQL schema type alias
pub type AppSchema = Schema<Query, Mutation, Subscription>;

/// GraphQL query/mutation handler with singleton schema
pub async fn graphql_handler(req: Request) -> ViewResult<Response> {
	let schema = get_schema();

	// Parse request body as GraphQL request
	let graphql_request: async_graphql::Request = req
		.json()
		.map_err(|e| format!("Failed to parse GraphQL request: {}", e))?;

	// Extract JWT token if present and add claims to context
	let graphql_request = if let Some(auth_header) = req
		.headers
		.get("authorization")
		.and_then(|h| h.to_str().ok())
	{
		if let Some(token) = auth_header.strip_prefix("Bearer ") {
			let jwt_auth = JwtAuth::new(b"your-secret-key-change-in-production");
			if let Ok(claims) = jwt_auth.verify_token(token) {
				graphql_request.data(claims)
			} else {
				graphql_request
			}
		} else {
			graphql_request
		}
	} else {
		graphql_request
	};

	// Execute GraphQL request
	let response = schema.execute(graphql_request).await;

	// Serialize response
	let json = serde_json::to_string(&response)
		.map_err(|e| format!("Failed to serialize GraphQL response: {}", e))?;

	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(json))
}

/// GraphQL Playground handler (development only)
pub async fn graphql_playground(_req: Request) -> ViewResult<Response> {
	let html = playground_source(GraphQLPlaygroundConfig::new("/graphql"));

	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "text/html")
		.with_body(html))
}

/// Create CORS middleware for the application.
///
/// Development configuration allowing cross-origin requests for GraphQL Playground
/// and frontend development.
fn create_cors_middleware() -> CorsMiddleware {
	let config = CorsConfig {
		allow_origins: vec!["*".to_string()], // Development only
		allow_methods: vec!["GET".to_string(), "POST".to_string(), "OPTIONS".to_string()],
		allow_headers: vec!["Content-Type".to_string(), "Authorization".to_string()],
		allow_credentials: false,
		max_age: Some(3600),
	};
	CorsMiddleware::new(config)
}

/// Build URL patterns for the application
#[routes]
pub fn routes() -> UnifiedRouter {
	UnifiedRouter::new()
		.endpoint(views::health_check)
		.function("/graphql", reinhardt::Method::POST, graphql_handler)
		.function("/graphql", reinhardt::Method::GET, graphql_playground)
		.with_middleware(create_cors_middleware())
}
