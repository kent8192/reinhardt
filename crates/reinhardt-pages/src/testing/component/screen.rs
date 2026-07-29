//! Rendered native component screen.

use std::cell::RefCell;
use std::rc::Rc;

use reinhardt_core::reactive::ReactiveScope;
use reinhardt_core::types::page::{IntoPage, Page, PageElement};

use crate::reactive::query::{
	QueryClient, QueryClientGuard, QueryDefaults, provide_query_client, with_query_client,
	with_query_client_async,
};

use super::error::QueryError;
use super::events::ElementHandle;
use super::pretty::pretty_dom;
use super::query;
use super::role::Role;
use super::scheduler::{SchedulerScope, SettleError};
#[cfg(feature = "msw")]
use super::server_fn_mock::{self, ServerFnCallQuery, SharedServerFnMocks};
use super::text_match::TextMatch;
use super::tree::{ScreenInner, TestDom, shared_screen_inner};

/// Rendered native component test screen.
#[derive(Clone)]
pub struct Screen {
	inner: Rc<RefCell<ScreenInner>>,
	query_context: Rc<ScreenQueryContext>,
}

struct ScreenQueryContext {
	client: QueryClient,
	_guard: QueryClientGuard,
}

/// Input accepted by [`render`] and [`Screen::render`].
pub trait TestRender {
	/// Builds the page while the native test harness scopes are active.
	fn render_page(self) -> Page;
}

impl TestRender for Page {
	fn render_page(self) -> Page {
		self
	}
}

impl TestRender for PageElement {
	fn render_page(self) -> Page {
		self.into_page()
	}
}

impl<F, P> TestRender for F
where
	F: FnOnce() -> P,
	P: IntoPage,
{
	fn render_page(self) -> Page {
		self().into_page()
	}
}

/// Renders a Page into a native component test screen.
pub fn render(view: impl TestRender) -> Screen {
	Screen::render(view)
}

impl Screen {
	/// Renders a Page into a native component test screen.
	pub fn render(view: impl TestRender) -> Self {
		let scheduler = Rc::new(SchedulerScope::new());
		let reactive_scope = ReactiveScope::new();
		let query_client = QueryClient::new(QueryDefaults::default());
		let query_client_guard = provide_query_client(query_client.clone());
		#[cfg(feature = "msw")]
		{
			let mocks = SharedServerFnMocks::default();
			let dom = with_query_client(&query_client, || {
				scheduler.with_current(|| {
					server_fn_mock::with_active(mocks.clone(), || {
						reactive_scope.enter(|| TestDom::render(view.render_page()))
					})
				})
			});
			Self {
				inner: shared_screen_inner(dom, reactive_scope, scheduler, mocks),
				query_context: Rc::new(ScreenQueryContext {
					client: query_client,
					_guard: query_client_guard,
				}),
			}
		}
		#[cfg(not(feature = "msw"))]
		{
			let dom = with_query_client(&query_client, || {
				scheduler
					.with_current(|| reactive_scope.enter(|| TestDom::render(view.render_page())))
			});
			Self {
				inner: shared_screen_inner(dom, reactive_scope, scheduler),
				query_context: Rc::new(ScreenQueryContext {
					client: query_client,
					_guard: query_client_guard,
				}),
			}
		}
	}

	/// Returns a stable pretty representation of the rendered DOM.
	pub fn pretty(&self) -> String {
		pretty_dom(&self.inner.borrow().dom)
	}

	/// Returns exactly one element by exact text.
	pub fn get(&self, text: impl Into<TextMatch>) -> ElementHandle {
		self.get_by_text(text)
	}

	/// Tries to return exactly one element by exact text.
	pub fn try_get(&self, text: impl Into<TextMatch>) -> Result<ElementHandle, QueryError> {
		self.try_get_by_text(text)
	}

	/// Returns exactly one element by exact text.
	pub fn get_by_text(&self, text: impl Into<TextMatch>) -> ElementHandle {
		self.try_get_by_text(text).expect("text query failed")
	}

	/// Tries to return exactly one element by exact text.
	pub fn try_get_by_text(&self, text: impl Into<TextMatch>) -> Result<ElementHandle, QueryError> {
		query::by_text(&self.inner, text.into())
	}

	/// Returns zero or one element by exact text.
	pub fn query_by_text(&self, text: impl Into<TextMatch>) -> Option<ElementHandle> {
		match query::query_by_text(&self.inner, text.into()) {
			Ok(handle) => handle,
			Err(QueryError::NotFound) => None,
			Err(err) => panic!("{err}"),
		}
	}

	/// Returns exactly one element by accessible role and name.
	pub fn get_by_role(&self, role: Role, name: impl Into<TextMatch>) -> ElementHandle {
		self.try_get_by_role(role, name)
			.expect("named role query failed")
	}

	/// Tries to return exactly one element by accessible role and name.
	pub fn try_get_by_role(
		&self,
		role: Role,
		name: impl Into<TextMatch>,
	) -> Result<ElementHandle, QueryError> {
		query::by_role_named(&self.inner, role, name.into())
	}

