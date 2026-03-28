//! URL configuration for examples-github-issues project
//!
//! This module configures the unified GraphQL schema and URL patterns.
//! Admin panel routes are integrated via `AdminSite::get_urls()`.

use std::env;

use reinhardt::graphql::{
	MergedObject, MergedSubscription, Schema,
	http::{GraphQLPlaygroundConfig, playground_source},
};
use reinhardt::middleware::CorsMiddleware;
use reinhardt::middleware::cors::CorsConfig;
use reinhardt::routes;
use reinhardt::{JwtAuth, Request, Response, StatusCode, UnifiedRouter, ViewResult};

use crate::config::admin::configure_admin;

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

/// Load JWT secret from environment variable with fallback default
fn jwt_secret() -> Vec<u8> {
	env::var("JWT_SECRET")
		.unwrap_or_else(|_| "your-secret-key-change-in-production".to_string())
		.into_bytes()
}

/// GraphQL query/mutation handler with singleton schema
pub async fn graphql_handler(req: Request) -> ViewResult<Response> {
	let schema = get_schema();

	// Parse request body as GraphQL request
	let graphql_request: reinhardt::graphql::Request = req
		.json()
		.map_err(|e| format!("Failed to parse GraphQL request: {}", e))?;

	// Extract JWT token if present and add claims to context
	let graphql_request = if let Some(auth_header) = req
		.headers
		.get("authorization")
		.and_then(|h| h.to_str().ok())
	{
		if let Some(token) = auth_header.strip_prefix("Bearer ") {
			let jwt_auth = JwtAuth::new(&jwt_secret());
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
	let mut config = CorsConfig::default();
	config.allow_origins = vec!["*".to_string()]; // Development only
	config.allow_methods = vec!["GET".to_string(), "POST".to_string(), "OPTIONS".to_string()];
	config.allow_headers = vec!["Content-Type".to_string(), "Authorization".to_string()];
	config.allow_credentials = false;
	config.max_age = Some(3600);
	CorsMiddleware::new(config)
}

/// Build URL patterns for the application
///
/// Includes GraphQL endpoints and admin panel integration.
/// Admin routes require a `DatabaseConnection` via `AdminSite::get_urls()`.
#[routes]
pub fn routes() -> UnifiedRouter {
	// Configure admin site (registration only, no DB needed yet)
	let _admin = configure_admin();

	// Admin routes require DatabaseConnection for query execution.
	// In production, mount admin routes like this:
	//
	//   let db = DatabaseConnection::connect("postgres://...").await?;
	//   let admin_router = admin.get_urls(db);
	//   router.mount("/admin", admin_router)
	//
	// For this example, admin is configured but not mounted since
	// get_urls() requires an async DatabaseConnection.

	UnifiedRouter::new()
		.endpoint(views::health_check)
		.function("/graphql", reinhardt::Method::POST, graphql_handler)
		.function("/graphql", reinhardt::Method::GET, graphql_playground)
		.with_middleware(create_cors_middleware())
}
