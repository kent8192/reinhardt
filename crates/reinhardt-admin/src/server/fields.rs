//! Field definitions Server Function
//!
//! Provides field information for dynamic form generation.

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
use crate::adapters::{AdminDatabase, AdminRecord, AdminSite, FieldInfo, FieldType};
use crate::types::FieldsResponse;
use reinhardt_di::Depends;
#[cfg(server)]
use reinhardt_pages::server_fn::ServerFnRequest;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};

#[cfg(server)]
use super::error::{AdminAuth, MapServerFnError, ModelPermission};
#[cfg(server)]
use crate::server::type_inference::{get_field_metadata, infer_admin_field_type, infer_required};
#[cfg(server)]
use reinhardt_utils::utils_core::text::humanize_field_name;

/// Get field definitions for dynamic form generation
///
/// Retrieves field metadata for creating or editing model instances.
/// When `id` is provided, also retrieves the existing field values for editing.
///
/// # Server Function
///
/// This function is automatically exposed as an HTTP endpoint by the `#[server_fn]` macro.
/// AdminSite and AdminDatabase dependencies are automatically injected via the DI system.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::server::get_fields;
///
/// // Client-side usage for create form
/// let response = get_fields("User".to_string(), None).await?;
/// println!("Fields: {:?}", response.fields);
///
/// // Client-side usage for edit form
/// let response = get_fields("User".to_string(), Some("42".to_string())).await?;
/// println!("Existing values: {:?}", response.values);
/// ```
#[server_fn]
pub async fn get_fields(
	model_name: String,
	id: Option<String>,
	#[inject] site: Depends<AdminSite>,
	#[inject] db: Depends<AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] AdminAuthenticatedUser(user): AdminAuthenticatedUser,
) -> Result<FieldsResponse, ServerFnError> {
	// Authentication and authorization check
	let auth = AdminAuth::from_request(&http_request);
	let model_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	auth.require_model_permission(model_admin.as_ref(), user.as_ref(), ModelPermission::View)
		.await?;
	let field_names = model_admin
		.fields()
		.unwrap_or_else(|| model_admin.list_display());
	let readonly_fields = model_admin.readonly_fields();

	// Build field metadata with type inference from global registry
	let table_name = model_admin.table_name();
	let fields = field_names
		.iter()
		.map(|&name| {
			let is_readonly = readonly_fields.contains(&name);

			// Try to get field metadata from the global model registry
			let (field_type, required) = get_field_metadata(table_name, name)
				.map(|meta| {
					let admin_type = infer_admin_field_type(&meta.field_type);
					let is_required = infer_required(&meta);
					(admin_type, is_required)
				})
				.unwrap_or_else(|| (FieldType::Text, false));

			FieldInfo {
				name: name.to_string(),
				label: humanize_field_name(name),
				field_type,
				required,
				readonly: is_readonly,
				help_text: None,
				placeholder: None,
			}
		})
		.collect();

	// Fetch existing values if editing
	let values = if let Some(id) = id {
		db.get::<AdminRecord>(model_admin.table_name(), model_admin.pk_field(), &id)
			.await
			.map_server_fn_error()?
	} else {
		None
	};

	Ok(FieldsResponse {
		model_name,
		fields,
		values,
	})
}
