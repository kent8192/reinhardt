//! Dashboard Server Function
//!
//! Provides dashboard data retrieval functionality.

use crate::adapters::{AdminSite, DashboardResponse, ModelInfo};
use reinhardt_pages::server_fn::{ServerFnError, ServerFnRequest, server_fn};
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use super::error::AdminAuth;
#[cfg(not(target_arch = "wasm32"))]
use super::security::generate_csrf_token;

/// Get dashboard data
///
/// Returns dashboard information including registered models and site metadata.
///
/// # Server Function
///
/// This function is automatically exposed as an HTTP endpoint by the `#[server_fn]` macro.
/// The AdminSite dependency is automatically injected via the DI system.
///
/// # Authentication
///
/// Requires staff (admin) permission to access the admin panel.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::server::get_dashboard;
///
/// // Client-side usage (automatically generates HTTP request)
/// let dashboard = get_dashboard().await?;
/// println!("Site: {}", dashboard.site_name);
/// ```
#[server_fn(use_inject = true)]
pub async fn get_dashboard(
	#[inject] site: Arc<AdminSite>,
	#[inject] http_request: ServerFnRequest,
) -> Result<DashboardResponse, ServerFnError> {
	// Authentication and authorization check
	let auth = AdminAuth::from_request(&http_request);
	auth.require_staff()?;

	// Collect model information
	let models: Vec<ModelInfo> = site
		.registered_models()
		.into_iter()
		.map(|name| {
			let list_url = format!("{}/{}/", site.url_prefix(), name.to_lowercase());
			ModelInfo { name, list_url }
		})
		.collect();

	// Build dashboard response with CSRF token for mutation requests
	let csrf_token = generate_csrf_token();
	Ok(DashboardResponse {
		site_name: site.name().to_string(),
		url_prefix: site.url_prefix().to_string(),
		models,
		csrf_token: Some(csrf_token),
	})
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::types::ModelInfo;

	#[tokio::test]
	async fn test_dashboard_response_structure() {
		// Create a mock AdminSite
		let site = Arc::new(AdminSite::new("Test Admin"));

		// Note: This test verifies the response structure without actually calling
		// the Server Function, since that would require full DI context setup
		let expected_site_name = site.name().to_string();
		let expected_url_prefix = site.url_prefix().to_string();

		// Verify site configuration
		assert_eq!(expected_site_name, "Test Admin");
		assert_eq!(expected_url_prefix, "/admin");
	}

	#[test]
	fn test_model_info_construction() {
		let model_name = "User".to_string();
		let list_url = format!("/admin/{}/", model_name.to_lowercase());

		let model_info = ModelInfo {
			name: model_name.clone(),
			list_url: list_url.clone(),
		};

		assert_eq!(model_info.name, "User");
		assert_eq!(model_info.list_url, "/admin/user/");
	}
}
