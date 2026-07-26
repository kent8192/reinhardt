//! Route compilation and validation for [`ServerRouter`].
//!
//! These methods build the per-method `matchit` routers and surface any
//! pattern errors at startup. Compilation is lazy and caches the first result.

use super::ServerRouter;
#[cfg(feature = "viewsets")]
use super::handlers::ViewSetHandler;
use super::types::{CompiledRoutes, RouteHandler};
use hyper::Method;
#[cfg(feature = "viewsets")]
use reinhardt_views::viewsets::Action;
use std::borrow::Cow;
use std::sync::Arc;

fn extract_path_param_names(path: &str) -> Arc<[String]> {
	let mut names = Vec::new();
	let mut search_start = 0;
	while let Some(start_offset) = path[search_start..].find('{') {
		let start = search_start + start_offset;
		let after_start = &path[start + 1..];
		if after_start.starts_with('{') {
			search_start = start + 2;
			continue;
		}
		let Some(end) = after_start.find('}') else {
			break;
		};
		let raw_name = &after_start[..end];
		let raw_name = raw_name.strip_prefix('*').unwrap_or(raw_name);
		let name = raw_name.split_once(':').map_or(raw_name, |(name, _)| name);
		if !name.is_empty() {
			names.push(name.to_string());
		}
		search_start = start + end + 2;
	}
	names.into()
}

fn uses_matchit_escape(path: &str) -> bool {
	path.contains("{{") || path.contains("}}")
}

fn insert_compiled_route(
	compiled: &mut CompiledRoutes,
	method: &Method,
	route_path: &str,
	route_handler: RouteHandler,
) -> Result<(), String> {
	let exact_handler = (route_handler.param_names.is_empty() && !uses_matchit_escape(route_path))
		.then(|| route_handler.clone());
	compiled
		.router_for_method_mut(method)
		.ok_or_else(|| format!("unsupported HTTP method '{method}'"))?
		.insert(route_path, route_handler)
		.map_err(|error| error.to_string())?;
	if let Some(route_handler) = exact_handler {
		compiled
			.exact_for_method_mut(method)
			.ok_or_else(|| format!("unsupported HTTP method '{method}'"))?
			.insert(route_path.to_string(), route_handler);
	}
	Ok(())
}

struct MountedFunctionRoute {
	origin: usize,
	path: String,
	method: Method,
	handler: RouteHandler,
}

impl ServerRouter {
	/// Compile all routes into matchit routers.
	///
	/// This should be called after all routes have been registered.
	/// It converts patterns like "/users/{id}" to matchit format.
	///
	/// Returns a list of route compilation errors (if any). Empty list means
	/// all routes compiled successfully.
	pub(crate) fn compile_routes(&self) -> Vec<String> {
		self.compiled_routes().errors.clone()
	}

	pub(crate) fn compiled_routes(&self) -> &CompiledRoutes {
		self.compiled_routes
			.get_or_init(|| self.compile_routes_once())
	}

	pub(crate) fn invalidate_compiled_routes(&mut self) {
		self.compiled_routes.take();
	}

	fn collect_configuration_errors_recursive(&self) -> Vec<String> {
		let mut errors = self.configuration_errors.clone();
		for child in &self.children {
			errors.extend(child.collect_configuration_errors_recursive());
		}
		errors
	}

	fn compile_routes_recursive(&self) -> Vec<String> {
		let mut errors = self.compile_routes();
		for child in &self.children {
			errors.extend(child.compile_routes_recursive());
		}
		errors.extend(self.compile_mounted_function_route_conflicts());
		errors
	}

