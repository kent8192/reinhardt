//! Route tree registration scopes for `ClientRouter`.

use super::component::{ComponentInfo, FromLayoutRequest, LayoutInfo};
use super::core::{ClientRoute, RouteMetadata};
use super::error::RouteRegistrationError;
use super::from_request::FromRequest;
use super::handler::{
	from_layout_request_handler, from_request_handler, no_params_handler, outlet_layout_handler,
};
use super::pattern::ClientPathPattern;
use super::tree::{ResolvedRouteMetadata, RouteNode, RouteNodeKind};
use reinhardt_core::page::{Outlet, Page};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

/// The kind of route scope currently accepting declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
	/// Root scope: route paths must be absolute.
	Root,
	/// Layout child scope: route paths must be relative.
	Child,
}

#[derive(Debug, Default)]
struct RegistrationState {
	names: HashSet<String>,
	leaf_paths: HashSet<String>,
	flat_routes: Vec<ClientRoute>,
}

struct LeafRegistration<'a> {
	kind: RouteNodeKind,
	name: &'a str,
	own_path: &'a str,
	full_pattern: String,
	own_params: Vec<String>,
	route: ClientRoute,
	metadata: ResolvedRouteMetadata,
}

/// Registered routes collected from a `RouteScope`.
#[derive(Debug)]
pub(crate) struct RegisteredRouteScope {
	pub(crate) nodes: Vec<RouteNode>,
	pub(crate) routes: Vec<ClientRoute>,
}

/// Builder scope used by `ClientRouter::try_routes`.
pub struct RouteScope {
	kind: ScopeKind,
	prefix: String,
	ancestor_params: Vec<String>,
	nodes: Vec<RouteNode>,
	has_index: bool,
	error: Option<RouteRegistrationError>,
	state: Rc<RefCell<RegistrationState>>,
}

impl RouteScope {
	pub(crate) fn root(existing_names: Vec<String>, existing_leaf_paths: Vec<String>) -> Self {
		let state = RegistrationState {
			names: existing_names.into_iter().collect(),
			leaf_paths: existing_leaf_paths.into_iter().collect(),
			flat_routes: Vec::new(),
		};
		Self {
			kind: ScopeKind::Root,
			prefix: "/".to_string(),
			ancestor_params: Vec::new(),
			nodes: Vec::new(),
			has_index: false,
			error: None,
			state: Rc::new(RefCell::new(state)),
		}
	}

	pub(crate) fn finish(self) -> Result<RegisteredRouteScope, RouteRegistrationError> {
		if let Some(error) = self.error {
			return Err(error);
		}
		let routes = self.state.borrow().flat_routes.clone();
		Ok(RegisteredRouteScope {
			nodes: self.nodes,
			routes,
		})
	}

	/// Returns the scope kind.
	pub fn kind(&self) -> ScopeKind {
		self.kind
	}

	/// Returns the full pattern prefix owned by this scope.
	pub fn prefix(&self) -> &str {
		&self.prefix
	}

	/// Registers a component route using `ComponentInfo` metadata.
	pub fn component<F, P>(mut self, handler: F) -> Self
	where
		F: Fn(P) -> Page + Send + Sync + 'static,
		P: FromRequest + ComponentInfo + Send + Sync + 'static,
	{
		let result = self.try_component(handler);
		self.record_error(result);
		self
	}

	fn try_component<F, P>(&mut self, handler: F) -> Result<(), RouteRegistrationError>
	where
		F: Fn(P) -> Page + Send + Sync + 'static,
		P: FromRequest + ComponentInfo + Send + Sync + 'static,
	{
		let own_path = P::path();
		let full_pattern = self.full_pattern_for_child(own_path)?;
		let own_params = extract_param_names(own_path);
		self.validate_param_chain(&full_pattern, &own_params)?;
		let pattern = compile_pattern(&full_pattern)?;
		let mut route = ClientRoute::from_route_handler(
			Some(P::name().to_string()),
			pattern,
			from_request_handler(handler, full_pattern.clone()),
		);
		route.set_navigation_guard_id(P::navigation_guard_id());
		let metadata = ResolvedRouteMetadata::new(
			Some(P::name().to_string()),
			own_path,
			full_pattern.clone(),
			Some(P::component_name().to_string()),
			Some(P::function_name().to_string()),
			Some(P::props_type_name().to_string()),
			P::loader_id(),
			P::navigation_guard_id(),
			RouteMetadata::default(),
		);
		self.register_leaf(LeafRegistration {
			kind: RouteNodeKind::Leaf,
			name: P::name(),
			own_path,
			full_pattern,
			own_params,
			route,
			metadata,
		})
	}

