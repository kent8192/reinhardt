use hyper::Method;
use reinhardt_core::endpoint::{AuthProtection, EndpointInfo, EndpointMetadata};
use reinhardt_http::{Handler, Request, Response, Result as HttpResult};
use reinhardt_urls::routers::{
	RouteTopologyError, RouterFactory, ServerRouter, UrlPatternsRegistration,
	collect_resolved_endpoints_from_registration, get_router, get_router_di_context,
};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

struct MountedEndpoint;
struct DuplicateA;
struct DuplicateB;

macro_rules! endpoint {
	($type:ident, $identity:literal, $path:literal, $function:literal) => {
		impl EndpointInfo for $type {
			fn path() -> &'static str {
				$path
			}

			fn method() -> Method {
				Method::POST
			}

			fn name() -> &'static str {
				$function
			}

			fn handler_identity() -> &'static str {
				$identity
			}

			fn auth_protection() -> AuthProtection {
				AuthProtection::Protected
			}
		}

		#[async_trait::async_trait]
		impl Handler for $type {
			async fn handle(&self, _request: Request) -> HttpResult<Response> {
				Ok(Response::ok())
			}
		}
	};
}

endpoint!(
	MountedEndpoint,
	"contract_topology::mounted_endpoint",
	"/items",
	"mounted_endpoint"
);
endpoint!(
	DuplicateA,
	"contract_topology::duplicate_a",
	"/duplicates",
	"duplicate_a"
);
endpoint!(
	DuplicateB,
	"contract_topology::duplicate_b",
	"/duplicates",
	"duplicate_b"
);

macro_rules! metadata {
	($path:literal, $function:literal) => {
		EndpointMetadata {
			path: $path,
			method: "POST",
			name: None,
			function_name: $function,
			module_path: "contract_topology",
			request_body_type: None,
			request_content_type: None,
			responses: &[],
			headers: &[],
			security: &[],
			auth_protection: AuthProtection::Protected,
			guard_description: None,
		}
	};
}

inventory::submit! { metadata!("/items", "mounted_endpoint") }
inventory::submit! { metadata!("/duplicates", "duplicate_a") }
inventory::submit! { metadata!("/duplicates", "duplicate_b") }
inventory::submit! { metadata!("/unmounted", "unmounted_endpoint") }

fn mounted_factory() -> Arc<ServerRouter> {
	Arc::new(
		ServerRouter::new()
			.with_prefix("/root")
			.mount("/api/", ServerRouter::new().endpoint(|| MountedEndpoint)),
	)
}

fn duplicates_factory() -> Arc<ServerRouter> {
	Arc::new(
		ServerRouter::new()
			.endpoint(|| DuplicateB)
			.endpoint(|| DuplicateA),
	)
}

static ASYNC_FACTORY_CALLED: AtomicBool = AtomicBool::new(false);

fn dynamic_factory() -> Pin<
	Box<
		dyn Future<
				Output = std::result::Result<
					Arc<ServerRouter>,
					Box<dyn std::error::Error + Send + Sync>,
				>,
			> + Send,
	>,
> {
	ASYNC_FACTORY_CALLED.store(true, Ordering::SeqCst);
	Box::pin(async { Ok(Arc::new(ServerRouter::new())) })
}

#[test]
fn collection_uses_final_mounted_paths_without_global_side_effects() {
	let registration = UrlPatternsRegistration {
		factory: RouterFactory::Sync(mounted_factory),
	};

	let endpoints = collect_resolved_endpoints_from_registration(&registration)
		.expect("synchronous mounted topology should be available");

	assert_eq!(endpoints.len(), 1);
	let endpoint = &endpoints[0];
	assert_eq!(
		endpoint.handler_identity,
		"contract_topology::mounted_endpoint"
	);
	assert_eq!(endpoint.method, "POST");
	assert_eq!(endpoint.resolved_path, "/root/api/items");
	assert_eq!(endpoint.name.as_deref(), Some("mounted_endpoint"));
	assert_eq!(endpoint.auth_protection, AuthProtection::Protected);
	assert_eq!(endpoint.guard_description, None);
	assert_eq!(endpoint.module_path, "contract_topology");
	assert_eq!(endpoint.function_name, "mounted_endpoint");
	assert!(get_router().is_none());
	assert!(get_router_di_context().is_none());
}

#[test]
fn collection_keeps_duplicate_mounted_handlers_distinct_and_sorted() {
	let registration = UrlPatternsRegistration {
		factory: RouterFactory::Sync(duplicates_factory),
	};

	let endpoints = collect_resolved_endpoints_from_registration(&registration)
		.expect("duplicate handlers should remain independently inspectable");

	assert_eq!(
		endpoints
			.iter()
			.map(|endpoint| endpoint.handler_identity.as_str())
			.collect::<Vec<_>>(),
		vec![
			"contract_topology::duplicate_a",
			"contract_topology::duplicate_b",
		]
	);
}

#[test]
fn asynchronous_factory_is_not_invoked_during_collection() {
	ASYNC_FACTORY_CALLED.store(false, Ordering::SeqCst);
	let registration = UrlPatternsRegistration {
		factory: RouterFactory::Async(dynamic_factory),
	};

	let error = match collect_resolved_endpoints_from_registration(&registration) {
		Err(error) => error,
		Ok(_) => panic!("asynchronous registration must not be collected"),
	};

	assert_eq!(error, RouteTopologyError::DynamicFactory);
	assert!(!ASYNC_FACTORY_CALLED.load(Ordering::SeqCst));
}
