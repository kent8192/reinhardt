//! Route tree model for nested client layout routes.

use super::core::{ClientRoute, ClientRouteMatch, RouteGuard, RouteMetadata};
use super::loader::RouteLoaderId;
use super::navigation_guard::NavigationGuardId;
use std::collections::HashMap;

/// The structural kind of a route tree node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RouteNodeKind {
	/// Synthetic root node.
	Root,
	/// Layout route that wraps matched descendants.
	Layout,
	/// Leaf route that renders the final page.
	Leaf,
	/// Leaf route that matches its parent layout's base path.
	Index,
}

/// Metadata resolved during route registration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedRouteMetadata {
	name: Option<String>,
	own_pattern: String,
	full_pattern: String,
	component_name: Option<String>,
	function_name: Option<String>,
	props_type_name: Option<String>,
	loader_id: Option<RouteLoaderId>,
	navigation_guard_id: Option<NavigationGuardId>,
	route_metadata: RouteMetadata,
}

impl ResolvedRouteMetadata {
	/// Creates route metadata from static component or layout information.
	// Keep the generated route fields positional so component and layout
	// registration share one construction path without an intermediate builder.
	#[allow(clippy::too_many_arguments)]
	pub(crate) fn new(
		name: Option<String>,
		own_pattern: impl Into<String>,
		full_pattern: impl Into<String>,
		component_name: Option<String>,
		function_name: Option<String>,
		props_type_name: Option<String>,
		loader_id: Option<RouteLoaderId>,
		navigation_guard_id: Option<NavigationGuardId>,
		route_metadata: RouteMetadata,
	) -> Self {
		Self {
			name,
			own_pattern: own_pattern.into(),
			full_pattern: full_pattern.into(),
			component_name,
			function_name,
			props_type_name,
			loader_id,
			navigation_guard_id,
			route_metadata,
		}
	}

	/// Returns the route name.
	pub fn name(&self) -> Option<&str> {
		self.name.as_deref()
	}

	/// Returns the node-local pattern.
	pub fn own_pattern(&self) -> &str {
		&self.own_pattern
	}

	/// Returns the fully composed pattern.
	pub fn full_pattern(&self) -> &str {
		&self.full_pattern
	}

	/// Returns the component name.
	pub fn component_name(&self) -> Option<&str> {
		self.component_name.as_deref()
	}

	/// Returns the render function name.
	pub fn function_name(&self) -> Option<&str> {
		self.function_name.as_deref()
	}

	/// Returns the props type name.
	pub fn props_type_name(&self) -> Option<&str> {
		self.props_type_name.as_deref()
	}

	/// Returns the optional stable loader identifier.
	pub fn loader_id(&self) -> Option<RouteLoaderId> {
		self.loader_id
	}

	/// Returns the optional stable navigation guard identifier.
	pub fn navigation_guard_id(&self) -> Option<NavigationGuardId> {
		self.navigation_guard_id
	}

	/// Returns route-level metadata.
	pub fn route_metadata(&self) -> &RouteMetadata {
		&self.route_metadata
	}

	pub(crate) fn prefix_name(&mut self, namespace: &str) {
		if let Some(name) = &mut self.name {
			*name = format!("{namespace}:{name}");
		}
	}

	pub(crate) fn set_route_metadata(&mut self, metadata: RouteMetadata) {
		self.route_metadata = metadata;
	}
}

/// A node in the nested client route tree.
#[derive(Debug, Clone)]
pub struct RouteNode {
	kind: RouteNodeKind,
	route: Option<ClientRoute>,
	own_pattern: String,
	full_pattern: String,
	own_param_names: Vec<String>,
	metadata: ResolvedRouteMetadata,
	children: Vec<RouteNode>,
}

impl RouteNode {
	/// Creates the synthetic root node.
	pub(crate) fn root() -> Self {
		Self {
			kind: RouteNodeKind::Root,
			route: None,
			own_pattern: String::new(),
			full_pattern: "/".to_string(),
			own_param_names: Vec::new(),
			metadata: ResolvedRouteMetadata::default(),
			children: Vec::new(),
		}
	}

	/// Creates a route tree node.
	pub(crate) fn new(
		kind: RouteNodeKind,
		route: ClientRoute,
		own_pattern: impl Into<String>,
		full_pattern: impl Into<String>,
		own_param_names: Vec<String>,
		metadata: ResolvedRouteMetadata,
		children: Vec<RouteNode>,
	) -> Self {
		Self {
			kind,
			route: Some(route),
			own_pattern: own_pattern.into(),
			full_pattern: full_pattern.into(),
			own_param_names,
			metadata,
			children,
		}
	}

