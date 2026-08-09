//! Introspection and URL reversal helpers for [`ServerRouter`].
//!
//! Exposes router metadata (prefix, namespace, middleware registry),
//! recursive route enumeration, and namespace-aware URL reversal.

use super::ServerRouter;
use super::types::{
	MiddlewareInfo, MountedRouteContract, RouteContractMetadata, RouteInfo, join_path,
};
use hyper::Method;

fn introspection_route_name(name: &Option<String>) -> Option<String> {
	name.as_deref()
		.map(|name| name.strip_prefix('!').unwrap_or(name).to_string())
}

fn mounted_contract_name(name: &Option<String>, namespace: Option<&str>) -> Option<String> {
	introspection_route_name(name).map(|name| match namespace {
		Some(namespace) => format!("{namespace}:{name}"),
		None => name,
	})
}

impl ServerRouter {
	/// Get the prefix of this router
	pub fn prefix(&self) -> &str {
		&self.prefix
	}

	/// Get the namespace of this router
	pub fn namespace(&self) -> Option<&str> {
		self.namespace.as_deref()
	}

	/// Get the number of child routers
	pub fn children_count(&self) -> usize {
		self.children.len()
	}

	/// Get all registered middleware information
	///
	/// Returns a deduplicated list of middleware registered on this router
	/// and all child routers. The order reflects registration order.
	pub fn get_registered_middleware(&self) -> Vec<MiddlewareInfo> {
		let mut all = self.middleware_names.clone();

		// Collect from children recursively
		for child in &self.children {
			all.extend(child.get_registered_middleware());
		}

		// Deduplicate while preserving order
		let mut seen = std::collections::HashSet::new();
		all.retain(|info| seen.insert(info.type_name.clone()));
		all
	}

	/// Get all routes from this router and its children
	///
	/// Returns a vector of tuples containing (full_path, name, namespace, methods).
	/// This recursively collects routes from all child routers.
	///
	/// # Examples
	///
	/// ```ignore
	/// let router = ServerRouter::new()
	///     .with_prefix("/api/v1")
	///     .endpoint(users);
	///
	/// let routes = router.get_all_routes();
	/// // Returns: [("/api/v1/users", Some("users"), None, vec![Method::GET])]
	/// ```
	pub fn get_all_routes(&self) -> RouteInfo {
		let mut routes = Vec::new();

		// Collect routes from this router
		for route in &self.routes {
			let full_path = if self.prefix.is_empty() {
				route.path.clone()
			} else {
				join_path(&self.prefix, &route.path)
			};

			routes.push((
				full_path,
				introspection_route_name(&route.name),
				route.namespace.clone().or_else(|| self.namespace.clone()),
				vec![], // Method-agnostic handlers accept all HTTP methods (shown as "ALL" in showurls)
			));
		}

		// Collect endpoint routes
		for func_route in &self.functions {
			let full_path = if self.prefix.is_empty() {
				func_route.path.clone()
			} else {
				join_path(&self.prefix, &func_route.path)
			};

			routes.push((
				full_path,
				introspection_route_name(&func_route.name),
				self.namespace.clone(), // Use router's namespace
				vec![func_route.method.clone()],
			));
		}

		// Collect view routes
		for view_route in &self.views {
			let full_path = if self.prefix.is_empty() {
				view_route.path.clone()
			} else {
				join_path(&self.prefix, &view_route.path)
			};

			routes.push((
				full_path,
				None,                   // View routes don't have names
				self.namespace.clone(), // Use router's namespace
				vec![], // Class-based views handle method dispatch internally (accepts all methods)
			));
		}

		// Collect ViewSet routes
		#[cfg(feature = "viewsets")]
		for prefix in self.viewsets.keys() {
			// Same composition rule as `collect_routes_recursive` below: normalize
			// the viewset prefix to a single leading `/` and join via
			// `join_prefix_path` so a trailing `/` on `self.prefix` and a leading
			// `/` on `prefix` do not stack into a double / triple slash. Refs
			// Issue #4581.
			let prefix_normalized = format!("/{}", prefix.trim_matches('/'));
			let base_path =
				crate::routers::path_utils::join_prefix_path(&self.prefix, &prefix_normalized);

			// ViewSets generate standard CRUD routes
			let viewset_routes = vec![
				(format!("{}/", base_path), vec![Method::GET, Method::POST]),
				(
					format!("{}/<id>/", base_path),
					vec![Method::GET, Method::PUT, Method::DELETE],
				),
			];

			for (path, methods) in viewset_routes {
				routes.push((
					path,
					None,                   // ViewSet routes don't have individual names
					self.namespace.clone(), // Use router's namespace
					methods,
				));
			}
		}

		// Recursively collect from child routers.
		//
		// `child.get_all_routes()` already prepends `child.prefix` to each route
		// it owns, but it has no knowledge of *this* router's prefix. We must
		// therefore prepend `self.prefix` here so nested routers report fully
		// qualified paths (e.g., parent `/api` + child `/users/` + route `/foo`
		// yields `/api/users/foo`, not `/users/foo`).
		for child in &self.children {
			for (path, name, namespace, methods) in child.get_all_routes() {
				let full_path = if self.prefix.is_empty() {
					path
				} else {
					join_path(&self.prefix, &path)
				};

				// Combine namespaces (parent:child)
				let combined_namespace = match (self.namespace.as_ref(), namespace.as_ref()) {
					(Some(parent), Some(child)) => Some(format!("{}:{}", parent, child)),
					(Some(parent), None) => Some(parent.clone()),
					(None, Some(child)) => Some(child.clone()),
					(None, None) => None,
				};

				routes.push((full_path, name, combined_namespace, methods));
			}
		}

		routes
	}

