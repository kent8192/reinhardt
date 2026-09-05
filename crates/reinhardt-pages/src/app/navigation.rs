//! Pages-owned navigation preparation and commit coordination.

use crate::cancellation::{AbortableTaskGuard, CancellationSource};
use crate::reactive::Signal;
use crate::reactive::hooks::router::NavigateError;
use crate::reactive::query::{QueryClient, current_query_client, with_query_client};
use crate::reactive::{QueryConsumer, QueryDefaults};
use crate::router::NavigationType;
use crate::router::loader::{LoaderStore, RouteLoaderError, route_context};
use crate::router::loader_registry::{LoaderConsumer, LoaderRegistry, execute_loader};
use crate::router::navigation_guard::{
	NavigationContext, NavigationDecision, NavigationGuardError, NavigationKind,
};
use crate::router::navigation_guard_registry::{
	NavigationGuardRegistry, execute_navigation_guards,
};
use futures_util::future::{join_all, try_join_all};
use reinhardt_urls::routers::client_router::{ClientRouteTreeMatch, ClientRouter};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;
use url::Url;

const REDIRECT_NORMALIZATION_BASE: &str = "http://reinhardt.invalid/";

#[derive(Clone, Debug)]
struct RedirectChain {
	visited: HashSet<String>,
}

impl RedirectChain {
	fn new(destination: &str) -> Result<Self, NavigationGuardError> {
		let mut visited = HashSet::new();
		visited.insert(Self::normalize(destination)?);
		Ok(Self { visited })
	}

	fn redirect(&self, destination: &str) -> Result<(Self, String), NavigationGuardError> {
		let normalized = Self::normalize(destination)?;
		let mut chain = self.clone();
		if !chain.visited.insert(normalized.clone()) {
			return Err(NavigationGuardError::with_status(
				"navigation guard redirect loop detected",
				500,
			));
		}
		Ok((chain, normalized))
	}