	/// Registers a layout route and its child scope.
	pub fn layout<F, P, C>(mut self, handler: F, children: C) -> Self
	where
		F: Fn(P) -> Page + Send + Sync + 'static,
		P: FromLayoutRequest + LayoutInfo + 'static,
		C: FnOnce(RouteScope) -> RouteScope,
	{
		let result = self.try_layout(handler, children);
		self.record_error(result);
		self
	}

	fn try_layout<F, P, C>(&mut self, handler: F, children: C) -> Result<(), RouteRegistrationError>
	where
		F: Fn(P) -> Page + Send + Sync + 'static,
		P: FromLayoutRequest + LayoutInfo + 'static,
		C: FnOnce(RouteScope) -> RouteScope,
	{
		let own_path = P::path();
		let full_pattern = self.full_pattern_for_child(own_path)?;
		let own_params = extract_param_names(own_path);
		self.validate_param_chain(&full_pattern, &own_params)?;
		let pattern = compile_pattern(&full_pattern)?;
		self.reserve_name(P::name())?;

		let child_scope = children(self.child_scope(full_pattern.clone(), own_params.clone()));
		if let Some(error) = child_scope.error {
			return Err(error);
		}
		let child_nodes = child_scope.nodes;

		let mut route = ClientRoute::from_layout_handler(
			Some(P::name().to_string()),
			pattern,
			from_layout_request_handler(handler, full_pattern.clone()),
		);
		route.set_navigation_guard_id(P::navigation_guard_id());
		let metadata = ResolvedRouteMetadata::new(
			Some(P::name().to_string()),
			own_path,
			full_pattern.clone(),
			Some(P::component_name().to_string()),
			Some(P::function_name().to_string()),
			Some(P::props_type_name().to_string()),
			P::loader_id(),
			P::navigation_guard_id(),
			RouteMetadata::default(),
		);
		self.nodes.push(RouteNode::new(
			RouteNodeKind::Layout,
			route,
			own_path,
			full_pattern,
			own_params,
			metadata,
			child_nodes,
		));
		Ok(())
	}

	/// Registers an index route at this scope's base path.
	pub fn index<F, P>(mut self, handler: F) -> Self
	where
		F: Fn(P) -> Page + Send + Sync + 'static,
		P: FromRequest + ComponentInfo + Send + Sync + 'static,
	{
		let result = self.try_index(handler);
		self.record_error(result);
		self
	}

	fn try_index<F, P>(&mut self, handler: F) -> Result<(), RouteRegistrationError>
	where
		F: Fn(P) -> Page + Send + Sync + 'static,
		P: FromRequest + ComponentInfo + Send + Sync + 'static,
	{
		if self.has_index {
			return Err(RouteRegistrationError::DuplicateIndexRoute {
				scope: self.prefix.clone(),
			});
		}
		self.has_index = true;
		let full_pattern = self.prefix.clone();
		let pattern = compile_pattern(&full_pattern)?;
		let mut route = ClientRoute::from_route_handler(
			Some(P::name().to_string()),
			pattern,
			from_request_handler(handler, full_pattern.clone()),
		);
		route.set_navigation_guard_id(P::navigation_guard_id());
		let metadata = ResolvedRouteMetadata::new(
			Some(P::name().to_string()),
			"",
			full_pattern.clone(),
			Some(P::component_name().to_string()),
			Some(P::function_name().to_string()),
			Some(P::props_type_name().to_string()),
			P::loader_id(),
			P::navigation_guard_id(),
			RouteMetadata::default(),
		);
		self.register_leaf(LeafRegistration {
			kind: RouteNodeKind::Index,
			name: P::name(),
			own_path: "",
			full_pattern,
			own_params: Vec::new(),
			route,
			metadata,
		})
	}

