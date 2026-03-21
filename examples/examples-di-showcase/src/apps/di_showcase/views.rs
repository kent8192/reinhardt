//! HTTP view handlers for the di_showcase app
//!
//! Demonstrates dependency injection patterns with HTTP method decorators:
//!
//! - `#[get]` / `#[post]` handlers with `use_inject = true`
//! - `#[inject]` attribute on handler parameters for automatic DI
//! - Custom `Injectable` types with explicit implementation
//! - Nested dependency resolution (`DashboardService` depends on multiple services)
//! - Path parameter extraction combined with DI

use reinhardt::Injected;
use reinhardt::core::serde::json;
use reinhardt::di::{InjectionContext, SingletonScope};
use reinhardt::http::ViewResult;
use reinhardt::{Path, Response, StatusCode, get, post};
use reinhardt::core::serde::Serialize;
use std::sync::Arc;

use super::services::{AppConfig, DashboardService, GreetingService, RequestCounter};

// ---------------------------------------------------------------------------
// Response schemas
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct ConfigResponse {
	app_name: String,
	version: String,
	max_items_per_page: usize,
}

#[derive(Debug, Serialize)]
struct CounterResponse {
	count: u64,
	cached: bool,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Show application configuration injected via DI.
///
/// `AppConfig` implements `Injectable` with `Default`-based initialization.
///
/// GET /di/config/
#[get("/di/config/", name = "di_config_info", use_inject = true)]
pub async fn config_info(#[inject] config: AppConfig) -> ViewResult<Response> {
	let response = ConfigResponse {
		app_name: config.app_name.clone(),
		version: config.version.clone(),
		max_items_per_page: config.max_items_per_page,
	};
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(json::to_vec(&response)?))
}

/// Greet a user by name using an injected `GreetingService`.
///
/// `GreetingService` has an explicit `Injectable` impl with custom
/// initialization logic.
///
/// GET /di/greet/{name}/
#[get("/di/greet/{name}/", name = "di_greet_user", use_inject = true)]
pub async fn greet_user(
	Path(name): Path<String>,
	#[inject] greeter: GreetingService,
) -> ViewResult<Response> {
	let message = greeter.greet(&name);
	let body = json::json!({"message": message});
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(json::to_string(&body)?.into_bytes()))
}

/// Show the current request counter value (cached injection).
///
/// The `RequestCounter` is resolved from the request scope cache -- the same
/// instance is returned for every `#[inject]` in this request.
///
/// GET /di/counter/
#[get("/di/counter/", name = "di_request_counter", use_inject = true)]
pub async fn request_counter(#[inject] counter: RequestCounter) -> ViewResult<Response> {
	let count = counter.increment();
	let response = CounterResponse {
		count,
		cached: true,
	};
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(json::to_vec(&response)?))
}

/// Show the request counter resolved without cache.
///
/// Demonstrates `#[inject(cache = false)]`: a fresh `RequestCounter` instance
/// is created for every resolution, bypassing the request-scope cache.
///
/// POST /di/counter/uncached/
#[post(
	"/di/counter/uncached/",
	name = "di_uncached_injection",
	use_inject = true
)]
pub async fn uncached_injection(
	#[inject(cache = false)] counter: RequestCounter,
) -> ViewResult<Response> {
	// Counter starts from zero for each uncached resolution
	let count = counter.increment();
	let response = CounterResponse {
		count,
		cached: false,
	};
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(json::to_vec(&response)?))
}

/// Show dashboard built from nested dependencies.
///
/// `DashboardService` is an `Injectable` that itself resolves `AppConfig` and
/// `GreetingService` from the same `InjectionContext`, demonstrating nested
/// dependency resolution.
///
/// GET /di/dashboard/{name}/
#[get("/di/dashboard/{name}/", name = "di_dashboard", use_inject = true)]
pub async fn dashboard(
	Path(name): Path<String>,
	#[inject] svc: DashboardService,
) -> ViewResult<Response> {
	let summary = svc.summary(&name);
	let body = json::json!({"summary": summary});
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(json::to_string(&body)?.into_bytes()))
}

/// Demonstrate multiple injected dependencies in one handler.
///
/// Both `AppConfig` and `GreetingService` are injected independently; since
/// they are request-scoped and cached, resolving them here will reuse any
/// previously resolved instances within the same request.
///
/// GET /di/multiple/
#[get("/di/multiple/", name = "di_multiple_deps", use_inject = true)]
pub async fn multiple_deps(
	#[inject] config: AppConfig,
	#[inject] greeter: GreetingService,
) -> ViewResult<Response> {
	let message = greeter.greet("World");
	let body = json::json!({
		"app": config.app_name,
		"version": config.version,
		"message": message,
	});
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(json::to_string(&body)?.into_bytes()))
}

// ---------------------------------------------------------------------------
// Pattern: Manual `Injected<T>` resolution (without #[inject] macro)
// ---------------------------------------------------------------------------

/// Demonstrate programmatic `Injected<T>` resolution without `#[inject]` macro.
///
/// This handler manually creates an `InjectionContext` and resolves
/// dependencies using `Injected<T>::resolve()`, which provides:
/// - Circular dependency detection
/// - Request-scope caching
/// - Injection metadata tracking
///
/// GET /di/manual-injected/
#[get("/di/manual-injected/", name = "di_manual_injected")]
pub async fn manual_injected() -> ViewResult<Response> {
	// Arrange: create an injection context
	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();

	// Act: resolve dependencies using Injected<T>
	let config = Injected::<AppConfig>::resolve(&ctx).await?;
	let greeter = Injected::<GreetingService>::resolve(&ctx).await?;

	// Injected<T> provides metadata about the resolution
	let config_metadata = config.metadata();
	let greeter_metadata = greeter.metadata();

	// Access inner values via Deref
	let message = greeter.greet("Injected");

	let body = json::json!({
		"pattern": "manual Injected<T> resolution",
		"app": config.app_name,
		"message": message,
		"config_cached": config_metadata.cached,
		"greeter_cached": greeter_metadata.cached,
	});
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(json::to_string(&body)?.into_bytes()))
}

/// Demonstrate `Injected<T>::resolve_uncached()` for fresh instances.
///
/// Each call creates a new `RequestCounter` instance, bypassing the cache.
/// The counter metadata confirms `cached: false`.
///
/// GET /di/manual-uncached/
#[get("/di/manual-uncached/", name = "di_manual_uncached")]
pub async fn manual_uncached() -> ViewResult<Response> {
	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();

	// Resolve two separate uncached instances
	let counter1 = Injected::<RequestCounter>::resolve_uncached(&ctx).await?;
	let counter2 = Injected::<RequestCounter>::resolve_uncached(&ctx).await?;

	// Each is a fresh instance starting from zero
	let count1 = counter1.increment();
	let count2 = counter2.increment();

	let body = json::json!({
		"pattern": "manual Injected<T> uncached resolution",
		"counter1": count1,
		"counter2": count2,
		"both_start_from_one": count1 == 1 && count2 == 1,
		"counter1_cached": counter1.metadata().cached,
		"counter2_cached": counter2.metadata().cached,
	});
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(json::to_string(&body)?.into_bytes()))
}
