//! Request-scoped resource registration for server-side rendering.

use crate::reactive::{ResourceState, Signal};
use futures_util::future::join_all;
use reinhardt_core::reactive::{ReactiveScope, ScopeId, current_scope_id, scope::enter_scope};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::{Rc, Weak};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::time::timeout;

tokio::task_local! {
	static ACTIVE_CONTEXT: Rc<RefCell<SsrResourceContext>>;
}

thread_local! {
	static RENDER_OWNERS: RefCell<BTreeMap<ScopeId, Weak<ReactiveScope>>> = const { RefCell::new(BTreeMap::new()) };
}

/// Removes a render owner registration when the render operation completes.
pub(crate) struct RenderOwnerRegistration {
	scope: ScopeId,
}

impl Drop for RenderOwnerRegistration {
	fn drop(&mut self) {
		let _ = RENDER_OWNERS.try_with(|owners| {
			owners.borrow_mut().remove(&self.scope);
		});
	}
}

type PendingResourceFuture = Pin<Box<dyn Future<Output = (String, Value)> + 'static>>;
type PendingResourceSubscriber = Box<dyn Fn(Value) + 'static>;

/// Polls an SSR resource future with the scope that owns its reactive state active.
struct ScopedPendingResourceFuture {
	scope: ScopeId,
	future: PendingResourceFuture,
}

impl Future for ScopedPendingResourceFuture {
	type Output = (String, Value);

	fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.get_mut();
		enter_scope(this.scope, || this.future.as_mut().poll(context))
			.unwrap_or_else(|error| panic!("{error}"))
	}
}

struct PendingResource {
	id: String,
	external: bool,
	registered_at_top_level: bool,
	boundary_ids: Vec<String>,
	read_boundary_ids: Vec<String>,
	future: PendingResourceFuture,
	subscribers: Vec<PendingResourceSubscriber>,
}

struct TimedOutResource {
	id: String,
	external: bool,
	registered_at_top_level: bool,
	boundary_ids: Vec<String>,
}

struct ResolvedResource {
	id: String,
	value: Value,
	external: bool,
	registered_at_top_level: bool,
	boundary_ids: Vec<String>,
}

/// Request-scoped SSR resource registry.
pub(crate) struct SsrResourceContext {
	next_resource_index: usize,
	pending: Vec<PendingResource>,
	resolved: Vec<ResolvedResource>,
	timed_out: Vec<TimedOutResource>,
	boundary_stack: Vec<String>,
	timeout: Duration,
}

impl SsrResourceContext {
	/// Creates a resource registry for one SSR render.
	pub(crate) fn new(timeout: Duration) -> Self {
		Self {
			next_resource_index: 0,
			pending: Vec::new(),
			resolved: Vec::new(),
			timed_out: Vec::new(),
			boundary_stack: Vec::new(),
			timeout,
		}
	}

	/// Resets deterministic call-order resource IDs in resource-context tests.
	#[cfg(test)]
	pub(crate) fn reset_call_order_keys(&mut self) {
		self.next_resource_index = 0;
	}

	/// Returns the current call-order resource index.
	pub(crate) fn call_order_index(&self) -> usize {
		self.next_resource_index
	}

	/// Restores the call-order resource index.
	pub(crate) fn set_call_order_index(&mut self, index: usize) {
		self.next_resource_index = index;
	}

	/// Reserves an internal call-order key for explicit-key resource hooks.
	pub(crate) fn reserve_call_order_key(&mut self, key: &str) {
		if let Some(id) = key.strip_prefix("rh-res-")
			&& let Ok(index) = id.parse::<usize>()
		{
			self.next_resource_index = self.next_resource_index.max(index.saturating_add(1));
		}
	}

	/// Allocates the next call-order resource key.
	pub(crate) fn next_call_order_key(&mut self) -> String {
		loop {
			let id = self.next_resource_index;
			self.next_resource_index += 1;
			let key = format!("rh-res-{id}");
			if self.can_use_resource_key(&key) {
				return key;
			}
		}
	}