	/// Registers a closure-backed leaf route.
	pub fn route<F>(mut self, name: &str, path: &str, handler: F) -> Self
	where
		F: Fn() -> Page + Send + Sync + 'static,
	{
		let result = self.try_route(name, path, handler);
		self.record_error(result);
		self
	}

	fn try_route<F>(
		&mut self,
		name: &str,
		path: &str,
		handler: F,
	) -> Result<(), RouteRegistrationError>
	where
		F: Fn() -> Page + Send + Sync + 'static,
	{
		let full_pattern = self.full_pattern_for_child(path)?;
		let own_params = extract_param_names(path);
		self.validate_param_chain(&full_pattern, &own_params)?;
		let pattern = compile_pattern(&full_pattern)?;
		let route = ClientRoute::from_route_handler(
			Some(name.to_string()),
			pattern,
			no_params_handler(handler),
		);
		let metadata = ResolvedRouteMetadata::new(
			Some(name.to_string()),
			path,
			full_pattern.clone(),
			None,
			None,
			None,
			None,
			None,
			RouteMetadata::default(),
		);
		self.register_leaf(LeafRegistration {
			kind: RouteNodeKind::Leaf,
			name,
			own_path: path,
			full_pattern,
			own_params,
			route,
			metadata,
		})
	}

	/// Registers a closure-backed layout route.
	pub fn layout_route<F, C>(mut self, name: &str, path: &str, handler: F, children: C) -> Self
	where
		F: Fn(Outlet) -> Page + Send + Sync + 'static,
		C: FnOnce(RouteScope) -> RouteScope,
	{
		let result = self.try_layout_route(name, path, handler, children);
		self.record_error(result);
		self
	}

	fn try_layout_route<F, C>(
		&mut self,
		name: &str,
		path: &str,
		handler: F,
		children: C,
	) -> Result<(), RouteRegistrationError>
	where
		F: Fn(Outlet) -> Page + Send + Sync + 'static,
		C: FnOnce(RouteScope) -> RouteScope,
	{
		let full_pattern = self.full_pattern_for_child(path)?;
		let own_params = extract_param_names(path);
		self.validate_param_chain(&full_pattern, &own_params)?;
		let pattern = compile_pattern(&full_pattern)?;
		self.reserve_name(name)?;

		let child_scope = children(self.child_scope(full_pattern.clone(), own_params.clone()));
		if let Some(error) = child_scope.error {
			return Err(error);
		}
		let child_nodes = child_scope.nodes;

		let route = ClientRoute::from_layout_handler(
			Some(name.to_string()),
			pattern,
			outlet_layout_handler(handler),
		);
		let metadata = ResolvedRouteMetadata::new(
			Some(name.to_string()),
			path,
			full_pattern.clone(),
			None,
			None,
			None,
			None,
			None,
			RouteMetadata::default(),
		);
		self.nodes.push(RouteNode::new(
			RouteNodeKind::Layout,
			route,
			path,
			full_pattern,
			own_params,
			metadata,
			child_nodes,
		));
		Ok(())
	}

	/// Registers a closure-backed index route.
	pub fn index_route<F>(mut self, name: &str, handler: F) -> Self
	where
		F: Fn() -> Page + Send + Sync + 'static,
	{
		let result = self.try_index_route(name, handler);
		self.record_error(result);
		self
	}

	fn try_index_route<F>(&mut self, name: &str, handler: F) -> Result<(), RouteRegistrationError>
	where
		F: Fn() -> Page + Send + Sync + 'static,
	{
		if self.has_index {
			return Err(RouteRegistrationError::DuplicateIndexRoute {
				scope: self.prefix.clone(),
			});
		}
		self.has_index = true;
		let full_pattern = self.prefix.clone();
		let pattern = compile_pattern(&full_pattern)?;
		let route = ClientRoute::from_route_handler(
			Some(name.to_string()),
			pattern,
			no_params_handler(handler),
		);
		let metadata = ResolvedRouteMetadata::new(
			Some(name.to_string()),
			"",
			full_pattern.clone(),
			None,
			None,
			None,
			None,
			None,
			RouteMetadata::default(),
		);
		self.register_leaf(LeafRegistration {
			kind: RouteNodeKind::Index,
			name,
			own_path: "",
			full_pattern,
			own_params: Vec::new(),
			route,
			metadata,
		})
	}