	/// Get every executable route with application-contract metadata.
	///
	/// Routes are resolved through mounted prefixes and namespaces while preserving
	/// every mount instance. Erased handlers must provide metadata explicitly.
	pub fn get_mounted_route_contracts(&self) -> Result<Vec<MountedRouteContract>, String> {
		self.collect_mounted_route_contracts(None, "")
	}

	fn collect_mounted_route_contracts(
		&self,
		parent_namespace: Option<&str>,
		parent_prefix: &str,
	) -> Result<Vec<MountedRouteContract>, String> {
		let full_namespace = self.get_full_namespace(parent_namespace);
		let current_prefix =
			crate::routers::path_utils::join_prefix_path(parent_prefix, &self.prefix);
		let mut contracts = Vec::new();

		for route in &self.routes {
			let path = crate::routers::path_utils::join_prefix_path(&current_prefix, &route.path);
			let metadata = route.contract_metadata.clone().ok_or_else(|| {
				format!(
					"mounted route `{path}` has no application-contract metadata; use a typed registration method or handler_arc_with_contract_metadata"
				)
			})?;
			for method in [
				Method::GET,
				Method::POST,
				Method::PUT,
				Method::DELETE,
				Method::PATCH,
			] {
				contracts.push(MountedRouteContract {
					path: path.clone(),
					method,
					name: mounted_contract_name(&route.name, full_namespace.as_deref()),
					metadata: metadata.clone(),
				});
			}
		}

		for route in &self.functions {
			contracts.push(MountedRouteContract {
				path: crate::routers::path_utils::join_prefix_path(&current_prefix, &route.path),
				method: route.method.clone(),
				name: mounted_contract_name(&route.name, full_namespace.as_deref()),
				metadata: route.metadata.clone(),
			});
		}

		for route in &self.views {
			let path = crate::routers::path_utils::join_prefix_path(&current_prefix, &route.path);
			for method in [
				Method::GET,
				Method::POST,
				Method::PUT,
				Method::DELETE,
				Method::PATCH,
			] {
				contracts.push(MountedRouteContract {
					path: path.clone(),
					method,
					name: mounted_contract_name(&route.name, full_namespace.as_deref()),
					metadata: route.metadata.clone(),
				});
			}
		}

		#[cfg(feature = "viewsets")]
		for (prefix, viewset) in &self.viewsets {
			let prefix = format!("/{}", prefix.trim_matches('/'));
			let base_path = crate::routers::path_utils::join_prefix_path(&current_prefix, &prefix);
			let detail_path = format!("{}/{{{}}}/", base_path, viewset.get_lookup_field());
			let metadata = RouteContractMetadata {
				handler: viewset.type_name().to_string(),
				authentication: if viewset.requires_login() {
					reinhardt_core::endpoint::AuthProtection::Protected
				} else {
					reinhardt_core::endpoint::AuthProtection::None
				},
				guard: None,
			};
			for (path, method, action) in [
				(format!("{base_path}/"), Method::GET, "list"),
				(format!("{base_path}/"), Method::POST, "create"),
				(detail_path.clone(), Method::GET, "retrieve"),
				(detail_path.clone(), Method::PUT, "update"),
				(detail_path, Method::DELETE, "destroy"),
			] {
				contracts.push(MountedRouteContract {
					path,
					method,
					name: None,
					metadata: RouteContractMetadata {
						handler: format!("{}::{action}", viewset.type_name()),
						..metadata.clone()
					},
				});
			}
		}

		for child in &self.children {
			contracts.extend(
				child
					.collect_mounted_route_contracts(full_namespace.as_deref(), &current_prefix)?,
			);
		}

		Ok(contracts)
	}

