//! Field definitions Server Function
//!
//! Provides field information for dynamic form generation.

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
use crate::adapters::{AdminDatabase, AdminRecord, AdminSite, FieldInfo, FieldType};
#[cfg(server)]
use crate::core::{AdminDatabaseKey, AdminSiteKey};
use crate::types::FieldsResponse;
#[cfg(server)]
use reinhardt_di::KeyedDepends;
#[cfg(server)]
use reinhardt_pages::server_fn::ServerFnRequest;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};

#[cfg(server)]
use super::error::{AdminAuth, MapServerFnError, ModelPermission};
#[cfg(server)]
use super::relation::{current_relation_options, relation_options_with_executor, resolve_relation};
#[cfg(server)]
use crate::server::type_inference::{get_field_metadata, infer_admin_field_type, infer_required};
#[cfg(server)]
use reinhardt_utils::utils_core::text::humanize_field_name;
#[cfg(server)]
use std::collections::HashSet;

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
	#[inject] site: KeyedDepends<AdminSiteKey, AdminSite>,
	#[inject] db: KeyedDepends<AdminDatabaseKey, AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] AdminAuthenticatedUser(user): AdminAuthenticatedUser,
) -> Result<FieldsResponse, ServerFnError> {
	// Authentication and authorization check
	let auth = AdminAuth::from_request(&http_request);
	let model_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	auth.require_model_permission(model_admin.as_ref(), user.as_ref(), ModelPermission::View)
		.await?;
	let mut field_names = model_admin
		.fields()
		.unwrap_or_else(|| model_admin.list_display());
	let readonly_fields = model_admin.readonly_fields();
	let horizontal_fields = model_admin.filter_horizontal();
	let vertical_fields = model_admin.filter_vertical();
	for &name in horizontal_fields.iter().chain(&vertical_fields) {
		if !field_names.contains(&name) {
			field_names.push(name);
		}
	}
	let selector_fields = horizontal_fields
		.into_iter()
		.chain(vertical_fields)
		.collect::<HashSet<_>>();

	// Build field metadata with type inference from global registry
	let table_name = model_admin.table_name();
	let mut fields = Vec::with_capacity(field_names.len());
	let mut connection = *db.connection();
	for &name in &field_names {
		let is_readonly = readonly_fields.contains(&name);
		let (field_type, required) = if selector_fields.contains(name) {
			let descriptor =
				resolve_relation(&site, model_admin.as_ref(), name).map_server_fn_error()?;
			auth.require_model_permission(
				descriptor.target_admin.as_ref(),
				user.as_ref(),
				ModelPermission::View,
			)
			.await?;
			let mut lookup = relation_options_with_executor(&descriptor, "", 1, &mut connection)
				.await
				.map_server_fn_error()?;
			let selected = if let Some(source_id) = id.as_deref() {
				current_relation_options(&descriptor, source_id, &mut connection)
					.await
					.map_server_fn_error()?
			} else {
				Vec::new()
			};
			let selected_values = selected
				.iter()
				.map(|option| option.value.as_str())
				.collect::<HashSet<_>>();
			lookup
				.options
				.retain(|option| !selected_values.contains(option.value.as_str()));
			(
				FieldType::ManyToManySelector {
					layout: descriptor.layout,
					available: lookup.options,
					selected,
					has_more: lookup.has_more,
				},
				false,
			)
		} else {
			get_field_metadata(table_name, name)
				.map(|meta| {
					let admin_type = infer_admin_field_type(&meta.field_type);
					let is_required = infer_required(&meta);
					(admin_type, is_required)
				})
				.unwrap_or((FieldType::Text, false))
		};

		fields.push(FieldInfo {
			name: name.to_string(),
			label: humanize_field_name(name),
			field_type,
			required,
			readonly: is_readonly,
			help_text: None,
			placeholder: None,
		});
	}

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