	fn can_use_resource_key(&self, key: &str) -> bool {
		let active_boundary = self.current_boundary_id();
		let active_boundary = active_boundary.as_deref();

		let key_matches_active_scope = self.resolved.iter().any(|resolved| {
			resolved.id == key
				&& Self::matches_active_scope(
					active_boundary,
					resolved.external,
					resolved.registered_at_top_level,
					&resolved.boundary_ids,
				)
		}) || self.pending.iter().any(|pending| {
			pending.id == key
				&& Self::matches_active_scope(
					active_boundary,
					pending.external,
					pending.registered_at_top_level,
					&pending.boundary_ids,
				)
		}) || self.timed_out.iter().any(|resource| {
			resource.id == key
				&& Self::matches_active_scope(
					active_boundary,
					resource.external,
					resource.registered_at_top_level,
					&resource.boundary_ids,
				)
		});
		if key_matches_active_scope {
			return true;
		}

		if active_boundary.is_none() {
			!self
				.resolved
				.iter()
				.any(|resolved| resolved.id == key && resolved.external)
				&& !self
					.pending
					.iter()
					.any(|pending| pending.id == key && pending.external)
				&& !self
					.timed_out
					.iter()
					.any(|resource| resource.id == key && resource.external)
		} else {
			!self.resolved.iter().any(|resolved| resolved.id == key)
				&& !self.pending.iter().any(|pending| pending.id == key)
				&& !self.timed_out.iter().any(|resource| resource.id == key)
		}
	}

	fn matches_active_scope(
		active_boundary: Option<&str>,
		external: bool,
		registered_at_top_level: bool,
		boundary_ids: &[String],
	) -> bool {
		if let Some(boundary_id) = active_boundary {
			boundary_ids
				.iter()
				.any(|candidate| candidate == boundary_id)
		} else {
			external || registered_at_top_level || boundary_ids.is_empty()
		}
	}

	fn push_boundary(&mut self, boundary_id: String) {
		self.boundary_stack.push(boundary_id);
	}

	fn pop_boundary(&mut self) {
		self.boundary_stack.pop();
	}

	fn current_boundary_id(&self) -> Option<String> {
		self.boundary_stack.last().cloned()
	}

	/// Registers a resource future unless the key is already known.
	#[cfg(test)]
	pub(crate) fn register_resource<T, E, F, Fut>(
		&mut self,
		key: String,
		fetcher: F,
		state: Signal<ResourceState<T, E>>,
	) where
		T: Clone + Serialize + DeserializeOwned + 'static,
		E: Clone + Serialize + DeserializeOwned + 'static,
		F: FnOnce() -> Fut,
		Fut: Future<Output = Result<T, E>> + 'static,
	{
		self.register_resource_with_owner(key, fetcher, state, None);
	}

	/// Registers a resource and retains its reactive scope until notifications finish.
	pub(crate) fn register_resource_with_owner<T, E, F, Fut>(
		&mut self,
		key: String,
		fetcher: F,
		state: Signal<ResourceState<T, E>>,
		owner: Option<Rc<ReactiveScope>>,
	) where
		T: Clone + Serialize + DeserializeOwned + 'static,
		E: Clone + Serialize + DeserializeOwned + 'static,
		F: FnOnce() -> Fut,
		Fut: Future<Output = Result<T, E>> + 'static,
	{
		let current_boundary_id = self.current_boundary_id();
		let active_boundary = current_boundary_id.as_deref();
		if let Some(value) = self.resolved_value_for_scope(&key) {
			if let Ok(resource_state) = serde_json::from_value(value.clone()) {
				state.set(resource_state);
			}
			return;
		}
		if self.timed_out_for_scope(&key) {
			return;
		}

		let subscriber_owner = owner.clone();
		let subscriber = Box::new(move |value: Value| {
			// Keep the owner alive while the copied signal key is updated.
			let _owner = &subscriber_owner;
			if let Ok(resource_state) = serde_json::from_value(value) {
				state.set(resource_state);
			}
		});
		let registered_at_top_level = current_boundary_id.is_none();

		if let Some(pending) = self
			.pending
			.iter_mut()
			.find(|pending| pending_matches_registration_scope(pending, &key, active_boundary))
		{
			if let Some(boundary_id) = current_boundary_id.as_ref()
				&& !pending
					.boundary_ids
					.iter()
					.any(|candidate| candidate == boundary_id)
			{
				pending.boundary_ids.push(boundary_id.clone());
			}
			pending.subscribers.push(subscriber);
			return;
		}

		let id = key.clone();
		let future = fetcher();
		let future: PendingResourceFuture = Box::pin(async move {
			let resource_state = match future.await {
				Ok(value) => ResourceState::Success(value),
				Err(error) => ResourceState::Error(error),
			};
			let value = serde_json::to_value(resource_state).unwrap_or(Value::Null);
			(id, value)
		});
		let future: PendingResourceFuture = if let Some(owner) = owner {
			Box::pin(ScopedPendingResourceFuture {
				scope: owner.id(),
				future,
			})
		} else {
			future
		};

		self.pending.push(PendingResource {
			id: key,
			external: false,
			registered_at_top_level,
			boundary_ids: current_boundary_id.into_iter().collect(),
			read_boundary_ids: Vec::new(),
			future,
			subscribers: vec![subscriber],
		});
	}