	fn compile_mounted_function_route_conflicts(&self) -> Vec<String> {
		let mut routes = Vec::new();
		let mut next_origin = 0;
		self.collect_mounted_function_routes("", &mut next_origin, &mut routes);

		let mut errors = Vec::new();
		for current_index in 0..routes.len() {
			let current = &routes[current_index];
			for previous in &routes[..current_index] {
				if current.origin == previous.origin || current.method != previous.method {
					continue;
				}
				if mounted_paths_overlap(&previous.path, &current.path) {
					errors.push(format!(
						"Failed to compile mounted route '{}' ({}): overlaps mounted route '{}'",
						current.path, current.method, previous.path
					));
					break;
				}
				let mut compiled = CompiledRoutes::default();
				if insert_compiled_route(
					&mut compiled,
					&previous.method,
					&previous.path,
					previous.handler.clone(),
				)
				.is_err()
				{
					continue;
				}
				if let Err(error) = insert_compiled_route(
					&mut compiled,
					&current.method,
					&current.path,
					current.handler.clone(),
				) {
					errors.push(format!(
						"Failed to compile mounted route '{}' ({}): {error}",
						current.path, current.method
					));
					break;
				}
			}
		}
		errors
	}

	fn collect_mounted_function_routes(
		&self,
		parent_prefix: &str,
		next_origin: &mut usize,
		routes: &mut Vec<MountedFunctionRoute>,
	) {
		let current_prefix =
			crate::routers::path_utils::join_prefix_path(parent_prefix, &self.prefix);
		let origin = *next_origin;
		*next_origin += 1;
		for func_route in &self.functions {
			let route_path = Self::strip_prefix_normalized(&self.prefix, &func_route.path)
				.unwrap_or_else(|| Cow::Borrowed(&func_route.path));
			routes.push(MountedFunctionRoute {
				origin,
				path: crate::routers::path_utils::join_prefix_path(&current_prefix, &route_path),
				method: func_route.method.clone(),
				handler: RouteHandler {
					handler: func_route.handler.clone(),
					sync_handler: func_route.sync_handler.clone(),
					requestless_sync_handler: func_route.requestless_sync_handler.clone(),
					middleware: func_route.middleware.clone(),
					param_names: extract_path_param_names(&func_route.path),
				},
			});
		}
		for child in &self.children {
			child.collect_mounted_function_routes(&current_prefix, next_origin, routes);
		}
	}

	fn compile_routes_once(&self) -> CompiledRoutes {
		let mut compiled = CompiledRoutes::default();

		// Compile endpoint routes
		for func_route in &self.functions {
			let route_handler = RouteHandler {
				handler: func_route.handler.clone(),
				sync_handler: func_route.sync_handler.clone(),
				requestless_sync_handler: func_route.requestless_sync_handler.clone(),
				middleware: func_route.middleware.clone(),
				param_names: extract_path_param_names(&func_route.path),
			};

			// Strip prefix from route path to avoid double-prefix matching.
			// Routes may be registered with absolute paths that already include the prefix
			// (e.g., server functions register as "/api/server_fn/login"). Since resolve()
			// strips the prefix from incoming request paths before matching against matchit,
			// we must also strip the prefix here during compilation.
			let route_path_owned = Self::strip_prefix_normalized(&self.prefix, &func_route.path)
				.unwrap_or_else(|| Cow::Borrowed(&func_route.path));
			let route_path: &str = &route_path_owned;

			// matchit uses {name} format which matches our pattern
			if let Err(e) =
				insert_compiled_route(&mut compiled, &func_route.method, route_path, route_handler)
			{
				compiled.errors.push(format!(
					"Failed to compile route '{}' ({}): {}",
					func_route.path, func_route.method, e
				));
			}
		}

		// Compile view routes (views handle all methods internally)
		for view_route in &self.views {
			let route_handler = RouteHandler {
				handler: view_route.handler.clone(),
				sync_handler: view_route.sync_handler.clone(),
				requestless_sync_handler: view_route.requestless_sync_handler.clone(),
				middleware: view_route.middleware.clone(),
				param_names: extract_path_param_names(&view_route.path),
			};

			// Strip prefix from route path (same reason as endpoint routes above)
			let route_path_owned = Self::strip_prefix_normalized(&self.prefix, &view_route.path)
				.unwrap_or_else(|| Cow::Borrowed(&view_route.path));
			let route_path: &str = &route_path_owned;

			// Register view for all common HTTP methods
			for method in [
				Method::GET,
				Method::POST,
				Method::PUT,
				Method::DELETE,
				Method::PATCH,
			] {
				if let Err(e) =
					insert_compiled_route(&mut compiled, &method, route_path, route_handler.clone())
				{
					compiled.errors.push(format!(
						"Failed to compile view route '{}': {}",
						view_route.path, e
					));
				}
			}
		}

		// Compile raw routes (routes handle all methods internally)
		for route in &self.routes {
			let route_handler = RouteHandler {
				handler: route.handler_arc(),
				sync_handler: route.sync_handler_arc(),
				requestless_sync_handler: route.requestless_sync_handler_arc(),
				middleware: route.middleware.clone(),
				param_names: extract_path_param_names(&route.path),
			};

			// Strip prefix from route path (same reason as endpoint routes above)
			let route_path_owned = Self::strip_prefix_normalized(&self.prefix, &route.path)
				.unwrap_or_else(|| Cow::Borrowed(&route.path));
			let route_path: &str = &route_path_owned;

			// Register raw route for all common HTTP methods
			for method in [
				Method::GET,
				Method::POST,
				Method::PUT,
				Method::DELETE,
				Method::PATCH,
			] {
				if let Err(e) =
					insert_compiled_route(&mut compiled, &method, route_path, route_handler.clone())
				{
					compiled.errors.push(format!(
						"Failed to compile raw route '{}': {}",
						route.path, e
					));
				}
			}
		}

		#[cfg(feature = "viewsets")]
		self.compile_viewset_routes(&mut compiled);

		compiled
	}

