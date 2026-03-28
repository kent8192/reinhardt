//! HTML injection

use crate::error::{ToolbarError, ToolbarResult};
use crate::panels::PanelStats;
use crate::ui::render_toolbar;
use axum::body::Body;
use axum::response::Response;
use http_body_util::BodyExt;

/// Inject toolbar HTML into response
pub async fn inject_toolbar(
	response: Response<Body>,
	panel_stats: &[PanelStats],
) -> ToolbarResult<Response<Body>> {
	// Only inject into HTML responses
	let content_type = response
		.headers()
		.get("content-type")
		.and_then(|v| v.to_str().ok())
		.unwrap_or("");

	if !content_type.contains("text/html") {
		return Ok(response);
	}

	// Read response body
	let (parts, body) = response.into_parts();
	let body_bytes = body
		.collect()
		.await
		.map_err(|e| ToolbarError::HttpError(e.to_string()))?
		.to_bytes();

	let html = String::from_utf8_lossy(&body_bytes);

	// Generate toolbar HTML
	let toolbar_html = render_toolbar(panel_stats)?;

	// Find injection point (</body> tag)
	let injected_html = if let Some(pos) = html.rfind("</body>") {
		format!("{}{}{}", &html[..pos], toolbar_html, &html[pos..])
	} else {
		// No </body> tag, append to end
		format!("{}{}", html, toolbar_html)
	};

	// Rebuild response
	let response = Response::from_parts(parts, Body::from(injected_html));
	Ok(response)
}