	/// Returns the node kind.
	pub fn kind(&self) -> RouteNodeKind {
		self.kind
	}

	/// Returns the route, except for the synthetic root node.
	pub fn route(&self) -> Option<&ClientRoute> {
		self.route.as_ref()
	}

	/// Returns the node-local pattern.
	pub fn own_pattern(&self) -> &str {
		&self.own_pattern
	}

	/// Returns the fully composed pattern.
	pub fn full_pattern(&self) -> &str {
		&self.full_pattern
	}

	/// Returns node-local path parameter names.
	pub fn own_param_names(&self) -> &[String] {
		&self.own_param_names
	}

	/// Returns resolved metadata.
	pub fn metadata(&self) -> &ResolvedRouteMetadata {
		&self.metadata
	}

	/// Returns child nodes.
	pub fn children(&self) -> &[RouteNode] {
		&self.children
	}

	pub(crate) fn extend_children(&mut self, children: Vec<RouteNode>) {
		self.children.extend(children);
	}

	pub(crate) fn collect_route_names(&self, names: &mut Vec<String>) {
		if let Some(name) = self.metadata.name() {
			names.push(name.to_string());
		}
		for child in &self.children {
			child.collect_route_names(names);
		}
	}

	pub(crate) fn prefix_names(&mut self, namespace: &str) {
		if let Some(route) = &mut self.route {
			route.prefix_name(namespace);
		}
		self.metadata.prefix_name(namespace);
		for child in &mut self.children {
			child.prefix_names(namespace);
		}
	}

	pub(crate) fn update_metadata_for_name(&mut self, name: &str, metadata: RouteMetadata) -> bool {
		let mut updated = false;
		if self.metadata.name() == Some(name) {
			if let Some(route) = &mut self.route {
				route.set_metadata(metadata.clone());
			}
			self.metadata.set_route_metadata(metadata.clone());
			updated = true;
		}
		for child in &mut self.children {
			updated |= child.update_metadata_for_name(name, metadata.clone());
		}
		updated
	}

	pub(crate) fn update_guard_for_name(&mut self, name: &str, guard: RouteGuard) -> bool {
		let mut updated = false;
		if self.metadata.name() == Some(name) {
			if let Some(route) = &mut self.route {
				route.set_guard(guard.clone());
			}
			updated = true;
		}
		for child in &mut self.children {
			updated |= child.update_guard_for_name(name, guard.clone());
		}
		updated
	}

	pub(crate) fn match_path(
		&self,
		path: &str,
		query: Option<String>,
	) -> Option<ClientRouteTreeMatch> {
		let mut layouts = Vec::new();
		self.match_path_inner(path, query, &mut layouts)
	}

	fn match_path_inner(
		&self,
		path: &str,
		query: Option<String>,
		layouts: &mut Vec<RouteNode>,
	) -> Option<ClientRouteTreeMatch> {
		for child in &self.children {
			match child.kind {
				RouteNodeKind::Root => {}
				RouteNodeKind::Layout => {
					layouts.push(child.clone());
					if let Some(matched) = child.match_path_inner(path, query.clone(), layouts) {
						return Some(matched);
					}
					layouts.pop();
				}
				RouteNodeKind::Leaf | RouteNodeKind::Index => {
					let route = child.route.as_ref()?;
					if let Some((params, param_values)) = route.pattern().matches(path) {
						let leaf_match = ClientRouteMatch {
							route: route.clone(),
							path: path.to_string(),
							params,
							param_values,
							query: query.clone(),
						};
						if !route.check_guard(&leaf_match) {
							continue;
						}
						let matched_layouts = layouts
							.iter()
							.filter_map(|layout| layout.to_matched_layout(&leaf_match))
							.collect::<Vec<_>>();
						if matched_layouts
							.iter()
							.any(|layout| !layout.route.check_guard(&leaf_match))
						{
							continue;
						}
						return Some(ClientRouteTreeMatch::new(
							leaf_match,
							matched_layouts,
							child.metadata.clone(),
						));
					}
				}
			}
		}
		None
	}

	fn to_matched_layout(&self, leaf_match: &ClientRouteMatch) -> Option<MatchedLayout> {
		let route = self.route.as_ref()?.clone();
		let key_params = route
			.pattern()
			.param_names()
			.iter()
			.filter_map(|name| {
				leaf_match
					.params
					.get(name)
					.map(|value| (name.clone(), value.clone()))
			})
			.collect::<Vec<_>>();
		Some(MatchedLayout {
			route,
			metadata: self.metadata.clone(),
			key: LayoutKey {
				name: self.metadata.name.clone(),
				full_pattern: self.full_pattern.clone(),
				params: key_params,
			},
		})
	}
}