	/// Returns exactly one element by accessible role and name.
	pub fn get_by_role_named(&self, role: Role, name: impl Into<TextMatch>) -> ElementHandle {
		self.get_by_role(role, name)
	}

	/// Returns zero or one element by accessible role and name.
	pub fn query_by_role(&self, role: Role, name: impl Into<TextMatch>) -> Option<ElementHandle> {
		match query::query_by_role_named(&self.inner, role, name.into()) {
			Ok(handle) => handle,
			Err(QueryError::NotFound) => None,
			Err(err) => panic!("{err}"),
		}
	}

	/// Returns zero or one element by accessible role and name.
	pub fn query_by_role_named(
		&self,
		role: Role,
		name: impl Into<TextMatch>,
	) -> Option<ElementHandle> {
		self.query_by_role(role, name)
	}

	/// Tries to return exactly one element by accessible role and name.
	pub fn try_get_by_role_named(
		&self,
		role: Role,
		name: impl Into<TextMatch>,
	) -> Result<ElementHandle, QueryError> {
		self.try_get_by_role(role, name)
	}

	/// Returns exactly one element by accessible label.
	pub fn get_by_label(&self, label: impl Into<TextMatch>) -> ElementHandle {
		self.try_get_by_label(label).expect("label query failed")
	}

	/// Tries to return exactly one element by accessible label.
	pub fn try_get_by_label(
		&self,
		label: impl Into<TextMatch>,
	) -> Result<ElementHandle, QueryError> {
		query::by_label(&self.inner, label.into())
	}

	/// Returns zero or one element by accessible label.
	pub fn query_by_label(&self, label: impl Into<TextMatch>) -> Option<ElementHandle> {
		match query::query_by_label(&self.inner, label.into()) {
			Ok(handle) => handle,
			Err(QueryError::NotFound) => None,
			Err(err) => panic!("{err}"),
		}
	}

	/// Returns exactly one element by placeholder text.
	pub fn get_by_placeholder(&self, placeholder: impl Into<TextMatch>) -> ElementHandle {
		self.try_get_by_placeholder(placeholder)
			.expect("placeholder query failed")
	}

	/// Tries to return exactly one element by placeholder text.
	pub fn try_get_by_placeholder(
		&self,
		placeholder: impl Into<TextMatch>,
	) -> Result<ElementHandle, QueryError> {
		query::by_placeholder(&self.inner, placeholder.into())
	}

	/// Returns zero or one element by placeholder text.
	pub fn query_by_placeholder(&self, placeholder: impl Into<TextMatch>) -> Option<ElementHandle> {
		match query::query_by_placeholder(&self.inner, placeholder.into()) {
			Ok(handle) => handle,
			Err(QueryError::NotFound) => None,
			Err(err) => panic!("{err}"),
		}
	}

	/// Waits for scheduled native component work and rerenders reactive anchors.
	pub async fn settle(&self) {
		self.try_settle()
			.await
			.unwrap_or_else(|err| panic!("{err}"));
	}

	/// Tries to settle scheduled native component work.
	pub async fn try_settle(&self) -> Result<(), SettleError> {
		#[cfg(feature = "msw")]
		let (scheduler, mocks, query_client) = {
			let inner = self.inner.borrow();
			(
				Rc::clone(&inner.scheduler),
				inner.mocks.clone(),
				self.query_context.client.clone(),
			)
		};
		#[cfg(not(feature = "msw"))]
		let (scheduler, query_client) = (
			Rc::clone(&self.inner.borrow().scheduler),
			self.query_context.client.clone(),
		);

		for _ in 0..100 {
			#[cfg(feature = "msw")]
			let result = with_query_client_async(
				query_client.clone(),
				scheduler.settle_with_context(
					|| self.pretty(),
					|poll_tasks| server_fn_mock::with_active(mocks.clone(), poll_tasks),
				),
			)
			.await;
			#[cfg(not(feature = "msw"))]
			let result = with_query_client_async(
				query_client.clone(),
				scheduler.settle_with_context(|| self.pretty(), |poll_tasks| poll_tasks()),
			)
			.await;
			#[cfg(feature = "msw")]
			with_query_client(&query_client, || {
				server_fn_mock::with_active(mocks.clone(), || {
					scheduler.with_current(|| {
						let mut inner = self.inner.borrow_mut();
						inner.rerender_reactive_anchors();
						inner.dom.refresh_control_bindings();
					});
				});
			});
			#[cfg(not(feature = "msw"))]
			{
				with_query_client(&query_client, || {
					scheduler.with_current(|| {
						let mut inner = self.inner.borrow_mut();
						inner.rerender_reactive_anchors();
						inner.dom.refresh_control_bindings();
					});
				});
			}
			match result {
				Ok(()) if scheduler.pending_task_count() == 0 => return Ok(()),
				Ok(()) => {}
				Err(err) => return Err(err),
			}
		}
		Err(SettleError::DidNotQuiesce {
			iterations: 100,
			pending_tasks: scheduler.pending_task_count(),
			dom: self.pretty(),
		})
	}

