//! Atomic changelist inline-edit Server Function.

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
#[cfg(server)]
use super::error::{AdminAuth, IntoServerFnError, MapServerFnError, ModelPermission};
#[cfg(server)]
use super::security::{require_csrf_token, sanitize_mutation_values};
#[cfg(server)]
use super::type_inference::{get_field_metadata, infer_admin_field_type, infer_required};
#[cfg(server)]
use super::validation::validate_mutation_data;
#[cfg(server)]
use crate::adapters::{AdminDatabase, AdminRecord, AdminSite, ModelAdmin};
#[cfg(server)]
use crate::core::{AdminBatchAtomicError, AdminBatchMutation, AdminDatabaseKey, AdminSiteKey};
#[cfg(server)]
use crate::types::{FieldType, InlineEditError, InlineEditOutcome};
#[cfg(server)]
use reinhardt_di::KeyedDepends;
#[cfg(server)]
use reinhardt_pages::server_fn::ServerFnRequest;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};
#[cfg(server)]
use std::collections::HashSet;

#[cfg(server)]
fn inline_error(
	object_id: &str,
	field: Option<&str>,
	message: impl Into<String>,
) -> InlineEditError {
	InlineEditError {
		object_id: object_id.to_string(),
		field: field.map(str::to_string),
		message: message.into(),
	}
}

#[cfg(server)]
fn validate_value_shape(
	object_id: &str,
	field: &str,
	value: &serde_json::Value,
	field_type: &FieldType,
	required: bool,
) -> Option<InlineEditError> {
	let empty = value.is_null()
		|| value.as_str().is_some_and(|value| value.trim().is_empty())
		|| value.as_array().is_some_and(Vec::is_empty);
	if required && empty {
		return Some(inline_error(
			object_id,
			Some(field),
			"This field is required",
		));
	}
	if value.is_null() {
		return None;
	}

	let valid = match field_type {
		FieldType::Number => value.is_number(),
		FieldType::Boolean => value.is_boolean(),
		FieldType::MultiSelect { choices } => value.as_array().is_some_and(|values| {
			values.iter().all(|value| {
				value.as_str().is_some_and(|value| {
					choices.is_empty() || choices.iter().any(|(choice, _)| choice == value)
				})
			})
		}),
		FieldType::Select { choices } => value
			.as_str()
			.is_some_and(|value| choices.iter().any(|(choice, _)| choice == value)),
		_ => value.is_string(),
	};
	(!valid).then(|| inline_error(object_id, Some(field), "Invalid value type"))
}

#[cfg(server)]
fn validate_update(
	update: &crate::types::InlineEditMutation,
	model_admin: &dyn ModelAdmin,
) -> Vec<InlineEditError> {
	let mut errors = Vec::new();
	let editable = model_admin.list_editable();
	let readonly = model_admin.readonly_fields();
	let pk_field = model_admin.pk_field();

	if update.object_id.trim().is_empty() {
		errors.push(inline_error("", None, "Object ID must not be empty"));
	}
	if update.changes.is_empty() {
		errors.push(inline_error(
			&update.object_id,
			None,
			"At least one changed field is required",
		));
	}

	for (field, value) in &update.changes {
		if field == pk_field {
			errors.push(inline_error(
				&update.object_id,
				Some(field),
				"Primary key fields cannot be edited",
			));
			continue;
		}
		if readonly.contains(&field.as_str()) {
			errors.push(inline_error(
				&update.object_id,
				Some(field),
				"Read-only fields cannot be edited",
			));
			continue;
		}
		if !editable.contains(&field.as_str()) {
			errors.push(inline_error(
				&update.object_id,
				Some(field),
				"Field is not editable in the changelist",
			));
			continue;
		}

		let Some(metadata) = get_field_metadata(model_admin.table_name(), field) else {
			errors.push(inline_error(
				&update.object_id,
				Some(field),
				"Field metadata is unavailable",
			));
			continue;
		};
		if metadata.generated.is_some() {
			errors.push(inline_error(
				&update.object_id,
				Some(field),
				"Generated fields cannot be edited",
			));
			continue;
		}
		if let Some(error) = validate_value_shape(
			&update.object_id,
			field,
			value,
			&infer_admin_field_type(&metadata.field_type),
			infer_required(&metadata),
		) {
			errors.push(error);
		}
	}

	if errors.is_empty()
		&& let Err(error) = validate_mutation_data(&update.changes, model_admin, true)
	{
		errors.push(inline_error(&update.object_id, None, error.to_string()));
	}
	errors
}

/// Applies dirty changelist row edits in one database transaction.
#[server_fn]
pub async fn update_inline_edits(
	model_name: String,
	request: crate::types::InlineEditRequest,
	#[inject] site: KeyedDepends<AdminSiteKey, AdminSite>,
	#[inject] db: KeyedDepends<AdminDatabaseKey, AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] AdminAuthenticatedUser(user): AdminAuthenticatedUser,
) -> Result<crate::types::InlineEditResponse, ServerFnError> {
	require_csrf_token(&request.csrf_token, &http_request.inner().headers)?;

	let auth = AdminAuth::from_request(&http_request);
	let model_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	auth.require_model_permission(model_admin.as_ref(), user.as_ref(), ModelPermission::Change)
		.await?;

	let mut errors = Vec::new();
	if request.updates.len() > super::limits::MAX_PAGE_SIZE as usize {
		return Ok(crate::types::InlineEditResponse {
			updated: 0,
			outcomes: Vec::new(),
			errors: vec![inline_error(
				"",
				None,
				format!(
					"Too many rows in request: {} (max {})",
					request.updates.len(),
					super::limits::MAX_PAGE_SIZE
				),
			)],
		});
	}
	let mut object_ids = HashSet::new();
	for update in &request.updates {
		if !object_ids.insert(update.object_id.as_str()) {
			errors.push(inline_error(&update.object_id, None, "Duplicate object ID"));
		}
		errors.extend(validate_update(update, model_admin.as_ref()));
	}
	if !errors.is_empty() {
		return Ok(crate::types::InlineEditResponse {
			updated: 0,
			outcomes: Vec::new(),
			errors,
		});
	}

	let table_name = model_admin.table_name();
	let pk_field = model_admin.pk_field();
	let mutations = request
		.updates
		.into_iter()
		.map(|update| {
			let mut changes = update.changes;
			sanitize_mutation_values(&mut changes);
			super::create::inject_auto_now_timestamps(&mut changes, table_name);
			AdminBatchMutation::new(update.object_id, changes)
		})
		.collect::<Vec<_>>();
	let outcomes = mutations
		.iter()
		.map(|mutation| InlineEditOutcome {
			object_id: mutation.object_id().to_string(),
			changed_fields: mutation.changed_fields().to_vec(),
		})
		.collect::<Vec<_>>();

	match db
		.update_batch_with::<AdminRecord, _>(
			table_name,
			pk_field,
			mutations,
			async |_transaction, _mutations| Ok(()),
		)
		.await
	{
		Ok(updated) => Ok(crate::types::InlineEditResponse {
			updated,
			outcomes,
			errors: Vec::new(),
		}),
		Err(AdminBatchAtomicError::ZeroAffected { object_id, .. }) => {
			Ok(crate::types::InlineEditResponse {
				updated: 0,
				outcomes: Vec::new(),
				errors: vec![inline_error(&object_id, None, "Object was not found")],
			})
		}
		Err(AdminBatchAtomicError::Admin(error)) => Err(error.into_server_fn_error()),
		Err(AdminBatchAtomicError::Core(_)) => {
			Err(ServerFnError::server(500, "Database operation failed"))
		}
	}
}
