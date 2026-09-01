//! SSR Renderer for Component-based server-side rendering.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, VecDeque};
use std::fmt::Write as _;
use std::rc::Rc;
use std::time::Duration;

use super::control_binding::{SsrControlProjection, SsrSelectionState, project};
use super::markers::{HydrationMarker, HydrationStrategy};
use super::resource_context::{
	RenderOwnerRegistration, SsrResourceContext, enter_boundary, register_render_owner,
	resolve_boundary_resources, resolve_external_resources, resolve_pending_resources,
	scope_context,
};
use super::state::SsrState;
use super::stream::{SsrChunk, SsrStream};
use crate::auth::AuthData;
use crate::component::{
	Component, ControlKind, Head, IntoPage, Page, PageElement, scope_reactive_node_store,
	scope_reactive_node_transaction,
};
use crate::document_head::registry::ResolvedHeadEntry;
use crate::document_head::{
	DocumentHeadManager, DocumentHeadRegistration, report_document_head_error,
	with_document_head_manager,
};
use crate::reactive::hooks::id::{
	id_counter_snapshot, reset_id_counter, restore_id_counter, scope_id_counter,
	scope_id_counter_with,
};
use crate::reactive::query::{QueryClient, QueryDefaults, with_query_client_async};
use futures_util::StreamExt;
use futures_util::future::{FutureExt, LocalBoxFuture};
use futures_util::stream::{self, FuturesUnordered};
use reinhardt_core::reactive::ReactiveScope;
use reinhardt_core::types::page::{SuspenseNode, is_boolean_attr, is_boolean_attr_truthy};

/// Options for SSR rendering.
#[derive(Debug, Clone)]
pub struct SsrOptions {
	/// Whether to include hydration markers.
	pub include_hydration_markers: bool,
	/// Whether to minify the output.
	pub minify: bool,
	/// Whether to include SSR state script.
	pub include_state_script: bool,
	/// Language attribute for HTML element.
	pub lang: String,
	/// CSRF token to embed.
	pub csrf_token: Option<String>,
	/// Authentication data to embed.
	pub auth_data: Option<AuthData>,
	/// Enable partial hydration (Island Architecture, Phase 2-B).
	///
	/// When enabled, only components marked as islands are hydrated on the client.
	/// Static content is preserved without hydration, improving performance.
	pub enable_partial_hydration: bool,
	/// Default hydration strategy for components (Phase 2-B).
	///
	/// Determines how unmarked components should be hydrated.
	/// - `Full`: Traditional full hydration (default)
	/// - `Island`: Mark as interactive islands
	/// - `Static`: Mark as static content (no hydration)
	pub default_hydration_strategy: HydrationStrategy,
	/// Maximum time to wait for server resource resolution.
	///
	/// This single budget includes every query fetch attempt and retry delay.
	pub resource_timeout: Duration,
	/// Defaults used by the request-owned SSR query client.
	pub query_defaults: QueryDefaults,
	/// Enables streaming Suspense replacement chunks.
	pub suspense_streaming: bool,
	/// Optional nonce for inline streaming scripts.
	pub script_nonce: Option<String>,
}

impl Default for SsrOptions {
	fn default() -> Self {
		Self {
			include_hydration_markers: true,
			minify: false,
			include_state_script: true,
			lang: "en".to_string(),
			csrf_token: None,
			auth_data: None,
			enable_partial_hydration: false,
			default_hydration_strategy: HydrationStrategy::Full,
			resource_timeout: Duration::from_secs(2),
			query_defaults: QueryDefaults::default(),
			suspense_streaming: true,
			script_nonce: None,
		}
	}
}

impl SsrOptions {
	/// Creates new default options.
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets the language.
	pub fn lang(mut self, lang: impl Into<String>) -> Self {
		self.lang = lang.into();
		self
	}

	/// Sets the reactive i18n context for SSR and hydration.
	#[cfg(feature = "i18n")]
	pub fn i18n_context(mut self, context: crate::i18n::I18nContext) -> SsrRenderConfig {
		self.lang = context.locale_untracked();
		SsrRenderConfig {
			options: self,
			i18n_context: Some(context),
		}
	}

	/// Disables hydration markers.
	pub fn no_hydration(mut self) -> Self {
		self.include_hydration_markers = false;
		self
	}

	/// Enables minification.
	pub fn minify(mut self) -> Self {
		self.minify = true;
		self
	}

	/// Sets the CSRF token.
	pub fn csrf(mut self, token: impl Into<String>) -> Self {
		self.csrf_token = Some(token.into());
		self
	}

	/// Sets the authentication data.
	pub fn auth(mut self, auth_data: AuthData) -> Self {
		self.auth_data = Some(auth_data);
		self
	}

	/// Enables partial hydration (Island Architecture, Phase 2-B).
	///
	/// When enabled, only components marked as islands will be hydrated on the client.
	/// Static content is preserved without hydration, improving performance.
	///
	/// # Example
	///
	/// ```ignore
	/// let options = SsrOptions::new()
	///     .partial_hydration(true)
	///     .default_strategy(HydrationStrategy::Static);
	/// ```
	pub fn partial_hydration(mut self, enable: bool) -> Self {
		self.enable_partial_hydration = enable;
		self
	}

	/// Sets the default hydration strategy (Phase 2-B).
	///
	/// Determines how unmarked components should be hydrated:
	/// - `Full`: Traditional full hydration (default)
	/// - `Island`: Mark as interactive islands
	/// - `Static`: Mark as static content (no hydration)
	///
	/// # Example
	///
	/// ```ignore
	/// let options = SsrOptions::new()
	///     .default_strategy(HydrationStrategy::Island);
	/// ```
	pub fn default_strategy(mut self, strategy: HydrationStrategy) -> Self {
		self.default_hydration_strategy = strategy;
		self
	}

	/// Enables island-only rendering (convenience method, Phase 2-B).
	///
	/// Shortcut for enabling partial hydration with island strategy.
	/// Equivalent to:
	/// ```ignore
	/// options.partial_hydration(true).default_strategy(HydrationStrategy::Island)
	/// ```
	pub fn islands_only(mut self) -> Self {
		self.enable_partial_hydration = true;
		self.default_hydration_strategy = HydrationStrategy::Island;
		self
	}

	/// Sets the server resource timeout.
	///
	/// For query retries, this is one budget covering fetch attempts, backoff,
	/// and jitter rather than a timeout that restarts for each attempt.
	pub fn resource_timeout(mut self, timeout: Duration) -> Self {
		self.resource_timeout = timeout;
		self
	}

	/// Sets defaults for request-owned SSR queries.
	pub fn query_defaults(mut self, defaults: QueryDefaults) -> Self {
		let retry_enabled = self.query_defaults.ssr_query_retries_enabled();
		self.query_defaults = defaults.with_ssr_query_retries(retry_enabled);
		self
	}

	/// Enables or disables retries for request-owned SSR queries.
	///
	/// SSR retries require both this gate and a retry policy installed through
	/// [`QueryOptions::retry`](crate::reactive::QueryOptions::retry). Without
	/// either opt-in, an SSR query performs only its initial attempt.
	pub fn query_retries(mut self, enabled: bool) -> Self {
		self.query_defaults = self.query_defaults.with_ssr_query_retries(enabled);
		self
	}

	/// Enables or disables Suspense streaming.
	pub fn suspense_streaming(mut self, enabled: bool) -> Self {
		self.suspense_streaming = enabled;
		self
	}

	/// Sets the nonce used by inline Suspense replacement scripts.
	pub fn script_nonce(mut self, nonce: impl Into<String>) -> Self {
		self.script_nonce = Some(nonce.into());
		self
	}
}

/// Builder-owned SSR renderer configuration.
#[derive(Debug, Clone)]
pub struct SsrRenderConfig {
	options: SsrOptions,
	#[cfg(feature = "i18n")]
	i18n_context: Option<crate::i18n::I18nContext>,
}

/// Converts public renderer option builders into the renderer's internal config.
pub trait IntoSsrRendererConfig {
	/// Convert into a renderer configuration.
	fn into_ssr_renderer_config(self) -> SsrRenderConfig;
}

impl IntoSsrRendererConfig for SsrOptions {
	fn into_ssr_renderer_config(self) -> SsrRenderConfig {
		SsrRenderConfig {
			options: self,
			#[cfg(feature = "i18n")]
			i18n_context: None,
		}
	}
}

impl IntoSsrRendererConfig for SsrRenderConfig {
	fn into_ssr_renderer_config(self) -> SsrRenderConfig {
		self
	}
}

/// The main SSR renderer.
pub struct SsrRenderer {
	options: SsrOptions,
	#[cfg(feature = "i18n")]
	i18n_context: Option<crate::i18n::I18nContext>,
	state: SsrState,
	head_collector: SsrHeadCollector,
	hydration_marker_counter: u64,
	suspense_boundary_counter: u64,
	marker_resource_context: Rc<RefCell<SsrResourceContext>>,
	marker_id_counter: Rc<Cell<usize>>,
	active_reactive_scope: Rc<RefCell<Option<Rc<ReactiveScope>>>>,
	query_client: Option<QueryClient>,
}

struct SsrHeadCollector {
	manager: DocumentHeadManager,
	static_registrations: Vec<DocumentHeadRegistration>,
}

impl SsrHeadCollector {
	fn new() -> Self {
		Self {
			manager: DocumentHeadManager::new(Head::new()),
			static_registrations: Vec::new(),
		}
	}

	fn reset(&mut self) {
		*self = Self::new();
	}

	fn record_static(&mut self, head: &Head) {
		match self.manager.register_static_page(head.clone()) {
			Ok(registration) => self.static_registrations.push(registration),
			Err(error) => report_document_head_error(&error),
		}
	}

	fn checkpoint(&self) -> usize {
		self.static_registrations.len()
	}

	fn rollback(&mut self, checkpoint: usize) {
		assert!(checkpoint <= self.static_registrations.len());
		self.static_registrations.truncate(checkpoint);
	}

	fn resolved_entries(&self) -> Vec<ResolvedHeadEntry> {
		self.manager.resolved_entries()
	}
}

/// Restores the previous reactive scope when an SSR render entrypoint exits.
struct ActiveReactiveScopeGuard {
	active_scope: Rc<RefCell<Option<Rc<ReactiveScope>>>>,
	previous_scope: Option<Rc<ReactiveScope>>,
}

impl ActiveReactiveScopeGuard {
	fn install(
		active_scope: Rc<RefCell<Option<Rc<ReactiveScope>>>>,
		reactive_scope: Rc<ReactiveScope>,
	) -> Self {
		let previous_scope = active_scope.borrow_mut().replace(reactive_scope);
		Self {
			active_scope,
			previous_scope,
		}
	}
}

impl Drop for ActiveReactiveScopeGuard {
	fn drop(&mut self) {
		drop(self.active_scope.replace(self.previous_scope.take()));
	}
}

impl Clone for SsrRenderer {
	fn clone(&self) -> Self {
		Self {
			options: self.options.clone(),
			#[cfg(feature = "i18n")]
			i18n_context: self.i18n_context.clone(),
			state: self.state.clone(),
			head_collector: SsrHeadCollector::new(),
			hydration_marker_counter: self.hydration_marker_counter,
			suspense_boundary_counter: self.suspense_boundary_counter,
			marker_resource_context: Rc::new(RefCell::new(SsrResourceContext::new(
				self.options.resource_timeout,
			))),
			marker_id_counter: Rc::new(Cell::new(0)),
			active_reactive_scope: Rc::new(RefCell::new(
				self.active_reactive_scope.borrow().clone(),
			)),
			query_client: None,
		}
	}
}

#[derive(Clone, Copy)]
enum AsyncRenderMode {
	Discovery,
	Buffered,
}

struct PendingSuspenseBoundary {
	boundary_id: String,
	node: SuspenseNode,
	boundary_start: DeterministicRenderSnapshot,
	selection: Option<SsrSelectionState>,
}

#[derive(Clone, Copy)]
struct DeterministicRenderSnapshot {
	resource_call_order_index: Option<usize>,
	suspense_boundary_counter: u64,
	id_counter: usize,
}

type SuspenseBoundaryResult = (PendingSuspenseBoundary, bool);
type SuspenseBoundaryFuture = LocalBoxFuture<'static, Vec<SuspenseBoundaryResult>>;

struct SuspenseStreamRuntime {
	renderer: SsrRenderer,
	reactive_scope: Rc<ReactiveScope>,
	_render_owner: RenderOwnerRegistration,
	context: Rc<RefCell<SsrResourceContext>>,
	id_counter: Rc<Cell<usize>>,
	boundaries: FuturesUnordered<SuspenseBoundaryFuture>,
	ready_boundaries: VecDeque<SuspenseBoundaryResult>,
	query_client: QueryClient,
}