	#[cfg(feature = "viewsets")]
	fn compile_viewset_routes(&self, compiled: &mut CompiledRoutes) {
		// ViewSet base_path must NOT include self.prefix because resolve() strips
		// the prefix from incoming request paths before matching against matchit.
		for (prefix, viewset) in &self.viewsets {
			let base_path = format!("/{}", prefix.trim_start_matches('/'));

			let collection_path = format!("{}/", base_path.trim_end_matches('/'));

			let list_handler = RouteHandler {
				handler: Arc::new(ViewSetHandler {
					viewset: viewset.clone(),
					action: Action::list(),
				}),
				sync_handler: None,
				requestless_sync_handler: None,
				middleware: Vec::new(),
				param_names: extract_path_param_names(&collection_path),
			};
			if let Err(e) =
				insert_compiled_route(compiled, &Method::GET, &collection_path, list_handler)
			{
				compiled.errors.push(format!(
					"Failed to compile ViewSet list route '{}': {}",
					collection_path, e
				));
			}

			let create_handler = RouteHandler {
				handler: Arc::new(ViewSetHandler {
					viewset: viewset.clone(),
					action: Action::create(),
				}),
				sync_handler: None,
				requestless_sync_handler: None,
				middleware: Vec::new(),
				param_names: extract_path_param_names(&collection_path),
			};
			if let Err(e) =
				insert_compiled_route(compiled, &Method::POST, &collection_path, create_handler)
			{
				compiled.errors.push(format!(
					"Failed to compile ViewSet create route '{}': {}",
					collection_path, e
				));
			}

			let lookup_field = viewset.get_lookup_field();
			let detail_path = format!("{}/{{{}}}/", base_path.trim_end_matches('/'), lookup_field);

			let retrieve_handler = RouteHandler {
				handler: Arc::new(ViewSetHandler {
					viewset: viewset.clone(),
					action: Action::retrieve(),
				}),
				sync_handler: None,
				requestless_sync_handler: None,
				middleware: Vec::new(),
				param_names: extract_path_param_names(&detail_path),
			};
			if let Err(e) =
				insert_compiled_route(compiled, &Method::GET, &detail_path, retrieve_handler)
			{
				compiled.errors.push(format!(
					"Failed to compile ViewSet retrieve route '{}': {}",
					detail_path, e
				));
			}

			let update_handler = RouteHandler {
				handler: Arc::new(ViewSetHandler {
					viewset: viewset.clone(),
					action: Action::update(),
				}),
				sync_handler: None,
				requestless_sync_handler: None,
				middleware: Vec::new(),
				param_names: extract_path_param_names(&detail_path),
			};
			if let Err(e) =
				insert_compiled_route(compiled, &Method::PUT, &detail_path, update_handler)
			{
				compiled.errors.push(format!(
					"Failed to compile ViewSet update route '{}': {}",
					detail_path, e
				));
			}

			let destroy_handler = RouteHandler {
				handler: Arc::new(ViewSetHandler {
					viewset: viewset.clone(),
					action: Action::destroy(),
				}),
				sync_handler: None,
				requestless_sync_handler: None,
				middleware: Vec::new(),
				param_names: extract_path_param_names(&detail_path),
			};
			if let Err(e) =
				insert_compiled_route(compiled, &Method::DELETE, &detail_path, destroy_handler)
			{
				compiled.errors.push(format!(
					"Failed to compile ViewSet destroy route '{}': {}",
					detail_path, e
				));
			}
		}
	}