	/// Finds an element by text after settling scheduled work.
	pub async fn find_by_text(&self, text: impl Into<TextMatch>) -> ElementHandle {
		self.try_find_by_text(text)
			.await
			.unwrap_or_else(|err| panic!("{err}"))
	}

	/// Tries to find an element by text after settling scheduled work.
	pub async fn try_find_by_text(
		&self,
		text: impl Into<TextMatch>,
	) -> Result<ElementHandle, QueryError> {
		let text = text.into();
		for _ in 0..50 {
			match self.try_get_by_text(text.clone()) {
				Ok(handle) => return Ok(handle),
				Err(QueryError::NotFound) => {
					let _ = self.try_settle().await;
				}
				Err(err) => return Err(err),
			}
			tokio::task::yield_now().await;
		}
		self.try_get_by_text(text)
	}

	/// Finds an element by role and accessible name after settling scheduled work.
	pub async fn find_by_role(&self, role: Role, name: impl Into<TextMatch>) -> ElementHandle {
		self.try_find_by_role(role, name)
			.await
			.unwrap_or_else(|err| panic!("{err}"))
	}

	/// Tries to find an element by role and accessible name after settling scheduled work.
	pub async fn try_find_by_role(
		&self,
		role: Role,
		name: impl Into<TextMatch>,
	) -> Result<ElementHandle, QueryError> {
		let name = name.into();
		for _ in 0..50 {
			match self.try_get_by_role(role, name.clone()) {
				Ok(handle) => return Ok(handle),
				Err(QueryError::NotFound) => {
					let _ = self.try_settle().await;
				}
				Err(err) => return Err(err),
			}
			tokio::task::yield_now().await;
		}
		self.try_get_by_role(role, name)
	}

	/// Finds an element by role and accessible name after settling scheduled work.
	pub async fn find_by_role_named(
		&self,
		role: Role,
		name: impl Into<TextMatch>,
	) -> ElementHandle {
		self.find_by_role(role, name).await
	}

	/// Tries to find an element by role and accessible name after settling scheduled work.
	pub async fn try_find_by_role_named(
		&self,
		role: Role,
		name: impl Into<TextMatch>,
	) -> Result<ElementHandle, QueryError> {
		self.try_find_by_role(role, name).await
	}

	/// Finds an element by label after settling scheduled work.
	pub async fn find_by_label(&self, label: impl Into<TextMatch>) -> ElementHandle {
		self.try_find_by_label(label)
			.await
			.unwrap_or_else(|err| panic!("{err}"))
	}

	/// Tries to find an element by label after settling scheduled work.
	pub async fn try_find_by_label(
		&self,
		label: impl Into<TextMatch>,
	) -> Result<ElementHandle, QueryError> {
		let label = label.into();
		for _ in 0..50 {
			match self.try_get_by_label(label.clone()) {
				Ok(handle) => return Ok(handle),
				Err(QueryError::NotFound) => {
					let _ = self.try_settle().await;
				}
				Err(err) => return Err(err),
			}
			tokio::task::yield_now().await;
		}
		self.try_get_by_label(label)
	}

	/// Finds an element by placeholder after settling scheduled work.
	pub async fn find_by_placeholder(&self, placeholder: impl Into<TextMatch>) -> ElementHandle {
		self.try_find_by_placeholder(placeholder)
			.await
			.unwrap_or_else(|err| panic!("{err}"))
	}

	/// Tries to find an element by placeholder after settling scheduled work.
	pub async fn try_find_by_placeholder(
		&self,
		placeholder: impl Into<TextMatch>,
	) -> Result<ElementHandle, QueryError> {
		let placeholder = placeholder.into();
		for _ in 0..50 {
			match self.try_get_by_placeholder(placeholder.clone()) {
				Ok(handle) => return Ok(handle),
				Err(QueryError::NotFound) => {
					let _ = self.try_settle().await;
				}
				Err(err) => return Err(err),
			}
			tokio::task::yield_now().await;
		}
		self.try_get_by_placeholder(placeholder)
	}

	/// Registers a typed server function mock for this screen.
	#[cfg(feature = "msw")]
	pub fn mock_server_fn<S>(
		&self,
		handler: impl Fn(S::Args) -> Result<S::Response, crate::server_fn::ServerFnError> + 'static,
	) where
		S: crate::server_fn::MockableServerFn + 'static,
		S::Args: 'static,
		S::Response: 'static,
	{
		self.inner.borrow().mocks.mock_server_fn::<S>(handler);
	}

	/// Returns recorded calls for a typed server function mock.
	#[cfg(feature = "msw")]
	pub fn calls_to_server_fn<S>(&self) -> ServerFnCallQuery<S>
	where
		S: crate::server_fn::MockableServerFn + 'static,
		S::Args: 'static,
	{
		self.inner.borrow().mocks.calls_to_server_fn::<S>()
	}
}
