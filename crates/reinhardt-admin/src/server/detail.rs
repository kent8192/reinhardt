//! Detail view Server Function
//!
//! Provides detail view operations for admin models.

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
use crate::adapters::{AdminDatabase, AdminRecord, AdminSite, DetailResponse};
use reinhardt_di::Depends;
#[cfg(server)]
use reinhardt_pages::server_fn::ServerFnRequest;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};

#[cfg(server)]
use super::error::{AdminAuth, MapServerFnError, ModelPermission};

/// Get detail view data for a single model instance
///
/// Retrieves a single record by model name and ID, returning all fields
/// as a HashMap of field names to JSON values.
///
/// # Server Function
///
/// This function is automatically exposed as an HTTP endpoint by the `#[server_fn]` macro.
/// AdminSite and AdminDatabase dependencies are automatically injected via the DI system.
///
/// # Authentication
///
/// Requires staff (admin) permission and view permission for the model.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::server::get_detail;
///
/// // Client-side usage (automatically generates HTTP request)
/// let response = get_detail("User".to_string(), "42".to_string()).await?;
/// println!("User data: {:?}", response.data);
/// ```
#[server_fn]
pub async fn get_detail(
	model_name: String,
	id: String,
	#[inject] site: Depends<AdminSite>,
	#[inject] db: Depends<AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] AdminAuthenticatedUser(user): AdminAuthenticatedUser,
) -> Result<DetailResponse, ServerFnError> {
	// Authentication and authorization check
	let auth = AdminAuth::from_request(&http_request);
	let model_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	auth.require_model_permission(model_admin.as_ref(), user.as_ref(), ModelPermission::View)
		.await?;
	let table_name = model_admin.table_name();
	let pk_field = model_admin.pk_field();

	let data = db
		.get::<AdminRecord>(table_name, pk_field, &id)
		.await
		.map_server_fn_error()?
		.ok_or_else(|| {
			ServerFnError::server(404, format!("{} with id '{}' not found", model_name, id))
		})?;

	Ok(DetailResponse { model_name, data })
}