enum SuspenseStreamState {
	Shell {
		shell: String,
		runtime: SuspenseStreamRuntime,
	},
	Boundaries(SuspenseStreamRuntime),
	Done,
}

fn suspense_boundary_futures(
	context: &Rc<RefCell<SsrResourceContext>>,
	boundaries: Vec<PendingSuspenseBoundary>,
	#[cfg(feature = "i18n")] i18n_context: Option<crate::i18n::I18nContext>,
) -> FuturesUnordered<SuspenseBoundaryFuture> {
	suspense_boundary_groups(context, boundaries)
		.into_iter()
		.map(|boundaries| {
			let context = Rc::clone(context);
			#[cfg(feature = "i18n")]
			let i18n_context = i18n_context.clone();
			let future = async move {
				let mut results = Vec::new();

				for boundary in boundaries {
					let boundary_resolved =
						resolve_boundary_resources(&context, &boundary.boundary_id).await;
					results.push((boundary, boundary_resolved));
				}

				results
			};
			#[cfg(feature = "i18n")]
			let future = with_i18n_context_future(i18n_context, future);
			future.boxed_local()
		})
		.collect()
}

fn suspense_boundary_groups(
	context: &Rc<RefCell<SsrResourceContext>>,
	boundaries: Vec<PendingSuspenseBoundary>,
) -> Vec<Vec<PendingSuspenseBoundary>> {
	let pending_ids_by_boundary = {
		let context = context.borrow();
		boundaries
			.iter()
			.map(|boundary| {
				context
					.pending_ids_for_boundary(&boundary.boundary_id)
					.into_iter()
					.collect::<BTreeSet<_>>()
			})
			.collect::<Vec<_>>()
	};

	let mut groups: Vec<(BTreeSet<String>, Vec<PendingSuspenseBoundary>)> = Vec::new();
	for (boundary, pending_ids) in boundaries.into_iter().zip(pending_ids_by_boundary) {
		let mut group_pending_ids = pending_ids;
		let mut group_boundaries = vec![boundary];
		let mut index = 0;

		while index < groups.len() {
			if !group_pending_ids.is_empty() && !group_pending_ids.is_disjoint(&groups[index].0) {
				let (pending_ids, mut boundaries) = groups.remove(index);
				group_pending_ids.extend(pending_ids);
				group_boundaries.append(&mut boundaries);
			} else {
				index += 1;
			}
		}

		groups.push((group_pending_ids, group_boundaries));
	}

	groups
		.into_iter()
		.map(|(_, boundaries)| boundaries)
		.collect()
}

#[cfg(feature = "i18n")]
async fn with_i18n_context_future<R, Fut>(
	context: Option<crate::i18n::I18nContext>,
	future: Fut,
) -> R
where
	Fut: std::future::Future<Output = R>,
{
	let mut future = std::pin::pin!(future);
	std::future::poll_fn(move |cx| {
		if let Some(context) = context.as_ref() {
			let _guard = crate::i18n::provide_i18n_context(context.clone());
			future.as_mut().poll(cx)
		} else {
			future.as_mut().poll(cx)
		}
	})
	.await
}

impl Default for SsrRenderer {
	fn default() -> Self {
		Self::new()
	}
}

impl SsrRenderer {
	/// Creates a new renderer with default options.
	pub fn new() -> Self {
		Self {
			options: SsrOptions::default(),
			#[cfg(feature = "i18n")]
			i18n_context: None,
			state: SsrState::new(),
			head_collector: SsrHeadCollector::new(),
			hydration_marker_counter: 0,
			suspense_boundary_counter: 0,
			marker_resource_context: Rc::new(RefCell::new(SsrResourceContext::new(
				SsrOptions::default().resource_timeout,
			))),
			marker_id_counter: Rc::new(Cell::new(0)),
			active_reactive_scope: Rc::new(RefCell::new(None)),
			query_client: None,
		}
	}

	/// Creates a renderer with custom options.
	pub fn with_options(options: impl IntoSsrRendererConfig) -> Self {
		let config = options.into_ssr_renderer_config();
		let resource_timeout = config.options.resource_timeout;
		Self {
			options: config.options,
			#[cfg(feature = "i18n")]
			i18n_context: config.i18n_context,
			state: SsrState::new(),
			head_collector: SsrHeadCollector::new(),
			hydration_marker_counter: 0,
			suspense_boundary_counter: 0,
			marker_resource_context: Rc::new(RefCell::new(SsrResourceContext::new(
				resource_timeout,
			))),
			marker_id_counter: Rc::new(Cell::new(0)),
			active_reactive_scope: Rc::new(RefCell::new(None)),
			query_client: None,
		}
	}

	/// Returns a reference to the SSR state.
	///
	/// For streamed pages, resources resolved by later Suspense replacement
	/// chunks are serialized into the emitted stream and may not appear in this
	/// renderer snapshot after the stream has been returned.
	pub fn state(&self) -> &SsrState {
		&self.state
	}

	/// Returns a mutable reference to the SSR state.
	pub fn state_mut(&mut self) -> &mut SsrState {
		&mut self.state
	}

	/// Returns the configured timeout used by entry-blocking route loaders.
	pub(crate) fn route_loader_timeout(&self) -> Duration {
		self.options.resource_timeout
	}

	/// Clears resource state before a route render installs its loader payload.
	pub(crate) fn begin_route_loader_render(&mut self) {
		self.begin_render(true);
	}

	pub(crate) fn request_query_client(&self) -> QueryClient {
		self.query_client
			.clone()
			.expect("SSR request query client must be initialized")
	}

	/// Renders a component to an HTML string.
	pub async fn render<C: Component>(&mut self, component: &C) -> String {
		self.render_view_factory(|| component.render()).await
	}

	/// Renders an IntoPage to an HTML string.
	pub async fn render_into_page<V: IntoPage>(&mut self, view: V) -> String {
		let view = view.into_page();
		self.render_view(&view).await
	}

	/// Renders a View to an HTML string.
	pub async fn render_view(&mut self, view: &Page) -> String {
		self.render_view_factory(|| view.clone()).await
	}

	/// Renders a View and syncs renderer-owned SSR state.
	pub async fn render_view_with_state(&mut self, view: &Page) -> String {
		self.render_view(view).await
	}

	fn html_lang(&self) -> String {
		#[cfg(feature = "i18n")]
		if let Some(context) = self.i18n_context.as_ref() {
			return context.locale();
		}
		self.options.lang.clone()
	}

	fn state_script_tag(&self) -> Option<String> {
		if !self.options.include_state_script {
			return None;
		}

		#[cfg(feature = "i18n")]
		let mut state = self.state.clone();
		#[cfg(not(feature = "i18n"))]
		let state = self.state.clone();
		#[cfg(feature = "i18n")]
		if let Some(context) = self.i18n_context.as_ref() {
			crate::i18n::write_i18n_ssr_state(&mut state, context);
		}

		if state.is_empty() {
			None
		} else {
			Some(state.to_script_tag())
		}
	}

	fn sync_i18n_state(&mut self) {
		#[cfg(feature = "i18n")]
		if let Some(context) = self.i18n_context.as_ref() {
			crate::i18n::write_i18n_ssr_state(&mut self.state, context);
		}
	}

	fn next_hydration_marker_id(&mut self) -> String {
		let id = self.hydration_marker_counter;
		self.hydration_marker_counter += 1;
		format!("rh-{}", id)
	}

	fn next_suspense_boundary_id(&mut self) -> String {
		let id = self.suspense_boundary_counter;
		self.suspense_boundary_counter += 1;
		format!("rh-suspense-{id}")
	}

	fn suspense_boundary_id(&mut self, node: &SuspenseNode) -> String {
		node.boundary_id()
			.map(normalize_suspense_boundary_id)
			.unwrap_or_else(|| self.next_suspense_boundary_id())
	}

	fn reset_deterministic_render_counters(&mut self) {
		self.suspense_boundary_counter = 0;
		reset_id_counter();
	}

	fn deterministic_render_snapshot(&self) -> DeterministicRenderSnapshot {
		DeterministicRenderSnapshot {
			resource_call_order_index: super::resource_context::with_active_context(|context| {
				context.borrow().call_order_index()
			}),
			suspense_boundary_counter: self.suspense_boundary_counter,
			id_counter: id_counter_snapshot(),
		}
	}

	fn restore_deterministic_render_snapshot(&mut self, snapshot: DeterministicRenderSnapshot) {
		if let Some(index) = snapshot.resource_call_order_index
			&& let Some(context) = super::resource_context::with_active_context(Rc::clone)
		{
			context.borrow_mut().set_call_order_index(index);
		}
		self.suspense_boundary_counter = snapshot.suspense_boundary_counter;
		restore_id_counter(snapshot.id_counter);
	}

	fn begin_render(&mut self, clear_resource_states: bool) {
		if clear_resource_states {
			self.query_client = Some(QueryClient::new_ssr(self.options.query_defaults.clone()));
			self.state.clear_resource_states();
			self.marker_resource_context = Rc::new(RefCell::new(SsrResourceContext::new(
				self.options.resource_timeout,
			)));
			self.marker_id_counter = Rc::new(Cell::new(0));
			self.reset_deterministic_render_counters();
		}
		self.head_collector.reset();
	}

	fn should_resolve_resources(&self) -> bool {
		self.options.include_state_script
	}

	fn with_active_reactive_scope<R>(&self, f: impl FnOnce() -> R) -> R {
		let scope = self.active_reactive_scope.borrow().clone();
		let manager = self.head_collector.manager.clone();
		with_document_head_manager(&manager, || {
			if let Some(scope) = scope {
				scope.enter(f)
			} else {
				f()
			}
		})
	}

	fn begin_buffered_render_pass(&mut self) {
		self.head_collector.reset();
	}

	fn record_rendered_head(&mut self, head: &Head) {
		self.head_collector.record_static(head);
	}

	fn head_checkpoint(&self) -> usize {
		self.head_collector.checkpoint()
	}

	fn rollback_head_collector(&mut self, checkpoint: usize) {
		self.head_collector.rollback(checkpoint);
	}

	fn snapshot_rendered_head_entries(&self) -> Vec<ResolvedHeadEntry> {
		self.head_collector.resolved_entries()
	}

	/// Renders a component to a full HTML page.
	pub async fn render_page<C: Component>(&mut self, component: &C) -> SsrStream {
		if self.options.suspense_streaming && !self.options.minify {
			return self
				.render_page_stream_from_factory(|| component.render())
				.await;
		}

		let (_, content, body_tail, head_entries) = self
			.render_view_parts_from_factory(|| component.render(), true)
			.await;
		SsrStream::from_chunks(self.wrap_in_html_with_head_and_body_tail_chunks(
			&content,
			&body_tail,
			&head_entries,
		))
	}

	/// Renders an IntoPage to a full HTML page.
	pub async fn render_page_into_page<V: IntoPage>(&mut self, view: V) -> SsrStream {
		let view = view.into_page();
		if self.options.suspense_streaming && !self.options.minify {
			return self.render_page_stream_from_factory(|| view.clone()).await;
		}

		let (_, content, body_tail, head_entries) = self
			.render_view_parts_from_factory(|| view.clone(), true)
			.await;
		SsrStream::from_chunks(self.wrap_in_html_with_head_and_body_tail_chunks(
			&content,
			&body_tail,
			&head_entries,
		))
	}

	/// Renders a View to a full HTML page, using the View's attached head if present.
	///
	/// This method collects active static and retained-hook declarations in
	/// structural render order and serializes their effective HTML `<head>`.
	///
	/// # Arguments
	///
	/// * `view` - The View to render, potentially with an attached Head
	///
	/// # Example
	///
	/// ```ignore
	/// use reinhardt_pages::{head, page, View, SsrRenderer};
	///
	/// let my_head = head!(|| {
	///     title { "My Page" }
	///     meta { name: "description", content: "A page" }
	/// });
	///
	/// let view = page!(|| { div { "Hello" } })().with_head(my_head);
	///
	/// let mut renderer = SsrRenderer::new();
	/// let html = renderer.render_page_with_view_head_to_string(view).await;
	/// // html contains <title>My Page</title> in the head
	/// ```
	pub async fn render_page_with_view_head(&mut self, view: Page) -> SsrStream {
		if self.options.suspense_streaming && !self.options.minify {
			return self.render_page_stream_from_factory(|| view.clone()).await;
		}

		let (_, content, body_tail, head_entries) = self
			.render_view_parts_from_factory(|| view.clone(), true)
			.await;
		SsrStream::from_chunks(self.wrap_in_html_with_head_and_body_tail_chunks(
			&content,
			&body_tail,
			&head_entries,
		))
	}

	/// Renders a component to a buffered full HTML page.
	pub async fn render_page_to_string<C: Component>(&mut self, component: &C) -> String {
		let (_, content, body_tail, head_entries) = self
			.render_view_parts_from_factory(|| component.render(), true)
			.await;
		self.wrap_in_html_with_head_and_body_tail(&content, &body_tail, &head_entries)
	}