/// Stable identity for a matched layout instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutKey {
	name: Option<String>,
	full_pattern: String,
	params: Vec<(String, String)>,
}

impl LayoutKey {
	/// Returns the layout route name.
	pub fn name(&self) -> Option<&str> {
		self.name.as_deref()
	}

	/// Returns the layout full pattern.
	pub fn full_pattern(&self) -> &str {
		&self.full_pattern
	}

	/// Returns parameter values that make this layout instance stable.
	pub fn params(&self) -> &[(String, String)] {
		&self.params
	}
}

/// A layout route matched before the leaf route.
#[derive(Debug, Clone)]
pub struct MatchedLayout {
	pub(crate) route: ClientRoute,
	metadata: ResolvedRouteMetadata,
	key: LayoutKey,
}

impl MatchedLayout {
	/// Returns the matched layout route.
	pub fn route(&self) -> &ClientRoute {
		&self.route
	}

	/// Returns resolved layout metadata.
	pub fn metadata(&self) -> &ResolvedRouteMetadata {
		&self.metadata
	}

	/// Returns the layout persistence key.
	pub fn key(&self) -> &LayoutKey {
		&self.key
	}
}

/// Result of matching a path against the route tree.
#[derive(Debug, Clone)]
pub struct ClientRouteTreeMatch {
	leaf: ClientRouteMatch,
	layouts: Vec<MatchedLayout>,
	metadata: Vec<ResolvedRouteMetadata>,
	loader_ids: Vec<RouteLoaderId>,
	navigation_guard_ids: Vec<NavigationGuardId>,
}

impl ClientRouteTreeMatch {
	pub(crate) fn new(
		leaf: ClientRouteMatch,
		layouts: Vec<MatchedLayout>,
		leaf_metadata: ResolvedRouteMetadata,
	) -> Self {
		let mut metadata = layouts
			.iter()
			.map(|layout| layout.metadata.clone())
			.collect::<Vec<_>>();
		metadata.push(leaf_metadata);
		let loader_ids = metadata
			.iter()
			.filter_map(ResolvedRouteMetadata::loader_id)
			.collect();
		let navigation_guard_ids = metadata
			.iter()
			.filter_map(ResolvedRouteMetadata::navigation_guard_id)
			.collect();
		Self {
			leaf,
			layouts,
			metadata,
			loader_ids,
			navigation_guard_ids,
		}
	}

	/// Returns the matched leaf route.
	pub fn leaf(&self) -> &ClientRoute {
		&self.leaf.route
	}

	/// Returns the full leaf match.
	pub fn leaf_match(&self) -> &ClientRouteMatch {
		&self.leaf
	}

	/// Returns matched layouts from root to deepest.
	pub fn layouts(&self) -> &[MatchedLayout] {
		&self.layouts
	}

	/// Returns merged path parameters.
	pub fn params(&self) -> &HashMap<String, String> {
		&self.leaf.params
	}

	/// Returns ordered parameter values for extractor compatibility.
	pub fn param_values(&self) -> &[String] {
		&self.leaf.param_values
	}

	/// Returns the matched path without query string.
	pub fn path(&self) -> &str {
		&self.leaf.path
	}

	/// Returns the raw query string.
	pub fn query(&self) -> Option<&str> {
		self.leaf.query.as_deref()
	}

	/// Returns the resolved metadata chain.
	pub fn metadata_chain(&self) -> &[ResolvedRouteMetadata] {
		&self.metadata
	}

	/// Returns matched loader identifiers in root-layout-to-leaf order.
	pub fn loader_ids(&self) -> &[RouteLoaderId] {
		&self.loader_ids
	}

	/// Returns matched navigation guard identifiers in root-layout-to-leaf order.
	pub fn navigation_guard_ids(&self) -> &[NavigationGuardId] {
		&self.navigation_guard_ids
	}

	/// Re-evaluates every matched route guard against the current application state.
	///
	/// Navigation preparation can outlive state changes such as session expiry.
	/// Callers that commit an asynchronously prepared match should use this
	/// method immediately before committing it.
	pub fn guards_allow(&self) -> bool {
		self.leaf().check_guard(self.leaf_match())
			&& self
				.layouts()
				.iter()
				.all(|layout| layout.route().check_guard(self.leaf_match()))
	}
}
