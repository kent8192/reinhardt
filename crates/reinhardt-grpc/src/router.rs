//! Composable registration for generated Tonic services.
//!
//! Configure the generated service before adding it to a router:
//!
//! ```rust,ignore
//! use reinhardt_grpc::GrpcRouter;
//!
//! pub fn grpc_services() -> GrpcRouter {
//!     GrpcRouter::new().service(ChatServiceServer::new(ChatService::default()))
//! }
//! ```

use std::sync::Arc;
use tonic::service::{Routes, RoutesBuilder};

#[derive(Clone)]
struct GrpcServiceRegistration {
	name: &'static str,
	namespace: Option<String>,
	register: Arc<dyn Fn(&mut RoutesBuilder) + Send + Sync>,
}

/// A configuration error found while composing gRPC routers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GrpcRouteError {
	/// The same Tonic service was registered by two routers.
	DuplicateService {
		/// The generated Tonic service name.
		service: &'static str,
		/// Namespace of the first registration, if configured.
		first_namespace: Option<String>,
		/// Namespace of the duplicate registration, if configured.
		second_namespace: Option<String>,
	},
	/// A non-root prefix was used to mount gRPC services.
	NonRootMount {
		/// The unsupported mount prefix.
		prefix: String,
	},
}

/// Collects generated Tonic services for later route construction.
#[derive(Clone, Default)]
pub struct GrpcRouter {
	entries: Vec<GrpcServiceRegistration>,
	errors: Vec<GrpcRouteError>,
	namespace: Option<String>,
}

impl GrpcRouter {
	/// Creates an empty gRPC router.
	pub fn new() -> Self {
		Self::default()
	}

	/// Registers a generated Tonic service.
	pub fn service<S>(mut self, service: S) -> Self
	where
		S: tonic::codegen::Service<
				tonic::codegen::http::Request<tonic::body::Body>,
				Response = tonic::codegen::http::Response<tonic::body::Body>,
				Error = std::convert::Infallible,
			> + tonic::server::NamedService
			+ Clone
			+ Send
			+ Sync
			+ 'static,
		S::Future: Send + 'static,
	{
		let name = S::NAME;
		let namespace = self.namespace.clone();
		self.record_duplicate(name, &namespace);
		self.entries.push(GrpcServiceRegistration {
			name,
			namespace,
			register: Arc::new(move |builder| {
				builder.add_service(service.clone());
			}),
		});
		self
	}

	/// Appends another router's services and validation errors.
	pub fn merge(mut self, child: Self) -> Self {
		let GrpcRouter {
			entries,
			errors,
			namespace: _,
		} = child;
		let existing_len = self.entries.len();

		self.errors.extend(errors);
		for entry in entries {
			if let Some(first) = self.entries[..existing_len]
				.iter()
				.find(|candidate| candidate.name == entry.name)
			{
				self.errors.push(GrpcRouteError::DuplicateService {
					service: entry.name,
					first_namespace: first.namespace.clone(),
					second_namespace: entry.namespace.clone(),
				});
			}
			self.entries.push(entry);
		}
		self
	}

	/// Associates subsequent registrations with a namespace for diagnostics.
	pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
		self.namespace = Some(namespace.into());
		self
	}

	/// Returns whether no services have been registered.
	pub fn is_empty(&self) -> bool {
		self.entries.is_empty()
	}

	/// Returns the number of registered services.
	pub fn len(&self) -> usize {
		self.entries.len()
	}

	/// Returns the validation errors collected during composition.
	pub fn validation_errors(&self) -> &[GrpcRouteError] {
		&self.errors
	}

	/// Merges root-mounted services or records an unsupported prefix.
	#[doc(hidden)]
	pub fn mount(mut self, prefix: &str, child: Self) -> Self {
		if prefix == "/" {
			return self.merge(child);
		}

		let child_has_entries = !child.entries.is_empty();
		self.errors.extend(child.errors);
		if child_has_entries {
			self.errors.push(GrpcRouteError::NonRootMount {
				prefix: prefix.into(),
			});
		}
		self
	}

	/// Builds Tonic routes from the registered services.
	#[doc(hidden)]
	pub fn build_routes(&self) -> Routes {
		let mut builder = Routes::builder();
		for entry in &self.entries {
			(entry.register)(&mut builder);
		}
		builder.routes()
	}

	fn record_duplicate(&mut self, name: &'static str, namespace: &Option<String>) {
		if let Some(first) = self.entries.iter().find(|entry| entry.name == name) {
			self.errors.push(GrpcRouteError::DuplicateService {
				service: name,
				first_namespace: first.namespace.clone(),
				second_namespace: namespace.clone(),
			});
		}
	}
}