	/// Renders an IntoPage to a buffered full HTML page.
	pub async fn render_page_into_page_to_string<V: IntoPage>(&mut self, view: V) -> String {
		let view = view.into_page();
		let (_, content, body_tail, head_entries) = self
			.render_view_parts_from_factory(|| view.clone(), true)
			.await;
		self.wrap_in_html_with_head_and_body_tail(&content, &body_tail, &head_entries)
	}

	/// Renders a full page while retaining already prepared resource state.
	///
	/// Route loaders install their serialized values before page wrapping so the
	/// generated state script can hydrate the initial route without another
	/// client fetch.
	pub(crate) async fn render_page_into_page_to_string_preserving_resource_state<V: IntoPage>(
		&mut self,
		view: V,
	) -> String {
		let view = view.into_page();
		let (_, content, body_tail, head_entries) = self
			.render_view_parts_from_factory(|| view.clone(), false)
			.await;
		self.wrap_in_html_with_head_and_body_tail(&content, &body_tail, &head_entries)
	}

	/// Renders a View to a buffered full HTML page, using attached head data.
	pub async fn render_page_with_view_head_to_string(&mut self, view: Page) -> String {
		let (_, content, body_tail, head_entries) = self
			.render_view_parts_from_factory(|| view.clone(), true)
			.await;
		self.wrap_in_html_with_head_and_body_tail(&content, &body_tail, &head_entries)
	}

	async fn render_page_stream_from_factory<F>(&mut self, mut view_factory: F) -> SsrStream
	where
		F: FnMut() -> Page,
	{
		self.begin_render(true);
		let query_client = self.request_query_client();
		if !self.should_resolve_resources() {
			let reactive_scope = Rc::new(ReactiveScope::new());
			let _render_owner = register_render_owner(&reactive_scope);
			let _active_scope_guard = ActiveReactiveScopeGuard::install(
				Rc::clone(&self.active_reactive_scope),
				Rc::clone(&reactive_scope),
			);
			let context = Rc::new(RefCell::new(SsrResourceContext::new(
				self.options.resource_timeout,
			)));
			#[cfg(feature = "i18n")]
			let i18n_context = self.i18n_context.clone();
			let render = scope_context(Rc::clone(&context), async move {
				let render_start = self.deterministic_render_snapshot();
				let (_, content, head_entries) = scope_reactive_node_store(async {
					self.restore_deterministic_render_snapshot(render_start);
					self.begin_buffered_render_pass();
					let view = self.with_active_reactive_scope(&mut view_factory);
					let mut boundaries = Vec::new();
					let content = self.render_stream_shell_page(&view, &mut boundaries).await;
					let head_entries = self.snapshot_rendered_head_entries();
					(view, content, head_entries)
				})
				.await;
				self.sync_i18n_state();
				SsrStream::from_chunks(self.wrap_in_html_with_head_and_body_tail_chunks(
					&content,
					"",
					&head_entries,
				))
			});
			let render = with_query_client_async(query_client, render);
			#[cfg(feature = "i18n")]
			let render = with_i18n_context_future(i18n_context, render);
			return scope_id_counter(render).await;
		}

		let reactive_scope = Rc::new(ReactiveScope::new());
		let render_owner = register_render_owner(&reactive_scope);
		let _active_scope_guard = ActiveReactiveScopeGuard::install(
			Rc::clone(&self.active_reactive_scope),
			Rc::clone(&reactive_scope),
		);
		let context = Rc::new(RefCell::new(SsrResourceContext::new(
			self.options.resource_timeout,
		)));
		let id_counter = Rc::new(Cell::new(0));
		let scoped_id_counter = Rc::clone(&id_counter);
		#[cfg(feature = "i18n")]
		let i18n_context = self.i18n_context.clone();
		let stream_query_client = query_client.clone();
		let state_query_client = query_client.clone();
		let render = scope_context(Rc::clone(&context), async move {
			let render_start = self.deterministic_render_snapshot();
			let discovery_scope = Rc::clone(&reactive_scope);
			scope_reactive_node_store(async {
				let discovery_view = self.with_active_reactive_scope(&mut view_factory);
				let _ = self
					.render_async_page(&discovery_view, AsyncRenderMode::Discovery)
					.await;
			})
			.await;

			resolve_external_resources(&context).await;
			if !state_query_client.has_normalized_queries() {
				self.restore_deterministic_render_snapshot(render_start);
				scope_reactive_node_store(async {
					let discovery_view = self.with_active_reactive_scope(&mut view_factory);
					let _ = self
						.render_async_page(&discovery_view, AsyncRenderMode::Discovery)
						.await;
				})
				.await;
			}
			drop(discovery_scope);
			if state_query_client.has_normalized_queries()
				|| state_query_client.has_ssr_entity_reads()
			{
				resolve_pending_resources(&context).await;
				state_query_client.settle_inline_tasks().await;
				loop {
					state_query_client.reset_ssr_entity_reads();
					self.restore_deterministic_render_snapshot(render_start);
					self.begin_buffered_render_pass();
					let (view, content, mut has_pending, head_entries) =
						scope_reactive_node_store(async {
							let view = self.with_active_reactive_scope(&mut view_factory);
							let content = self
								.render_async_page(&view, AsyncRenderMode::Buffered)
								.await;
							let has_pending = context.borrow().has_pending();
							let head_entries = self.snapshot_rendered_head_entries();
							(view, content, has_pending, head_entries)
						})
						.await;
					has_pending |= state_query_client.settle_inline_tasks().await;

					if !has_pending {
						self.add_resolved_resources_to_state(&context);
						self.add_entity_hydration_table(&state_query_client);
						self.sync_i18n_state();
						let chunks = self.wrap_in_html_with_head_and_body_tail_chunks(
							&content,
							"",
							&head_entries,
						);
						return SsrStream::from_chunks(chunks);
					}

					drop(view);
					resolve_pending_resources(&context).await;
					state_query_client.settle_inline_tasks().await;
				}
			}
			let (_, content, boundaries, head_entries) = loop {
				self.restore_deterministic_render_snapshot(render_start);
				self.begin_buffered_render_pass();

				let (view, content, boundaries, has_pending_external, head_entries) =
					scope_reactive_node_store(async {
						let view = self.with_active_reactive_scope(&mut view_factory);
						let mut boundaries = Vec::new();
						let content = self.render_stream_shell_page(&view, &mut boundaries).await;
						let has_pending_external = context.borrow().has_pending_external();
						let head_entries = self.snapshot_rendered_head_entries();
						(
							view,
							content,
							boundaries,
							has_pending_external,
							head_entries,
						)
					})
					.await;

				if !has_pending_external {
					break (view, content, boundaries, head_entries);
				}

				drop(view);
				resolve_external_resources(&context).await;
			};
			self.add_resolved_resources_to_state(&context);
			self.add_entity_hydration_table(&state_query_client);
			self.sync_i18n_state();

			let shell = self.wrap_in_html_shell(&content, &head_entries);
			let boundary_futures = suspense_boundary_futures(
				&context,
				boundaries,
				#[cfg(feature = "i18n")]
				self.i18n_context.clone(),
			);

			let runtime = SuspenseStreamRuntime {
				renderer: self.clone(),
				reactive_scope,
				_render_owner: render_owner,
				context,
				id_counter,
				boundaries: boundary_futures,
				ready_boundaries: VecDeque::new(),
				query_client: stream_query_client,
			};

			SsrStream::from_stream(stream::unfold(
				SuspenseStreamState::Shell { shell, runtime },
				|state| async move {
					let query_client = match &state {
						SuspenseStreamState::Shell { runtime, .. }
						| SuspenseStreamState::Boundaries(runtime) => Some(runtime.query_client.clone()),
						SuspenseStreamState::Done => None,
					};
					let next = async move {
						match state {
							SuspenseStreamState::Shell { shell, runtime } => Some((
								SsrChunk::Html(shell),
								SuspenseStreamState::Boundaries(runtime),
							)),
							SuspenseStreamState::Boundaries(mut runtime) => {
								loop {
									let Some((boundary, resolved)) =
										runtime.ready_boundaries.pop_front()
									else {
										let Some(resolved_boundaries) =
											runtime.boundaries.next().await
										else {
											break;
										};
										runtime.ready_boundaries.extend(resolved_boundaries);
										continue;
									};

									if !resolved {
										continue;
									}

									#[cfg(feature = "i18n")]
									let i18n_context = runtime.renderer.i18n_context.clone();
									let replacement = scope_id_counter_with(
										Rc::clone(&runtime.id_counter),
										scope_context(Rc::clone(&runtime.context), async {
											if runtime
												.reactive_scope
												.enter(|| boundary.node.is_pending())
											{
												return None;
											}
											let (replacement, nested_boundaries) = loop {
												let (
													replacement,
													nested_boundaries,
													has_pending_boundary_resource,
													has_pending_external_resource,
												) = scope_reactive_node_store(async {
													runtime
														.renderer
														.restore_deterministic_render_snapshot(
															boundary.boundary_start,
														);
													let boundary_guard = enter_boundary(
														&runtime.context,
														boundary.boundary_id.clone(),
													);
													let replacement_page = runtime
														.reactive_scope
														.enter(|| boundary.node.render_content());
													let mut nested_boundaries = Vec::new();
													let replacement = runtime
														.renderer
														.render_stream_shell_page_with_selection(
															&replacement_page,
															&mut nested_boundaries,
															boundary.selection.clone(),
														)
														.await;
													drop(boundary_guard);

													let has_pending_boundary_resource = runtime
														.context
														.borrow()
														.has_pending_for_boundary(
															&boundary.boundary_id,
														);
													let has_pending_external_resource = runtime
														.context
														.borrow()
														.has_pending_external();
													(
														replacement,
														nested_boundaries,
														has_pending_boundary_resource,
														has_pending_external_resource,
													)
												})
												.await;
												if !has_pending_boundary_resource
													&& !has_pending_external_resource
												{
													break (replacement, nested_boundaries);
												}

												if has_pending_boundary_resource {
													resolve_boundary_resources(
														&runtime.context,
														&boundary.boundary_id,
													)
													.await;
												}
												if has_pending_external_resource {
													resolve_external_resources(&runtime.context)
														.await;
												}
											};
											runtime
												.renderer
												.add_resolved_resources_to_state(&runtime.context);
											runtime
												.renderer
												.add_entity_hydration_table(&runtime.query_client);
											runtime.renderer.sync_i18n_state();
											for future in suspense_boundary_futures(
												&runtime.context,
												nested_boundaries,
												#[cfg(feature = "i18n")]
												runtime.renderer.i18n_context.clone(),
											) {
												runtime.boundaries.push(future);
											}
											Some(replacement)
										}),
									);
									#[cfg(feature = "i18n")]
									let replacement = with_i18n_context_future(i18n_context, replacement);
									let replacement = replacement.await;

									let Some(replacement) = replacement else {
										continue;
									};

									let chunk = runtime.renderer.render_suspense_replacement(
										&boundary.boundary_id,
										replacement,
									);
									return Some((
										SsrChunk::Html(chunk),
										SuspenseStreamState::Boundaries(runtime),
									));
								}

								runtime
									.renderer
									.add_resolved_resources_to_state(&runtime.context);
								runtime
									.renderer
									.add_entity_hydration_table(&runtime.query_client);
								runtime.renderer.sync_i18n_state();
								Some((
									SsrChunk::Html(runtime.renderer.wrap_in_html_suffix()),
									SuspenseStreamState::Done,
								))
							}
							SuspenseStreamState::Done => None,
						}
					};
					if let Some(query_client) = query_client {
						with_query_client_async(query_client, next).await
					} else {
						next.await
					}
				},
			))
		});
		let render = with_query_client_async(query_client, render);
		#[cfg(feature = "i18n")]
		let render = with_i18n_context_future(i18n_context, render);
		scope_id_counter_with(scoped_id_counter, render).await
	}

	async fn render_view_factory<F>(&mut self, view_factory: F) -> String
	where
		F: FnMut() -> Page,
	{
		let (_, content, body_tail, _) = self
			.render_view_parts_from_factory(view_factory, true)
			.await;
		format!("{content}{body_tail}")
	}

	async fn render_view_factory_preserving_resource_state<F>(&mut self, view_factory: F) -> String
	where
		F: FnMut() -> Page,
	{
		let (_, content, body_tail, _) = self
			.render_view_parts_from_factory(view_factory, false)
			.await;
		format!("{content}{body_tail}")
	}