	/// Validate all routes by compiling them and returning any errors.
	///
	/// Call this at application startup to detect invalid route patterns early.
	/// Returns `Ok(())` if all routes compiled successfully, or `Err` with
	/// a list of compilation error messages.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::ServerRouter;
	/// # use hyper::Method;
	/// # use reinhardt_core::endpoint::EndpointInfo;
	/// # use reinhardt_http::{Handler, Request, Response, Result};
	///
	/// # struct UsersDetail;
	/// # impl EndpointInfo for UsersDetail {
	/// #     fn path() -> &'static str { "/users/{id}" }
	/// #     fn method() -> Method { Method::GET }
	/// #     fn name() -> &'static str { "users-detail" }
	/// # }
	/// # #[async_trait::async_trait]
	/// # impl Handler for UsersDetail {
	/// #     async fn handle(&self, _req: Request) -> Result<Response> { Ok(Response::ok()) }
	/// # }
	/// let router = ServerRouter::new()
	///     .endpoint(|| UsersDetail);
	///
	/// // Validate routes at startup
	/// assert!(router.validate_routes().is_ok());
	/// ```
	pub fn validate_routes(&self) -> std::result::Result<(), Vec<String>> {
		let mut errors = self.collect_configuration_errors_recursive();
		errors.extend(self.compile_routes_recursive());
		if let Err(name_errors) = self.validate_route_names() {
			errors.extend(name_errors);
		}
		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
	}

	/// Validate that no duplicate route names exist among all collected routes.
	///
	/// Returns `Ok(())` if all names are unique, or `Err(errors)` with details
	/// about each duplicate.
	pub fn validate_route_names(&self) -> std::result::Result<(), Vec<String>> {
		let registrations = self.collect_routes_recursive(None, "");
		let mut seen: std::collections::HashMap<String, String> = std::collections::HashMap::new();
		let mut errors = Vec::new();
		for (name, path) in registrations {
			if let Some(existing_path) = seen.insert(name.clone(), path.clone()) {
				errors.push(format!(
					"Duplicate route name '{}': path '{}' conflicts with existing path '{}'",
					name, path, existing_path
				));
			}
		}
		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
	}
}

fn mounted_paths_overlap(left: &str, right: &str) -> bool {
	let left = left
		.trim_end_matches('/')
		.split('/')
		.filter(|segment| !segment.is_empty());
	let right = right
		.trim_end_matches('/')
		.split('/')
		.filter(|segment| !segment.is_empty());
	let left = left.collect::<Vec<_>>();
	let right = right.collect::<Vec<_>>();
	for (left, right) in left.iter().zip(&right) {
		if left.starts_with("{*") && left.ends_with('}')
			|| right.starts_with("{*") && right.ends_with('}')
		{
			return true;
		}
		if !(left.starts_with('{') && left.ends_with('}')
			|| right.starts_with('{') && right.ends_with('}')
			|| left == right)
		{
			return false;
		}
	}
	left.len() == right.len()
}

#[cfg(test)]
mod tests {
	use super::extract_path_param_names;

	#[test]
	fn path_param_names_ignore_escaped_literal_braces() {
		let names = extract_path_param_names("/{{hello}}/");
		let expected: &[String] = &[];

		assert_eq!(&*names, expected);
	}

	#[test]
	fn path_param_names_keep_params_after_escaped_literal_braces() {
		let names = extract_path_param_names("/{{hello}}/{id}/{{literal}}/{*tail}");

		assert_eq!(&*names, ["id".to_string(), "tail".to_string()].as_slice());
	}
}
