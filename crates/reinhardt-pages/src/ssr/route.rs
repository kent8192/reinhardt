//! Route-level loader preparation for server-side rendering.

use super::SsrRenderer;
use crate::cancellation::CancellationSource;
use crate::component::{IntoPage, Page, PageElement};
use crate::reactive::QueryConsumer;
use crate::reactive::query::with_query_client_async;
use crate::router::loader::{
	LoaderStore, RouteLoaderError, loader_cache_id_with_optional_queries, route_context,
	with_loader_store,
};
use crate::router::loader_registry::{LoaderConsumer, LoaderRegistry, execute_loader};
use crate::router::navigation_guard::{
	NavigationContext, NavigationDecision, NavigationGuardError,
};
use crate::router::navigation_guard_registry::{
	NavigationGuardRegistry, execute_navigation_guards,
};
use futures_util::future::try_join_all;
use reinhardt_urls::routers::client_router::ClientRouter;
use std::cell::RefCell;
use std::rc::Rc;
use url::Url;

const REDIRECT_NORMALIZATION_BASE: &str = "http://reinhardt.invalid/";

/// Buffered output from a route render, including its HTTP-like status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsrRouteOutput {
	/// Rendered HTML document or error body.
	pub html: String,
	/// Status selected by route matching or route-preparation failure.
	pub status: u16,
}

impl SsrRenderer {
	/// Prepares all matched layout and leaf loaders before rendering the route.
	///
	/// Successful values are installed in a request-local [`LoaderStore`] and
	/// serialized into the renderer's normal SSR resource payload. Loader
	/// failures expose only their safe public message and status.
	pub async fn render_route_to_string(
		&mut self,
		router: &ClientRouter,
		path: &str,
	) -> SsrRouteOutput {
		self.begin_route_loader_render();
		let query_client = self.request_query_client();
		with_query_client_async(query_client, self.render_route_in_request(router, path)).await
	}

	async fn render_route_in_request(
		&mut self,
		router: &ClientRouter,
		path: &str,
	) -> SsrRouteOutput {
		let Some(matched) = router.match_tree(path) else {
			return SsrRouteOutput {
				html: router.__render_not_found().render_to_string(),
				status: 404,
			};
		};

		let registry = match NavigationGuardRegistry::global() {
			Ok(registry) => registry,
			Err(error) => return navigation_guard_error_output(error),
		};
		let collector = Rc::new(RefCell::new(Vec::new()));
		let cancellation = CancellationSource::new();
		let context = NavigationContext::new(
			path.to_owned(),
			route_context(&matched),
			crate::router::NavigationKind::Initial,
			self.request_query_client(),
			cancellation.handle(),
			QueryConsumer::Navigation(1),
			Some(Rc::clone(&collector)),
		);
		let guard_ids = matched.navigation_guard_ids().to_vec();
		let attempt = async {
			let decision = execute_navigation_guards(&registry, &guard_ids, context.clone())
				.await
				.map_err(RouteAttemptError::Guard)?;
			if decision != NavigationDecision::Allow {
				return Ok(RouteAttempt::Decision(decision));
			}
			let prepared = prepare_route_loaders(&matched)
				.await
				.map_err(RouteAttemptError::Loader)?;
			let decision = execute_navigation_guards(&registry, &guard_ids, context)
				.await
				.map_err(RouteAttemptError::Guard)?;
			if decision != NavigationDecision::Allow {
				return Ok(RouteAttempt::Decision(decision));
			}
			Ok(RouteAttempt::Prepared(prepared))
		};
		let attempt = match tokio::time::timeout(self.route_loader_timeout(), attempt).await {
			Ok(Ok(attempt)) => attempt,
			Ok(Err(RouteAttemptError::Guard(error))) => {
				return navigation_guard_error_output(error);
			}
			Ok(Err(RouteAttemptError::Loader(error))) => {
				return route_loader_error_output(error);
			}
			Err(_) => return route_preparation_timeout_output(),
		};

		let (store, serialized_loaders) = match attempt {
			RouteAttempt::Decision(decision) => {
				return self.navigation_decision_output(router, path, decision);
			}
			RouteAttempt::Prepared(prepared) => prepared,
		};

		let Some(page) = with_loader_store(&store, || render_matched_page(router, &matched)) else {
			return SsrRouteOutput {
				html: PageElement::new("div")
					.attr("data-route-error", "render")
					.child("route render failed")
					.into_page()
					.render_to_string(),
				status: 500,
			};
		};

		for (key, value) in collector.borrow_mut().drain(..) {
			self.state_mut().add_resource_state(key, value);
		}

		for (id, cache_key, value) in serialized_loaders {
			self.state_mut()
				.add_route_loader_state(id.as_str(), value.clone());
			self.state_mut()
				.add_route_loader_query_state(cache_key, value);
		}
		let html = self
			.render_page_into_page_to_string_preserving_resource_state(page)
			.await;
		SsrRouteOutput { html, status: 200 }
	}

