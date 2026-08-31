//! Detail view Server Function
//!
//! Provides detail view operations for admin models.

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
use crate::adapters::{AdminDatabase, AdminSite, DetailResponse};
#[cfg(server)]
use crate::core::{AdminDatabaseKey, AdminQuery, AdminRequestContext, AdminSiteKey};
#[cfg(server)]
use reinhardt_di::KeyedDepends;
#[cfg(server)]
use reinhardt_pages::server_fn::ServerFnRequest;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};

#[cfg(server)]
use super::error::{AdminAuth, MapServerFnError, ModelPermission};
#[cfg(server)]
use super::form::resolve_admin_form;
#[cfg(server)]
use super::type_inference::translate_physical_field_names_to_logical;
#[cfg(server)]
use super::validation::retain_allowed_fields;

/// Get detail view data for a single model instance
///
/// Retrieves a single record by model name and ID, returning only fields
/// configured for the admin detail form.
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
	#[inject] site: KeyedDepends<AdminSiteKey, AdminSite>,
	#[inject] db: KeyedDepends<AdminDatabaseKey, AdminDatabase>,
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
	let request_context = AdminRequestContext::new(http_request.into_inner());
	let admin_query = model_admin
		.get_queryset(user.as_ref(), &request_context, AdminQuery::new(table_name))
		.await
		.map_server_fn_error()?;

	let mut data = db
		.get_admin_query(&admin_query, pk_field, &id)
		.await
		.map_server_fn_error()?
		.ok_or_else(|| {
			ServerFnError::server(404, format!("{} with id '{}' not found", model_name, id))
		})?;
	translate_physical_field_names_to_logical(table_name, &mut data).map_server_fn_error()?;
	let form = resolve_admin_form(&site, model_admin.as_ref()).map_server_fn_error()?;
	let visible_fields = form
		.fields
		.iter()
		.map(|field| field.name.as_str())
		.collect::<Vec<_>>();
	retain_allowed_fields(&mut data, &visible_fields);

	Ok(DetailResponse { model_name, data })
}