	/// Get the fully qualified namespace for this router
	///
	/// Returns the complete namespace chain from root to this router.
	/// For example, if this router has namespace "users" and its parent has "v1",
	/// this returns "v1:users".
	///
	/// # Arguments
	///
	/// * `parent_namespace` - The parent router's namespace (if any)
	///
	/// # Examples
	///
	/// ```ignore
	/// let router = ServerRouter::new().with_namespace("users");
	/// assert_eq!(router.get_full_namespace(Some("v1")), Some("v1:users".to_string()));
	/// assert_eq!(router.get_full_namespace(None), Some("users".to_string()));
	/// ```
	pub fn get_full_namespace(&self, parent_namespace: Option<&str>) -> Option<String> {
		match (parent_namespace, self.namespace.as_deref()) {
			(Some(parent), Some(child)) => Some(format!("{}:{}", parent, child)),
			(Some(parent), None) => Some(parent.to_string()),
			(None, Some(child)) => Some(child.to_string()),
			(None, None) => None,
		}
	}

	/// Register all routes with the URL reverser
	///
	/// This recursively registers all routes from this router and its children
	/// with their fully qualified names (namespace:name format).
	///
	/// # Examples
	///
	/// ```ignore
	/// let mut router = ServerRouter::new()
	///     .with_namespace("v1");
	///
	/// // After registering routes, you can reverse them:
	/// router.register_all_routes();
	/// let url = router.reverse("v1:users:detail", &[("id", "123")]);
	/// ```
	#[must_use]
	pub fn register_all_routes(&mut self) -> Vec<String> {
		// Flush any middleware-contributed DI registrations that were staged
		// when no `InjectionContext` was attached, walking children as well.
		// If a context was attached later via `with_di_context` (or inherited
		// via `mount`), the staging list is already empty. If no context is
		// ever attached on this subtree, push the staged registrations onto
		// the global deferred list so server startup can apply them. See #4426.
		Self::flush_pending_middleware_di_recursive(self);
		let registrations = self.collect_routes_recursive(None, "");
		let mut errors = Vec::new();
		for (name, path) in registrations {
			if let Err(e) = self.reverser.register_path(&name, &path) {
				errors.push(e);
			}
		}
		errors
	}

	/// Recursively drain middleware-contributed DI registrations that were
	/// staged before a context could be attached, pushing them to the global
	/// deferred list when the owning subtree has no `InjectionContext`. See
	/// #4426.
	fn flush_pending_middleware_di_recursive(router: &mut ServerRouter) {
		if !router.pending_middleware_di.is_empty() && router.di_context.is_none() {
			let pending = std::mem::take(&mut router.pending_middleware_di);
			crate::routers::register_di_registrations(pending);
		}
		for child in router.children.iter_mut() {
			Self::flush_pending_middleware_di_recursive(child);
		}
	}