	/// Registers an already serialized resource future for request hydration.
	pub(crate) fn register_serialized_resource_with_owner<F, Fut>(
		&mut self,
		key: String,
		fetcher: F,
		owner: Option<Rc<ReactiveScope>>,
	) where
		F: FnOnce() -> Fut + 'static,
		Fut: Future<Output = Value> + 'static,
	{
		let current_boundary_id = self.current_boundary_id();
		let active_boundary = current_boundary_id.as_deref();
		if self.resolved_value_for_scope(&key).is_some() || self.timed_out_for_scope(&key) {
			return;
		}
		let registered_at_top_level = current_boundary_id.is_none();
		if let Some(pending) = self
			.pending
			.iter_mut()
			.find(|pending| pending_matches_registration_scope(pending, &key, active_boundary))
		{
			if let Some(boundary_id) = current_boundary_id.as_ref()
				&& !pending
					.boundary_ids
					.iter()
					.any(|candidate| candidate == boundary_id)
			{
				pending.boundary_ids.push(boundary_id.clone());
			}
			return;
		}

		let id = key.clone();
		let future: PendingResourceFuture = Box::pin(async move { (id, fetcher().await) });
		let future: PendingResourceFuture = if let Some(owner) = owner {
			Box::pin(ScopedPendingResourceFuture {
				scope: owner.id(),
				future,
			})
		} else {
			future
		};
		self.pending.push(PendingResource {
			id: key,
			external: false,
			registered_at_top_level,
			boundary_ids: current_boundary_id.into_iter().collect(),
			read_boundary_ids: Vec::new(),
			future,
			subscribers: Vec::new(),
		});
	}

	pub(crate) fn mark_resource_read(&mut self, key: &str) {
		let current_boundary_id = self.current_boundary_id();
		if let Some(pending) = self.pending.iter_mut().find(|pending| {
			pending_matches_registration_scope(pending, key, current_boundary_id.as_deref())
		}) {
			if let Some(boundary_id) = current_boundary_id {
				if !pending
					.read_boundary_ids
					.iter()
					.any(|candidate| candidate == &boundary_id)
				{
					pending.read_boundary_ids.push(boundary_id);
				}
			} else {
				pending.external = true;
			}
		}
	}

	/// Assigns already registered resource IDs to a Suspense boundary.
	pub(crate) fn assign_resources_to_boundary(&mut self, ids: &[String], boundary_id: &str) {
		if ids.is_empty() {
			return;
		}

		let current_boundary_id = self.current_boundary_id();
		for pending in &mut self.pending {
			if let Some(current_boundary_id) = current_boundary_id.as_deref()
				&& current_boundary_id != boundary_id
				&& ids.iter().any(|id| id == &pending.id)
				&& !pending
					.read_boundary_ids
					.iter()
					.any(|candidate| candidate == current_boundary_id)
			{
				pending
					.boundary_ids
					.retain(|candidate| candidate != current_boundary_id);
			}
			if ids.iter().any(|id| id == &pending.id)
				&& !pending
					.boundary_ids
					.iter()
					.any(|candidate| candidate == boundary_id)
			{
				pending.boundary_ids.push(boundary_id.to_string());
			}
		}
	}

	/// Returns a resolved resource state by key.
	pub(crate) fn resolved_resource_state<T, E>(&self, key: &str) -> Option<ResourceState<T, E>>
	where
		T: Clone + DeserializeOwned + 'static,
		E: Clone + DeserializeOwned + 'static,
	{
		self.resolved_value_for_scope(key)
			.and_then(|value| serde_json::from_value(value.clone()).ok())
	}

	/// Returns resolved resource payloads.
	pub(crate) fn resolved_resources(&self) -> Vec<(&str, &Value)> {
		let mut by_id: BTreeMap<&str, Vec<&ResolvedResource>> = BTreeMap::new();
		for resource in &self.resolved {
			by_id
				.entry(resource.id.as_str())
				.or_default()
				.push(resource);
		}

		by_id
			.into_values()
			.filter_map(resolved_resource_for_hydration)
			.map(|resource| (resource.id.as_str(), &resource.value))
			.collect()
	}

