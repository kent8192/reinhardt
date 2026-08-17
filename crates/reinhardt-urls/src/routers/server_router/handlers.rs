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
		let middleware = self.viewset.get_middleware();
		if let Some(middleware) = &middleware
			&& let Some(response) = middleware.process_request(&mut req).await?
		{
			return middleware.process_response(&req, response).await;
		}

		let request_for_response = middleware
			.as_ref()
			.map(|_| req.clone_for_response())
			.transpose()?;
		let response = self.viewset.dispatch(req, self.action.clone()).await?;
		match (middleware, request_for_response) {
			(Some(middleware), Some(request)) => {
				middleware.process_response(&request, response).await
			}
			(None, None) => Ok(response),
			_ => unreachable!("middleware and response request snapshot must be paired"),
		}
	}
}