	/// Returns a reference to the internal URL reverser.
	///
	/// The reverser is populated after [`register_all_routes()`](Self::register_all_routes)
	/// has been called.
	pub fn reverser(&self) -> &crate::routers::UrlReverser {
		&self.reverser
	}

	/// Register an alias for a route name in this router's reverser.
	///
	/// See `UrlReverser::add_name_alias` for details.
	pub fn add_name_alias(&mut self, alias: &str, canonical: &str) {
		self.reverser.add_name_alias(alias, canonical);
	}

	/// Recursively collect all routes with accumulated prefixes and namespaces.
	///
	/// Returns a list of `(qualified_name, full_path)` pairs to be registered
	/// in the root router's reverser.
	pub(crate) fn collect_routes_recursive(
		&self,
		parent_namespace: Option<&str>,
		parent_prefix: &str,
	) -> Vec<(String, String)> {
		let full_namespace = self.get_full_namespace(parent_namespace);
		let current_prefix =
			crate::routers::path_utils::join_prefix_path(parent_prefix, &self.prefix);
		let mut registrations = Vec::new();

		// Collect routes from this router
		for route in &self.routes {
			if let Some(name) = &route.name {
				let qualified_name = if let Some(ref ns) = full_namespace {
					format!("{}:{}", ns, name)
				} else {
					name.clone()
				};

				let full_path =
					crate::routers::path_utils::join_prefix_path(&current_prefix, &route.path);
				registrations.push((qualified_name, full_path));
			}
		}

		// Collect endpoint routes
		for func_route in &self.functions {
			if let Some(ref name) = func_route.name {
				let qualified_name = if let Some(ref ns) = full_namespace {
					format!("{}:{}", ns, name)
				} else {
					name.clone()
				};

				let full_path =
					crate::routers::path_utils::join_prefix_path(&current_prefix, &func_route.path);
				registrations.push((qualified_name, full_path));
			}
		}

		// Collect view routes
		for view_route in &self.views {
			if let Some(ref name) = view_route.name {
				let qualified_name = if let Some(ref ns) = full_namespace {
					format!("{}:{}", ns, name)
				} else {
					name.clone()
				};

				let full_path =
					crate::routers::path_utils::join_prefix_path(&current_prefix, &view_route.path);
				registrations.push((qualified_name, full_path));
			}
		}

		// Collect ViewSet routes with standard names (Django convention: basename, not prefix)
		#[cfg(feature = "viewsets")]
		for (prefix, viewset) in &self.viewsets {
			// Normalize the viewset prefix to a single leading `/` (and no
			// trailing `/`) before composing it under `current_prefix` with
			// `join_prefix_path`. Function and view routes already go through
			// `join_prefix_path` (see above), but the viewset branch used to
			// concatenate with a raw `format!("{}/{}", current_prefix, prefix)`,
			// which produced a triple slash whenever `current_prefix` carried
			// a trailing `/` (e.g. from `UnifiedRouter::mount("/api/", ...)`)
			// AND `prefix` carried a leading `/` (the common user input from
			// `.viewset("/snippets-viewset", ...)`). Routing the viewset prefix
			// through the same slash-collapsing helper mirrors the runtime
			// `register_viewset` normalization (see `router.rs::register_viewset`,
			// which uses `prefix.trim_matches('/')`) and brings reversal in
			// line with function-route composition. Refs Issue #4581.
			let prefix_normalized = format!("/{}", prefix.trim_matches('/'));
			let base_path =
				crate::routers::path_utils::join_prefix_path(&current_prefix, &prefix_normalized);

			let basename = viewset.get_basename();
			let lookup_field = viewset.get_lookup_field();
			// Use Reinhardt's `{name}` placeholder syntax (the format
			// `PathPattern::new` parses) rather than Django's legacy `<name>`
			// notation. Without this, `router.reverse("snippet-detail",
			// &[("id", "42")])` returned the literal pattern
			// `/snippet/<id>/` instead of substituting the parameter — the
			// pattern parser only recognises `{id}` / `{<int:id>}` forms.
			// Refs Issue #4507.
			let viewset_routes = [
				(format!("{}-list", basename), format!("{}/", base_path)),
				(
					format!("{}-detail", basename),
					format!("{}/{{{}}}/", base_path, lookup_field),
				),
			];

			for (name, path) in viewset_routes {
				let qualified_name = if let Some(ref ns) = full_namespace {
					format!("{}:{}", ns, name)
				} else {
					name
				};

				registrations.push((qualified_name, path));
			}

			// Also collect routes for `#[action]`-decorated extra actions so
			// `router.reverse("<namespace>:<basename>-<action>", ...)` resolves
			// against the same routing table as the typed accessors emitted
			// by `#[viewset]` + `#[action]`. Without this, the typed accessor
			// `urls.server().<app>().<action>(id)` would panic with
			// `Route '<ns>:<basename>-<action>' not found in router`.
			// Refs Issue #4507.
			for action in viewset.get_extra_actions() {
				// `#[action]` enforces that `url_path` starts with `/`
				// (see `crates/reinhardt-core/macros/src/action.rs`). When
				// concatenated with `format!("{}/.../{}/", base_path, ...)`,
				// a stored value of `/children` would yield `.../{id}//children/`
				// (double slash). Strip the leading `/` here so the segment
				// joins cleanly, regardless of whether the value originated
				// from `#[action(url_path = "/...")]` or the
				// `action.name`-derived fallback (which has no slash).
				let raw_url_path = action.url_path.as_deref().unwrap_or(action.name.as_str());
				let action_url_path = raw_url_path.trim_start_matches('/');
				let action_url_name = action.url_name.as_deref().unwrap_or(action.name.as_str());

				let action_path = if action.detail {
					format!("{}/{{{}}}/{}/", base_path, lookup_field, action_url_path)
				} else {
					format!("{}/{}/", base_path, action_url_path)
				};
				let action_name = format!("{}-{}", basename, action_url_name);
				let qualified_name = if let Some(ref ns) = full_namespace {
					format!("{}:{}", ns, action_name)
				} else {
					action_name
				};
				registrations.push((qualified_name, action_path));
			}
		}

		// Recursively collect child routes
		for child in &self.children {
			registrations
				.extend(child.collect_routes_recursive(full_namespace.as_deref(), &current_prefix));
		}

		registrations
	}