	/// Returns whether any resource is pending for a Suspense boundary.
	pub(crate) fn has_pending_for_boundary(&self, boundary_id: &str) -> bool {
		self.pending
			.iter()
			.any(|pending| pending.boundary_ids.iter().any(|id| id == boundary_id))
	}

	/// Returns whether any resource is still pending.
	pub(crate) fn has_pending(&self) -> bool {
		!self.pending.is_empty()
	}

	/// Returns whether any pending resource is not assigned to a Suspense boundary.
	pub(crate) fn has_pending_external(&self) -> bool {
		self.pending.iter().any(|pending| pending.external)
	}

	/// Returns pending resource IDs currently assigned to a Suspense boundary.
	pub(crate) fn pending_ids_for_boundary(&self, boundary_id: &str) -> Vec<String> {
		self.pending
			.iter()
			.filter(|pending| pending.boundary_ids.iter().any(|id| id == boundary_id))
			.map(|pending| pending.id.clone())
			.collect()
	}

	fn take_matching<F>(&mut self, matches: &F) -> Vec<PendingResource>
	where
		F: Fn(Option<&str>) -> bool,
	{
		let mut selected = Vec::new();
		let mut remaining = Vec::new();
		for pending in std::mem::take(&mut self.pending) {
			let is_match = (pending.external && matches(None))
				|| pending
					.boundary_ids
					.iter()
					.any(|boundary_id| matches(Some(boundary_id)));
			if is_match {
				selected.push(pending);
			} else {
				remaining.push(pending);
			}
		}
		self.pending = remaining;
		selected
	}

	fn has_timed_out_matching<F>(&self, matches: &F) -> bool
	where
		F: Fn(Option<&str>) -> bool,
	{
		self.timed_out.iter().any(|resource| {
			(resource.external && matches(None))
				|| resource
					.boundary_ids
					.iter()
					.any(|boundary_id| matches(Some(boundary_id)))
		})
	}

	fn record_resolved(
		&mut self,
		id: String,
		external: bool,
		registered_at_top_level: bool,
		boundary_ids: Vec<String>,
		subscribers: Vec<PendingResourceSubscriber>,
		value: Value,
	) {
		for subscriber in subscribers {
			subscriber(value.clone());
		}
		if let Some(existing) = self.resolved.iter_mut().find(|resource| {
			resource.id == id
				&& resource.external == external
				&& resource.registered_at_top_level == registered_at_top_level
				&& resource.boundary_ids == boundary_ids
		}) {
			existing.value = value;
		} else {
			self.resolved.push(ResolvedResource {
				id,
				external,
				registered_at_top_level,
				boundary_ids,
				value,
			});
		}
	}

	fn resolved_value_for_scope(&self, key: &str) -> Option<&Value> {
		let current_boundary_id = self.current_boundary_id();
		if current_boundary_id.is_none()
			&& let Some(value) = self.resolved_value_for_top_level_replay(key)
		{
			return Some(value);
		}

		self.resolved
			.iter()
			.rev()
			.find(|resource| {
				resource.id == key
					&& resolved_resource_matches_scope(
						resource,
						key,
						current_boundary_id.as_deref(),
					)
			})
			.map(|resource| &resource.value)
	}

	fn resolved_value_for_top_level_replay(&self, key: &str) -> Option<&Value> {
		if !is_internal_call_order_key(key) {
			return None;
		}

		let mut scoped_match = None;
		for resource in self
			.resolved
			.iter()
			.rev()
			.filter(|resource| resource.id == key)
		{
			if resource.external {
				return Some(&resource.value);
			}
			if resource.boundary_ids.is_empty() {
				return Some(&resource.value);
			}
			if !resource.registered_at_top_level {
				continue;
			}
			if scoped_match.is_some() {
				return None;
			}
			scoped_match = Some(&resource.value);
		}
		scoped_match
	}

	fn timed_out_for_scope(&self, key: &str) -> bool {
		let current_boundary_id = self.current_boundary_id();
		self.timed_out.iter().any(|resource| {
			resource.id == key
				&& timed_out_resource_matches_scope(resource, key, current_boundary_id.as_deref())
		})
	}