	fn child_scope(&self, prefix: String, own_params: Vec<String>) -> Self {
		let mut ancestor_params = self.ancestor_params.clone();
		ancestor_params.extend(own_params);
		Self {
			kind: ScopeKind::Child,
			prefix,
			ancestor_params,
			nodes: Vec::new(),
			has_index: false,
			error: None,
			state: Rc::clone(&self.state),
		}
	}

	fn record_error(&mut self, result: Result<(), RouteRegistrationError>) {
		if self.error.is_none() {
			self.error = result.err();
		}
	}

	fn full_pattern_for_child(&self, path: &str) -> Result<String, RouteRegistrationError> {
		match self.kind {
			ScopeKind::Root if !path.starts_with('/') => {
				Err(RouteRegistrationError::RootPathMustBeAbsolute {
					path: path.to_string(),
				})
			}
			ScopeKind::Child if path.starts_with('/') => {
				Err(RouteRegistrationError::AbsolutePathInChildScope {
					path: path.to_string(),
					parent: self.prefix.clone(),
				})
			}
			ScopeKind::Root => Ok(path.to_string()),
			ScopeKind::Child => Ok(compose_child_pattern(&self.prefix, path)),
		}
	}

	fn validate_param_chain(
		&self,
		full_pattern: &str,
		own_params: &[String],
	) -> Result<(), RouteRegistrationError> {
		let mut seen = self.ancestor_params.iter().cloned().collect::<HashSet<_>>();
		for name in own_params {
			if !seen.insert(name.clone()) {
				return Err(RouteRegistrationError::DuplicatePathParam {
					name: name.clone(),
					pattern: full_pattern.to_string(),
				});
			}
		}
		Ok(())
	}

	fn reserve_name(&self, name: &str) -> Result<(), RouteRegistrationError> {
		let mut state = self.state.borrow_mut();
		if !state.names.insert(name.to_string()) {
			return Err(RouteRegistrationError::DuplicateRouteName {
				name: name.to_string(),
			});
		}
		Ok(())
	}

	fn register_leaf(
		&mut self,
		registration: LeafRegistration<'_>,
	) -> Result<(), RouteRegistrationError> {
		let LeafRegistration {
			kind,
			name,
			own_path,
			full_pattern,
			own_params,
			route,
			metadata,
		} = registration;
		self.reserve_name(name)?;
		let mut state = self.state.borrow_mut();
		if !state.leaf_paths.insert(full_pattern.clone()) {
			return Err(RouteRegistrationError::PathConflict { path: full_pattern });
		}
		state.flat_routes.push(route.clone());
		drop(state);
		self.nodes.push(RouteNode::new(
			kind,
			route,
			own_path,
			full_pattern,
			own_params,
			metadata,
			Vec::new(),
		));
		Ok(())
	}
}

fn compile_pattern(pattern: &str) -> Result<ClientPathPattern, RouteRegistrationError> {
	ClientPathPattern::new(pattern).map_err(|source| RouteRegistrationError::InvalidPattern {
		pattern: pattern.to_string(),
		source,
	})
}

fn compose_child_pattern(prefix: &str, child: &str) -> String {
	if child.is_empty() {
		return prefix.to_string();
	}
	if prefix.ends_with('/') {
		format!("{prefix}{child}")
	} else {
		format!("{prefix}/{child}")
	}
}

fn extract_param_names(pattern: &str) -> Vec<String> {
	let mut names = Vec::new();
	let mut rest = pattern;
	while let Some(start) = rest.find('{') {
		rest = &rest[start + 1..];
		let Some(end) = rest.find('}') else {
			break;
		};
		let raw = &rest[..end];
		let name = raw.split_once(':').map(|(name, _)| name).unwrap_or(raw);
		if !name.is_empty() {
			names.push(name.to_string());
		}
		rest = &rest[end + 1..];
	}
	names
}