	async fn render_view_parts_from_factory<F>(
		&mut self,
		mut view_factory: F,
		clear_resource_states: bool,
	) -> (Page, String, String, Vec<ResolvedHeadEntry>)
	where
		F: FnMut() -> Page,
	{
		self.begin_render(clear_resource_states);
		if self.query_client.is_none() {
			self.query_client = Some(QueryClient::new_ssr(self.options.query_defaults.clone()));
		}
		let query_client = self.request_query_client();
		let state_query_client = query_client.clone();
		let reactive_scope = Rc::new(ReactiveScope::new());
		let _render_owner = register_render_owner(&reactive_scope);
		let _active_scope_guard = ActiveReactiveScopeGuard::install(
			Rc::clone(&self.active_reactive_scope),
			Rc::clone(&reactive_scope),
		);
		let context = if clear_resource_states {
			Rc::new(RefCell::new(SsrResourceContext::new(
				self.options.resource_timeout,
			)))
		} else {
			Rc::clone(&self.marker_resource_context)
		};
		let marker_id_counter =
			(!clear_resource_states).then(|| Rc::clone(&self.marker_id_counter));
		#[cfg(feature = "i18n")]
		let i18n_context = self.i18n_context.clone();
		let render = scope_context(Rc::clone(&context), async move {
			let render_start = self.deterministic_render_snapshot();
			if !self.should_resolve_resources() {
				let (view, content, head_entries) = scope_reactive_node_store(async {
					self.restore_deterministic_render_snapshot(render_start);
					self.begin_buffered_render_pass();
					let view = self.with_active_reactive_scope(&mut view_factory);
					let content = self
						.render_async_page(&view, AsyncRenderMode::Buffered)
						.await;
					let head_entries = self.snapshot_rendered_head_entries();
					(view, content, head_entries)
				})
				.await;
				self.sync_i18n_state();
				return (view, content, String::new(), head_entries);
			}

			let discovery_scope = Rc::clone(&reactive_scope);
			scope_reactive_node_store(async {
				let discovery_view = self.with_active_reactive_scope(&mut view_factory);
				let _ = self
					.render_async_page(&discovery_view, AsyncRenderMode::Discovery)
					.await;
			})
			.await;

			resolve_external_resources(&context).await;
			resolve_pending_resources(&context).await;
			drop(discovery_scope);

			loop {
				self.restore_deterministic_render_snapshot(render_start);
				self.begin_buffered_render_pass();

				let (view, content, has_pending, head_entries) = scope_reactive_node_store(async {
					let view = self.with_active_reactive_scope(&mut view_factory);
					let content = self
						.render_async_page(&view, AsyncRenderMode::Buffered)
						.await;
					let has_pending = context.borrow().has_pending();
					let head_entries = self.snapshot_rendered_head_entries();
					(view, content, has_pending, head_entries)
				})
				.await;

				if !has_pending {
					self.add_resolved_resources_to_state(&context);
					self.add_entity_hydration_table(&state_query_client);
					self.sync_i18n_state();
					return (view, content, String::new(), head_entries);
				}

				drop(view);
				resolve_pending_resources(&context).await;
			}
		});
		let render = with_query_client_async(query_client, render);
		#[cfg(feature = "i18n")]
		let render = with_i18n_context_future(i18n_context, render);
		if let Some(marker_id_counter) = marker_id_counter {
			scope_id_counter_with(marker_id_counter, render).await
		} else {
			scope_id_counter(render).await
		}
	}

	fn add_resolved_resources_to_state(&mut self, context: &Rc<RefCell<SsrResourceContext>>) {
		for (id, value) in context.borrow().resolved_resources() {
			self.state.add_resource_state(id, value.clone());
		}
	}

	fn add_entity_hydration_table(&mut self, query_client: &QueryClient) {
		for (id, refresh) in query_client.normalized_recipe_refreshes() {
			match refresh {
				crate::reactive::query::NormalizedRecipeRefresh::Success(recipe) => {
					if let Some(mut snapshot) = self.state.take_resource_state(&id) {
						if let Some(stored_recipe) =
							snapshot.pointer_mut("/state/Success/projection")
						{
							*stored_recipe = recipe;
						}
						self.state.add_resource_state(id, snapshot);
					}
				}
				crate::reactive::query::NormalizedRecipeRefresh::Preserve => {}
				crate::reactive::query::NormalizedRecipeRefresh::Remove => {
					self.state.take_resource_state(&id);
				}
			}
		}
		let envelope = query_client.reachable_entity_hydration_envelope();
		self.state
			.take_resource_state(crate::reactive::entity::ENTITY_TABLE_HYDRATION_ID);
		if !envelope.entities.is_empty() {
			self.state.add_resource_state(
				crate::reactive::entity::ENTITY_TABLE_HYDRATION_ID,
				serde_json::to_value(envelope)
					.expect("normalized entity hydration table must serialize"),
			);
		}
	}

