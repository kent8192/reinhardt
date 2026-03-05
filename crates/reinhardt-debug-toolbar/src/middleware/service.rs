//! Debug toolbar Tower service

use crate::context::{RequestInfo, TOOLBAR_CONTEXT, ToolbarContext};
use crate::middleware::ToolbarConfig;
use crate::panels::PanelRegistry;
use crate::ui::inject_toolbar;
use axum::body::Body;
use axum::http::{Request, Response};
use chrono::Utc;
use std::future::Future;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tower::Service;

/// Debug toolbar Tower service
pub struct DebugToolbarService<S> {
	pub(crate) inner: S,
	pub(crate) config: Arc<ToolbarConfig>,
	pub(crate) registry: Arc<PanelRegistry>,
}

impl<S> Service<Request<Body>> for DebugToolbarService<S>
where
	S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
	S::Future: Send + 'static,
	S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
	type Response = S::Response;
	type Error = Box<dyn std::error::Error + Send + Sync>;
	type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

	fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		self.inner.poll_ready(cx).map_err(Into::into)
	}

	fn call(&mut self, req: Request<Body>) -> Self::Future {
		let config = self.config.clone();
		let registry = self.registry.clone();
		let inner = self.inner.clone();
		let mut inner_service = std::mem::replace(&mut self.inner, inner);

		Box::pin(async move {
			// Extract client IP from request
			let client_ip = extract_client_ip(&req);

			// Check if toolbar should be shown
			if !config.should_show(&client_ip) {
				return inner_service.call(req).await.map_err(Into::into);
			}

			// Create toolbar context
			let request_info = extract_request_info(&req);
			let toolbar_ctx = ToolbarContext::new(request_info);

			// Enable instrumentation for all panels
			for panel in registry.all() {
				if let Err(e) = panel.enable_instrumentation().await {
					tracing::warn!(
						"Failed to enable instrumentation for panel {}: {}",
						panel.name(),
						e
					);
				}
			}

			// Execute request with toolbar context in scope
			let response = TOOLBAR_CONTEXT
				.scope(toolbar_ctx.clone(), async {
					inner_service.call(req).await.map_err(Into::into)
				})
				.await;

			// Disable instrumentation for all panels
			for panel in registry.all() {
				if let Err(e) = panel.disable_instrumentation().await {
					tracing::warn!(
						"Failed to disable instrumentation for panel {}: {}",
						panel.name(),
						e
					);
				}
			}

			// Handle response
			let response = match response {
				Ok(resp) => resp,
				Err(e) => {
					tracing::error!("Request handler error: {}", e);
					return Err(e);
				}
			};

			// Generate panel statistics
			let panel_stats = match generate_panel_stats(&registry, &toolbar_ctx).await {
				Ok(stats) => stats,
				Err(e) => {
					tracing::error!("Failed to generate panel stats: {}", e);
					// Graceful degradation: return response without toolbar
					return Ok(response);
				}
			};

			// Inject toolbar HTML into response
			match inject_toolbar(response, &panel_stats).await {
				Ok(modified_response) => Ok(modified_response),
				Err(e) => {
					tracing::error!("Failed to inject toolbar: {}", e);
					// Graceful degradation: return original response
					// Note: original response was already consumed, so we return an error
					Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
				}
			}
		})
	}
}

/// Extract client IP from request
fn extract_client_ip(req: &Request<Body>) -> IpAddr {
	// Check X-Forwarded-For header
	if let Some(forwarded) = req.headers().get("x-forwarded-for")
		&& let Ok(forwarded_str) = forwarded.to_str()
		&& let Some(ip_str) = forwarded_str.split(',').next()
		&& let Ok(ip) = ip_str.trim().parse()
	{
		return ip;
	}

	// Fallback to localhost
	"127.0.0.1".parse().unwrap()
}

/// Extract request information
fn extract_request_info(req: &Request<Body>) -> RequestInfo {
	let method = req.method().to_string();
	let path = req.uri().path().to_string();
	let query = req.uri().query().map(String::from);

	let headers: Vec<(String, String)> = req
		.headers()
		.iter()
		.map(|(name, value)| (name.to_string(), value.to_str().unwrap_or("").to_string()))
		.collect();

	let client_ip = extract_client_ip(req).to_string();
	let timestamp = Utc::now();

	RequestInfo {
		method,
		path,
		query,
		headers,
		client_ip,
		timestamp,
	}
}

/// Generate statistics for all panels
async fn generate_panel_stats(
	registry: &PanelRegistry,
	ctx: &ToolbarContext,
) -> Result<Vec<crate::panels::PanelStats>, crate::error::ToolbarError> {
	let mut stats = Vec::new();

	for panel in registry.all() {
		match panel.generate_stats(ctx).await {
			Ok(panel_stats) => stats.push(panel_stats),
			Err(e) => {
				tracing::warn!("Panel {} failed to generate stats: {}", panel.name(), e);
			}
		}
	}

	Ok(stats)
}