	fn record_timeout(&mut self, timed_out: TimedOutResource) {
		if let Some(existing) = self.timed_out.iter_mut().find(|resource| {
			resource.id == timed_out.id
				&& resource.external == timed_out.external
				&& resource.registered_at_top_level == timed_out.registered_at_top_level
				&& resource.boundary_ids == timed_out.boundary_ids
		}) {
			existing.external |= timed_out.external;
			for boundary_id in timed_out.boundary_ids {
				if !existing
					.boundary_ids
					.iter()
					.any(|candidate| candidate == &boundary_id)
				{
					existing.boundary_ids.push(boundary_id);
				}
			}
		} else {
			self.timed_out.push(timed_out);
		}
	}
}

/// Guard that restores the active Suspense boundary stack on early exit.
pub(crate) struct SsrBoundaryGuard {
	context: Rc<RefCell<SsrResourceContext>>,
	active: bool,
}

impl Drop for SsrBoundaryGuard {
	fn drop(&mut self) {
		if self.active {
			self.context.borrow_mut().pop_boundary();
		}
	}
}

/// Runs an async render operation with a request-scoped SSR resource context.
pub(crate) async fn scope_context<R>(
	context: Rc<RefCell<SsrResourceContext>>,
	future: impl Future<Output = R>,
) -> R {
	ACTIVE_CONTEXT.scope(context, future).await
}

/// Makes a render scope available to resources registered during that render.
pub(crate) fn register_render_owner(owner: &Rc<ReactiveScope>) -> RenderOwnerRegistration {
	RENDER_OWNERS.with(|owners| {
		owners.borrow_mut().insert(owner.id(), Rc::downgrade(owner));
	});
	RenderOwnerRegistration { scope: owner.id() }
}

/// Returns the owner of the currently active SSR render scope, if registered.
pub(crate) fn current_render_owner() -> Option<Rc<ReactiveScope>> {
	let scope = current_scope_id()?;
	RENDER_OWNERS.with(|owners| owners.borrow().get(&scope)?.upgrade())
}

/// Pushes an active Suspense boundary and restores it on drop.
pub(crate) fn enter_boundary(
	context: &Rc<RefCell<SsrResourceContext>>,
	boundary_id: String,
) -> SsrBoundaryGuard {
	context.borrow_mut().push_boundary(boundary_id);
	SsrBoundaryGuard {
		context: Rc::clone(context),
		active: true,
	}
}

/// Executes a closure with the active SSR resource context.
pub(crate) fn with_active_context<R>(
	f: impl FnOnce(&Rc<RefCell<SsrResourceContext>>) -> R,
) -> Option<R> {
	ACTIVE_CONTEXT.try_with(f).ok()
}

/// Marks a pending resource as read by the active render scope.
pub(crate) fn mark_resource_read(key: &str) {
	let _ = with_active_context(|context| context.borrow_mut().mark_resource_read(key));
}

/// Resolves resources outside Suspense boundaries.
pub(crate) async fn resolve_external_resources(context: &Rc<RefCell<SsrResourceContext>>) -> bool {
	resolve_matching(context, |boundary_id| boundary_id.is_none()).await
}

/// Resolves resources inside a Suspense boundary.
pub(crate) async fn resolve_boundary_resources(
	context: &Rc<RefCell<SsrResourceContext>>,
	boundary_id: &str,
) -> bool {
	resolve_matching(context, |candidate| candidate == Some(boundary_id)).await
}

/// Resolves every still-pending resource for buffered SSR.
pub(crate) async fn resolve_pending_resources(context: &Rc<RefCell<SsrResourceContext>>) -> bool {
	resolve_matching(context, |_| true).await
}

async fn resolve_matching(
	context: &Rc<RefCell<SsrResourceContext>>,
	matches: impl Fn(Option<&str>) -> bool,
) -> bool {
	let (timeout_duration, already_timed_out, pending) = {
		let mut context = context.borrow_mut();
		let timeout_duration = context.timeout;
		let already_timed_out = context.has_timed_out_matching(&matches);
		let pending = context.take_matching(&matches);
		(timeout_duration, already_timed_out, pending)
	};

	let results = join_all(pending.into_iter().map(|pending| async move {
		let PendingResource {
			id,
			external,
			registered_at_top_level,
			boundary_ids,
			read_boundary_ids: _,
			future,
			subscribers,
		} = pending;
		match timeout(timeout_duration, future).await {
			Ok((_id, value)) => Ok((
				id,
				external,
				registered_at_top_level,
				boundary_ids,
				subscribers,
				value,
			)),
			Err(_) => Err(TimedOutResource {
				id,
				external,
				registered_at_top_level,
				boundary_ids,
			}),
		}
	}))
	.await;

	let mut all_resolved = !already_timed_out;
	for result in results {
		match result {
			Ok((id, external, registered_at_top_level, boundary_ids, subscribers, value)) => {
				context.borrow_mut().record_resolved(
					id,
					external,
					registered_at_top_level,
					boundary_ids,
					subscribers,
					value,
				)
			}
			Err(timed_out) => {
				context.borrow_mut().record_timeout(timed_out);
				all_resolved = false;
			}
		}
	}
	all_resolved
}