	fn render_stream_shell_page<'a>(
		&'a mut self,
		view: &'a Page,
		boundaries: &'a mut Vec<PendingSuspenseBoundary>,
	) -> LocalBoxFuture<'a, String> {
		self.render_stream_shell_page_with_selection(view, boundaries, None)
	}

	fn render_stream_shell_page_with_selection<'a>(
		&'a mut self,
		view: &'a Page,
		boundaries: &'a mut Vec<PendingSuspenseBoundary>,
		selection: Option<SsrSelectionState>,
	) -> LocalBoxFuture<'a, String> {
		Box::pin(async move {
			match view {
				Page::Element(el) => {
					let projection = project(el.bound_control());
					let mut html = render_element_opening(el, &projection, selection.as_ref());

					if el.is_void() {
						html.push_str(" />");
					} else {
						html.push('>');
						if el.tag_name().eq_ignore_ascii_case("textarea")
							&& let Some(text) = projection.textarea_text.as_deref()
						{
							html.push_str(&html_escape(text));
						} else {
							let child_selection = if el.tag_name().eq_ignore_ascii_case("select") {
								el.bound_control().map(|binding| {
									SsrSelectionState::new(
										binding,
										projection.selected_values.clone(),
									)
								})
							} else {
								selection
							};
							for child in el.child_views() {
								html.push_str(
									&self
										.render_stream_shell_page_with_selection(
											child,
											boundaries,
											child_selection.clone(),
										)
										.await,
								);
							}
						}
						html.push_str("</");
						html.push_str(el.tag_name());
						html.push('>');
					}

					html
				}
				Page::Text(text) => html_escape(text),
				Page::Fragment(children) => {
					let mut html = String::new();
					for child in children {
						html.push_str(
							&self
								.render_stream_shell_page_with_selection(
									child,
									boundaries,
									selection.clone(),
								)
								.await,
						);
					}
					html
				}
				Page::KeyedFragment(children) => {
					let mut html = String::new();
					for (_, child) in children {
						html.push_str(
							&self
								.render_stream_shell_page_with_selection(
									child,
									boundaries,
									selection.clone(),
								)
								.await,
						);
					}
					html
				}
				Page::Empty => String::new(),
				Page::WithHead { view, head } => {
					self.record_rendered_head(head);
					self.render_stream_shell_page_with_selection(view, boundaries, selection)
						.await
				}
				#[cfg(feature = "hmr")]
				Page::DevTemplate { view, .. } | Page::DevSlot { view, .. } => {
					self.render_stream_shell_page_with_selection(view, boundaries, selection)
						.await
				}
				Page::ReactiveIf(reactive_if) => {
					let branch = self.with_active_reactive_scope(|| {
						if reactive_if.condition() {
							reactive_if.then_view()
						} else {
							reactive_if.else_view()
						}
					});
					self.render_stream_shell_page_with_selection(&branch, boundaries, selection)
						.await
				}
				Page::Reactive(reactive) => {
					let rendered = self.with_active_reactive_scope(|| reactive.render());
					self.render_stream_shell_page_with_selection(&rendered, boundaries, selection)
						.await
				}
				Page::Suspense(node) => {
					let boundary_id = self.suspense_boundary_id(node);
					let boundary_selection = selection.as_ref().map(SsrSelectionState::fork);
					let content_selection =
						boundary_selection.as_ref().map(SsrSelectionState::fork);
					let head_checkpoint = self.head_checkpoint();

					if let Some(context) = super::resource_context::with_active_context(Rc::clone) {
						context.borrow_mut().assign_resources_to_boundary(
							node.tracked_resource_ids(),
							&boundary_id,
						);
					}

					let boundary_start = self.deterministic_render_snapshot();
					let nested_boundary_start = boundaries.len();
					let boundary_guard = super::resource_context::with_active_context(|context| {
						enter_boundary(context, boundary_id.clone())
					});
					let (content, mut content_reactive_nodes) =
						scope_reactive_node_transaction(async {
							let content_page =
								self.with_active_reactive_scope(|| node.render_content());
							self.render_stream_shell_page_with_selection(
								&content_page,
								boundaries,
								content_selection.clone(),
							)
							.await
						})
						.await;

					drop(boundary_guard);

					let has_pending = super::resource_context::with_active_context(|context| {
						context.borrow().has_pending_for_boundary(&boundary_id)
					})
					.unwrap_or(false);
					let mut inline_single_select_uses_fallback = false;

					if has_pending
						&& self.should_resolve_resources()
						&& selection
							.as_ref()
							.is_some_and(SsrSelectionState::selects_one)
						&& let Some(context) =
							super::resource_context::with_active_context(Rc::clone)
					{
						// A single select must know whether this earlier boundary contains the
						// first matching option before later options are emitted in the shell.
						// Buffer only this boundary; unrelated and multiple-select boundaries
						// retain the normal streaming path.
						let boundary_resolved =
							resolve_boundary_resources(&context, &boundary_id).await;
						self.restore_deterministic_render_snapshot(boundary_start);
						boundaries.truncate(nested_boundary_start);
						if boundary_resolved
							&& !self.with_active_reactive_scope(|| node.is_pending())
						{
							drop(content_reactive_nodes);
							self.rollback_head_collector(head_checkpoint);
							let boundary_guard =
								super::resource_context::with_active_context(|context| {
									enter_boundary(context, boundary_id.clone())
								});
							let resolved_page =
								self.with_active_reactive_scope(|| node.render_content());
							let resolved_selection =
								boundary_selection.as_ref().map(SsrSelectionState::fork);
							let resolved = self
								.render_stream_shell_page_with_selection(
									&resolved_page,
									boundaries,
									resolved_selection.clone(),
								)
								.await;
							drop(boundary_guard);
							if let (Some(parent), Some(rendered)) =
								(selection.as_ref(), resolved_selection.as_ref())
							{
								parent.commit_from(rendered);
							}
							return resolved;
						}
						inline_single_select_uses_fallback = true;
					}

					if has_pending {
						drop(content_reactive_nodes);
						self.rollback_head_collector(head_checkpoint);
						if self.should_resolve_resources()
							&& !inline_single_select_uses_fallback
							&& let Some(parent) = selection.as_ref()
						{
							parent.reserve_pending_match();
						}
						self.restore_deterministic_render_snapshot(boundary_start);
						let fallback_page =
							self.with_active_reactive_scope(|| node.render_fallback());
						let fallback_selection = if inline_single_select_uses_fallback {
							selection.clone()
						} else if self.should_resolve_resources() {
							boundary_selection
								.as_ref()
								.map(SsrSelectionState::fork_after_pending_match)
						} else {
							boundary_selection.as_ref().map(SsrSelectionState::fork)
						};
						let fallback = self
							.render_stream_shell_page_with_selection(
								&fallback_page,
								boundaries,
								fallback_selection.clone(),
							)
							.await;
						if !self.should_resolve_resources()
							&& let (Some(parent), Some(rendered)) =
								(selection.as_ref(), fallback_selection.as_ref())
						{
							parent.commit_from(rendered);
						}
						let pending_selection = if inline_single_select_uses_fallback {
							selection.as_ref().map(SsrSelectionState::fork)
						} else {
							boundary_selection
						};
						boundaries.push(PendingSuspenseBoundary {
							boundary_id: boundary_id.clone(),
							node: node.clone(),
							boundary_start,
							selection: pending_selection,
						});
						self.render_suspense_fallback(&boundary_id, fallback)
					} else if self.with_active_reactive_scope(|| node.is_pending()) {
						drop(content_reactive_nodes);
						self.rollback_head_collector(head_checkpoint);
						self.restore_deterministic_render_snapshot(boundary_start);
						let fallback_page =
							self.with_active_reactive_scope(|| node.render_fallback());
						let fallback = self
							.render_stream_shell_page_with_selection(
								&fallback_page,
								boundaries,
								selection,
							)
							.await;
						self.render_suspense_fallback(&boundary_id, fallback)
					} else {
						content_reactive_nodes.commit();
						if let (Some(parent), Some(rendered)) =
							(selection.as_ref(), content_selection.as_ref())
						{
							parent.commit_from(rendered);
						}
						content
					}
				}
				Page::Deferred(node) => {
					let content = self.with_active_reactive_scope(|| node.render_content());
					self.render_stream_shell_page_with_selection(&content, boundaries, selection)
						.await
				}
				Page::Outlet(outlet) => {
					if let Some(child) = outlet.child() {
						self.render_stream_shell_page_with_selection(child, boundaries, selection)
							.await
					} else {
						String::new()
					}
				}
			}
		})
	}

	fn render_async_page<'a>(
		&'a mut self,
		view: &'a Page,
		mode: AsyncRenderMode,
	) -> LocalBoxFuture<'a, String> {
		self.render_async_page_with_selection(view, mode, None)
	}

	fn render_async_page_with_selection<'a>(
		&'a mut self,
		view: &'a Page,
		mode: AsyncRenderMode,
		selection: Option<SsrSelectionState>,
	) -> LocalBoxFuture<'a, String> {
		Box::pin(async move {
			match view {
				Page::Element(el) => {
					let projection = project(el.bound_control());
					let mut html = render_element_opening(el, &projection, selection.as_ref());

					if el.is_void() {
						html.push_str(" />");
					} else {
						html.push('>');
						if el.tag_name().eq_ignore_ascii_case("textarea")
							&& let Some(text) = projection.textarea_text.as_deref()
						{
							html.push_str(&html_escape(text));
						} else {
							let child_selection = if el.tag_name().eq_ignore_ascii_case("select") {
								el.bound_control().map(|binding| {
									SsrSelectionState::new(
										binding,
										projection.selected_values.clone(),
									)
								})
							} else {
								selection
							};
							for child in el.child_views() {
								html.push_str(
									&self
										.render_async_page_with_selection(
											child,
											mode,
											child_selection.clone(),
										)
										.await,
								);
							}
						}
						html.push_str("</");
						html.push_str(el.tag_name());
						html.push('>');
					}

					html
				}
				Page::Text(text) => html_escape(text),
				Page::Fragment(children) => {
					let mut html = String::new();
					for child in children {
						html.push_str(
							&self
								.render_async_page_with_selection(child, mode, selection.clone())
								.await,
						);
					}
					html
				}
				Page::KeyedFragment(children) => {
					let mut html = String::new();
					for (_, child) in children {
						html.push_str(
							&self
								.render_async_page_with_selection(child, mode, selection.clone())
								.await,
						);
					}
					html
				}
				Page::Empty => String::new(),
				Page::WithHead { view, head } => {
					if !matches!(mode, AsyncRenderMode::Discovery) {
						self.record_rendered_head(head);
					}
					self.render_async_page_with_selection(view, mode, selection)
						.await
				}
				#[cfg(feature = "hmr")]
				Page::DevTemplate { view, .. } | Page::DevSlot { view, .. } => {
					self.render_async_page_with_selection(view, mode, selection)
						.await
				}
				Page::ReactiveIf(reactive_if) => {
					let branch = self.with_active_reactive_scope(|| {
						if reactive_if.condition() {
							reactive_if.then_view()
						} else {
							reactive_if.else_view()
						}
					});
					self.render_async_page_with_selection(&branch, mode, selection)
						.await
				}
				Page::Reactive(reactive) => {
					let rendered = self.with_active_reactive_scope(|| reactive.render());
					self.render_async_page_with_selection(&rendered, mode, selection)
						.await
				}
				Page::Suspense(node) => {
					let boundary_id = self.suspense_boundary_id(node);
					let boundary_selection = selection.as_ref().map(SsrSelectionState::fork);
					let content_selection =
						boundary_selection.as_ref().map(SsrSelectionState::fork);
					let head_checkpoint = self.head_checkpoint();

					if let Some(context) = super::resource_context::with_active_context(Rc::clone) {
						context.borrow_mut().assign_resources_to_boundary(
							node.tracked_resource_ids(),
							&boundary_id,
						);
					}

					let boundary_start = self.deterministic_render_snapshot();
					let boundary_guard = super::resource_context::with_active_context(|context| {
						enter_boundary(context, boundary_id.clone())
					});
					let (content, mut content_reactive_nodes) =
						scope_reactive_node_transaction(async {
							let content_page =
								self.with_active_reactive_scope(|| node.render_content());
							self.render_async_page_with_selection(
								&content_page,
								mode,
								content_selection.clone(),
							)
							.await
						})
						.await;
					let boundary_end_index =
						super::resource_context::with_active_context(|context| {
							context.borrow().call_order_index()
						});

					drop(boundary_guard);

					let has_pending = super::resource_context::with_active_context(|context| {
						context.borrow().has_pending_for_boundary(&boundary_id)
					})
					.unwrap_or(false);

					if matches!(mode, AsyncRenderMode::Discovery) {
						let candidate_is_pending =
							has_pending || self.with_active_reactive_scope(|| node.is_pending());
						if let (Some(parent), Some(rendered)) =
							(selection.as_ref(), content_selection.as_ref())
						{
							parent.commit_from(rendered);
						}
						if candidate_is_pending {
							drop(content_reactive_nodes);
							self.restore_deterministic_render_snapshot(boundary_start);
						} else {
							content_reactive_nodes.commit();
						}
						return content;
					}

					if has_pending || self.with_active_reactive_scope(|| node.is_pending()) {
						drop(content_reactive_nodes);
						self.rollback_head_collector(head_checkpoint);
						self.restore_deterministic_render_snapshot(boundary_start);
						let fallback_head_checkpoint = self.head_checkpoint();
						let fallback_selection =
							boundary_selection.as_ref().map(SsrSelectionState::fork);
						let (fallback, mut fallback_reactive_nodes) =
							scope_reactive_node_transaction(async {
								let fallback_page =
									self.with_active_reactive_scope(|| node.render_fallback());
								self.render_async_page_with_selection(
									&fallback_page,
									AsyncRenderMode::Buffered,
									fallback_selection.clone(),
								)
								.await
							})
							.await;

						if has_pending
							&& let Some(context) =
								super::resource_context::with_active_context(Rc::clone)
						{
							if !self.should_resolve_resources() {
								if let (Some(parent), Some(rendered)) =
									(selection.as_ref(), fallback_selection.as_ref())
								{
									parent.commit_from(rendered);
								}
								fallback_reactive_nodes.commit();
								return self.render_suspense_fallback(&boundary_id, fallback);
							}
							if context.borrow().has_pending_external() {
								if let (Some(parent), Some(rendered)) =
									(selection.as_ref(), fallback_selection.as_ref())
								{
									parent.commit_from(rendered);
								}
								fallback_reactive_nodes.commit();
								return self.render_suspense_fallback(&boundary_id, fallback);
							}
							let boundary_resolved =
								resolve_boundary_resources(&context, &boundary_id).await;
							if !boundary_resolved {
								if let (Some(parent), Some(rendered)) =
									(selection.as_ref(), fallback_selection.as_ref())
								{
									parent.commit_from(rendered);
								}
								fallback_reactive_nodes.commit();
								return self.render_suspense_fallback(&boundary_id, fallback);
							}
							self.restore_deterministic_render_snapshot(boundary_start);
							if self.with_active_reactive_scope(|| node.is_pending()) {
								if let (Some(parent), Some(rendered)) =
									(selection.as_ref(), fallback_selection.as_ref())
								{
									parent.commit_from(rendered);
								}
								fallback_reactive_nodes.commit();
								return self.render_suspense_fallback(&boundary_id, fallback);
							}
						} else {
							if let (Some(parent), Some(rendered)) =
								(selection.as_ref(), fallback_selection.as_ref())
							{
								parent.commit_from(rendered);
							}
							fallback_reactive_nodes.commit();
							return self.render_suspense_fallback(&boundary_id, fallback);
						}

						let boundary_guard =
							super::resource_context::with_active_context(|context| {
								enter_boundary(context, boundary_id.clone())
							});
						drop(fallback_reactive_nodes);
						self.rollback_head_collector(fallback_head_checkpoint);
						let replacement_page =
							self.with_active_reactive_scope(|| node.render_content());
						let replacement_selection =
							boundary_selection.as_ref().map(SsrSelectionState::fork);
						let replacement = self
							.render_async_page_with_selection(
								&replacement_page,
								AsyncRenderMode::Buffered,
								replacement_selection.clone(),
							)
							.await;
						drop(boundary_guard);
						if let Some(index) = boundary_end_index
							&& let Some(context) =
								super::resource_context::with_active_context(Rc::clone)
						{
							context.borrow_mut().set_call_order_index(index);
						}
						if let (Some(parent), Some(rendered)) =
							(selection.as_ref(), replacement_selection.as_ref())
						{
							parent.commit_from(rendered);
						}

						replacement
					} else {
						content_reactive_nodes.commit();
						if let (Some(parent), Some(rendered)) =
							(selection.as_ref(), content_selection.as_ref())
						{
							parent.commit_from(rendered);
						}
						content
					}
				}
				Page::Deferred(node) => {
					let content = self.with_active_reactive_scope(|| node.render_content());
					self.render_async_page_with_selection(&content, mode, selection)
						.await
				}
				Page::Outlet(outlet) => {
					if let Some(child) = outlet.child() {
						self.render_async_page_with_selection(child, mode, selection)
							.await
					} else {
						String::new()
					}
				}
			}
		})
	}

	fn render_suspense_fallback(&self, id: &str, fallback: String) -> String {
		let fallback = if fallback.contains("data-rh-suspense=\"pending\"") {
			fallback
		} else {
			format!(r#"<div data-rh-suspense="pending">{fallback}</div>"#)
		};
		format!("<!--rh-suspense-start:{id}-->{fallback}<!--rh-suspense-end:{id}-->")
	}

	fn render_suspense_replacement(&self, id: &str, content: String) -> String {
		let escaped_id = html_escape(id);
		let nonce = self
			.options
			.script_nonce
			.as_ref()
			.map(|value| format!(" nonce=\"{}\"", html_escape(value)))
			.unwrap_or_default();
		let id_json = serde_json::to_string(id).unwrap_or_else(|_| "\"\"".to_string());
		let safe_id_json = escape_json_for_script(&id_json);
		format!(
			"<template data-rh-suspense-chunk=\"{escaped_id}\">{content}</template><script{nonce}>(function(id){{var template=document.querySelector('template[data-rh-suspense-chunk=\"'+id+'\"]');if(!template)return;var start=document.createComment('');var walker=document.createTreeWalker(document.body,128);while(start=walker.nextNode()){{if(start.nodeValue==='rh-suspense-start:'+id)break;}}if(!start)return;var end=start;while(end=end.nextSibling){{if(end.nodeType===8&&end.nodeValue==='rh-suspense-end:'+id)break;}}if(!end)return;var fragment=template.content.cloneNode(true);var parent=start.parentNode;var node=start.nextSibling;while(node&&node!==end){{var next=node.nextSibling;parent.removeChild(node);node=next;}}parent.insertBefore(fragment,end);template.remove();}})({safe_id_json});</script>"
		)
	}

	fn write_html_head(&self, html: &mut String, head_entries: &[ResolvedHeadEntry]) {
		let mut seen_head_entries = BTreeSet::new();

		html.push_str("<!DOCTYPE html>\n");
		html.push_str(&format!(
			"<html lang=\"{}\">\n",
			html_escape(&self.html_lang())
		));
		html.push_str("<head>\n");
		for default_entry in DEFAULT_DOCUMENT_HEAD_ENTRIES {
			if head_entries
				.iter()
				.all(|entry| serialize_managed_head_entry(entry) != default_entry)
			{
				push_unique_head_entry(html, &mut seen_head_entries, default_entry);
			}
		}

		for entry in head_entries {
			write_managed_head_entry(html, &mut seen_head_entries, entry);
		}

		if let Some(ref token) = self.options.csrf_token {
			html.push_str(&format!(
				"<meta name=\"csrf-token\" content=\"{}\">\n",
				html_escape(token)
			));
		}

		html.push_str("</head>\n");
	}

	fn wrap_in_html_shell(&self, content: &str, head_entries: &[ResolvedHeadEntry]) -> String {
		let mut shell = String::with_capacity(content.len() + 1024);

		self.write_html_head(&mut shell, head_entries);
		shell.push_str("<body>\n<div id=\"app\">");
		shell.push_str(content);
		shell.push_str("</div>\n");
		shell
	}

	fn wrap_in_html_suffix(&self) -> String {
		let mut suffix = String::new();
		if let Some(ref auth_data) = self.options.auth_data
			&& let Ok(json) = serde_json::to_string(auth_data)
		{
			let safe_json = escape_json_for_script(&json);
			suffix.push_str(&format!(
				"<script id=\"auth-data\" type=\"application/json\">{}</script>\n",
				safe_json
			));
		}

		if let Some(state_script) = self.state_script_tag() {
			suffix.push_str(&state_script);
			suffix.push('\n');
		}
		suffix.push_str("</body>\n</html>");
		suffix
	}

	/// Wraps content in a full HTML document with View's head elements.
	///
	/// Head elements (title, meta tags, CSS links, JS scripts) are sourced
	/// from the View's attached Head. Use the `head!` macro to define
	/// head elements.
	///
	/// # Arguments
	///
	/// * `content` - The rendered body content
	/// * `head_entries` - Effective framework-managed head descriptors
	fn wrap_in_html_with_head_and_body_tail_chunks(
		&self,
		content: &str,
		body_tail: &str,
		head_entries: &[ResolvedHeadEntry],
	) -> Vec<SsrChunk> {
		if self.options.minify {
			return vec![SsrChunk::Html(self.wrap_in_html_with_head_and_body_tail(
				content,
				body_tail,
				head_entries,
			))];
		}

		let mut shell = String::with_capacity(content.len() + 1024);

		self.write_html_head(&mut shell, head_entries);
		shell.push_str("<body>\n<div id=\"app\">");
		shell.push_str(content);
		shell.push_str("</div>\n");

		let mut suffix = String::new();
		if let Some(ref auth_data) = self.options.auth_data
			&& let Ok(json) = serde_json::to_string(auth_data)
		{
			let safe_json = escape_json_for_script(&json);
			suffix.push_str(&format!(
				"<script id=\"auth-data\" type=\"application/json\">{}</script>\n",
				safe_json
			));
		}

		if let Some(state_script) = self.state_script_tag() {
			suffix.push_str(&state_script);
			suffix.push('\n');
		}
		suffix.push_str("</body>\n</html>");

		let mut chunks = vec![SsrChunk::Html(shell)];
		if !body_tail.is_empty() {
			chunks.push(SsrChunk::Html(body_tail.to_string()));
		}
		chunks.push(SsrChunk::Html(suffix));
		chunks
	}

	fn wrap_in_html_with_head_and_body_tail(
		&self,
		content: &str,
		body_tail: &str,
		head_entries: &[ResolvedHeadEntry],
	) -> String {
		let mut html = String::with_capacity(content.len() + 1024);

		self.write_html_head(&mut html, head_entries);

		// Body section
		html.push_str("<body>\n");
		html.push_str("<div id=\"app\">");
		html.push_str(content);
		html.push_str("</div>\n");
		html.push_str(body_tail);

		// Auth data script (if provided)
		// Note: We escape </script> sequences to prevent XSS attacks where
		// user-controlled data (like username) could break out of the script context
		if let Some(ref auth_data) = self.options.auth_data
			&& let Ok(json) = serde_json::to_string(auth_data)
		{
			let safe_json = escape_json_for_script(&json);
			html.push_str(&format!(
				"<script id=\"auth-data\" type=\"application/json\">{}</script>\n",
				safe_json
			));
		}

		// SSR state script (if enabled)
		if let Some(state_script) = self.state_script_tag() {
			html.push_str(&state_script);
			html.push('\n');
		}

		html.push_str("</body>\n");
		html.push_str("</html>");

		if self.options.minify {
			minify_html(&html)
		} else {
			html
		}
	}

	/// Wraps content in a full HTML document.
	///
	/// This method creates a minimal HTML document without head elements.
	/// Use `render_page_with_view_head` with the `head!` macro for pages
	/// that require title, meta tags, CSS, or JS.
	pub fn wrap_in_html(&self, content: &str) -> String {
		let mut html = String::with_capacity(content.len() + 1024);
		self.write_html_head(&mut html, &[]);

		// Body section
		html.push_str("<body>\n");

		// Main content
		html.push_str("<div id=\"app\">");
		html.push_str(content);
		html.push_str("</div>\n");

		// Auth data script (if provided)
		// Note: We escape </script> sequences to prevent XSS attacks where
		// user-controlled data (like username) could break out of the script context
		if let Some(ref auth_data) = self.options.auth_data
			&& let Ok(json) = serde_json::to_string(auth_data)
		{
			let safe_json = escape_json_for_script(&json);
			html.push_str(&format!(
				"<script id=\"auth-data\" type=\"application/json\">{}</script>\n",
				safe_json
			));
		}

		// SSR state script (if enabled)
		if let Some(state_script) = self.state_script_tag() {
			html.push_str(&state_script);
			html.push('\n');
		}

		html.push_str("</body>\n");
		html.push_str("</html>");

		if self.options.minify {
			minify_html(&html)
		} else {
			html
		}
	}

	/// Renders a component with hydration marker.
	pub async fn render_with_marker<C: Component>(&mut self, component: &C) -> String {
		let content = self
			.render_view_factory_preserving_resource_state(|| component.render())
			.await;

		if self.options.include_hydration_markers {
			let marker = HydrationMarker {
				id: self.next_hydration_marker_id(),
				component_name: Some(C::name().to_string()),
				props: None,
				strategy: HydrationStrategy::default(),
			};
			format!("<div {}>{}</div>", marker.to_attr_string(), content)
		} else {
			content
		}
	}
}

