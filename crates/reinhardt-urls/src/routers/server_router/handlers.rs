//! Handler adapters for ViewSets and functions

use async_trait::async_trait;
use reinhardt_http::{Handler, Request, Response, Result};
use reinhardt_views::viewsets::{Action, ViewSet};
use std::sync::Arc;

/// Handler adapter for ViewSets
pub(crate) struct ViewSetHandler {
	pub viewset: Arc<dyn ViewSet>,
	pub action: Action,
}

#[async_trait]
impl Handler for ViewSetHandler {
	async fn handle(&self, req: Request) -> Result<Response> {
		// ViewSets use constructor-level dependency injection via the `Injectable` trait.
		// Dependencies are injected once at ViewSet creation time using `ViewSet::inject(&ctx)`,
		// and the `dispatch()` method uses those pre-injected dependencies.
		// This pattern avoids runtime DI context lookups and provides better performance.
		self.viewset.dispatch(req, self.action.clone()).await
	}
}

/// Function handler adapter
pub struct FunctionHandler<F> {
	pub func: F,
}

#[async_trait]
impl<F, Fut> Handler for FunctionHandler<F>
where
	F: Fn(Request) -> Fut + Send + Sync,
	Fut: std::future::Future<Output = Result<Response>> + Send,
{
	async fn handle(&self, req: Request) -> Result<Response> {
		(self.func)(req).await
	}
}