fn is_internal_call_order_key(key: &str) -> bool {
	key.strip_prefix("rh-res-").is_some_and(|suffix| {
		!suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
	})
}

fn pending_matches_registration_scope(
	pending: &PendingResource,
	key: &str,
	current_boundary_id: Option<&str>,
) -> bool {
	if pending.id != key {
		return false;
	}
	if !is_internal_call_order_key(key) {
		return true;
	}
	match current_boundary_id {
		Some(boundary_id) => pending.boundary_ids.iter().any(|id| id == boundary_id),
		None => {
			pending.external || pending.registered_at_top_level || pending.boundary_ids.is_empty()
		}
	}
}

fn resolved_resource_matches_scope(
	resource: &ResolvedResource,
	key: &str,
	current_boundary_id: Option<&str>,
) -> bool {
	if !is_internal_call_order_key(key) {
		return true;
	}
	match current_boundary_id {
		Some(boundary_id) => resource.boundary_ids.iter().any(|id| id == boundary_id),
		None => resource.external,
	}
}

fn timed_out_resource_matches_scope(
	resource: &TimedOutResource,
	key: &str,
	current_boundary_id: Option<&str>,
) -> bool {
	if !is_internal_call_order_key(key) {
		return true;
	}
	match current_boundary_id {
		Some(boundary_id) => resource.boundary_ids.iter().any(|id| id == boundary_id),
		None => {
			resource.external
				|| resource.registered_at_top_level
				|| resource.boundary_ids.is_empty()
		}
	}
}

fn resolved_resource_for_hydration(resources: Vec<&ResolvedResource>) -> Option<&ResolvedResource> {
	let mut external = resources
		.iter()
		.copied()
		.filter(|resource| resource.external);
	if let Some(resource) = external.next() {
		return Some(resource);
	}
	if resources.len() == 1 {
		return resources.first().copied();
	}
	None
}

#[cfg(test)]
mod tests {
	use super::*;

	fn resource_signal(
		scope: &reinhardt_core::reactive::ReactiveScope,
	) -> Signal<ResourceState<String, String>> {
		scope.enter(|| Signal::new(ResourceState::Loading))
	}

	fn resource_value(value: &Value) -> ResourceState<String, String> {
		serde_json::from_value(value.clone()).expect("resource state should deserialize")
	}

	#[tokio::test]
	async fn internal_call_order_resources_keep_boundary_scopes_separate() {
		let scope = reinhardt_core::reactive::ReactiveScope::new();
		let context = Rc::new(RefCell::new(SsrResourceContext::new(Duration::from_secs(
			1,
		))));
		let boundary_state = resource_signal(&scope);
		let outside_state = resource_signal(&scope);

		{
			let _guard = enter_boundary(&context, "boundary".to_string());
			context.borrow_mut().register_resource(
				"rh-res-0".to_string(),
				|| async { Ok::<_, String>("boundary".to_string()) },
				boundary_state.clone(),
			);
		}
		context.borrow_mut().register_resource(
			"rh-res-0".to_string(),
			|| async { Ok::<_, String>("outside".to_string()) },
			outside_state.clone(),
		);
		context.borrow_mut().mark_resource_read("rh-res-0");

		assert_eq!(context.borrow().pending.len(), 2);

		assert!(resolve_external_resources(&context).await);
		assert_eq!(
			outside_state.get(),
			ResourceState::Success("outside".to_string())
		);
		assert_eq!(boundary_state.get(), ResourceState::Loading);
		{
			let context_ref = context.borrow();
			let resources = context_ref.resolved_resources();
			assert_eq!(resources.len(), 1);
			assert_eq!(
				resource_value(resources[0].1),
				ResourceState::Success("outside".to_string())
			);
		}

		assert!(resolve_boundary_resources(&context, "boundary").await);
		assert_eq!(
			boundary_state.get(),
			ResourceState::Success("boundary".to_string())
		);
		assert_eq!(
			context
				.borrow()
				.resolved_resource_state::<String, String>("rh-res-0"),
			Some(ResourceState::Success("outside".to_string()))
		);
		{
			let _guard = enter_boundary(&context, "boundary".to_string());
			assert_eq!(
				context
					.borrow()
					.resolved_resource_state::<String, String>("rh-res-0"),
				Some(ResourceState::Success("boundary".to_string()))
			);
		}
		{
			let context_ref = context.borrow();
			let resources = context_ref.resolved_resources();
			assert_eq!(resources.len(), 1);
			assert_eq!(
				resource_value(resources[0].1),
				ResourceState::Success("outside".to_string())
			);
		}
	}