	/// Reverse a URL by route name
	///
	/// Supports hierarchical namespace notation (e.g., "v1:users:detail").
	///
	/// # Arguments
	///
	/// * `name` - The route name, optionally with namespace (e.g., "users-detail" or "v1:users-detail")
	/// * `params` - URL parameters as key-value pairs
	///
	/// # Examples
	///
	/// ```ignore
	/// let router = ServerRouter::new()
	///     .with_namespace("v1");
	///
	/// // Reverse with namespace
	/// let url = router.reverse("v1:users:detail", &[("id", "123")]).unwrap();
	/// assert_eq!(url, "/users/123/");
	///
	/// // Reverse without namespace (searches all routes)
	/// let url = router.reverse("users-detail", &[("id", "123")]).unwrap();
	/// ```
	pub fn reverse(&self, name: &str, params: &[(&str, &str)]) -> Option<String> {
		// Try own reverser first
		if let Ok(url) = self.reverser.reverse_with(name, params) {
			return Some(url);
		}

		// Try child routers
		for child in &self.children {
			if let Some(url) = child.reverse(name, params) {
				return Some(url);
			}
		}

		None
	}
}

#[cfg(all(test, feature = "viewsets"))]
mod viewset_path_composition_tests {
	use super::*;
	use async_trait::async_trait;
	use reinhardt_http::{Request, Response, Result};
	use reinhardt_views::viewsets::{Action, ViewSet};
	use rstest::rstest;

