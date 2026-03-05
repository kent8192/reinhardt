//! Tests for struct field-level dependency injection with `#[inject]`

use async_trait::async_trait;
use bytes::Bytes;
use hyper::{HeaderMap, Method, StatusCode, Version};
use reinhardt_core::exception::Result;
use reinhardt_di::{Injectable, Injected, InjectionContext, SingletonScope, injectable};
use reinhardt_http::{Request, Response};
use reinhardt_views::viewsets::{Action, ViewSet};
use std::sync::Arc;

/// Mock database dependency
#[derive(Clone)]
#[injectable]
struct Database {
	#[no_inject]
	connection_string: String,
}

impl Default for Database {
	fn default() -> Self {
		Self {
			connection_string: "postgres://localhost/testdb".to_string(),
		}
	}
}

/// Mock cache dependency
#[derive(Clone)]
#[injectable]
struct RedisCache {
	#[no_inject]
	host: String,
}

impl Default for RedisCache {
	fn default() -> Self {
		Self {
			host: "redis://localhost:6379".to_string(),
		}
	}
}

/// ViewSet with field-level dependency injection
#[derive(Clone)]
#[injectable]
struct UserViewSet {
	#[inject]
	db: Injected<Database>,
	#[inject]
	cache: Injected<RedisCache>,
	#[no_inject]
	#[allow(dead_code)]
	name: String,
}

#[async_trait]
impl ViewSet for UserViewSet {
	fn get_basename(&self) -> &str {
		"users"
	}

	async fn dispatch(&self, _request: Request, action: Action) -> Result<Response> {
		use reinhardt_views::viewsets::ActionType;

		match action.action_type {
			ActionType::List => {
				let body = format!(
					r#"{{"db":"{}","cache":"{}"}}"#,
					self.db.connection_string, self.cache.host
				);
				Ok(Response::ok().with_body(body))
			}
			_ => Err(reinhardt_core::exception::Error::NotFound(
				"Action not found".to_string(),
			)),
		}
	}
}

#[tokio::test]
async fn test_field_injection_basic() {
	// Create DI context
	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	// Inject dependencies into ViewSet
	let viewset = UserViewSet::inject(&ctx).await.unwrap();

	// Verify dependencies were injected
	assert_eq!(viewset.db.connection_string, "postgres://localhost/testdb");
	assert_eq!(viewset.cache.host, "redis://localhost:6379");
	assert_eq!(viewset.name, ""); // Default::default()
}

#[tokio::test]
async fn test_field_injection_with_viewset_dispatch() {
	// Create DI context
	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	// Inject dependencies into ViewSet
	let viewset = UserViewSet::inject(&ctx).await.unwrap();

	// Create request
	let request = Request::builder()
		.method(Method::GET)
		.uri("/users/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let action = Action::list();

	// Dispatch request
	let response = viewset.dispatch(request, action).await.unwrap();

	// Verify response
	assert_eq!(response.status, StatusCode::OK);

	let body = String::from_utf8(response.body.to_vec()).unwrap();
	assert!(body.contains("postgres://localhost/testdb"));
	assert!(body.contains("redis://localhost:6379"));
}

/// ViewSet with cache control on field injection
#[derive(Clone)]
#[injectable]
struct ServiceViewSet {
	#[inject]
	cached_db: Injected<Database>,
	#[inject(cache = false)]
	fresh_db: Injected<Database>,
}

#[async_trait]
impl ViewSet for ServiceViewSet {
	fn get_basename(&self) -> &str {
		"services"
	}

	async fn dispatch(&self, _request: Request, _action: Action) -> Result<Response> {
		Ok(Response::ok())
	}
}

#[tokio::test]
async fn test_field_injection_with_cache_control() {
	// Create DI context
	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	// Inject dependencies into ViewSet
	let viewset = ServiceViewSet::inject(&ctx).await.unwrap();

	// Both should be injected successfully
	assert_eq!(
		viewset.cached_db.connection_string,
		"postgres://localhost/testdb"
	);
	assert_eq!(
		viewset.fresh_db.connection_string,
		"postgres://localhost/testdb"
	);
}

#[tokio::test]
async fn test_field_injection_caching_behavior() {
	// Create DI context
	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton.clone()).build();

	// Inject first ViewSet
	let viewset1 = UserViewSet::inject(&ctx).await.unwrap();

	// Inject second ViewSet - should reuse cached dependencies
	let viewset2 = UserViewSet::inject(&ctx).await.unwrap();

	// Both should have the same dependency values (cached)
	assert_eq!(viewset1.db.connection_string, viewset2.db.connection_string);
	assert_eq!(viewset1.cache.host, viewset2.cache.host);
}
