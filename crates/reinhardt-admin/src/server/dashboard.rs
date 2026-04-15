//! Dashboard Server Function
//!
//! Provides dashboard data retrieval functionality.

use crate::adapters::{AdminSite, DashboardResponse, ModelInfo};
use reinhardt_di::Depends;
#[cfg(server)]
use reinhardt_pages::server_fn::ServerFnRequest;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
#[cfg(server)]
use super::error::AdminAuth;
#[cfg(server)]
use super::security::{build_csrf_cookie, generate_csrf_token};

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
#[server_fn]
pub async fn get_dashboard(
	#[inject] site: Depends<AdminSite>,
	#[inject] http_request: ServerFnRequest,
	#[inject] AdminAuthenticatedUser(_user): AdminAuthenticatedUser,
) -> Result<DashboardResponse, ServerFnError> {
	// Authentication and authorization check (Fixes #3679)
	// AdminAuthenticatedUser injection performs DB lookup to verify is_active and is_staff.
	// AdminAuth::require_staff() provides the HTTP-level error response.
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

	// Set the CSRF token as a cookie via the shared cookie jar.
	// The server function router reads SharedResponseCookies and
	// applies them as Set-Cookie headers on the HTTP response.
	let is_secure = http_request.inner().is_secure;
	let cookie_value = build_csrf_cookie(&csrf_token, is_secure);
	http_request.add_response_cookie(cookie_value);

	let admin_settings = crate::settings::get_admin_settings();

	Ok(DashboardResponse {
		site_name: site.name().to_string(),
		site_header: admin_settings.site_header.clone(),
		url_prefix: site.url_prefix().to_string(),
		login_url: admin_settings.login_url.clone(),
		logout_url: admin_settings.logout_url.clone(),
		models,
		csrf_token: Some(csrf_token),
	})
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::types::ModelInfo;
	use std::sync::Arc;

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