	/// Minimal `ViewSet` fixture used purely for URL-pattern composition
	/// assertions — the dispatch body is irrelevant because these tests only
	/// inspect what `collect_routes_recursive` reports back.
	#[derive(Debug, Clone)]
	struct DummyViewSet {
		basename: String,
	}

	#[async_trait]
	impl ViewSet for DummyViewSet {
		fn get_basename(&self) -> &str {
			&self.basename
		}

		async fn dispatch(&self, _request: Request, _action: Action) -> Result<Response> {
			Ok(Response::ok())
		}
	}

	fn find_path<'a>(routes: &'a [(String, String)], name: &str) -> Option<&'a str> {
		routes
			.iter()
			.find_map(|(n, p)| if n == name { Some(p.as_str()) } else { None })
	}

	/// Regression for Issue #4581: when a router carries a `with_prefix("/api/")`
	/// (which is what `UnifiedRouter::mount("/api/", child)` plants on the
	/// child) AND the user passes a viewset prefix that starts with `/`
	/// (the natural form, e.g. `.viewset("/snippets-viewset", _)`), the
	/// typed-accessor URL must be a single-slashed `/api/snippets-viewset/`,
	/// not the previously observed triple-slash `/api///snippets-viewset/`.
	#[rstest]
	fn viewset_under_mount_prefix_emits_single_slash() {
		// Arrange — mirror what `UnifiedRouter::mount("/api/", url_patterns())`
		// plants: the child ServerRouter ends up with `prefix = "/api/"` and a
		// viewset registered at `/snippets-viewset`.
		let router = ServerRouter::new().with_prefix("/api/").viewset(
			"/snippets-viewset",
			DummyViewSet {
				basename: "snippet".to_string(),
			},
		);

		// Act — `register_all_routes` calls into this with `parent_prefix=""`,
		// so we replicate that contract directly.
		let routes = router.collect_routes_recursive(None, "");

		// Assert — both the list and detail forms must compose to single-slash
		// paths. The previously broken values were `/api///snippets-viewset/`
		// and `/api///snippets-viewset/{id}/`.
		assert_eq!(
			find_path(&routes, "snippet-list"),
			Some("/api/snippets-viewset/"),
			"snippet-list path corrupted (got {:?})",
			find_path(&routes, "snippet-list"),
		);
		assert_eq!(
			find_path(&routes, "snippet-detail"),
			Some("/api/snippets-viewset/{id}/"),
			"snippet-detail path corrupted (got {:?})",
			find_path(&routes, "snippet-detail"),
		);
	}

	/// Sanity check — the historical "no mount" path is unchanged. Both
	/// fed-prefix-less invocations and explicit-no-leading-slash prefixes
	/// must still produce single-slashed URLs.
	#[rstest]
	#[case::leading_slash_no_mount("", "/snippets-viewset", "/snippets-viewset/")]
	#[case::no_leading_slash_no_mount("", "snippets-viewset", "/snippets-viewset/")]
	#[case::mount_with_no_leading_slash_viewset(
		"/api/",
		"snippets-viewset",
		"/api/snippets-viewset/"
	)]
	#[case::mount_without_trailing_slash("/api", "/snippets-viewset", "/api/snippets-viewset/")]
	fn viewset_prefix_normalization_matrix(
		#[case] router_prefix: &str,
		#[case] viewset_prefix: &str,
		#[case] expected_list_path: &str,
	) {
		// Arrange
		let mut builder = ServerRouter::new();
		if !router_prefix.is_empty() {
			builder = builder.with_prefix(router_prefix);
		}
		let router = builder.viewset(
			viewset_prefix,
			DummyViewSet {
				basename: "snippet".to_string(),
			},
		);

		// Act
		let routes = router.collect_routes_recursive(None, "");

		// Assert
		assert_eq!(find_path(&routes, "snippet-list"), Some(expected_list_path));
	}
}