	fn normalize(destination: &str) -> Result<String, NavigationGuardError> {
		let base = Url::parse(REDIRECT_NORMALIZATION_BASE).expect("fixed redirect base is valid");
		let mut url = base.join(destination).map_err(|error| {
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
}

#[derive(Clone)]
struct NavigationCompletionGuard {
	_owner: Rc<dyn std::any::Any>,
}

impl NavigationCompletionGuard {
	fn new<T: 'static>(owner: T) -> Self {
		Self {
			_owner: Rc::new(owner),
		}
	}
}

// Pop and initial intents are supplied by the browser launcher.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NavigationIntent {
	Initial,
	Push,
	Replace,
	Pop {
		target_index: Option<i64>,
	},
	Redirect {
		replace: bool,
		pop_origin: Option<PopOrigin>,
	},
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PopOrigin {
	target_index: Option<i64>,
	committed_index: i64,
}

impl NavigationIntent {
	fn navigation_type(self) -> NavigationType {
		match self {
			Self::Initial => NavigationType::Initial,
			Self::Push => NavigationType::Push,
			Self::Replace => NavigationType::Replace,
			Self::Pop { .. } => NavigationType::Pop,
			Self::Redirect { replace, .. } => {
				if replace {
					NavigationType::Replace
				} else {
					NavigationType::Push
				}
			}
		}
	}

	fn navigation_kind(self) -> NavigationKind {
		match self {
			Self::Initial => NavigationKind::Initial,
			Self::Push => NavigationKind::Push,
			Self::Replace => NavigationKind::Replace,
			Self::Pop { .. } => NavigationKind::Pop,
			Self::Redirect { replace, .. } => {
				if replace {
					NavigationKind::Replace
				} else {
					NavigationKind::Push
				}
			}
		}
	}

	fn pop_origin(self, committed_index: i64) -> Option<PopOrigin> {
		match self {
			Self::Pop { target_index } => Some(PopOrigin {
				target_index,
				committed_index,
			}),
			Self::Redirect { pop_origin, .. } => pop_origin,
			_ => None,
		}
	}

	fn entry_index(self, committed_index: i64) -> i64 {
		match self {
			Self::Initial | Self::Replace => committed_index,
			Self::Push => committed_index.saturating_add(1),
			Self::Pop { target_index } => target_index.unwrap_or(committed_index),
			Self::Redirect {
				replace,
				pop_origin: Some(origin),
			} => match origin.target_index {
				Some(target_index) if replace => target_index,
				Some(target_index) => target_index.saturating_add(1),
				None if replace => origin.committed_index,
				None => origin.committed_index.saturating_add(1),
			},
			Self::Redirect {
				replace,
				pop_origin: None,
			} => {
				if replace {
					committed_index
				} else {
					committed_index.saturating_add(1)
				}
			}
		}
	}

	/// Converts this attempt into a redirect while preserving uncommitted push history.
	///
	/// `replace: true` replaces a denied destination that already occupies a
	/// history entry. An uncommitted push never inserted that entry, so the
	/// redirect is pushed and the source page remains the Back target.
	fn into_redirect(self, replace: bool, committed_index: i64) -> Self {
		let pop_origin = self.pop_origin(committed_index);
		let replace = match self {
			Self::Push => false,
			Self::Redirect {
				replace: already_replacing,
				pop_origin: None,
			} => already_replacing && replace,
			_ => replace,
		};
		Self::Redirect {
			replace,
			pop_origin,
		}
	}
}

// Fields mirror the navigation attempt contract and are retained until the
// current generation commits or is cancelled. Some fields are diagnostic
// ownership anchors rather than read by the commit path itself.
#[allow(dead_code)]
struct NavigationAttempt {
	generation: u64,
	intent: NavigationIntent,
	pop_origin: Option<PopOrigin>,
	path: String,
	matched: ClientRouteTreeMatch,
	redirect_chain: RedirectChain,
	cancellation: CancellationSource,
	_task: AbortableTaskGuard,
	_completion: Option<NavigationCompletionGuard>,
}

/// Coordinates asynchronous route-loader preparation with synchronous URL
/// matching and commit operations.
pub(crate) struct NavigationCoordinator {
	router: Rc<ClientRouter>,
	query_client: QueryClient,
	registry: LoaderRegistry,
	guard_registry: NavigationGuardRegistry,
	next_generation: Cell<u64>,
	next_prefetch_id: Cell<u64>,
	committed_index: Cell<i64>,
	pending: Signal<bool>,
	error: Signal<Option<RouteLoaderError>>,
	active_attempt: RefCell<Option<NavigationAttempt>>,
	mounted_store: RefCell<Option<LoaderStore>>,
	restoring_pop: Cell<bool>,
	// Prefetch work stays owned until its future settles or the coordinator drops.
	prefetch_tasks: RefCell<Vec<(u64, CancellationSource, AbortableTaskGuard)>>,
}

// Accessors are used by the launcher, transition hooks, and prefetch path on
// WASM; native builds keep them available for deterministic integration tests.
#[allow(dead_code)]
impl NavigationCoordinator {
	pub(crate) fn new(router: Rc<ClientRouter>) -> Result<Rc<Self>, RouteLoaderError> {
		let query_client =
			current_query_client().unwrap_or_else(|| QueryClient::new(QueryDefaults::default()));
		let registry = LoaderRegistry::global()
			.map_err(|error| RouteLoaderError::with_status(error.to_string(), 500))?;
		let guard_registry = NavigationGuardRegistry::global()
			.map_err(|error| RouteLoaderError::with_status(error.to_string(), 500))?;
		Ok(Rc::new(Self {
			router,
			query_client,
			registry,
			guard_registry,
			next_generation: Cell::new(0),
			next_prefetch_id: Cell::new(0),
			committed_index: Cell::new(0),
			pending: Signal::new(false),
			error: Signal::new(None),
			active_attempt: RefCell::new(None),
			mounted_store: RefCell::new(None),
			restoring_pop: Cell::new(false),
			prefetch_tasks: RefCell::new(Vec::new()),
		}))
	}

	pub(crate) fn pending(&self) -> Signal<bool> {
		self.pending
	}

	pub(crate) fn error(&self) -> Signal<Option<RouteLoaderError>> {
		self.error
	}

	pub(crate) fn mounted_store(&self) -> Option<LoaderStore> {
		self.mounted_store.borrow().clone()
	}

	pub(crate) fn current_path(&self) -> String {
		self.router.current_path().get()
	}

	pub(crate) fn clear_for_authentication_change(&self) {
		self.query_client.clear_for_authentication_change();
		self.cancel_active_attempt();
		self.cancel_prefetch_tasks();
		self.pending.set(false);
		#[cfg(wasm)]
		super::launcher::clear_mounted_route_for_authentication_change();
	}

	#[cfg(test)]
	pub(crate) fn set_mounted_store_for_test(&self, store: LoaderStore) {
		self.mounted_store.borrow_mut().replace(store);
	}

	/// Returns the currently committed history index used for legacy popstate
	/// entries that do not carry framework metadata.
	pub(crate) fn committed_index(&self) -> i64 {
		self.committed_index.get()
	}

	/// Seeds the coordinator with the index of the entry rendered at launch.
	///
	/// The browser may preserve a framework-owned index across a reload. The
	/// launcher normalizes legacy entries before calling this method so future
	/// push and pop preparations use the same monotonic sequence.
	pub(crate) fn initialize_committed_index(&self, index: i64) {
		self.committed_index.set(index);
	}

	/// Consumes the one-shot pop generated while restoring a failed navigation.
	pub(crate) fn consume_restoration_pop(&self) -> bool {
		self.restoring_pop.replace(false)
	}

	/// Restores the initial route's prepared loader values from the SSR state.
	///
	/// When an SSR state script is present, hydration is intentionally strict for
	/// matched loader routes: rendering a destination without its entry-blocking
	/// values would violate the loader contract and cause the generated component
	/// binding to panic. Without an SSR state script, the caller prepares the
	/// initial route on the client before mounting it.
	#[cfg(wasm)]
	pub(crate) fn hydrate_initial_store(
		&self,
		client: &QueryClient,
		path: &str,
	) -> Result<bool, RouteLoaderError> {
		let Some(matched) = self.router.match_tree(path) else {
			return Ok(true);
		};
		let has_navigation_guards = !matched.navigation_guard_ids().is_empty();
		if matched.loader_ids().is_empty() {
			return Ok(!has_navigation_guards);
		}
		let has_ssr_state = web_sys::window()
			.and_then(|window| window.document())
			.and_then(|document| document.get_element_by_id("ssr-state"))
			.is_some();
		if !has_ssr_state {
			return Ok(false);
		}
		let hydration = crate::hydration::HydrationContext::from_window().map_err(|error| {
			RouteLoaderError::with_status(
				format!("route loader hydration state is unavailable: {error}"),
				500,
			)
		})?;
		let loader_context = route_context(&matched);
		let store = LoaderStore::new();
		let has_loader_state = matched
			.loader_ids()
			.iter()
			.any(|id| hydration.get_route_loader_state(id.as_str()).is_some());
		if !has_loader_state {
			return Ok(false);
		}
		for id in matched.loader_ids() {
			let prepared =
				self.registry
					.seed_hydrated_query(client, *id, &loader_context, &hydration)?;
			store.insert_prepared(prepared);
		}
		if has_navigation_guards {
			// Guarded branches must be prepared before any protected renderer is
			// mounted. The seeded query entries remain in `client`; the next
			// preparation acquires them without another SSR fetch.
			drop(store);
			Ok(false)
		} else {
			self.mounted_store.borrow_mut().replace(store);
			Ok(true)
		}
	}

	pub(crate) fn navigate(
		self: &Rc<Self>,
		path: String,
		intent: NavigationIntent,
	) -> Result<(), NavigateError> {
		let query_client = self.query_client.clone();
		with_query_client(&query_client, || {
			self.navigate_in_context(path, intent, None)
		})
	}

	pub(crate) fn navigate_with_completion_guard<T: 'static>(
		self: &Rc<Self>,
		path: String,
		intent: NavigationIntent,
		completion_guard: T,
	) -> Result<(), NavigateError> {
		let query_client = self.query_client.clone();
		let completion = NavigationCompletionGuard::new(completion_guard);
		with_query_client(&query_client, || {
			self.navigate_in_context(path, intent, Some(completion))
		})
	}

	fn navigate_in_context(
		self: &Rc<Self>,
		path: String,
		intent: NavigationIntent,
		completion: Option<NavigationCompletionGuard>,
	) -> Result<(), NavigateError> {
		let redirect_chain = RedirectChain::new(&path)
			.map_err(|error| NavigateError::RouterRejected(error.to_string()))?;
		self.navigate_with_redirect_chain_in_context(path, intent, redirect_chain, completion)
	}

	fn navigate_with_redirect_chain(
		self: &Rc<Self>,
		path: String,
		intent: NavigationIntent,
		redirect_chain: RedirectChain,
		completion: Option<NavigationCompletionGuard>,
	) -> Result<(), NavigateError> {
		let query_client = self.query_client.clone();
		with_query_client(&query_client, || {
			self.navigate_with_redirect_chain_in_context(path, intent, redirect_chain, completion)
		})
	}

	fn navigate_with_redirect_chain_in_context(
		self: &Rc<Self>,
		path: String,
		intent: NavigationIntent,
		redirect_chain: RedirectChain,
		completion: Option<NavigationCompletionGuard>,
	) -> Result<(), NavigateError> {
		let pop_origin = intent.pop_origin(self.committed_index.get());
		let matched = self.router.match_tree(&path);

		self.cancel_active_attempt();
		let generation = self.next_generation.get().wrapping_add(1);
		self.next_generation.set(generation);
		self.error.set(None);
		if matched.is_none() {
			self.pending.set(false);
			return self.commit_unmatched(generation, path, intent, pop_origin);
		}
		let matched = matched.expect("matched routes are handled above");

		if matched.loader_ids().is_empty() && matched.navigation_guard_ids().is_empty() {
			self.pending.set(false);
			return self.commit_success(
				generation,
				path,
				intent,
				matched,
				LoaderStore::new(),
				pop_origin,
			);
		}

		self.pending.set(true);
		let cancellation = CancellationSource::new();
		let cancellation_handle = cancellation.handle();
		let loader_ids = matched.loader_ids().to_vec();
		let guard_ids = matched.navigation_guard_ids().to_vec();
		let route_context = route_context(&matched);
		let coordinator = Rc::clone(self);
		let path_for_task = path.clone();
		let matched_for_task = matched.clone();
		let redirect_chain_for_task = redirect_chain.clone();
		let completion_for_task = completion.clone();
		let task_cancellation = cancellation_handle.clone();
		let task = crate::cancellation::spawn_abortable_task(async move {
			let guard_context = NavigationContext::new(
				path_for_task.clone(),
				route_context.clone(),
				intent.navigation_kind(),
				coordinator.query_client.clone(),
				task_cancellation.clone(),
				QueryConsumer::Navigation(generation),
				#[cfg(native)]
				None,
			);
			let decision = match execute_navigation_guards(
				&coordinator.guard_registry,
				&guard_ids,
				guard_context,
			)
			.await
			{
				Ok(decision) => decision,
				Err(error) => {
					if !task_cancellation.is_cancelled() {
						coordinator.finish_error(generation, error.into());
					}
					return;
				}
			};
			if task_cancellation.is_cancelled() || !coordinator.is_current_generation(generation) {
				return;
			}
			if decision != NavigationDecision::Allow {
				let _ = coordinator.finish_guard_decision(
					generation,
					path_for_task,
					intent,
					redirect_chain_for_task.clone(),
					decision,
					completion_for_task.clone(),
				);
				return;
			}

			let futures = loader_ids.into_iter().map(|id| {
				execute_loader(
					&coordinator.registry,
					id,
					&route_context,
					task_cancellation.clone(),
					LoaderConsumer::Navigation(generation),
				)
			});
			let results = match try_join_all(futures).await {
				Ok(results) => results,
				Err(error) => {
					if !task_cancellation.is_cancelled() {
						coordinator.finish_error(generation, error);
					}
					return;
				}
			};
			if task_cancellation.is_cancelled() || !coordinator.is_current_generation(generation) {
				return;
			}
			let store = LoaderStore::new();
			for prepared in results {
				store.insert_prepared(prepared);
			}
			let guard_context = NavigationContext::new(
				path_for_task.clone(),
				route_context,
				intent.navigation_kind(),
				coordinator.query_client.clone(),
				task_cancellation.clone(),
				QueryConsumer::Navigation(generation),
				#[cfg(native)]
				None,
			);
			let decision = match execute_navigation_guards(
				&coordinator.guard_registry,
				&guard_ids,
				guard_context,
			)
			.await
			{
				Ok(decision) => decision,
				Err(error) => {
					drop(store);
					if !task_cancellation.is_cancelled() {
						coordinator.finish_error(generation, error.into());
					}
					return;
				}
			};
			if task_cancellation.is_cancelled() || !coordinator.is_current_generation(generation) {
				return;
			}
			if decision != NavigationDecision::Allow {
				drop(store);
				let _ = coordinator.finish_guard_decision(
					generation,
					path_for_task,
					intent,
					redirect_chain_for_task,
					decision,
					completion_for_task.clone(),
				);
				return;
			}
			let _ = coordinator.commit_success(
				generation,
				path_for_task,
				intent,
				matched_for_task,
				store,
				pop_origin,
			);
		});

		*self.active_attempt.borrow_mut() = Some(NavigationAttempt {
			generation,
			intent,
			pop_origin,
			path,
			matched,
			redirect_chain,
			cancellation,
			_task: task,
			_completion: completion,
		});
		Ok(())
	}

	fn finish_guard_decision(
		self: &Rc<Self>,
		generation: u64,
		path: String,
		intent: NavigationIntent,
		redirect_chain: RedirectChain,
		decision: NavigationDecision,
		completion: Option<NavigationCompletionGuard>,
	) -> Result<(), NavigateError> {
		if !self.is_current_generation(generation) {
			return Ok(());
		}
		match decision {
			NavigationDecision::Allow => Ok(()),
			NavigationDecision::NotFound => self.commit_unmatched(
				generation,
				path,
				intent,
				intent.pop_origin(self.committed_index.get()),
			),
			NavigationDecision::Forbidden => {
				self.finish_error(
					generation,
					RouteLoaderError::with_status("navigation is forbidden", 403),
				);
				Ok(())
			}
			NavigationDecision::Redirect { location, replace } => {
				let (redirect_chain, location) = match redirect_chain.redirect(&location) {
					Ok(result) => result,
					Err(error) => {
						self.finish_error(generation, error.into());
						return Ok(());
					}
				};
				let redirect_intent = intent.into_redirect(replace, self.committed_index.get());
				self.navigate_with_redirect_chain(
					location,
					redirect_intent,
					redirect_chain,
					completion,
				)
			}
		}
	}

	pub(crate) fn prefetch(self: &Rc<Self>, path: String) -> Result<(), NavigateError> {
		let query_client = self.query_client.clone();
		with_query_client(&query_client, || self.prefetch_in_context(path))
	}

	fn prefetch_in_context(self: &Rc<Self>, path: String) -> Result<(), NavigateError> {
		let Some(matched) = self.router.match_tree(&path) else {
			return Ok(());
		};
		let loader_ids = matched.loader_ids().to_vec();
		let guard_ids = matched.navigation_guard_ids().to_vec();
		if loader_ids.is_empty() && guard_ids.is_empty() {
			return Ok(());
		}
		let route_context = route_context(&matched);
		let cancellation = CancellationSource::new();
		let handle = cancellation.handle();
		let prefetch_id = self.next_prefetch_id.get().wrapping_add(1);
		self.next_prefetch_id.set(prefetch_id);
		let coordinator = Rc::clone(self);
		let task = crate::cancellation::spawn_abortable_task(async move {
			let guard_context = NavigationContext::new(
				path,
				route_context.clone(),
				NavigationKind::Prefetch,
				coordinator.query_client.clone(),
				handle.clone(),
				QueryConsumer::Prefetch,
				#[cfg(native)]
				None,
			);
			if !matches!(
				execute_navigation_guards(&coordinator.guard_registry, &guard_ids, guard_context)
					.await,
				Ok(NavigationDecision::Allow)
			) {
				coordinator.finish_prefetch(prefetch_id);
				return;
			}
			let futures = loader_ids.into_iter().map(|id| {
				execute_loader(
					&coordinator.registry,
					id,
					&route_context,
					handle.clone(),
					LoaderConsumer::Prefetch,
				)
			});
			let _ = join_all(futures).await;
			coordinator.finish_prefetch(prefetch_id);
		});
		self.prefetch_tasks
			.borrow_mut()
			.push((prefetch_id, cancellation, task));
		Ok(())
	}

	fn finish_prefetch(&self, prefetch_id: u64) {
		self.prefetch_tasks
			.borrow_mut()
			.retain(|(id, _, _)| *id != prefetch_id);
	}

	fn cancel_active_attempt(&self) {
		if let Some(attempt) = self.active_attempt.borrow_mut().take() {
			attempt.cancellation.cancel();
			// Dropping the attempt's task guard aborts any obsolete future.
		}
	}

	fn cancel_prefetch_tasks(&self) {
		let tasks = std::mem::take(&mut *self.prefetch_tasks.borrow_mut());
		for (_, cancellation, _task) in tasks {
			cancellation.cancel();
		}
	}

	pub(crate) fn generation(&self) -> u64 {
		self.next_generation.get()
	}

	fn is_current_generation(&self, generation: u64) -> bool {
		self.next_generation.get() == generation
	}

	fn finish_error(&self, generation: u64, error: RouteLoaderError) {
		self.finish_error_with_origin(generation, error, None);
	}

	fn finish_error_with_origin(
		&self,
		generation: u64,
		error: RouteLoaderError,
		pop_origin: Option<PopOrigin>,
	) {
		if !self.is_current_generation(generation) {
			return;
		}
		self.pending.set(false);
		self.error.set(Some(error));
		let pop_origin = pop_origin.or_else(|| {
			self.active_attempt
				.borrow()
				.as_ref()
				.and_then(|attempt| attempt.pop_origin)
		});
		if let Some(origin) = pop_origin {
			// The browser already traversed to the pop destination. Restore the
			// entry that was committed before preparation started when it fails.
			let delta = origin
				.target_index
				.map(|target_index| origin.committed_index.saturating_sub(target_index))
				.unwrap_or(1);
			if delta != 0 {
				self.restoring_pop.set(true);
				if reinhardt_urls::routers::client_router::history::go(
					delta.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
				)
				.is_err()
				{
					self.restoring_pop.set(false);
				}
			}
		}
		self.active_attempt.borrow_mut().take();
	}

	fn commit_success(
		&self,
		generation: u64,
		path: String,
		intent: NavigationIntent,
		matched: ClientRouteTreeMatch,
		store: LoaderStore,
		pop_origin: Option<PopOrigin>,
	) -> Result<(), NavigateError> {
		if !self.is_current_generation(generation) {
			return Ok(());
		}
		if !matched.guards_allow() {
			return self.commit_unmatched(generation, path, intent, pop_origin);
		}
		store.promote_navigation_leases(generation);
		let entry_index = intent.entry_index(self.committed_index.get());
		let previous_store = self.mounted_store.borrow_mut().replace(store.clone());
		let result = crate::router::loader::with_loader_store(&store, || {
			if matched.navigation_guard_ids().is_empty() {
				self.router
					.commit_match(&path, &matched, intent.navigation_type(), entry_index)
			} else {
				// SAFETY: This coordinator has just re-evaluated every synchronous
				// route guard and completed every asynchronous navigation guard for
				// this exact match before committing it.
				unsafe {
					self.router.__commit_match_after_navigation_guard(
						&path,
						&matched,
						intent.navigation_type(),
						entry_index,
					)
				}
			}
		});
		if let Err(error) = result {
			*self.mounted_store.borrow_mut() = previous_store;
			self.finish_error_with_origin(
				generation,
				RouteLoaderError::with_status(error.to_string(), 500),
				pop_origin,
			);
			return Err(NavigateError::RouterRejected(error.to_string()));
		}
		self.committed_index.set(entry_index);
		self.pending.set(false);
		self.error.set(None);
		self.active_attempt.borrow_mut().take();
		Ok(())
	}

	fn commit_unmatched(
		&self,
		generation: u64,
		path: String,
		intent: NavigationIntent,
		pop_origin: Option<PopOrigin>,
	) -> Result<(), NavigateError> {
		if !self.is_current_generation(generation) {
			return Ok(());
		}
		let entry_index = intent.entry_index(self.committed_index.get());
		let previous_store = self.mounted_store.borrow_mut().replace(LoaderStore::new());
		if let Err(error) =
			self.router
				.commit_unmatched(&path, intent.navigation_type(), entry_index)
		{
			*self.mounted_store.borrow_mut() = previous_store;
			self.finish_error_with_origin(
				generation,
				RouteLoaderError::with_status(error.to_string(), 500),
				pop_origin,
			);
			return Err(NavigateError::RouterRejected(error.to_string()));
		}
		self.committed_index.set(entry_index);
		self.pending.set(false);
		self.error.set(None);
		self.active_attempt.borrow_mut().take();
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_core::reactive::ReactiveScope;

	#[test]
	fn no_loader_navigation_commits_synchronously() {
		ReactiveScope::run(|| {
			let router = Rc::new(
				ClientRouter::new().route("home", "/", || reinhardt_core::page::Page::empty()),
			);
			let coordinator = NavigationCoordinator::new(router.clone()).expect("registry builds");
			coordinator
				.navigate("/".to_string(), NavigationIntent::Push)
				.expect("known route commits");
			assert_eq!(router.current_path().get(), "/");
			assert!(!coordinator.pending().get());
		});
	}

	#[test]
	fn push_navigation_assigns_monotonic_history_indices() {
		ReactiveScope::run(|| {
			let router = Rc::new(
				ClientRouter::new()
					.route("home", "/", || reinhardt_core::page::Page::empty())
					.route("next", "/next/", || reinhardt_core::page::Page::empty()),
			);
			let coordinator = NavigationCoordinator::new(router).expect("registry builds");

			coordinator
				.navigate("/".to_string(), NavigationIntent::Push)
				.expect("initial push commits");
			assert_eq!(coordinator.committed_index(), 1);

			coordinator
				.navigate("/next/".to_string(), NavigationIntent::Push)
				.expect("second push commits");
			assert_eq!(coordinator.committed_index(), 2);

			coordinator
				.navigate("/".to_string(), NavigationIntent::Replace)
				.expect("replace commits");
			assert_eq!(coordinator.committed_index(), 2);
		});
	}

	#[test]
	fn redirect_chain_normalizes_paths_without_collapsing_distinct_queries() {
		let chain =
			RedirectChain::new("/protected?next=one").expect("initial destination is valid");
		let (chain, normalized) = chain
			.redirect("/protected?next=two")
			.expect("different query destination remains distinct");
		assert_eq!(normalized, "/protected?next=two");
		assert!(chain.redirect("/protected?next=one").is_err());

		let chain = RedirectChain::new("/protected/").expect("initial destination is valid");
		assert!(chain.redirect("/protected/./").is_err());

		let chain = RedirectChain::new("/protected/#one").expect("initial destination is valid");
		assert!(chain.redirect("/protected/#two").is_err());
	}

	#[cfg(native)]
	mod native_async_tests {
		use super::*;
		use crate::reactive::query::{
			QueryClient, QueryClientGuard, QueryDefaults, QueryFamily, QueryOptions,
			TestQueryRuntime, provide_query_client,
		};
		use crate::router::loader::{loader_cache_id, route_context, with_loader_store};
		use crate::{
			Loader, NavigationContext, NavigationDecision, NavigationGuardError, Page, RouteLoader,
			component, layout, loader, navigation_guard,
		};
		use reinhardt_core::page::{IntoPage, Outlet};
		use rstest::rstest;
		use std::cell::{Cell, RefCell};
		use std::collections::{HashMap, VecDeque};
		use std::future::{Future, poll_fn};
		use std::pin::Pin;
		use std::rc::Rc;
		use std::task::{Context, Poll, Waker};
		use std::time::Duration;

		thread_local! {
			static GATE_OPEN: Cell<bool> = const { Cell::new(false) };
			static GUARD_ALLOWS: Cell<bool> = const { Cell::new(true) };
			static SLOW_LOADER_STARTS: Cell<usize> = const { Cell::new(0) };
			static LAYOUT_LOADER_STARTS: Cell<usize> = const { Cell::new(0) };
			static LEAF_LOADER_STARTS: Cell<usize> = const { Cell::new(0) };
			static NAVIGATION_GUARD_ORDER: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
			static CONTROLLED_GUARD_RESULTS: RefCell<VecDeque<Result<NavigationDecision, NavigationGuardError>>> = const { RefCell::new(VecDeque::new()) };
			static CONTROLLED_GUARD_RUNS: Cell<usize> = const { Cell::new(0) };
			static CONTROLLED_GUARD_GATE: Cell<bool> = const { Cell::new(false) };
			static CONTROLLED_GUARD_SESSION_QUERY: Cell<bool> = const { Cell::new(false) };
			static SESSION_FETCHES: Cell<usize> = const { Cell::new(0) };
			static ROOT_GUARD_RESULTS: RefCell<VecDeque<Result<NavigationDecision, NavigationGuardError>>> = const { RefCell::new(VecDeque::new()) };
			static ROOT_GUARD_BLOCKED: Cell<bool> = const { Cell::new(false) };
			static ROOT_GUARD_OPEN: Cell<bool> = const { Cell::new(false) };
			static CHILD_GUARD_BLOCKED: Cell<bool> = const { Cell::new(false) };
			static CHILD_GUARD_OPEN: Cell<bool> = const { Cell::new(false) };
			static AUTHENTICATION_GUARD_RUNS: Cell<usize> = const { Cell::new(0) };
			static AUTHENTICATION_GUARD_TRIGGER_401: Cell<bool> = const { Cell::new(false) };
			static REDIRECTS: RefCell<HashMap<String, NavigationDecision>> = RefCell::new(HashMap::new());
		}

		fn record_navigation_guard(name: &'static str) -> NavigationDecision {
			NAVIGATION_GUARD_ORDER.with(|order| order.borrow_mut().push(name));
			NavigationDecision::Allow
		}

		#[navigation_guard]
		async fn coordinator_root_guard(
			_context: NavigationContext,
		) -> Result<NavigationDecision, NavigationGuardError> {
			record_navigation_guard("root");
			if ROOT_GUARD_BLOCKED.with(Cell::get) {
				poll_fn(|_| {
					if ROOT_GUARD_OPEN.with(Cell::get) {
						Poll::Ready(())
					} else {
						Poll::Pending
					}
				})
				.await;
			}
			ROOT_GUARD_RESULTS.with(|results| {
				results
					.borrow_mut()
					.pop_front()
					.unwrap_or(Ok(NavigationDecision::Allow))
			})
		}

		#[navigation_guard]
		async fn coordinator_child_guard(
			_context: NavigationContext,
		) -> Result<NavigationDecision, NavigationGuardError> {
			record_navigation_guard("child");
			if CHILD_GUARD_BLOCKED.with(Cell::get) {
				poll_fn(|_| {
					if CHILD_GUARD_OPEN.with(Cell::get) {
						Poll::Ready(())
					} else {
						Poll::Pending
					}
				})
				.await;
			}
			Ok(NavigationDecision::Allow)
		}

		#[navigation_guard]
		async fn coordinator_leaf_guard(
			_context: NavigationContext,
		) -> Result<NavigationDecision, NavigationGuardError> {
			Ok(record_navigation_guard("leaf"))
		}

		#[navigation_guard]
		async fn coordinator_controlled_guard(
			context: NavigationContext,
		) -> Result<NavigationDecision, NavigationGuardError> {
			CONTROLLED_GUARD_RUNS.with(|runs| runs.set(runs.get() + 1));
			NAVIGATION_GUARD_ORDER.with(|order| order.borrow_mut().push("controlled"));
			if CONTROLLED_GUARD_SESSION_QUERY.with(Cell::get) {
				let descriptor =
					QueryFamily::<(), String, NavigationGuardError>::new("coordinator.session")
						.query((), || async {
							SESSION_FETCHES.with(|fetches| fetches.set(fetches.get() + 1));
							Ok("session".to_owned())
						});
				context.query(descriptor, QueryOptions::new()).await?;
			}
			if CONTROLLED_GUARD_GATE.with(Cell::get) {
				poll_fn(|_| {
					if GATE_OPEN.with(Cell::get) {
						Poll::Ready(())
					} else {
						Poll::Pending
					}
				})
				.await;
			}
			CONTROLLED_GUARD_RESULTS.with(|results| {
				results
					.borrow_mut()
					.pop_front()
					.unwrap_or(Ok(NavigationDecision::Allow))
			})
		}

		#[navigation_guard]
		async fn coordinator_redirect_guard(
			context: NavigationContext,
		) -> Result<NavigationDecision, NavigationGuardError> {
			Ok(REDIRECTS.with(|redirects| {
				redirects
					.borrow()
					.get(context.destination())
					.cloned()
					.unwrap_or(NavigationDecision::Allow)
			}))
		}

		#[navigation_guard]
		async fn coordinator_authentication_401_guard(
			_context: NavigationContext,
		) -> Result<NavigationDecision, NavigationGuardError> {
			AUTHENTICATION_GUARD_RUNS.with(|runs| runs.set(runs.get() + 1));
			if AUTHENTICATION_GUARD_TRIGGER_401.with(Cell::get) {
				crate::auth::observe_server_fn_status(401);
			}
			Ok(NavigationDecision::Allow)
		}

		#[component(
			"/authentication-401/",
			name = "coordinator-authentication-401",
			navigation_guard = coordinator_authentication_401_guard,
		)]
		fn coordinator_authentication_401() -> Page {
			Page::text("authentication guard")
		}

		#[layout(
			"/guarded/",
			name = "coordinator-root-guarded",
			navigation_guard = coordinator_root_guard,
		)]
		fn coordinator_root_guarded(outlet: Outlet) -> Page {
			outlet.into_page()
		}

		#[layout(
			"child/",
			name = "coordinator-child-guarded",
			navigation_guard = coordinator_child_guard,
		)]
		fn coordinator_child_guarded(outlet: Outlet) -> Page {
			outlet.into_page()
		}

		#[component(
			"leaf/",
			name = "coordinator-leaf-guarded",
			navigation_guard = coordinator_leaf_guard,
		)]
		fn coordinator_leaf_guarded() -> Page {
			Page::text("guarded leaf")
		}

		async fn gated_value(value: &'static str) -> Result<String, String> {
			poll_fn(|_| {
				GATE_OPEN.with(|gate| {
					if gate.get() {
						Poll::Ready(Ok(value.to_owned()))
					} else {
						Poll::Pending
					}
				})
			})
			.await
		}

		#[loader]
		async fn coordinator_slow_loader() -> Result<String, String> {
			SLOW_LOADER_STARTS.with(|starts| starts.set(starts.get() + 1));
			gated_value("prepared slow route").await
		}

		#[component(
			"/loaded/",
			name = "coordinator-loaded",
			loader = coordinator_slow_loader,
		)]
		fn coordinator_loaded(Loader(value): Loader<String>) -> Page {
			Page::text(value)
		}

		#[component(
			"/guarded-loaded/",
			name = "coordinator-guarded-loaded",
			loader = coordinator_slow_loader,
			navigation_guard = coordinator_controlled_guard,
		)]
		fn coordinator_guarded_loaded(Loader(value): Loader<String>) -> Page {
			Page::text(value)
		}

		#[component(
			"/redirect-a/",
			name = "coordinator-redirect-a",
			navigation_guard = coordinator_redirect_guard,
		)]
		fn coordinator_redirect_a() -> Page {
			Page::text("redirect a")
		}

		#[component(
			"/redirect-b/",
			name = "coordinator-redirect-b",
			navigation_guard = coordinator_redirect_guard,
		)]
		fn coordinator_redirect_b() -> Page {
			Page::text("redirect b")
		}

		#[component("/login/", name = "coordinator-login")]
		fn coordinator_login() -> Page {
			Page::text("login")
		}

		#[loader]
		async fn coordinator_layout_loader() -> Result<String, String> {
			LAYOUT_LOADER_STARTS.with(|starts| starts.set(starts.get() + 1));
			gated_value("prepared layout").await
		}

		#[loader]
		async fn coordinator_leaf_loader() -> Result<String, String> {
			LEAF_LOADER_STARTS.with(|starts| starts.set(starts.get() + 1));
			gated_value("prepared leaf").await
		}

		#[layout(
			"/parallel/",
			name = "coordinator-layout",
			loader = coordinator_layout_loader,
			navigation_guard = coordinator_root_guard,
		)]
		fn coordinator_layout(Loader(value): Loader<String>, outlet: Outlet) -> Page {
			Page::fragment([Page::text(value), outlet.into_page()])
		}

		#[component(
			"child/",
			name = "coordinator-leaf",
			loader = coordinator_leaf_loader,
			navigation_guard = coordinator_child_guard,
		)]
		fn coordinator_leaf(Loader(value): Loader<String>) -> Page {
			Page::text(value)
		}

		#[loader]
		async fn coordinator_error_loader() -> Result<String, String> {
			Err("safe route-loader failure".to_owned())
		}

		#[loader]
		async fn coordinator_fail_fast_layout_loader() -> Result<String, String> {
			Err("fail fast route-loader failure".to_owned())
		}

		#[loader]
		async fn coordinator_fail_fast_leaf_loader() -> Result<String, String> {
			gated_value("unreachable slow loader").await
		}

		#[component(
			"/error/",
			name = "coordinator-error",
			loader = coordinator_error_loader,
		)]
		fn coordinator_error(Loader(_value): Loader<String>) -> Page {
			Page::text("unreachable")
		}

		#[layout(
			"/fail-fast/",
			name = "coordinator-fail-fast-layout",
			loader = coordinator_fail_fast_layout_loader,
		)]
		fn coordinator_fail_fast_layout(Loader(_value): Loader<String>, outlet: Outlet) -> Page {
			outlet.into_page()
		}

		#[component(
			"child/",
			name = "coordinator-fail-fast-leaf",
			loader = coordinator_fail_fast_leaf_loader,
		)]
		fn coordinator_fail_fast_leaf(Loader(_value): Loader<String>) -> Page {
			Page::text("unreachable")
		}

		type Task = Pin<Box<dyn Future<Output = ()> + 'static>>;

		fn poll_rounds(tasks: &Rc<RefCell<VecDeque<Task>>>, rounds: usize) {
			for _ in 0..rounds {
				let count = tasks.borrow().len();
				if count == 0 {
					return;
				}
				for _ in 0..count {
					let Some(mut task) = tasks.borrow_mut().pop_front() else {
						break;
					};
					let mut context = Context::from_waker(Waker::noop());
					if task.as_mut().poll(&mut context).is_pending() {
						tasks.borrow_mut().push_back(task);
					}
				}
			}
		}

		fn reset_test_state() {
			GATE_OPEN.with(|gate| gate.set(false));
			GUARD_ALLOWS.with(|allows| allows.set(true));
			SLOW_LOADER_STARTS.with(|starts| starts.set(0));
			LAYOUT_LOADER_STARTS.with(|starts| starts.set(0));
			LEAF_LOADER_STARTS.with(|starts| starts.set(0));
			NAVIGATION_GUARD_ORDER.with(|order| order.borrow_mut().clear());
			CONTROLLED_GUARD_RUNS.with(|runs| runs.set(0));
			CONTROLLED_GUARD_RESULTS.with(|results| results.borrow_mut().clear());
			CONTROLLED_GUARD_GATE.with(|gate| gate.set(false));
			CONTROLLED_GUARD_SESSION_QUERY.with(|query| query.set(false));
			SESSION_FETCHES.with(|fetches| fetches.set(0));
			ROOT_GUARD_RESULTS.with(|results| results.borrow_mut().clear());
			ROOT_GUARD_BLOCKED.with(|blocked| blocked.set(false));
			ROOT_GUARD_OPEN.with(|open| open.set(false));
			CHILD_GUARD_BLOCKED.with(|blocked| blocked.set(false));
			CHILD_GUARD_OPEN.with(|open| open.set(false));
			AUTHENTICATION_GUARD_RUNS.with(|runs| runs.set(0));
			AUTHENTICATION_GUARD_TRIGGER_401.with(|trigger| trigger.set(false));
			REDIRECTS.with(|redirects| redirects.borrow_mut().clear());
		}

		fn provide_test_query_client() -> QueryClientGuard {
			provide_query_client(QueryClient::new(QueryDefaults::default()))
		}

		fn router_with_loaded_routes() -> ClientRouter {
			ClientRouter::new()
				.route("root", "/", || Page::text("old route"))
				.component(coordinator_loaded)
				.component(coordinator_guarded_loaded)
				.component(coordinator_redirect_a)
				.component(coordinator_redirect_b)
				.component(coordinator_login)
				.component(coordinator_authentication_401)
				.component(coordinator_error)
				.routes(|routes| {
					routes
						.layout(coordinator_layout, |children| {
							children.component(coordinator_leaf)
						})
						.layout(coordinator_fail_fast_layout, |children| {
							children.component(coordinator_fail_fast_leaf)
						})
				})
		}

		fn router_with_navigation_guards() -> ClientRouter {
			ClientRouter::new().routes(|routes| {
				routes.layout(coordinator_root_guarded, |children| {
					children.layout(coordinator_child_guarded, |children| {
						children.component(coordinator_leaf_guarded)
					})
				})
			})
		}

		#[test]
		fn authentication_invalidation_coalesces_and_revalidates_with_replace() {
			ReactiveScope::run(|| {
				reset_test_state();
				let _query_client = provide_test_query_client();
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				let router = Rc::new(router_with_loaded_routes());
				crate::app::__install_client_router_for_test((*router).clone());
				let coordinator = crate::app::try_with_navigation_coordinator(Rc::clone)
					.expect("the test app installs a navigation coordinator");
				GATE_OPEN.with(|gate| gate.set(true));

				coordinator
					.navigate("/".to_owned(), NavigationIntent::Initial)
					.expect("initial route commits");
				coordinator
					.navigate("/guarded-loaded/".to_owned(), NavigationIntent::Push)
					.expect("protected route starts");
				poll_rounds(&tasks, 12);
				assert_eq!(router.current_path().get(), "/guarded-loaded/");
				assert_eq!(coordinator.committed_index(), 1);
				let runs_before = CONTROLLED_GUARD_RUNS.with(Cell::get);
				GATE_OPEN.with(|gate| gate.set(false));
				CONTROLLED_GUARD_GATE.with(|gate| gate.set(true));
				coordinator
					.navigate("/guarded-loaded/".to_owned(), NavigationIntent::Push)
					.expect("a second protected navigation starts a pending guard");
				poll_rounds(&tasks, 2);
				assert!(
					coordinator.pending().get(),
					"the replacement must cancel pending preparation"
				);

				crate::auth::invalidate_authentication();
				crate::auth::invalidate_authentication();
				assert!(
					!coordinator.pending().get(),
					"invalidation cancels active preparation immediately"
				);
				GATE_OPEN.with(|gate| gate.set(true));
				poll_rounds(&tasks, 12);

				assert_eq!(router.current_path().get(), "/guarded-loaded/");
				assert_eq!(
					coordinator.committed_index(),
					1,
					"replace revalidation does not push history"
				);
				assert_eq!(
					CONTROLLED_GUARD_RUNS.with(Cell::get),
					runs_before + 3,
					"coalesced invalidations perform one guard pipeline"
				);
				crate::app::__clear_spa_router_for_test();
			});
		}

		#[test]
		fn authentication_invalidation_redirects_anonymous_branch_with_replace() {
			ReactiveScope::run(|| {
				reset_test_state();
				let _query_client = provide_test_query_client();
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				let router = Rc::new(router_with_loaded_routes());
				crate::app::__install_client_router_for_test((*router).clone());
				let coordinator = crate::app::try_with_navigation_coordinator(Rc::clone)
					.expect("the test app installs a navigation coordinator");
				GATE_OPEN.with(|gate| gate.set(true));
				coordinator
					.navigate("/guarded-loaded/".to_owned(), NavigationIntent::Push)
					.expect("protected route starts");
				poll_rounds(&tasks, 12);
				assert_eq!(coordinator.committed_index(), 1);

				CONTROLLED_GUARD_RESULTS.with(|results| {
					results
						.borrow_mut()
						.push_back(Ok(NavigationDecision::Redirect {
							location: "/redirect-a/".to_owned(),
							replace: true,
						}));
				});
				crate::auth::invalidate_authentication();
				poll_rounds(&tasks, 16);

				assert_eq!(router.current_path().get(), "/redirect-a/");
				assert_eq!(
					coordinator.committed_index(),
					1,
					"anonymous redirect replaces the protected entry"
				);
				crate::app::__clear_spa_router_for_test();
			});
		}

		#[test]
		fn authentication_invalidation_preserves_navigation_started_before_replacement() {
			ReactiveScope::run(|| {
				reset_test_state();
				let _query_client = provide_test_query_client();
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				let router = Rc::new(router_with_loaded_routes());
				crate::app::__install_client_router_for_test((*router).clone());
				let coordinator = crate::app::try_with_navigation_coordinator(Rc::clone)
					.expect("the test app installs a navigation coordinator");
				GATE_OPEN.with(|gate| gate.set(true));

				coordinator
					.navigate("/".to_owned(), NavigationIntent::Initial)
					.expect("initial route commits");
				coordinator
					.navigate("/guarded-loaded/".to_owned(), NavigationIntent::Push)
					.expect("protected route starts");
				poll_rounds(&tasks, 12);
				assert_eq!(router.current_path().get(), "/guarded-loaded/");
				assert_eq!(coordinator.committed_index(), 1);

				crate::auth::invalidate_authentication();
				coordinator
					.navigate("/loaded/".to_owned(), NavigationIntent::Push)
					.expect("post-invalidation navigation starts before replacement");
				poll_rounds(&tasks, 16);

				assert_eq!(router.current_path().get(), "/loaded/");
				assert_eq!(
					coordinator.committed_index(),
					2,
					"a newer navigation must not be cancelled by deferred replacement of the old branch"
				);
				let store = coordinator
					.mounted_store()
					.expect("successful navigation retains its loader store");
				let html = with_loader_store(&store, || router.render_current().render_to_string());
				assert_eq!(html, "prepared slow route");
				crate::app::__clear_spa_router_for_test();
			});
		}

		#[rstest]
		fn distinct_authentication_change_supersedes_pending_revalidation() {
			ReactiveScope::run(|| {
				// Arrange
				reset_test_state();
				let _query_client = provide_test_query_client();
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				let router = Rc::new(router_with_loaded_routes());
				crate::app::__install_client_router_for_test((*router).clone());
				let coordinator = crate::app::try_with_navigation_coordinator(Rc::clone)
					.expect("the test app installs a navigation coordinator");
				let auth = crate::auth::auth_state();
				auth.login("initial", "initial-user");
				CONTROLLED_GUARD_SESSION_QUERY.with(|query| query.set(true));
				GATE_OPEN.with(|gate| gate.set(true));
				coordinator
					.navigate("/guarded-loaded/".to_owned(), NavigationIntent::Initial)
					.expect("initial authenticated route starts");
				poll_rounds(&tasks, 16);
				assert_eq!(SESSION_FETCHES.with(Cell::get), 1);

				GATE_OPEN.with(|gate| gate.set(false));
				CONTROLLED_GUARD_GATE.with(|gate| gate.set(true));
				auth.login("intermediate", "intermediate-user");
				crate::auth::invalidate_authentication();
				poll_rounds(&tasks, 16);
				assert!(coordinator.pending().get());
				assert_eq!(SESSION_FETCHES.with(Cell::get), 2);

				// Act
				auth.login("latest", "latest-user");
				crate::auth::invalidate_authentication();

				// Assert
				assert!(
					!coordinator.pending().get(),
					"a newer authentication generation must cancel the pending replacement"
				);
				GATE_OPEN.with(|gate| gate.set(true));
				poll_rounds(&tasks, 24);
				assert_eq!(
					SESSION_FETCHES.with(Cell::get),
					3,
					"the newer generation must clear data fetched by the intermediate account"
				);
				assert_eq!(router.current_path().get(), "/guarded-loaded/");
				assert_eq!(auth.user_id(), Some("latest".to_owned()));
				auth.logout();
				crate::app::__clear_spa_router_for_test();
			});
		}

		#[test]
		fn persistent_401_during_authentication_revalidation_does_not_reschedule() {
			ReactiveScope::run(|| {
				reset_test_state();
				let _query_client = provide_test_query_client();
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				let router = Rc::new(router_with_loaded_routes());
				crate::app::__install_client_router_for_test((*router).clone());
				let coordinator = crate::app::try_with_navigation_coordinator(Rc::clone)
					.expect("the test app installs a navigation coordinator");

				coordinator
					.navigate("/authentication-401/".to_owned(), NavigationIntent::Initial)
					.expect("guard evaluation starts");
				poll_rounds(&tasks, 16);
				assert_eq!(router.current_path().get(), "/authentication-401/");
				let runs_before = AUTHENTICATION_GUARD_RUNS.with(Cell::get);
				AUTHENTICATION_GUARD_TRIGGER_401.with(|trigger| trigger.set(true));
				crate::auth::invalidate_authentication();
				poll_rounds(&tasks, 24);

				assert_eq!(
					AUTHENTICATION_GUARD_RUNS.with(Cell::get),
					runs_before + 2,
					"persistent 401 responses stay inside one two-pass revalidation"
				);
				assert!(
					tasks.borrow().is_empty(),
					"persistent 401 responses must not leave replacement tasks scheduled"
				);
				assert!(!coordinator.pending().get());
				assert_eq!(router.current_path().get(), "/authentication-401/");
				crate::app::__clear_spa_router_for_test();
			});
		}

		#[test]
		fn navigation_guards_run_from_root_to_leaf_before_commit() {
			ReactiveScope::run(|| {
				reset_test_state();
				let _query_client = provide_test_query_client();
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				let router = Rc::new(router_with_navigation_guards());
				let coordinator = NavigationCoordinator::new(Rc::clone(&router))
					.expect("the test guard registry should be valid");

				coordinator
					.navigate("/guarded/child/leaf/".to_owned(), NavigationIntent::Push)
					.expect("guarded navigation is accepted");
				poll_rounds(&tasks, 4);

				assert_eq!(
					NAVIGATION_GUARD_ORDER.with(|order| order.borrow().clone()),
					["root", "child", "leaf", "root", "child", "leaf"]
				);
				assert_eq!(router.current_path().get(), "/guarded/child/leaf/");
				assert_eq!(
					router.render_current().render_to_string(),
					"guarded leaf",
					"an approved coordinator commit must still render the guarded route"
				);
				assert!(!coordinator.pending().get());
			});
		}

		#[test]
		fn redirect_loop_detection_normalizes_destinations_and_preserves_query_distinctions() {
			ReactiveScope::run(|| {
				reset_test_state();
				let _query_client = provide_test_query_client();
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				let router = Rc::new(router_with_loaded_routes());
				let coordinator =
					NavigationCoordinator::new(Rc::clone(&router)).expect("registry builds");

				REDIRECTS.with(|redirects| {
					redirects.borrow_mut().insert(
						"/redirect-a/".to_owned(),
						NavigationDecision::Redirect {
							location: "/redirect-a/./".to_owned(),
							replace: false,
						},
					);
				});
				coordinator
					.navigate("/redirect-a/".to_owned(), NavigationIntent::Push)
					.expect("redirect navigation starts");
				poll_rounds(&tasks, 8);
				assert_eq!(coordinator.committed_index(), 0);
				assert_eq!(
					coordinator.error().get().and_then(|error| error.status()),
					Some(500)
				);

				reset_test_state();
				REDIRECTS.with(|redirects| {
					redirects.borrow_mut().extend([
						(
							"/redirect-a/?next=one".to_owned(),
							NavigationDecision::Redirect {
								location: "/redirect-b/?next=two".to_owned(),
								replace: false,
							},
						),
						(
							"/redirect-b/?next=two".to_owned(),
							NavigationDecision::Redirect {
								location: "/redirect-a/?next=one".to_owned(),
								replace: false,
							},
						),
					]);
				});
				coordinator
					.navigate("/redirect-a/?next=one".to_owned(), NavigationIntent::Push)
					.expect("multi-target redirect starts");
				poll_rounds(&tasks, 12);
				assert_eq!(coordinator.committed_index(), 0);
				assert_eq!(
					coordinator.error().get().and_then(|error| error.status()),
					Some(500)
				);

				reset_test_state();
				REDIRECTS.with(|redirects| {
					redirects.borrow_mut().insert(
						"/redirect-a/#one".to_owned(),
						NavigationDecision::Redirect {
							location: "/redirect-a/#two".to_owned(),
							replace: false,
						},
					);
				});
				coordinator
					.navigate("/redirect-a/#one".to_owned(), NavigationIntent::Push)
					.expect("fragment self redirect starts");
				poll_rounds(&tasks, 12);
				assert_eq!(router.current_path().get(), "/");
				assert_eq!(coordinator.committed_index(), 0);
				assert_eq!(
					coordinator.error().get().and_then(|error| error.status()),
					Some(500),
					"fragment-only self redirects must terminate as a loop"
				);
				assert!(!coordinator.pending().get());

				reset_test_state();
				REDIRECTS.with(|redirects| {
					redirects.borrow_mut().extend([
						(
							"/redirect-a/#one".to_owned(),
							NavigationDecision::Redirect {
								location: "/redirect-b/#two".to_owned(),
								replace: false,
							},
						),
						(
							"/redirect-b/".to_owned(),
							NavigationDecision::Redirect {
								location: "/redirect-a/#three".to_owned(),
								replace: false,
							},
						),
					]);
				});
				coordinator
					.navigate("/redirect-a/#one".to_owned(), NavigationIntent::Push)
					.expect("fragment alternating redirect starts");
				poll_rounds(&tasks, 12);
				assert_eq!(router.current_path().get(), "/");
				assert_eq!(coordinator.committed_index(), 0);
				assert_eq!(
					coordinator.error().get().and_then(|error| error.status()),
					Some(500),
					"fragment-varying alternating redirects must terminate as a loop"
				);
				assert!(!coordinator.pending().get());
			});
		}

		#[test]
		fn redirected_navigation_uses_the_normalized_destination() {
			ReactiveScope::run(|| {
				reset_test_state();
				let _query_client = provide_test_query_client();
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				let router = Rc::new(router_with_loaded_routes());
				let coordinator =
					NavigationCoordinator::new(Rc::clone(&router)).expect("registry builds");
				REDIRECTS.with(|redirects| {
					redirects.borrow_mut().insert(
						"/redirect-a/".to_owned(),
						NavigationDecision::Redirect {
							location: "/old/../login/".to_owned(),
							replace: false,
						},
					);
				});

				coordinator
					.navigate("/redirect-a/".to_owned(), NavigationIntent::Push)
					.expect("redirect navigation starts");
				poll_rounds(&tasks, 8);

				assert_eq!(router.current_path().get(), "/login/");
				assert_eq!(router.render_current().render_to_string(), "login");
			});
		}

		#[test]
		fn redirects_preserve_pop_target_history_indices() {
			ReactiveScope::run(|| {
				for (replace, target_index, expected_index) in [
					(true, Some(3), 3),
					(false, Some(3), 4),
					(true, None, 4),
					(false, None, 5),
				] {
					reset_test_state();
					let _query_client = provide_test_query_client();
					let tasks = Rc::new(RefCell::new(VecDeque::new()));
					let tasks_for_sink = Rc::clone(&tasks);
					let _sink = crate::platform::install_task_sink(move |task| {
						tasks_for_sink.borrow_mut().push_back(task);
					});
					REDIRECTS.with(|redirects| {
						redirects.borrow_mut().insert(
							"/redirect-a/".to_owned(),
							NavigationDecision::Redirect {
								location: "/".to_owned(),
								replace,
							},
						);
					});
					let router = Rc::new(router_with_loaded_routes());
					let coordinator =
						NavigationCoordinator::new(Rc::clone(&router)).expect("registry builds");
					coordinator.initialize_committed_index(4);

					coordinator
						.navigate(
							"/redirect-a/".to_owned(),
							NavigationIntent::Pop { target_index },
						)
						.expect("guarded pop starts");
					poll_rounds(&tasks, 8);

					assert_eq!(router.current_path().get(), "/");
					assert_eq!(coordinator.committed_index(), expected_index);
				}
			});
		}

		#[test]
		fn push_redirect_replace_preserves_source_history_entry() {
			ReactiveScope::run(|| {
				reset_test_state();
				let _query_client = provide_test_query_client();
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				let router = Rc::new(router_with_loaded_routes());
				let coordinator =
					NavigationCoordinator::new(Rc::clone(&router)).expect("registry builds");
				coordinator
					.navigate("/".to_owned(), NavigationIntent::Initial)
					.expect("source page commits");
				assert_eq!(coordinator.committed_index(), 0);

				REDIRECTS.with(|redirects| {
					redirects.borrow_mut().extend([
						(
							"/redirect-a/".to_owned(),
							NavigationDecision::Redirect {
								location: "/redirect-b/".to_owned(),
								replace: true,
							},
						),
						(
							"/redirect-b/".to_owned(),
							NavigationDecision::Redirect {
								location: "/login/".to_owned(),
								replace: true,
							},
						),
					]);
				});
				coordinator
					.navigate("/redirect-a/".to_owned(), NavigationIntent::Push)
					.expect("guarded push starts");
				poll_rounds(&tasks, 12);

				assert_eq!(router.current_path().get(), "/login/");
				assert_eq!(router.render_current().render_to_string(), "login");
				assert_eq!(
					coordinator.committed_index(),
					1,
					"replace redirects from an uncommitted push must not overwrite the source entry"
				);
			});
		}

		#[test]
		fn redirected_pop_rejection_restores_original_committed_entry() {
			ReactiveScope::run(|| {
				reset_test_state();
				let _query_client = provide_test_query_client();
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				REDIRECTS.with(|redirects| {
					redirects.borrow_mut().extend([
						(
							"/redirect-a/".to_owned(),
							NavigationDecision::Redirect {
								location: "/redirect-b/".to_owned(),
								replace: true,
							},
						),
						("/redirect-b/".to_owned(), NavigationDecision::Forbidden),
					]);
				});
				let router = Rc::new(router_with_loaded_routes());
				let coordinator =
					NavigationCoordinator::new(Rc::clone(&router)).expect("registry builds");
				coordinator.initialize_committed_index(4);

				coordinator
					.navigate(
						"/redirect-a/".to_owned(),
						NavigationIntent::Pop {
							target_index: Some(3),
						},
					)
					.expect("guarded pop starts");
				poll_rounds(&tasks, 12);

				assert_eq!(router.current_path().get(), "/");
				assert_eq!(coordinator.committed_index(), 4);
				assert_eq!(
					coordinator.error().get().and_then(|error| error.status()),
					Some(403)
				);
				assert!(coordinator.consume_restoration_pop());
				assert!(!coordinator.consume_restoration_pop());
			});
		}

		#[test]
		fn prefetch_only_loads_after_allow_and_click_reuses_the_guard_query() {
			ReactiveScope::run(|| {
				for decision in [
					Ok(NavigationDecision::Redirect {
						location: "/".to_owned(),
						replace: true,
					}),
					Ok(NavigationDecision::NotFound),
					Ok(NavigationDecision::Forbidden),
					Err(NavigationGuardError::new("prefetch guard failed")),
				] {
					reset_test_state();
					let _query_client = provide_test_query_client();
					let tasks = Rc::new(RefCell::new(VecDeque::new()));
					let tasks_for_sink = Rc::clone(&tasks);
					let _sink = crate::platform::install_task_sink(move |task| {
						tasks_for_sink.borrow_mut().push_back(task);
					});
					CONTROLLED_GUARD_RESULTS
						.with(|results| results.borrow_mut().push_back(decision));
					let router = Rc::new(router_with_loaded_routes());
					let coordinator =
						NavigationCoordinator::new(Rc::clone(&router)).expect("registry builds");
					let history_index_before = coordinator.committed_index();
					coordinator
						.prefetch("/guarded-loaded/".to_owned())
						.expect("prefetch starts");
					poll_rounds(&tasks, 8);
					assert_eq!(
						CONTROLLED_GUARD_RUNS.with(Cell::get),
						1,
						"prefetch must evaluate each non-allow guard decision"
					);
					assert_eq!(
						NAVIGATION_GUARD_ORDER.with(|order| order.borrow().clone()),
						["controlled"],
						"prefetch must stop after the rejecting guard"
					);
					assert_eq!(SLOW_LOADER_STARTS.with(Cell::get), 0);
					assert_eq!(router.current_path().get(), "/");
					assert!(coordinator.error().get().is_none());
					assert_eq!(
						coordinator.committed_index(),
						history_index_before,
						"prefetch rejection must not mutate committed history"
					);
				}

				reset_test_state();
				let runtime = TestQueryRuntime::new();
				let client = QueryClient::with_runtime(
					QueryDefaults::default().gc_time(Duration::ZERO),
					runtime.handle(),
				);
				let _query_client = provide_query_client(client);
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				CONTROLLED_GUARD_SESSION_QUERY.with(|query| query.set(true));
				GATE_OPEN.with(|gate| gate.set(true));
				let router = Rc::new(router_with_loaded_routes());
				let coordinator =
					NavigationCoordinator::new(Rc::clone(&router)).expect("registry builds");
				coordinator
					.prefetch("/guarded-loaded/".to_owned())
					.expect("allowed prefetch starts");
				poll_rounds(&tasks, 4);
				runtime.run_until_stalled();
				poll_rounds(&tasks, 8);
				runtime.run_until_stalled();
				poll_rounds(&tasks, 4);
				assert_eq!(SESSION_FETCHES.with(Cell::get), 1);
				assert_eq!(SLOW_LOADER_STARTS.with(Cell::get), 1);

				coordinator
					.navigate("/guarded-loaded/".to_owned(), NavigationIntent::Push)
					.expect("click starts a new guard evaluation");
				poll_rounds(&tasks, 4);
				runtime.run_until_stalled();
				poll_rounds(&tasks, 8);
				assert_eq!(CONTROLLED_GUARD_RUNS.with(Cell::get), 3);
				assert_eq!(SESSION_FETCHES.with(Cell::get), 1);
				assert_eq!(router.current_path().get(), "/guarded-loaded/");
			});
		}

		#[test]
		fn first_async_guard_short_circuits_each_non_allow_outcome_before_child_guard() {
			ReactiveScope::run(|| {
				for result in [
					Ok(NavigationDecision::NotFound),
					Ok(NavigationDecision::Forbidden),
					Ok(NavigationDecision::Redirect {
						location: "/".to_owned(),
						replace: true,
					}),
					Err(NavigationGuardError::new("root guard failed")),
				] {
					reset_test_state();
					let _query_client = provide_test_query_client();
					let tasks = Rc::new(RefCell::new(VecDeque::new()));
					let tasks_for_sink = Rc::clone(&tasks);
					let _sink = crate::platform::install_task_sink(move |task| {
						tasks_for_sink.borrow_mut().push_back(task);
					});
					ROOT_GUARD_RESULTS.with(|results| results.borrow_mut().push_back(result));
					let router = Rc::new(router_with_navigation_guards());
					let coordinator = NavigationCoordinator::new(router).expect("registry builds");

					coordinator
						.navigate("/guarded/child/leaf/".to_owned(), NavigationIntent::Push)
						.expect("guarded navigation starts");
					poll_rounds(&tasks, 4);

					assert_eq!(
						NAVIGATION_GUARD_ORDER.with(|order| order.borrow().clone()),
						["root"],
						"the second async guard must not run after a first-guard rejection"
					);
				}
			});
		}

		#[test]
		fn precommit_guard_error_drops_prepared_loader_lease() {
			ReactiveScope::run(|| {
				reset_test_state();
				let runtime = TestQueryRuntime::new();
				let client = QueryClient::with_runtime(
					QueryDefaults::default().gc_time(Duration::ZERO),
					runtime.handle(),
				);
				let _query_client = provide_query_client(client.clone());
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				CONTROLLED_GUARD_RESULTS.with(|results| {
					results.borrow_mut().extend([
						Ok(NavigationDecision::Allow),
						Err(NavigationGuardError::new("precommit guard failed")),
					]);
				});
				let router = Rc::new(router_with_loaded_routes());
				let coordinator = NavigationCoordinator::new(Rc::clone(&router))
					.expect("the test guard registry should be valid");

				coordinator
					.navigate("/guarded-loaded/".to_owned(), NavigationIntent::Push)
					.expect("guarded navigation is accepted");
				poll_rounds(&tasks, 4);
				runtime.run_until_stalled();
				poll_rounds(&tasks, 4);
				assert_eq!(SLOW_LOADER_STARTS.with(Cell::get), 1);
				GATE_OPEN.with(|gate| gate.set(true));
				runtime.run_until_stalled();
				poll_rounds(&tasks, 8);
				runtime.run_until_stalled();

				let matched = router
					.match_tree("/guarded-loaded/")
					.expect("guarded route matches");
				let cache_id = loader_cache_id(
					<coordinator_slow_loader::marker as RouteLoader>::ID,
					&route_context(&matched),
					coordinator_slow_loader::INPUTS,
				)
				.expect("loader cache id");
				let query_key = QueryFamily::<String, String, crate::RouteLoaderError>::new(
					<coordinator_slow_loader::marker as RouteLoader>::ID.as_str(),
				)
				.key(cache_id);
				runtime.run_due_maintenance();

				assert!(!coordinator.pending().get());
				assert_eq!(
					coordinator
						.error()
						.get()
						.map(|error| error.public_message().to_owned()),
					Some("precommit guard failed".to_owned())
				);
				assert!(
					!client.contains_for_test(&query_key),
					"dropping the prepared store must release its final loader lease"
				);
			});
		}

		#[test]
		fn precommit_forbidden_drops_prepared_loader_lease() {
			ReactiveScope::run(|| {
				reset_test_state();
				let runtime = TestQueryRuntime::new();
				let client = QueryClient::with_runtime(
					QueryDefaults::default().gc_time(Duration::ZERO),
					runtime.handle(),
				);
				let _query_client = provide_query_client(client.clone());
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				CONTROLLED_GUARD_RESULTS.with(|results| {
					results.borrow_mut().extend([
						Ok(NavigationDecision::Allow),
						Ok(NavigationDecision::Forbidden),
					]);
				});
				let router = Rc::new(router_with_loaded_routes());
				let coordinator = NavigationCoordinator::new(Rc::clone(&router))
					.expect("the test guard registry should be valid");

				coordinator
					.navigate("/guarded-loaded/".to_owned(), NavigationIntent::Push)
					.expect("guarded navigation is accepted");
				poll_rounds(&tasks, 4);
				runtime.run_until_stalled();
				poll_rounds(&tasks, 4);
				GATE_OPEN.with(|gate| gate.set(true));
				runtime.run_until_stalled();
				poll_rounds(&tasks, 8);
				runtime.run_until_stalled();

				let matched = router
					.match_tree("/guarded-loaded/")
					.expect("guarded route matches");
				let cache_id = loader_cache_id(
					<coordinator_slow_loader::marker as RouteLoader>::ID,
					&route_context(&matched),
					coordinator_slow_loader::INPUTS,
				)
				.expect("loader cache id");
				let query_key = QueryFamily::<String, String, crate::RouteLoaderError>::new(
					<coordinator_slow_loader::marker as RouteLoader>::ID.as_str(),
				)
				.key(cache_id);
				runtime.run_due_maintenance();

				assert_eq!(SLOW_LOADER_STARTS.with(Cell::get), 1);
				assert_eq!(
					coordinator.error().get().and_then(|error| error.status()),
					Some(403)
				);
				assert!(
					!client.contains_for_test(&query_key),
					"forbidden pre-commit must drop the final prepared loader lease"
				);
			});
		}

		#[test]
		fn non_allow_guard_decisions_prevent_loader_preparation() {
			ReactiveScope::run(|| {
				for decision in [
					NavigationDecision::NotFound,
					NavigationDecision::Forbidden,
					NavigationDecision::Redirect {
						location: "/".to_owned(),
						replace: true,
					},
				] {
					reset_test_state();
					let _query_client = provide_test_query_client();
					let tasks = Rc::new(RefCell::new(VecDeque::new()));
					let tasks_for_sink = Rc::clone(&tasks);
					let _sink = crate::platform::install_task_sink(move |task| {
						tasks_for_sink.borrow_mut().push_back(task);
					});
					CONTROLLED_GUARD_RESULTS
						.with(|results| results.borrow_mut().push_back(Ok(decision)));
					let router = Rc::new(router_with_loaded_routes());
					let coordinator = NavigationCoordinator::new(Rc::clone(&router))
						.expect("the test guard registry should be valid");

					coordinator
						.navigate("/guarded-loaded/".to_owned(), NavigationIntent::Push)
						.expect("guarded navigation is accepted");
					poll_rounds(&tasks, 4);

					assert_eq!(SLOW_LOADER_STARTS.with(Cell::get), 0);
					assert!(!coordinator.pending().get());
				}
			});
		}

		#[test]
		fn guard_error_short_circuits_loader_preparation_and_commit() {
			ReactiveScope::run(|| {
				reset_test_state();
				let _query_client = provide_test_query_client();
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				CONTROLLED_GUARD_RESULTS.with(|results| {
					results
						.borrow_mut()
						.push_back(Err(NavigationGuardError::new("guard failed")));
				});
				let router = Rc::new(router_with_loaded_routes());
				let coordinator = NavigationCoordinator::new(Rc::clone(&router))
					.expect("the test guard registry should be valid");

				coordinator
					.navigate("/guarded-loaded/".to_owned(), NavigationIntent::Push)
					.expect("guarded navigation is accepted");
				poll_rounds(&tasks, 4);

				assert_eq!(SLOW_LOADER_STARTS.with(Cell::get), 0);
				assert_eq!(router.current_path().get(), "/");
				assert_eq!(
					coordinator
						.error()
						.get()
						.map(|error| error.public_message().to_owned()),
					Some("guard failed".to_owned())
				);
			});
		}

		#[test]
		fn synchronous_route_guard_rejects_before_async_guard_runs() {
			ReactiveScope::run(|| {
				reset_test_state();
				let _query_client = provide_test_query_client();
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				let router = Rc::new(
					router_with_loaded_routes()
						.with_route_guard("coordinator-guarded-loaded", |_| false),
				);
				let coordinator = NavigationCoordinator::new(router).expect("registry builds");

				coordinator
					.navigate("/guarded-loaded/".to_owned(), NavigationIntent::Push)
					.expect("unmatched navigation is accepted");
				poll_rounds(&tasks, 4);

				assert_eq!(CONTROLLED_GUARD_RUNS.with(Cell::get), 0);
				assert_eq!(SLOW_LOADER_STARTS.with(Cell::get), 0);
			});
		}

		#[test]
		fn superseding_navigation_cancels_a_pending_guard_before_it_can_commit() {
			ReactiveScope::run(|| {
				reset_test_state();
				let _query_client = provide_test_query_client();
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				CONTROLLED_GUARD_GATE.with(|gate| gate.set(true));
				let router = Rc::new(router_with_loaded_routes());
				let coordinator = NavigationCoordinator::new(Rc::clone(&router))
					.expect("the test guard registry should be valid");

				coordinator
					.navigate("/guarded-loaded/".to_owned(), NavigationIntent::Push)
					.expect("guarded navigation starts");
				poll_rounds(&tasks, 4);
				assert!(coordinator.pending().get());
				assert_eq!(CONTROLLED_GUARD_RUNS.with(Cell::get), 1);
				assert_eq!(SLOW_LOADER_STARTS.with(Cell::get), 0);

				coordinator
					.navigate("/".to_owned(), NavigationIntent::Push)
					.expect("replacement navigation commits");
				GATE_OPEN.with(|gate| gate.set(true));
				poll_rounds(&tasks, 8);

				assert_eq!(router.current_path().get(), "/");
				assert_eq!(coordinator.committed_index(), 1);
				assert!(
					coordinator
						.mounted_store()
						.expect("the replacement route owns its empty loader store")
						.get::<String>(<coordinator_slow_loader::marker as RouteLoader>::ID)
						.is_err(),
					"the cancelled route must not mount its prepared loader value"
				);
				assert_eq!(SLOW_LOADER_STARTS.with(Cell::get), 0);
				assert!(!coordinator.pending().get());
			});
		}

		#[test]
		fn guard_reuses_one_fresh_session_query_across_precommit() {
			ReactiveScope::run(|| {
				reset_test_state();
				let runtime = TestQueryRuntime::new();
				let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
				let _query_client = provide_query_client(client);
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				CONTROLLED_GUARD_SESSION_QUERY.with(|query| query.set(true));
				GATE_OPEN.with(|gate| gate.set(true));
				let router = Rc::new(router_with_loaded_routes());
				let coordinator = NavigationCoordinator::new(Rc::clone(&router))
					.expect("the test guard registry should be valid");

				coordinator
					.navigate("/guarded-loaded/".to_owned(), NavigationIntent::Push)
					.expect("guarded navigation starts");
				poll_rounds(&tasks, 2);
				runtime.run_until_stalled();
				poll_rounds(&tasks, 8);
				runtime.run_until_stalled();
				poll_rounds(&tasks, 8);

				assert_eq!(SESSION_FETCHES.with(Cell::get), 1);
				assert_eq!(CONTROLLED_GUARD_RUNS.with(Cell::get), 2);
				assert_eq!(router.current_path().get(), "/guarded-loaded/");
			});
		}

		#[test]
		fn navigation_keeps_old_route_until_loader_commit() {
			ReactiveScope::run(|| {
				reset_test_state();
				let _query_client = provide_test_query_client();
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				let router = Rc::new(router_with_loaded_routes());
				let coordinator = NavigationCoordinator::new(Rc::clone(&router))
					.expect("the test loader registry should be valid");

				coordinator
					.navigate("/".to_owned(), NavigationIntent::Initial)
					.expect("initial route commits synchronously");
				coordinator
					.navigate("/loaded/".to_owned(), NavigationIntent::Push)
					.expect("loader navigation is accepted synchronously");
				poll_rounds(&tasks, 4);

				assert_eq!(router.current_path().get(), "/");
				assert!(coordinator.pending().get());
				assert_eq!(SLOW_LOADER_STARTS.with(Cell::get), 1);

				GATE_OPEN.with(|gate| gate.set(true));
				poll_rounds(&tasks, 8);

				assert_eq!(router.current_path().get(), "/loaded/");
				assert!(!coordinator.pending().get());
				let store = coordinator
					.mounted_store()
					.expect("successful navigation retains its loader store");
				let html = with_loader_store(&store, || router.render_current().render_to_string());
				assert_eq!(html, "prepared slow route");
			});
		}

		#[test]
		fn loader_navigation_rechecks_guards_before_commit() {
			ReactiveScope::run(|| {
				reset_test_state();
				let _query_client = provide_test_query_client();
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				let router = Rc::new(
					router_with_loaded_routes()
						.not_found(|| Page::text("guard denied"))
						.with_route_guard("coordinator-loaded", |_| GUARD_ALLOWS.with(Cell::get)),
				);
				let coordinator = NavigationCoordinator::new(Rc::clone(&router))
					.expect("the test loader registry should be valid");

				coordinator
					.navigate("/loaded/".to_owned(), NavigationIntent::Push)
					.expect("loader navigation is accepted synchronously");
				poll_rounds(&tasks, 4);
				assert!(coordinator.pending().get());

				GUARD_ALLOWS.with(|allows| allows.set(false));
				GATE_OPEN.with(|gate| gate.set(true));
				poll_rounds(&tasks, 8);

				assert_eq!(router.current_path().get(), "/loaded/");
				assert_eq!(router.current_route_name().get(), None);
				assert_eq!(router.render_current().render_to_string(), "guard denied");
				assert!(!coordinator.pending().get());
			});
		}

		#[test]
		fn nested_layout_and_leaf_loaders_start_in_parallel() {
			ReactiveScope::run(|| {
				reset_test_state();
				let _query_client = provide_test_query_client();
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				let router = Rc::new(router_with_loaded_routes());
				let coordinator = NavigationCoordinator::new(Rc::clone(&router))
					.expect("the test loader registry should be valid");
				coordinator
					.navigate("/parallel/child/".to_owned(), NavigationIntent::Push)
					.expect("nested navigation is accepted synchronously");
				poll_rounds(&tasks, 4);

				assert_eq!(router.current_path().get(), "/");
				assert_eq!(LAYOUT_LOADER_STARTS.with(Cell::get), 1);
				assert_eq!(LEAF_LOADER_STARTS.with(Cell::get), 1);
				assert!(coordinator.pending().get());

				GATE_OPEN.with(|gate| gate.set(true));
				poll_rounds(&tasks, 8);

				assert_eq!(router.current_path().get(), "/parallel/child/");
				let store = coordinator
					.mounted_store()
					.expect("successful nested navigation retains its loader store");
				let html = with_loader_store(&store, || router.render_current().render_to_string());
				assert_eq!(html, "prepared layoutprepared leaf");
			});
		}

		#[test]
		fn loaders_start_concurrently_only_after_every_guard_allows() {
			ReactiveScope::run(|| {
				reset_test_state();
				let _query_client = provide_test_query_client();
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				ROOT_GUARD_BLOCKED.with(|blocked| blocked.set(true));
				CHILD_GUARD_BLOCKED.with(|blocked| blocked.set(true));
				let router = Rc::new(router_with_loaded_routes());
				let coordinator = NavigationCoordinator::new(router).expect("registry builds");

				coordinator
					.navigate("/parallel/child/".to_owned(), NavigationIntent::Push)
					.expect("guarded nested navigation starts");
				poll_rounds(&tasks, 4);
				assert_eq!(
					NAVIGATION_GUARD_ORDER.with(|order| order.borrow().clone()),
					["root"]
				);
				assert_eq!(LAYOUT_LOADER_STARTS.with(Cell::get), 0);
				assert_eq!(LEAF_LOADER_STARTS.with(Cell::get), 0);

				ROOT_GUARD_OPEN.with(|open| open.set(true));
				poll_rounds(&tasks, 4);
				assert_eq!(
					NAVIGATION_GUARD_ORDER.with(|order| order.borrow().clone()),
					["root", "child"]
				);
				assert_eq!(LAYOUT_LOADER_STARTS.with(Cell::get), 0);
				assert_eq!(LEAF_LOADER_STARTS.with(Cell::get), 0);

				CHILD_GUARD_OPEN.with(|open| open.set(true));
				poll_rounds(&tasks, 4);
				assert_eq!(LAYOUT_LOADER_STARTS.with(Cell::get), 1);
				assert_eq!(LEAF_LOADER_STARTS.with(Cell::get), 1);
				assert!(coordinator.pending().get());
			});
		}

		#[test]
		fn superseded_generation_cannot_commit_obsolete_loader_result() {
			ReactiveScope::run(|| {
				reset_test_state();
				let _query_client = provide_test_query_client();
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				let router = Rc::new(router_with_loaded_routes());
				let coordinator = NavigationCoordinator::new(Rc::clone(&router))
					.expect("the test loader registry should be valid");
				coordinator
					.navigate("/loaded/".to_owned(), NavigationIntent::Push)
					.expect("first navigation is accepted");
				poll_rounds(&tasks, 4);

				coordinator
					.navigate("/".to_owned(), NavigationIntent::Push)
					.expect("new navigation supersedes the old one");
				assert_eq!(router.current_path().get(), "/");
				GATE_OPEN.with(|gate| gate.set(true));
				poll_rounds(&tasks, 8);

				assert_eq!(router.current_path().get(), "/");
				assert!(!coordinator.pending().get());
			});
		}

		#[test]
		fn failed_loader_retains_route_and_publishes_safe_error() {
			ReactiveScope::run(|| {
				reset_test_state();
				let _query_client = provide_test_query_client();
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				let router = Rc::new(router_with_loaded_routes());
				let coordinator = NavigationCoordinator::new(Rc::clone(&router))
					.expect("the test loader registry should be valid");
				coordinator
					.navigate("/".to_owned(), NavigationIntent::Initial)
					.expect("initial route commits");
				coordinator
					.navigate("/error/".to_owned(), NavigationIntent::Push)
					.expect("failed navigation is accepted before preparation");
				poll_rounds(&tasks, 8);

				assert_eq!(router.current_path().get(), "/");
				assert!(!coordinator.pending().get());
				assert_eq!(
					coordinator
						.error()
						.get()
						.map(|error| error.public_message().to_owned()),
					Some("safe route-loader failure".to_owned())
				);
			});
		}

		#[test]
		fn failed_loader_does_not_wait_for_a_slow_sibling() {
			ReactiveScope::run(|| {
				reset_test_state();
				let _query_client = provide_test_query_client();
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				let router = Rc::new(router_with_loaded_routes());
				let coordinator = NavigationCoordinator::new(router).expect("registry builds");

				coordinator
					.navigate("/fail-fast/child/".to_owned(), NavigationIntent::Push)
					.expect("navigation starts before loader preparation");
				poll_rounds(&tasks, 4);

				assert!(!coordinator.pending().get());
				assert_eq!(
					coordinator
						.error()
						.get()
						.map(|error| error.public_message().to_owned()),
					Some("fail fast route-loader failure".to_owned())
				);
			});
		}

		#[test]
		fn completed_prefetch_releases_its_task_guard() {
			ReactiveScope::run(|| {
				reset_test_state();
				let _query_client = provide_test_query_client();
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				let router = Rc::new(router_with_loaded_routes());
				let coordinator = NavigationCoordinator::new(router).expect("registry builds");

				coordinator
					.prefetch("/error/".to_owned())
					.expect("prefetch starts for a matched loader route");
				assert_eq!(coordinator.prefetch_tasks.borrow().len(), 1);
				poll_rounds(&tasks, 4);
				assert_eq!(coordinator.prefetch_tasks.borrow().len(), 0);
			});
		}

		#[test]
		fn authentication_change_cancels_and_drains_prefetch_tasks() {
			ReactiveScope::run(|| {
				reset_test_state();
				let _query_client = provide_test_query_client();
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				let router = Rc::new(router_with_loaded_routes());
				let coordinator = NavigationCoordinator::new(router).expect("registry builds");

				coordinator
					.prefetch("/loaded/".to_owned())
					.expect("prefetch starts for a matched loader route");
				assert_eq!(coordinator.prefetch_tasks.borrow().len(), 1);

				coordinator.clear_for_authentication_change();
				assert!(coordinator.prefetch_tasks.borrow().is_empty());
				poll_rounds(&tasks, 4);
				assert!(coordinator.prefetch_tasks.borrow().is_empty());
			});
		}

		#[test]
		fn failed_forward_pop_requests_history_restoration() {
			ReactiveScope::run(|| {
				reset_test_state();
				let _query_client = provide_test_query_client();
				let tasks = Rc::new(RefCell::new(VecDeque::new()));
				let tasks_for_sink = Rc::clone(&tasks);
				let _sink = crate::platform::install_task_sink(move |task| {
					tasks_for_sink.borrow_mut().push_back(task);
				});
				let router = Rc::new(router_with_loaded_routes());
				let coordinator = NavigationCoordinator::new(router).expect("registry builds");
				coordinator.initialize_committed_index(1);

				coordinator
					.navigate(
						"/error/".to_owned(),
						NavigationIntent::Pop {
							target_index: Some(2),
						},
					)
					.expect("pop preparation starts");
				poll_rounds(&tasks, 4);

				assert!(coordinator.consume_restoration_pop());
			});
		}
	}
}