fn render_element_opening(
	element: &PageElement,
	projection: &SsrControlProjection,
	selection: Option<&SsrSelectionState>,
) -> String {
	let mut html = String::new();
	html.push('<');
	html.push_str(element.tag_name());
	let reactive_input_type = element
		.reactive_attrs()
		.iter()
		.enumerate()
		.rfind(|(_, attribute)| attribute.name().eq_ignore_ascii_case("type"))
		.map(|(index, attribute)| (index, attribute.value()));
	let reactive_input_type_is_supported = reactive_input_type.as_ref().is_none_or(|(_, value)| {
		element.bound_control().is_none_or(|binding| {
			crate::control_binding::controlled_attribute_update_is_supported(
				element.tag_name(),
				binding.kind(),
				"type",
				value.as_deref(),
			)
		})
	});
	let reactive_select_multiple = element
		.reactive_attrs()
		.iter()
		.enumerate()
		.rfind(|(_, attribute)| attribute.name().eq_ignore_ascii_case("multiple"))
		.map(|(index, attribute)| (index, attribute.value()));
	let reactive_select_multiple_is_supported =
		reactive_select_multiple.as_ref().is_none_or(|(_, value)| {
			element.bound_control().is_none_or(|binding| {
				crate::control_binding::controlled_attribute_update_is_supported(
					element.tag_name(),
					binding.kind(),
					"multiple",
					value.as_deref(),
				)
			})
		});
	let mut input_type_is_password = element
		.attrs()
		.iter()
		.find(|(name, _)| name.eq_ignore_ascii_case("type"))
		.is_some_and(|(_, value)| value.eq_ignore_ascii_case("password"));
	if reactive_input_type_is_supported && let Some((_, value)) = reactive_input_type.as_ref() {
		input_type_is_password = value
			.as_deref()
			.is_some_and(|value| value.eq_ignore_ascii_case("password"));
	}
	let omits_bound_password_value = element.tag_name().eq_ignore_ascii_case("input")
		&& input_type_is_password
		&& element
			.bound_control()
			.is_some_and(|binding| binding.kind() == ControlKind::Text);

	let projects_value = element.tag_name().eq_ignore_ascii_case("input")
		&& projection.value.is_some()
		&& !omits_bound_password_value;
	let projects_checked = element.bound_control().is_some_and(|binding| {
		matches!(binding.kind(), ControlKind::Checkbox | ControlKind::Radio)
	});
	let projected_option_selection = if element.tag_name().eq_ignore_ascii_case("option") {
		selection.map(|selection| selection.option_selected(element))
	} else {
		None
	};

	for (name, value) in element.attrs() {
		let name = name.as_ref();
		let has_reactive_attribute = element
			.reactive_attrs()
			.iter()
			.any(|attribute| attribute.name().eq_ignore_ascii_case(name))
			&& (!(name.eq_ignore_ascii_case("type") && reactive_input_type.is_some())
				|| reactive_input_type_is_supported)
			&& (!(name.eq_ignore_ascii_case("multiple") && reactive_select_multiple.is_some())
				|| reactive_select_multiple_is_supported);
		if (name.eq_ignore_ascii_case("value") && (projects_value || omits_bound_password_value))
			|| (name.eq_ignore_ascii_case("checked") && projects_checked)
			|| (name.eq_ignore_ascii_case("selected") && projected_option_selection.is_some())
			|| (omits_bound_password_value
				&& name
					.eq_ignore_ascii_case(crate::control_binding::SSR_OMITTED_PASSWORD_ATTRIBUTE))
			|| has_reactive_attribute
		{
			continue;
		}
		if is_boolean_attr(name) && !is_boolean_attr_truthy(value) {
			continue;
		}

		push_escaped_attribute(&mut html, name, value);
	}
	for (index, attribute) in element.reactive_attrs().iter().enumerate() {
		if crate::component::into_page::controlled_attribute_is_overridden(
			element.bound_control(),
			attribute.name(),
		) {
			continue;
		}
		if element.reactive_attrs()[index + 1..]
			.iter()
			.any(|later| later.name().eq_ignore_ascii_case(attribute.name()))
		{
			continue;
		}
		let value = match reactive_input_type.as_ref() {
			Some((type_index, value)) if *type_index == index => value.clone(),
			_ => match reactive_select_multiple.as_ref() {
				Some((multiple_index, value)) if *multiple_index == index => value.clone(),
				_ => attribute.value(),
			},
		};
		if (reactive_input_type
			.as_ref()
			.is_some_and(|(type_index, _)| *type_index == index)
			&& !reactive_input_type_is_supported)
			|| (reactive_select_multiple
				.as_ref()
				.is_some_and(|(multiple_index, _)| *multiple_index == index)
				&& !reactive_select_multiple_is_supported)
		{
			continue;
		}
		if let Some(value) = value
			&& (!is_boolean_attr(attribute.name()) || is_boolean_attr_truthy(&value))
			&& !(omits_bound_password_value
				&& attribute
					.name()
					.eq_ignore_ascii_case(crate::control_binding::SSR_OMITTED_PASSWORD_ATTRIBUTE))
		{
			push_escaped_attribute(&mut html, attribute.name(), &value);
		}
	}

	if projects_value {
		push_escaped_attribute(
			&mut html,
			"value",
			projection.value.as_deref().unwrap_or_default(),
		);
	}
	if omits_bound_password_value {
		push_escaped_attribute(
			&mut html,
			crate::control_binding::SSR_OMITTED_PASSWORD_ATTRIBUTE,
			"true",
		);
	}
	if projects_checked && projection.checked {
		push_escaped_attribute(&mut html, "checked", "checked");
	}
	if projected_option_selection == Some(true) {
		push_escaped_attribute(&mut html, "selected", "selected");
	}

	html
}

fn push_escaped_attribute(html: &mut String, name: &str, value: &str) {
	html.push(' ');
	html.push_str(name);
	html.push_str("=\"");
	html.push_str(&html_escape(value));
	html.push('"');
}

/// Simple HTML escape function.
fn html_escape(s: &str) -> String {
	s.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
		.replace('"', "&quot;")
		.replace('\'', "&#x27;")
}

/// Escapes JSON content for safe embedding in HTML script tags.
///
/// This function prevents XSS attacks by escaping `</script>` sequences
/// that could break out of the script context. The escaping is done by
/// replacing `</` with `<\/`, which is safe because:
/// 1. JavaScript string literals interpret `<\/` as `</`
/// 2. HTML parsers don't recognize `<\/script>` as a closing tag
///
/// # Security Note
///
/// When embedding JSON data in `<script>` tags, the `</script>` sequence
/// must be escaped because HTML parsers don't understand JavaScript string
/// context - they will see `</script>` and close the tag, allowing XSS.
fn escape_json_for_script(json: &str) -> String {
	json.replace("</", "<\\/")
}

fn is_comment_safe_suspense_id(id: &str) -> bool {
	!id.is_empty()
		&& !id.contains("--")
		&& !id.ends_with('-')
		&& id.bytes().all(|byte| {
			matches!(
				byte,
				b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-' | b':' | b'.'
			)
		})
}

fn normalize_suspense_boundary_id(id: &str) -> String {
	if is_comment_safe_suspense_id(id)
		&& !id.starts_with("rh-suspense-id-")
		&& !is_generated_suspense_boundary_id(id)
	{
		return id.to_string();
	}

	let mut normalized = String::from("rh-suspense-id-");
	for byte in id.as_bytes() {
		let _ = write!(&mut normalized, "{byte:02x}");
	}
	normalized
}

fn is_generated_suspense_boundary_id(id: &str) -> bool {
	id.strip_prefix("rh-suspense-").is_some_and(|suffix| {
		!suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
	})
}

fn write_managed_head_entry(
	html: &mut String,
	seen: &mut BTreeSet<String>,
	entry: &ResolvedHeadEntry,
) {
	let serialized = serialize_managed_head_entry(entry);
	if !seen.insert(serialized.clone()) {
		return;
	}

	let tag_name_end = serialized
		.char_indices()
		.skip(1)
		.find_map(|(index, character)| {
			(character.is_ascii_whitespace() || character == '>').then_some(index)
		})
		.expect("resolved head entries must serialize as HTML elements");
	html.push_str(&serialized[..tag_name_end]);
	let _ = write!(
		html,
		" data-reinhardt-head=\"{}\"",
		html_escape(&entry.marker())
	);
	html.push_str(&serialized[tag_name_end..]);
	html.push('\n');
}

