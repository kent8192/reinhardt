//! Admin Logout Server Function
//!
//! Clears the admin authentication cookie to log out the user.

use reinhardt_pages::server_fn::{ServerFnError, server_fn};

#[cfg(not(target_arch = "wasm32"))]
use super::security::build_admin_auth_cookie_clear;
#[cfg(not(target_arch = "wasm32"))]
use reinhardt_pages::server_fn::ServerFnRequest;

/// Log out the current admin user by clearing the authentication cookie.
///
/// Sets a `Max-Age=0` cookie to instruct the browser to delete the
/// `reinhardt_admin_token` cookie.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::server::logout::admin_logout;
///
/// admin_logout().await?;
/// // Browser deletes the auth cookie, subsequent requests are unauthenticated
/// ```
#[server_fn]
pub async fn admin_logout(#[inject] http_request: ServerFnRequest) -> Result<(), ServerFnError> {
	let cookie = build_admin_auth_cookie_clear();
	http_request.add_response_cookie(cookie);
	Ok(())
}
