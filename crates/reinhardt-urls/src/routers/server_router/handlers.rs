//! Handler adapters for ViewSets.

#[cfg(feature = "viewsets")]
use async_trait::async_trait;
#[cfg(feature = "viewsets")]
use reinhardt_http::{Handler, Request, Response, Result};
#[cfg(feature = "viewsets")]
use reinhardt_views::viewsets::{Action, ViewSet};
#[cfg(feature = "viewsets")]
use std::sync::Arc;

/// Handler adapter for ViewSets
#[cfg(feature = "viewsets")]
pub(crate) struct ViewSetHandler {
	pub viewset: Arc<dyn ViewSet>,
	pub action: Action,
}

#[cfg(feature = "viewsets")]
#[async_trait]
impl Handler for ViewSetHandler {
	async fn handle(&self, mut req: Request) -> Result<Response> {
		// ViewSets use constructor-level dependency injection via the `Injectable` trait.
		// Dependencies are injected once at ViewSet creation time using `ViewSet::inject(&ctx)`,
		// and the `dispatch()` method uses those pre-injected dependencies.
		// This pattern avoids runtime DI context lookups and provides better performance.
		if let Some(middleware) = self.viewset.get_middleware()
			&& let Some(response) = middleware.process_request(&mut req).await?
		{
			return Ok(response);
		}
		self.viewset.dispatch(req, self.action.clone()).await
	}
}