fn serialize_managed_head_entry(entry: &ResolvedHeadEntry) -> String {
	match entry {
		ResolvedHeadEntry::Base { descriptor, .. } => {
			format!("<base href=\"{}\">", html_escape(descriptor))
		}
		ResolvedHeadEntry::Meta { descriptor, .. } => descriptor.to_html(),
		ResolvedHeadEntry::Title { descriptor, .. } => {
			format!("<title>{}</title>", html_escape(descriptor))
		}
		ResolvedHeadEntry::Link { descriptor, .. } => descriptor.to_html(),
		ResolvedHeadEntry::Style { descriptor, .. } => descriptor.to_html(),
		ResolvedHeadEntry::Script { descriptor, .. } => descriptor.to_html(),
	}
}

const DEFAULT_DOCUMENT_HEAD_ENTRIES: [&str; 2] = [
	"<meta charset=\"UTF-8\">",
	"<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">",
];

fn push_unique_head_entry(
	html: &mut String,
	seen: &mut BTreeSet<String>,
	entry: impl Into<String>,
) {
	let entry = entry.into();
	if seen.contains(&entry) {
		return;
	}
	html.push_str(&entry);
	html.push('\n');
	seen.insert(entry);
}

/// Maximum input size for HTML minification (1 MiB).
///
/// Inputs exceeding this limit are returned unmodified to prevent
/// denial-of-service via excessively large payloads.
const MINIFY_HTML_MAX_INPUT_SIZE: usize = 1024 * 1024;

/// Tag names whose content must be preserved verbatim during minification.
///
/// - `pre`, `textarea`: whitespace is semantically significant
/// - `script`, `style`: whitespace removal can break code/selectors
const PRESERVED_TAGS: [&str; 4] = ["pre", "textarea", "script", "style"];

/// Simple HTML minification (removes extra whitespace).
///
/// Returns the input unmodified when its byte length exceeds
/// `MINIFY_HTML_MAX_INPUT_SIZE` (1MB) to prevent denial-of-service attacks.
///
/// Whitespace inside `<pre>`, `<textarea>`, `<script>`, and `<style>` blocks
/// is preserved.
fn minify_html(html: &str) -> String {
	if html.len() > MINIFY_HTML_MAX_INPUT_SIZE {
		return html.to_string();
	}

	let mut result = String::with_capacity(html.len());
	let mut prev_was_whitespace = false;
	// When inside a preserved tag, holds the tag name for closing-tag matching
	let mut preserved_tag: Option<&str> = None;
	let mut chars = html.char_indices().peekable();

	while let Some((byte_pos, c)) = chars.next() {
		let remaining = &html[byte_pos..];

		// Detect opening preserved tag (case-insensitive, e.g. <pre>, <PRE>, <Pre class="...">)
		if preserved_tag.is_none() && c == '<' {
			for tag in &PRESERVED_TAGS {
				// Bounded, allocation-free case-insensitive comparison:
				// only inspect `<` + tag length + 1 char instead of lowercasing all remaining input
				let open_len = 1 + tag.len(); // "<" + tag name
				if remaining.len() >= open_len
					&& remaining[1..open_len].eq_ignore_ascii_case(tag)
					&& remaining[open_len..]
						.starts_with(|ch: char| ch == '>' || ch.is_ascii_whitespace())
				{
					preserved_tag = Some(tag);
					break;
				}
			}
		}

		// Detect closing tag for the currently preserved tag (case-insensitive)
		if let Some(tag) = preserved_tag {
			let close = format!("</{tag}>");
			if c == '<'
				&& remaining.len() >= close.len()
				&& remaining[..close.len()].eq_ignore_ascii_case(&close)
			{
				// Push the original-cased closing tag from the source
				result.push_str(&remaining[..close.len()]);
				// Skip the remaining chars of the closing tag (we already consumed '<')
				for _ in 0..close.len() - 1 {
					chars.next();
				}
				preserved_tag = None;
				prev_was_whitespace = false;
				continue;
			}
		}

		if preserved_tag.is_some() {
			result.push(c);
		} else if c.is_whitespace() {
			if !prev_was_whitespace {
				result.push(' ');
				prev_was_whitespace = true;
			}
		} else {
			result.push(c);
			prev_was_whitespace = false;
		}
	}

	result
}

/// Helper function for simple component rendering.
// Allow dead_code: convenience function for internal module use and tests
#[allow(dead_code)]
pub(super) async fn render<C: Component>(component: &C) -> String {
	let mut renderer = SsrRenderer::new();
	renderer.render(component).await
}

/// Helper function for rendering to a full HTML page.
// Allow dead_code: convenience function for internal module use and tests
#[allow(dead_code)]
pub(super) async fn render_page<C: Component>(component: &C, options: SsrOptions) -> String {
	let mut renderer = SsrRenderer::with_options(options);
	renderer.render_page_to_string(component).await
}

// Phase 2-B Tests: SsrOptions Extension

#[test]
fn test_ssr_options_partial_hydration_default() {
	let opts = SsrOptions::default();
	assert!(!opts.enable_partial_hydration);
	assert_eq!(opts.default_hydration_strategy, HydrationStrategy::Full);
	assert_eq!(opts.resource_timeout, Duration::from_secs(2));
	assert!(opts.suspense_streaming);
	assert_eq!(opts.script_nonce, None);
}

#[test]
fn test_ssr_options_partial_hydration_builder() {
	let opts = SsrOptions::new()
		.partial_hydration(true)
		.default_strategy(HydrationStrategy::Island);

	assert!(opts.enable_partial_hydration);
	assert_eq!(opts.default_hydration_strategy, HydrationStrategy::Island);
}

#[test]
fn test_ssr_options_islands_only() {
	let opts = SsrOptions::new().islands_only();

	assert!(opts.enable_partial_hydration);
	assert_eq!(opts.default_hydration_strategy, HydrationStrategy::Island);
}

#[test]
fn test_ssr_options_default_strategy_static() {
	let opts = SsrOptions::new().default_strategy(HydrationStrategy::Static);

	assert!(!opts.enable_partial_hydration);
	assert_eq!(opts.default_hydration_strategy, HydrationStrategy::Static);
}
#[cfg(test)]
mod tests {
	use super::*;
	use crate::component::{ControlBinding, PageElement};
	use crate::reactive::Signal;
	use crate::reactive::hooks::use_retained_effect;
	use crate::reactive::runtime::with_runtime;
	use reinhardt_core::deps;
	use reinhardt_core::types::page::DeferredNode;
	use rstest::rstest;
	use serial_test::serial;

	struct TestComponent {
		message: String,
	}

	#[test]
	fn empty_final_entity_envelope_removes_an_earlier_table() {
		let client = QueryClient::new_ssr(QueryDefaults::default());
		let mut renderer = SsrRenderer::new();
		renderer.state.add_resource_state(
			crate::reactive::entity::ENTITY_TABLE_HYDRATION_ID,
			serde_json::json!({ "version": 1, "entities": { "stale": [] } }),
		);

		renderer.add_entity_hydration_table(&client);

		assert_eq!(
			renderer
				.state
				.get_resource_state(crate::reactive::entity::ENTITY_TABLE_HYDRATION_ID),
			None
		);
	}

	impl Component for TestComponent {
		fn render(&self) -> Page {
			PageElement::new("div")
				.attr("class", "test")
				.child(self.message.clone())
				.into_page()
		}