	#[tokio::test]
	async fn pending_resource_polls_with_its_owner_scope_active() {
		let scope = Rc::new(reinhardt_core::reactive::ReactiveScope::new());
		let context = Rc::new(RefCell::new(SsrResourceContext::new(Duration::from_secs(
			1,
		))));
		let state = resource_signal(&scope);

		context.borrow_mut().register_resource_with_owner(
			"scoped".to_string(),
			|| async {
				let _scoped_signal = Signal::new(1_i32);
				Ok::<_, String>("resolved".to_string())
			},
			state.clone(),
			Some(Rc::clone(&scope)),
		);
		context.borrow_mut().mark_resource_read("scoped");

		assert!(resolve_external_resources(&context).await);
		assert_eq!(state.get(), ResourceState::Success("resolved".to_string()));
	}

	#[tokio::test]
	async fn top_level_replay_reuses_single_resolved_tracked_resource() {
		let scope = reinhardt_core::reactive::ReactiveScope::new();
		let context = Rc::new(RefCell::new(SsrResourceContext::new(Duration::from_secs(
			1,
		))));
		let discovery_state = resource_signal(&scope);

		context.borrow_mut().register_resource(
			"rh-res-0".to_string(),
			|| async { Ok::<_, String>("tracked".to_string()) },
			discovery_state,
		);
		context
			.borrow_mut()
			.assign_resources_to_boundary(&["rh-res-0".to_string()], "boundary");
		assert!(resolve_boundary_resources(&context, "boundary").await);

		context.borrow_mut().reset_call_order_keys();
		let replay_state = resource_signal(&scope);
		context.borrow_mut().register_resource(
			"rh-res-0".to_string(),
			|| async {
				panic!("resolved tracked resources should be reused during replay");
			},
			replay_state.clone(),
		);

		assert_eq!(
			replay_state.get(),
			ResourceState::Success("tracked".to_string())
		);
		assert!(!context.borrow().has_pending());
	}

	#[tokio::test]
	async fn top_level_replay_does_not_reuse_boundary_local_resolved_resource() {
		let scope = reinhardt_core::reactive::ReactiveScope::new();
		let context = Rc::new(RefCell::new(SsrResourceContext::new(Duration::from_secs(
			1,
		))));
		let boundary_state = resource_signal(&scope);

		{
			let _guard = enter_boundary(&context, "boundary".to_string());
			context.borrow_mut().register_resource(
				"rh-res-0".to_string(),
				|| async { Ok::<_, String>("boundary".to_string()) },
				boundary_state.clone(),
			);
			context.borrow_mut().mark_resource_read("rh-res-0");
		}
		assert!(resolve_boundary_resources(&context, "boundary").await);
		assert_eq!(
			boundary_state.get(),
			ResourceState::Success("boundary".to_string())
		);

		context.borrow_mut().reset_call_order_keys();
		let fetch_count = Rc::new(std::cell::Cell::new(0));
		let replay_fetch_count = Rc::clone(&fetch_count);
		let replay_state = resource_signal(&scope);
		context.borrow_mut().register_resource(
			"rh-res-0".to_string(),
			move || {
				replay_fetch_count.set(replay_fetch_count.get() + 1);
				async { Ok::<_, String>("top-level".to_string()) }
			},
			replay_state.clone(),
		);
		context.borrow_mut().mark_resource_read("rh-res-0");

		assert_eq!(fetch_count.get(), 1);
		assert_eq!(replay_state.get(), ResourceState::Loading);
		assert!(resolve_external_resources(&context).await);
		assert_eq!(
			replay_state.get(),
			ResourceState::Success("top-level".to_string())
		);
	}

