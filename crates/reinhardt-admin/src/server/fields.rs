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
use crate::server::relation::{
	relation_id_from_value, resolve_relation_configuration, resolve_relation_option,
};
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
	let field_names = model_admin
		.fields()
		.unwrap_or_else(|| model_admin.list_display());
	let readonly_fields = model_admin.readonly_fields();
	let relations = resolve_relation_configuration(&site, &model_admin).map_server_fn_error()?;

	// Fetch existing values before resolving edit-form relation labels.
	let values = if let Some(id) = id {
		db.get::<AdminRecord>(model_admin.table_name(), model_admin.pk_field(), &id)
			.await
			.map_server_fn_error()?
	} else {
		None
	};

	// Build field metadata with type inference from global registry
	let table_name = model_admin.table_name();
	let mut fields = Vec::with_capacity(field_names.len());
	for name in field_names {
		if let Some(relation) = relations.iter().find(|relation| {
			relation.foreign_key.logical_name == name || relation.foreign_key.column_name == name
		}) {
			let selected = match values
				.as_ref()
				.and_then(|record| record.get(&relation.foreign_key.column_name))
			{
				Some(value) => match relation_id_from_value(value).map_server_fn_error()? {
					Some(id) => Some(
						resolve_relation_option(&auth, user.as_ref(), &db, relation, &id).await?,
					),
					None => None,
				},
				None => None,
			};
			let is_readonly = readonly_fields.contains(&name)
				|| readonly_fields.contains(&relation.foreign_key.logical_name.as_str())
				|| readonly_fields.contains(&relation.foreign_key.column_name.as_str());

			fields.push(FieldInfo {
				name: relation.foreign_key.column_name.clone(),
				label: humanize_field_name(&relation.foreign_key.logical_name),
				field_type: FieldType::Relation {
					field_name: relation.foreign_key.logical_name.clone(),
					widget: relation.widget,
					selected,
					readonly: is_readonly,
				},
				required: infer_required(&relation.foreign_key.field_metadata),
				readonly: is_readonly,
				help_text: None,
				placeholder: None,
			});
			continue;
		}

		let is_readonly = readonly_fields.contains(&name);
		let (field_type, required) = get_field_metadata(table_name, name)
			.map(|meta| {
				let admin_type = infer_admin_field_type(&meta.field_type);
				let is_required = infer_required(&meta);
				(admin_type, is_required)
			})
			.unwrap_or_else(|| (FieldType::Text, false));

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

	Ok(FieldsResponse {
		model_name,
		fields,
		values,
	})
}
