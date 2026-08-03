//! Pages-owned navigation preparation and commit coordination.

use crate::cancellation::{AbortableTaskGuard, CancellationSource};
use crate::reactive::QueryDefaults;
use crate::reactive::Signal;
use crate::reactive::hooks::router::NavigateError;
use crate::reactive::query::{QueryClient, current_query_client, with_query_client};
use crate::router::NavigationType;
use crate::router::loader::{LoaderStore, RouteLoaderError, route_context};
use crate::router::loader_registry::{LoaderConsumer, LoaderRegistry, execute_loader};
use futures_util::future::{join_all, try_join_all};
use reinhardt_urls::routers::client_router::{ClientRouteTreeMatch, ClientRouter};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// Pop and initial intents are supplied by the browser launcher; native unit
// tests exercise the synchronous push path only.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NavigationIntent {
	Initial,
	Push,
	Replace,
	Pop { target_index: Option<i64> },
}

impl NavigationIntent {
	fn navigation_type(self) -> NavigationType {
		match self {
			Self::Initial => NavigationType::Initial,
			Self::Push => NavigationType::Push,
			Self::Replace => NavigationType::Replace,
			Self::Pop { .. } => NavigationType::Pop,
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
	path: String,
	matched: ClientRouteTreeMatch,
	cancellation: CancellationSource,
	_task: AbortableTaskGuard,
}

/// Coordinates asynchronous route-loader preparation with synchronous URL
/// matching and commit operations.
pub(crate) struct NavigationCoordinator {
	router: Rc<ClientRouter>,
	query_client: QueryClient,
	registry: LoaderRegistry,
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
		Ok(Rc::new(Self {
			router,
			query_client,
			registry,
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
		if matched.loader_ids().is_empty() {
			return Ok(true);
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
		self.mounted_store.borrow_mut().replace(store);
		Ok(true)
	}

	pub(crate) fn navigate(
		self: &Rc<Self>,
		path: String,
		intent: NavigationIntent,
	) -> Result<(), NavigateError> {
		let query_client = self.query_client.clone();
		with_query_client(&query_client, || self.navigate_in_context(path, intent))
	}

	fn navigate_in_context(
		self: &Rc<Self>,
		path: String,
		intent: NavigationIntent,
	) -> Result<(), NavigateError> {
		let matched = self.router.match_tree(&path);

		self.cancel_active_attempt();
		let generation = self.next_generation.get().wrapping_add(1);
		self.next_generation.set(generation);
		self.error.set(None);
		if matched.is_none() {
			self.pending.set(false);
			return self.commit_unmatched(generation, path, intent);
		}
		let matched = matched.expect("matched routes are handled above");

		if matched.loader_ids().is_empty() {
			self.pending.set(false);
			return self.commit_success(generation, path, intent, matched, LoaderStore::new());
		}

		self.pending.set(true);
		let cancellation = CancellationSource::new();
		let cancellation_handle = cancellation.handle();
		let ids = matched.loader_ids().to_vec();
		let context = route_context(&matched);
		let coordinator = Rc::clone(self);
		let path_for_task = path.clone();
		let matched_for_task = matched.clone();
		let task_cancellation = cancellation_handle.clone();
		let task = crate::cancellation::spawn_abortable_task(async move {
			let futures = ids.into_iter().map(|id| {
				execute_loader(
					&coordinator.registry,
					id,
					&context,
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
			if task_cancellation.is_cancelled() {
				return;
			}
			let store = LoaderStore::new();
			for prepared in results {
				store.insert_prepared(prepared);
			}
			let _ = coordinator.commit_success(
				generation,
				path_for_task,
				intent,
				matched_for_task,
				store,
			);
		});

		*self.active_attempt.borrow_mut() = Some(NavigationAttempt {
			generation,
			intent,
			path,
			matched,
			cancellation,
			_task: task,
		});
		Ok(())
	}

	pub(crate) fn prefetch(self: &Rc<Self>, path: String) -> Result<(), NavigateError> {
		let query_client = self.query_client.clone();
		with_query_client(&query_client, || self.prefetch_in_context(path))
	}

	fn prefetch_in_context(self: &Rc<Self>, path: String) -> Result<(), NavigateError> {
		let Some(matched) = self.router.match_tree(&path) else {
			return Ok(());
		};
		let ids = matched.loader_ids().to_vec();
		if ids.is_empty() {
			return Ok(());
		}
		let context = route_context(&matched);
		let cancellation = CancellationSource::new();
		let handle = cancellation.handle();
		let prefetch_id = self.next_prefetch_id.get().wrapping_add(1);
		self.next_prefetch_id.set(prefetch_id);
		let coordinator = Rc::clone(self);
		let task = crate::cancellation::spawn_abortable_task(async move {
			let futures = ids.into_iter().map(|id| {
				execute_loader(
					&coordinator.registry,
					id,
					&context,
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

	fn is_current_generation(&self, generation: u64) -> bool {
		self.next_generation.get() == generation
	}

	fn finish_error(&self, generation: u64, error: RouteLoaderError) {
		if !self.is_current_generation(generation) {
			return;
		}
		self.pending.set(false);
		self.error.set(Some(error));
		if let Some(attempt) = self.active_attempt.borrow().as_ref()
			&& let NavigationIntent::Pop { target_index } = attempt.intent
		{
			// Legacy history entries lack a framework index. The browser reached
			// one through a back traversal from the committed entry, so move forward
			// once rather than treating the destination as the committed entry.
			let delta = target_index
				.map(|target_index| self.committed_index.get().saturating_sub(target_index))
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
	) -> Result<(), NavigateError> {
		if !self.is_current_generation(generation) {
			return Ok(());
		}
		if !matched.guards_allow() {
			return self.commit_unmatched(generation, path, intent);
		}
		store.promote_navigation_leases(generation);
		let entry_index = match intent {
			NavigationIntent::Push => self.committed_index.get().saturating_add(1),
			NavigationIntent::Replace | NavigationIntent::Initial => self.committed_index.get(),
			NavigationIntent::Pop { target_index } => {
				target_index.unwrap_or(self.committed_index.get())
			}
		};
		let previous_store = self.mounted_store.borrow_mut().replace(store.clone());
		let result = crate::router::loader::with_loader_store(&store, || {
			self.router
				.commit_match(&path, &matched, intent.navigation_type(), entry_index)
		});
		if let Err(error) = result {
			*self.mounted_store.borrow_mut() = previous_store;
			self.finish_error(
				generation,
				RouteLoaderError::with_status(error.to_string(), 500),
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
	) -> Result<(), NavigateError> {
		if !self.is_current_generation(generation) {
			return Ok(());
		}
		let entry_index = match intent {
			NavigationIntent::Push => self.committed_index.get().saturating_add(1),
			NavigationIntent::Replace | NavigationIntent::Initial => self.committed_index.get(),
			NavigationIntent::Pop { target_index } => {
				target_index.unwrap_or(self.committed_index.get())
			}
		};
		let previous_store = self.mounted_store.borrow_mut().replace(LoaderStore::new());
		if let Err(error) =
			self.router
				.commit_unmatched(&path, intent.navigation_type(), entry_index)
		{
			*self.mounted_store.borrow_mut() = previous_store;
			self.finish_error(
				generation,
				RouteLoaderError::with_status(error.to_string(), 500),
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

	#[cfg(native)]
	mod native_async_tests {
		use super::*;
		use crate::reactive::query::{
			QueryClient, QueryClientGuard, QueryDefaults, provide_query_client,
		};
		use crate::router::loader::with_loader_store;
		use crate::{Loader, Page, component, layout, loader};
		use reinhardt_core::page::{IntoPage, Outlet};
		use std::cell::{Cell, RefCell};
		use std::collections::VecDeque;
		use std::future::{Future, poll_fn};
		use std::pin::Pin;
		use std::rc::Rc;
		use std::task::{Context, Poll, Waker};

		thread_local! {
			static GATE_OPEN: Cell<bool> = const { Cell::new(false) };
			static GUARD_ALLOWS: Cell<bool> = const { Cell::new(true) };
			static SLOW_LOADER_STARTS: Cell<usize> = const { Cell::new(0) };
			static LAYOUT_LOADER_STARTS: Cell<usize> = const { Cell::new(0) };
			static LEAF_LOADER_STARTS: Cell<usize> = const { Cell::new(0) };
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
		)]
		fn coordinator_layout(Loader(value): Loader<String>, outlet: Outlet) -> Page {
			Page::fragment([Page::text(value), outlet.into_page()])
		}

		#[component(
			"child/",
			name = "coordinator-leaf",
			loader = coordinator_leaf_loader,
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
		}

		fn provide_test_query_client() -> QueryClientGuard {
			provide_query_client(QueryClient::new(QueryDefaults::default()))
		}

		fn router_with_loaded_routes() -> ClientRouter {
			ClientRouter::new()
				.route("root", "/", || Page::text("old route"))
				.component(coordinator_loaded)
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