	fn navigation_decision_output(
		&mut self,
		router: &ClientRouter,
		request_path: &str,
		decision: NavigationDecision,
	) -> SsrRouteOutput {
		match decision {
			NavigationDecision::Allow => {
				unreachable!("allow decisions are rendered in the attempt")
			}
			NavigationDecision::Redirect { location, .. } => {
				let request_target = match normalize_redirect_target(request_path) {
					Ok(target) => target,
					Err(error) => return navigation_guard_error_output(error),
				};
				let redirect_target = match normalize_redirect_target(&location) {
					Ok(target) => target,
					Err(error) => return navigation_guard_error_output(error),
				};
				if request_target == redirect_target {
					return navigation_guard_error_output(NavigationGuardError::with_status(
						"navigation guard redirect loop detected",
						500,
					));
				}
				self.set_route_redirect_location(redirect_target);
				SsrRouteOutput {
					html: String::new(),
					status: 302,
				}
			}
			NavigationDecision::NotFound => SsrRouteOutput {
				html: router.__render_not_found().render_to_string(),
				status: 404,
			},
			NavigationDecision::Forbidden => SsrRouteOutput {
				html: PageElement::new("div")
					.attr("data-route-error", "navigation-guard")
					.child("navigation forbidden")
					.into_page()
					.render_to_string(),
				status: 403,
			},
		}
	}
}

fn normalize_redirect_target(target: &str) -> Result<String, NavigationGuardError> {
	let base = Url::parse(REDIRECT_NORMALIZATION_BASE).expect("fixed redirect base is valid");
	let mut url = base.join(target).map_err(|error| {
		NavigationGuardError::with_status(
			format!("navigation guard redirect destination is invalid: {error}"),
			500,
		)
	})?;
	if url.origin() != base.origin() {
		return Err(NavigationGuardError::with_status(
			"navigation guard redirect destination must be same-origin",
			500,
		));
	}
	url.set_fragment(None);
	let mut normalized = url.path().to_owned();
	if let Some(query) = url.query() {
		normalized.push('?');
		normalized.push_str(query);
	}
	Ok(normalized)
}

enum RouteAttempt {
	Decision(NavigationDecision),
	Prepared(
		(
			LoaderStore,
			Vec<(
				reinhardt_urls::routers::client_router::RouteLoaderId,
				String,
				serde_json::Value,
			)>,
		),
	),
}

enum RouteAttemptError {
	Guard(NavigationGuardError),
	Loader(RouteLoaderError),
}

fn navigation_guard_error_output(error: NavigationGuardError) -> SsrRouteOutput {
	let status = error.status().unwrap_or(500);
	SsrRouteOutput {
		html: PageElement::new("div")
			.attr("data-route-error", "navigation-guard")
			.child(error.public_message().to_owned())
			.into_page()
			.render_to_string(),
		status,
	}
}

fn route_loader_error_output(error: RouteLoaderError) -> SsrRouteOutput {
	let status = error.status().unwrap_or(500);
	SsrRouteOutput {
		html: PageElement::new("div")
			.attr("data-route-error", "loader")
			.child(error.public_message().to_owned())
			.into_page()
			.render_to_string(),
		status,
	}
}

fn route_preparation_timeout_output() -> SsrRouteOutput {
	SsrRouteOutput {
		html: PageElement::new("div")
			.attr("data-route-error", "route-preparation-timeout")
			.child("route preparation timed out")
			.into_page()
			.render_to_string(),
		status: 504,
	}
}

async fn prepare_route_loaders(
	matched: &reinhardt_urls::routers::client_router::ClientRouteTreeMatch,
) -> Result<
	(
		LoaderStore,
		Vec<(
			reinhardt_urls::routers::client_router::RouteLoaderId,
			String,
			serde_json::Value,
		)>,
	),
	RouteLoaderError,
> {
	let store = LoaderStore::new();
	if matched.loader_ids().is_empty() {
		return Ok((store, Vec::new()));
	}
	let registry = LoaderRegistry::global()
		.map_err(|error| RouteLoaderError::with_status(error.to_string(), 500))?;
	let source = CancellationSource::new();
	let handle = source.handle();
	let context = route_context(matched);
	let mut loader_ids = Vec::new();
	for id in matched.loader_ids().iter().copied() {
		if !loader_ids.contains(&id) {
			loader_ids.push(id);
		}
	}
	let results = match try_join_all(loader_ids.into_iter().map(|id| {
		execute_loader(
			&registry,
			id,
			&context,
			handle.clone(),
			LoaderConsumer::Maintenance,
		)
	}))
	.await
	{
		Ok(results) => results,
		Err(error) => {
			source.cancel();
			return Err(error);
		}
	};
	let mut serialized_loaders = Vec::with_capacity(results.len());
	for result in results {
		let prepared = result;
		let id = prepared.id();
		let registration = registry
			.get(id)
			.map_err(|error| RouteLoaderError::with_status(error.to_string(), 500))?;
		let cache_key = loader_cache_id_with_optional_queries(
			id,
			&context,
			registration.inputs,
			registry.optional_query_inputs(id),
		)
		.map_err(|error| RouteLoaderError::with_status(error.to_string(), 400))?;
		let serialized = prepared.serialized().clone();
		store.insert_prepared(prepared);
		serialized_loaders.push((id, cache_key, serialized));
	}
	Ok((store, serialized_loaders))
}

fn render_matched_page(
	router: &ClientRouter,
	matched: &reinhardt_urls::routers::client_router::ClientRouteTreeMatch,
) -> Option<Page> {
	// SAFETY: SSR has completed every asynchronous navigation guard for this
	// exact match before rendering the protected route.
	let mut page = unsafe { router.__render_tree_leaf(matched) }?;
	for index in (0..matched.layouts().len()).rev() {
		page = unsafe {
			router.__render_tree_layout(matched, index, crate::component::Outlet::inline(page))
		}?;
	}
	Some(page)
}