#[cfg(test)]
mod tests {
	use super::{GrpcRouteError, GrpcRouter};
	use std::convert::Infallible;
	use std::future::{Ready, ready};
	use std::task::{Context, Poll};
	use tonic::body::Body;
	use tonic::codegen::Service;
	use tonic::codegen::http::{Request, Response};
	use tonic::server::NamedService;

	#[derive(Clone)]
	struct ChatService;

	impl Service<Request<Body>> for ChatService {
		type Response = Response<Body>;
		type Error = Infallible;
		type Future = Ready<Result<Self::Response, Self::Error>>;

		fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
			Poll::Ready(Ok(()))
		}

		fn call(&mut self, _: Request<Body>) -> Self::Future {
			ready(Ok(Response::new(Body::empty())))
		}
	}

	impl NamedService for ChatService {
		const NAME: &'static str = "chat.ChatService";
	}

	#[derive(Clone)]
	struct HealthService;

	impl Service<Request<Body>> for HealthService {
		type Response = Response<Body>;
		type Error = Infallible;
		type Future = Ready<Result<Self::Response, Self::Error>>;

		fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
			Poll::Ready(Ok(()))
		}

		fn call(&mut self, _: Request<Body>) -> Self::Future {
			ready(Ok(Response::new(Body::empty())))
		}
	}

	impl NamedService for HealthService {
		const NAME: &'static str = "grpc.health.v1.Health";
	}

	#[test]
	fn service_registration_preserves_order_and_builds_from_clones() {
		let router = GrpcRouter::new()
			.with_namespace("chat")
			.service(ChatService)
			.service(HealthService);

		assert_eq!(
			router
				.entries
				.iter()
				.map(|entry| entry.name)
				.collect::<Vec<_>>(),
			["chat.ChatService", "grpc.health.v1.Health"]
		);
		assert_eq!(router.len(), 2);
		assert!(!router.is_empty());
		assert!(router.validation_errors().is_empty());

		let cloned = router.clone();
		let _ = router.build_routes();
		let _ = cloned.build_routes();
	}

	#[test]
	fn merge_and_root_mount_preserve_registration_order() {
		let child = GrpcRouter::new().service(HealthService);
		let merged = GrpcRouter::new().service(ChatService).merge(child.clone());
		let mounted = GrpcRouter::new().service(ChatService).mount("/", child);

		assert_eq!(
			merged
				.entries
				.iter()
				.map(|entry| entry.name)
				.collect::<Vec<_>>(),
			["chat.ChatService", "grpc.health.v1.Health"]
		);
		assert_eq!(
			mounted
				.entries
				.iter()
				.map(|entry| entry.name)
				.collect::<Vec<_>>(),
			["chat.ChatService", "grpc.health.v1.Health"]
		);
		assert!(merged.validation_errors().is_empty());
		assert!(mounted.validation_errors().is_empty());
	}

	#[test]
	fn empty_and_non_root_mount_behave_as_expected() {
		let empty = GrpcRouter::new();
		assert!(empty.is_empty());
		assert_eq!(empty.len(), 0);
		let _ = empty.build_routes();

		let router = GrpcRouter::new().mount("/api", GrpcRouter::new().service(ChatService));
		assert!(router.is_empty());
		assert_eq!(
			router.validation_errors(),
			&[GrpcRouteError::NonRootMount {
				prefix: "/api".into()
			}]
		);
	}

	#[test]
	fn non_root_mount_preserves_nested_validation_errors_without_entries() {
		let child = GrpcRouter::new().mount("/nested", GrpcRouter::new().service(ChatService));
		let router = GrpcRouter::new().mount("/api", child);

		assert!(router.is_empty());
		assert_eq!(
			router.validation_errors(),
			&[GrpcRouteError::NonRootMount {
				prefix: "/nested".into()
			}]
		);
	}

	#[test]
	fn duplicate_services_report_both_namespaces() {
		let router = GrpcRouter::new()
			.with_namespace("chat")
			.service(ChatService)
			.merge(
				GrpcRouter::new()
					.with_namespace("legacy_chat")
					.service(ChatService),
			);

		assert_eq!(
			router.validation_errors(),
			&[GrpcRouteError::DuplicateService {
				service: "chat.ChatService",
				first_namespace: Some("chat".into()),
				second_namespace: Some("legacy_chat".into()),
			}]
		);
	}
}