		fn name() -> &'static str {
			"TestComponent"
		}
	}

	#[test]
	fn normalize_suspense_boundary_id_reserves_generated_namespace() {
		let unsafe_id = normalize_suspense_boundary_id("--");
		let generated_id = normalize_suspense_boundary_id("rh-suspense-0");
		let safe_generated_prefix_id = normalize_suspense_boundary_id("rh-suspense-id-2d2d");

		assert_eq!(unsafe_id, "rh-suspense-id-2d2d");
		assert_eq!(generated_id, "rh-suspense-id-72682d73757370656e73652d30");
		assert_ne!(unsafe_id, safe_generated_prefix_id);
		assert!(safe_generated_prefix_id.starts_with("rh-suspense-id-"));
	}

	#[test]
	fn test_ssr_options_default() {
		let opts = SsrOptions::default();
		assert!(opts.include_hydration_markers);
		assert!(!opts.minify);
		assert_eq!(opts.lang, "en");
		assert_eq!(opts.resource_timeout, Duration::from_secs(2));
		assert!(opts.suspense_streaming);
	}

	#[test]
	fn render_element_opening_normalizes_controlled_html_names() {
		ReactiveScope::run(|| {
			let element = PageElement::new("INPUT")
				.attr("VALUE", "stale")
				.control_binding(ControlBinding::text(Signal::new("current".to_owned())));
			let projection = project(element.bound_control());

			assert_eq!(
				render_element_opening(&element, &projection, None),
				"<INPUT value=\"current\""
			);
		});
	}

	#[test]
	fn render_element_opening_marks_omitted_bound_password_values() {
		ReactiveScope::run(|| {
			let element = PageElement::new("input")
				.attr("type", "password")
				.attr("value", "stale")
				.control_binding(ControlBinding::text(Signal::new("secret".to_owned())));
			let projection = project(element.bound_control());

			assert_eq!(
				render_element_opening(&element, &projection, None),
				"<input type=\"password\" data-rh-password-omitted=\"true\""
			);
		});
	}

	#[rstest]
	fn render_element_opening_uses_the_first_static_password_type() {
		ReactiveScope::run(|| {
			let element = PageElement::new("input")
				.attr("type", "password")
				.attr("type", "text")
				.control_binding(ControlBinding::text(Signal::new("secret".to_owned())));
			let projection = project(element.bound_control());

			assert_eq!(
				render_element_opening(&element, &projection, None),
				"<input type=\"password\" type=\"text\" data-rh-password-omitted=\"true\""
			);
		});
	}

	#[test]
	fn render_element_opening_uses_the_effective_reactive_input_type() {
		ReactiveScope::run(|| {
			let type_evaluations = Rc::new(Cell::new(0));
			let reactive_type_evaluations = Rc::clone(&type_evaluations);
			let overridden_value_evaluations = Rc::new(Cell::new(0));
			let reactive_value_evaluations = Rc::clone(&overridden_value_evaluations);
			let superseded_class_evaluations = Rc::new(Cell::new(0));
			let first_class_evaluations = Rc::clone(&superseded_class_evaluations);
			let final_class_evaluations = Rc::new(Cell::new(0));
			let last_class_evaluations = Rc::clone(&final_class_evaluations);
			let element = PageElement::new("input")
				.attr("type", "text")
				.reactive_attr("type", move || {
					reactive_type_evaluations.set(reactive_type_evaluations.get() + 1);
					Some("password".into())
				})
				.reactive_attr("value", move || {
					reactive_value_evaluations.set(reactive_value_evaluations.get() + 1);
					Some("exposed".into())
				})
				.reactive_attr("class", move || {
					first_class_evaluations.set(first_class_evaluations.get() + 1);
					Some("superseded".into())
				})
				.reactive_attr("class", move || {
					last_class_evaluations.set(last_class_evaluations.get() + 1);
					Some("final".into())
				})
				.attr("value", "stale")
				.control_binding(ControlBinding::text(Signal::new("secret".to_owned())));
			let projection = project(element.bound_control());

			assert_eq!(
				render_element_opening(&element, &projection, None),
				"<input type=\"password\" class=\"final\" data-rh-password-omitted=\"true\""
			);
			assert_eq!(type_evaluations.get(), 1);
			assert_eq!(overridden_value_evaluations.get(), 0);
			assert_eq!(superseded_class_evaluations.get(), 0);
			assert_eq!(final_class_evaluations.get(), 1);
		});
	}

	#[tokio::test]
	async fn controlled_uppercase_textarea_and_select_render_in_buffered_and_streaming_paths() {
		let reactive_scope = ReactiveScope::new();
		let (textarea, select) = reactive_scope.enter(|| {
			let textarea = PageElement::new("TEXTAREA")
				.control_binding(ControlBinding::text(Signal::new("current".to_owned())))
				.child("stale child")
				.into_page();
			let select = PageElement::new("SELECT")
				.control_binding(ControlBinding::select_one(Signal::new(
					"current".to_owned(),
				)))
				.child(
					PageElement::new("OPTION")
						.attr("value", "current")
						.child("Current"),
				)
				.child(
					PageElement::new("OPTION")
						.attr("value", "stale")
						.child("Stale"),
				)
				.into_page();
			(textarea, select)
		});
		let view = Page::Fragment(vec![textarea, select]);
		let mut buffered_renderer = SsrRenderer::new();
		let mut streaming_renderer = SsrRenderer::new();

		let buffered = buffered_renderer.render_view(&view).await;
		let streaming = streaming_renderer
			.render_page_with_view_head(view)
			.await
			.collect_string()
			.await;

		for html in [&buffered, &streaming] {
			assert!(html.contains("<TEXTAREA>current</TEXTAREA>"), "{html}");
			assert!(
				html.contains("<OPTION value=\"current\" selected=\"selected\">Current</OPTION>"),
				"{html}"
			);
		}
	}

	#[tokio::test]
	async fn test_ssr_renderer_render() {
		let component = TestComponent {
			message: "Hello".to_string(),
		};
		let mut renderer = SsrRenderer::new();
		let html = renderer.render(&component).await;
		assert_eq!(html, "<div class=\"test\">Hello</div>");
	}

	#[tokio::test]
	async fn ssr_reactive_traversal_reenters_the_render_scope() {
		let mut renderer = SsrRenderer::new();
		let html = renderer
			.render_view_factory(|| {
				Page::reactive(|| {
					let value = Signal::new("scoped traversal");
					Page::text(value.get())
				})
			})
			.await;

		assert_eq!(html, "scoped traversal");
	}

	#[tokio::test]
	async fn streaming_ssr_reactive_traversal_reenters_the_render_scope() {
		let mut renderer = SsrRenderer::new();
		let html = renderer
			.render_page_with_view_head(Page::reactive(|| {
				let value = Signal::new("streamed scoped traversal");
				Page::text(value.get())
			}))
			.await
			.collect_string()
			.await;

		assert!(html.contains("streamed scoped traversal"));
	}

	#[tokio::test]
	#[serial]
	async fn test_ssr_renderer_disposes_retained_effects_after_each_render_pass() {
		struct RetainedEffectComponent {
			signal: Signal<i32>,
			render_count: Rc<RefCell<usize>>,
			effect_run_count: Rc<RefCell<usize>>,
			cleanup_count: Rc<RefCell<usize>>,
		}

		impl Component for RetainedEffectComponent {
			fn render(&self) -> Page {
				*self.render_count.borrow_mut() += 1;
				use_retained_effect(
					{
						let effect_run_count = Rc::clone(&self.effect_run_count);
						let cleanup_count = Rc::clone(&self.cleanup_count);
						move || {
							*effect_run_count.borrow_mut() += 1;
							let cleanup_count = Rc::clone(&cleanup_count);
							Some(move || *cleanup_count.borrow_mut() += 1)
						}
					},
					deps![self.signal],
				);
				PageElement::new("div").child("retained").into_page()
			}

			fn name() -> &'static str {
				"RetainedEffectComponent"
			}
		}

		let reactive_scope = ReactiveScope::new();
		let signal = reactive_scope.enter(|| Signal::new(0));
		let render_count = Rc::new(RefCell::new(0));
		let effect_run_count = Rc::new(RefCell::new(0));
		let cleanup_count = Rc::new(RefCell::new(0));
		let component = RetainedEffectComponent {
			signal: signal.clone(),
			render_count: Rc::clone(&render_count),
			effect_run_count: Rc::clone(&effect_run_count),
			cleanup_count: Rc::clone(&cleanup_count),
		};
		let mut renderer = SsrRenderer::new();

		let html = renderer.render(&component).await;

		assert_eq!(html, "<div>retained</div>");
		assert_eq!(*effect_run_count.borrow(), *render_count.borrow());
		assert_eq!(*cleanup_count.borrow(), *render_count.borrow());
		let runs_after_render = *effect_run_count.borrow();
		signal.set(1);
		with_runtime(|runtime| runtime.flush_updates());
		assert_eq!(*effect_run_count.borrow(), runs_after_render);
	}

	#[tokio::test]
	#[serial]
	async fn test_ssr_head_lookup_does_not_retain_deferred_content_effects() {
		let scope = Rc::new(ReactiveScope::new());
		let signal = scope.enter(|| Signal::new(0_i32));
		let effect_run_count = Rc::new(RefCell::new(0_usize));
		let view = Page::Deferred(DeferredNode::new(
			"retained-effect-head-lookup",
			|| Page::Empty,
			{
				let scope = Rc::clone(&scope);
				let effect_run_count = Rc::clone(&effect_run_count);
				move || {
					scope.enter(|| {
						use_retained_effect(
							{
								let effect_run_count = Rc::clone(&effect_run_count);
								move || {
									signal.get();
									*effect_run_count.borrow_mut() += 1;
								}
							},
							deps![signal],
						);
						PageElement::new("div").child("retained").into_page()
					})
				}
			},
		));
		let mut renderer = SsrRenderer::new();

		let html = renderer.render_page_with_view_head_to_string(view).await;

		assert!(html.contains("<div>retained</div>"));
		let runs_after_render = *effect_run_count.borrow();
		signal.set(1);
		with_runtime(|runtime| runtime.flush_updates());
		assert_eq!(
			*effect_run_count.borrow(),
			runs_after_render,
			"head lookup must not retain deferred content effects after SSR"
		);
	}

	#[tokio::test]
	async fn test_ssr_renderer_with_csrf() {
		let component = TestComponent {
			message: "Secure".to_string(),
		};
		let opts = SsrOptions::new().csrf("test-token-123");
		let mut renderer = SsrRenderer::with_options(opts);
		let html = renderer.render_page_to_string(&component).await;

		assert!(html.contains("csrf-token"));
		assert!(html.contains("test-token-123"));
	}

	#[tokio::test]
	async fn test_ssr_renderer_with_auth() {
		let component = TestComponent {
			message: "Auth".to_string(),
		};
		let auth = AuthData::authenticated("1", "testuser");
		let opts = SsrOptions::new().auth(auth);
		let mut renderer = SsrRenderer::with_options(opts);
		let html = renderer.render_page_to_string(&component).await;

		assert!(html.contains("auth-data"));
		assert!(html.contains("testuser"));
	}

	#[tokio::test]
	async fn test_ssr_renderer_with_marker() {
		let component = TestComponent {
			message: "Hydrate".to_string(),
		};
		let mut renderer = SsrRenderer::new();
		let html = renderer.render_with_marker(&component).await;

		assert!(html.contains("data-rh-id"));
		assert!(html.contains("data-rh-component=\"TestComponent\""));
	}

	#[tokio::test]
	async fn test_render_helper() {
		let component = TestComponent {
			message: "Helper".to_string(),
		};
		let html = render(&component).await;
		assert_eq!(html, "<div class=\"test\">Helper</div>");
	}

	#[test]
	fn test_html_escape() {
		assert_eq!(html_escape("<script>"), "&lt;script&gt;");
		assert_eq!(html_escape("a&b"), "a&amp;b");
		assert_eq!(html_escape("\"quoted\""), "&quot;quoted&quot;");
	}

	#[test]
	fn test_escape_json_for_script() {
		// Verify that </script> is escaped to prevent XSS
		assert_eq!(escape_json_for_script("</script>"), "<\\/script>");
		// Verify that </ is escaped in any context
		assert_eq!(
			escape_json_for_script("</script><script>alert(1)</script>"),
			"<\\/script><script>alert(1)<\\/script>"
		);
		// Normal JSON should not be modified
		assert_eq!(
			escape_json_for_script(r#"{"name":"test"}"#),
			r#"{"name":"test"}"#
		);
	}

	#[tokio::test]
	async fn test_ssr_renderer_with_auth_xss_prevention() {
		// Test that auth data with </script> in username is properly escaped
		let component = TestComponent {
			message: "Auth".to_string(),
		};
		// Simulate a malicious username that contains </script>
		let malicious_username = "</script><script>alert('xss')</script>";
		let auth = AuthData::authenticated("1", malicious_username);
		let opts = SsrOptions::new().auth(auth);
		let mut renderer = SsrRenderer::with_options(opts);
		let html = renderer.render_page_to_string(&component).await;

		// Verify the auth-data script tag exists
		assert!(html.contains("auth-data"));

		// Verify that </script> sequences are escaped in the JSON
		// The raw </script> should NOT appear in the HTML output
		assert!(!html.contains("</script><script>alert"));

		// The escaped version should be present
		assert!(html.contains("<\\/script>"));
	}

	#[tokio::test]
	async fn test_ssr_renderer_with_auth_xss_prevention_wrap_in_html_with_head() {
		use crate::component::PageElement;

		// Test XSS prevention via wrap_in_html_with_head path
		struct TestPage {
			message: String,
		}

		impl Component for TestPage {
			fn render(&self) -> Page {
				PageElement::new("div")
					.child(self.message.clone())
					.into_page()
			}

			fn name() -> &'static str {
				"TestPage"
			}
		}

		let component = TestPage {
			message: "Test".to_string(),
		};
		// Simulate a malicious username
		let malicious_username = "</script><img src=x onerror=alert(1)>";
		let auth = AuthData::authenticated("1", malicious_username);
		let opts = SsrOptions::new().auth(auth);
		let mut renderer = SsrRenderer::with_options(opts);
		let view = component.render();
		let html = renderer.render_page_with_view_head_to_string(view).await;

		// Verify that raw </script> does not appear (it should be escaped)
		assert!(!html.contains("</script><img"));
		// The escaped version should be present
		assert!(html.contains("<\\/script>"));
	}

	#[tokio::test]
	async fn test_ssr_renderer_lang_escaping() {
		// Arrange
		let component = TestComponent {
			message: "Test".to_string(),
		};
		let opts = SsrOptions::new().lang("en\" onload=\"alert(1)");
		let mut renderer = SsrRenderer::with_options(opts);

		// Act
		let html = renderer.render_page_to_string(&component).await;

		// Assert - the quote in the lang value should be escaped, preventing attribute breakout
		assert!(html.contains("&quot;"));
		// The entire malicious value should be contained within the lang attribute
		assert!(html.contains("lang=\"en&quot; onload=&quot;alert(1)\""));
	}

	#[rstest]
	#[case::pre("<pre>  hello\n  world  </pre>", "<pre>  hello\n  world  </pre>")]
	#[case::textarea(
		"<textarea>  hello\n  world  </textarea>",
		"<textarea>  hello\n  world  </textarea>"
	)]
	#[case::style(
		"<style>  .foo  {  color: red;  }  </style>",
		"<style>  .foo  {  color: red;  }  </style>"
	)]
	#[case::script(
		"<script>  var x  =  1;  </script>",
		"<script>  var x  =  1;  </script>"
	)]
	#[case::pre_with_attrs(
		"<pre class=\"code\">  spaced  </pre>",
		"<pre class=\"code\">  spaced  </pre>"
	)]
	#[case::textarea_with_attrs(
		"<textarea rows=\"5\">  multi\n  line  </textarea>",
		"<textarea rows=\"5\">  multi\n  line  </textarea>"
	)]
	#[case::surrounding_whitespace_collapsed(
		"<div>  hello  </div>  <pre>  keep  </pre>  <div>  world  </div>",
		"<div> hello </div> <pre>  keep  </pre> <div> world </div>"
	)]
	#[case::pre_uppercase("<PRE>  hello\n  world  </PRE>", "<PRE>  hello\n  world  </PRE>")]
	#[case::pre_mixed_case("<Pre>  hello\n  world  </Pre>", "<Pre>  hello\n  world  </Pre>")]
	#[case::textarea_uppercase(
		"<TEXTAREA>  multi\n  line  </TEXTAREA>",
		"<TEXTAREA>  multi\n  line  </TEXTAREA>"
	)]
	#[case::script_uppercase(
		"<SCRIPT>  var x  =  1;  </SCRIPT>",
		"<SCRIPT>  var x  =  1;  </SCRIPT>"
	)]
	#[case::style_mixed_case(
		"<Style>  .foo  {  color: red;  }  </Style>",
		"<Style>  .foo  {  color: red;  }  </Style>"
	)]
	#[case::pre_uppercase_with_attrs(
		"<PRE class=\"code\">  spaced  </PRE>",
		"<PRE class=\"code\">  spaced  </PRE>"
	)]
	fn test_minify_html_preserves_tag_content(#[case] input: &str, #[case] expected: &str) {
		// Arrange (input and expected provided by rstest cases)

		// Act
		let result = minify_html(input);

		// Assert
		assert_eq!(result, expected);
	}

	#[tokio::test]
	async fn test_ssr_renderer_lang_escaping_angle_brackets() {
		// Arrange
		let component = TestComponent {
			message: "Test".to_string(),
		};
		let opts = SsrOptions::new().lang("<script>alert(1)</script>");
		let mut renderer = SsrRenderer::with_options(opts);

		// Act
		let html = renderer.render_page_to_string(&component).await;

		// Assert - angle brackets should be escaped
		assert!(html.contains("&lt;script&gt;"));
		assert!(!html.contains("<html lang=\"<script>"));
	}
}