	#[tokio::test]
	async fn top_level_replay_reuses_pending_tracked_resource() {
		let scope = reinhardt_core::reactive::ReactiveScope::new();
		let context = Rc::new(RefCell::new(SsrResourceContext::new(Duration::from_secs(
			1,
		))));
		let fetch_count = Rc::new(std::cell::Cell::new(0));
		let discovery_fetch_count = Rc::clone(&fetch_count);
		let discovery_state = resource_signal(&scope);

		context.borrow_mut().register_resource(
			"rh-res-0".to_string(),
			move || {
				discovery_fetch_count.set(discovery_fetch_count.get() + 1);
				async { Ok::<_, String>("tracked".to_string()) }
			},
			discovery_state.clone(),
		);
		context
			.borrow_mut()
			.assign_resources_to_boundary(&["rh-res-0".to_string()], "boundary");

		context.borrow_mut().reset_call_order_keys();
		let replay_state = resource_signal(&scope);
		context.borrow_mut().register_resource(
			"rh-res-0".to_string(),
			|| async {
				panic!("pending tracked resources should be reused during replay");
			},
			replay_state.clone(),
		);

		assert_eq!(fetch_count.get(), 1);
		assert_eq!(context.borrow().pending.len(), 1);
		assert_eq!(discovery_state.get(), ResourceState::Loading);
		assert_eq!(replay_state.get(), ResourceState::Loading);

		assert!(resolve_boundary_resources(&context, "boundary").await);
		assert_eq!(
			discovery_state.get(),
			ResourceState::Success("tracked".to_string())
		);
		assert_eq!(
			replay_state.get(),
			ResourceState::Success("tracked".to_string())
		);
		assert!(!context.borrow().has_pending());
	}

	#[tokio::test]
	async fn top_level_replay_remembers_timed_out_tracked_resource() {
		let scope = reinhardt_core::reactive::ReactiveScope::new();
		let context = Rc::new(RefCell::new(SsrResourceContext::new(
			Duration::from_millis(1),
		)));
		let fetch_count = Rc::new(std::cell::Cell::new(0));
		let discovery_fetch_count = Rc::clone(&fetch_count);
		let discovery_state = resource_signal(&scope);

		context.borrow_mut().register_resource(
			"rh-res-0".to_string(),
			move || {
				discovery_fetch_count.set(discovery_fetch_count.get() + 1);
				async {
					tokio::time::sleep(Duration::from_millis(50)).await;
					Ok::<_, String>("tracked".to_string())
				}
			},
			discovery_state,
		);
		context
			.borrow_mut()
			.assign_resources_to_boundary(&["rh-res-0".to_string()], "boundary");

		assert!(!resolve_boundary_resources(&context, "boundary").await);
		assert_eq!(fetch_count.get(), 1);

		context.borrow_mut().reset_call_order_keys();
		let replay_key = context.borrow_mut().next_call_order_key();
		assert_eq!(replay_key, "rh-res-0");

		let replay_fetch_count = Rc::clone(&fetch_count);
		let replay_state = resource_signal(&scope);
		context.borrow_mut().register_resource(
			replay_key,
			move || {
				replay_fetch_count.set(replay_fetch_count.get() + 1);
				async { Ok::<_, String>("replayed".to_string()) }
			},
			replay_state.clone(),
		);

		assert_eq!(fetch_count.get(), 1);
		assert_eq!(replay_state.get(), ResourceState::Loading);
		assert!(!context.borrow().has_pending());
	}

	#[tokio::test]
	async fn nested_assignment_keeps_outer_boundary_scope_after_read() {
		let scope = reinhardt_core::reactive::ReactiveScope::new();
		let context = Rc::new(RefCell::new(SsrResourceContext::new(Duration::from_secs(
			1,
		))));
		let _outer = enter_boundary(&context, "outer".to_string());
		context.borrow_mut().register_resource(
			"rh-res-0".to_string(),
			|| async { Ok::<_, String>("shared".to_string()) },
			resource_signal(&scope),
		);
		context.borrow_mut().mark_resource_read("rh-res-0");

		context
			.borrow_mut()
			.assign_resources_to_boundary(&["rh-res-0".to_string()], "inner");

		assert_eq!(
			context.borrow().pending_ids_for_boundary("outer"),
			vec!["rh-res-0".to_string()]
		);
		assert_eq!(
			context.borrow().pending_ids_for_boundary("inner"),
			vec!["rh-res-0".to_string()]
		);
	}
}
