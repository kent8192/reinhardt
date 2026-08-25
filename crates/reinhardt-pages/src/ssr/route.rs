//! Route-level loader preparation for server-side rendering.

use super::SsrRenderer;
use crate::cancellation::CancellationSource;
use crate::component::{IntoPage, Page, PageElement};
use crate::reactive::query::with_query_client_async;
use crate::router::loader::{
	LoaderStore, RouteLoaderError, loader_cache_id_with_optional_queries, route_context,
	with_loader_store,
};
use crate::router::loader_registry::{LoaderConsumer, LoaderRegistry, execute_loader};
use futures_util::future::try_join_all;
use reinhardt_urls::routers::client_router::ClientRouter;

/// Buffered output from a route render, including its HTTP-like status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsrRouteOutput {
	/// Rendered HTML document or error body.
	pub html: String,
	/// Status selected by route matching or loader failure.
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
				html: router.render_path(path).render_to_string(),
				status: 404,
			};
		};

		let (store, serialized_loaders) = match tokio::time::timeout(
			self.route_loader_timeout(),
			prepare_route_loaders(&matched),
		)
		.await
		{
			Ok(result) => match result {
				Ok(prepared) => prepared,
				Err(error) => {
					let status = error.status().unwrap_or(500);
					return SsrRouteOutput {
						html: PageElement::new("div")
							.attr("data-route-error", "loader")
							.child(error.public_message().to_owned())
							.into_page()
							.render_to_string(),
						status,
					};
				}
			},
			Err(_) => {
				return SsrRouteOutput {
					html: PageElement::new("div")
						.attr("data-route-error", "loader-timeout")
						.child("route loader timed out")
						.into_page()
						.render_to_string(),
					status: 504,
				};
			}
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
	let mut page = router.__render_tree_leaf(matched)?;
	for index in (0..matched.layouts().len()).rev() {
		page =
			router.__render_tree_layout(matched, index, crate::component::Outlet::inline(page))?;
	}
	Some(page)
}
