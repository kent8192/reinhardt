//! Atomic changelist inline-edit Server Function.

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
#[cfg(server)]
use super::audit;
#[cfg(server)]
use super::error::{AdminAuth, IntoServerFnError, MapServerFnError, ModelPermission};
#[cfg(server)]
use super::security::{require_csrf_token, sanitize_mutation_values};
#[cfg(server)]
use super::type_inference::{get_field_metadata, infer_admin_field_type, infer_required};
#[cfg(server)]
use super::validation::validate_mutation_data;
#[cfg(server)]
use crate::adapters::{AdminDatabase, AdminSite, ModelAdmin};
#[cfg(server)]
use crate::core::history::insert_history_event;
#[cfg(server)]
use crate::core::{
	AdminBatchAtomicError, AdminBatchMutation, AdminDatabaseKey, AdminSiteKey,
	canonicalize_admin_primary_key, validate_admin_database_value,
};
#[cfg(server)]
use crate::types::{AdminError, FieldType, InlineEditError, InlineEditOutcome};
#[cfg(server)]
use reinhardt_db::migrations::FieldType as DbFieldType;
#[cfg(server)]
use reinhardt_di::KeyedDepends;
#[cfg(server)]
use reinhardt_pages::server_fn::ServerFnRequest;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};
#[cfg(server)]
use std::collections::HashSet;

#[cfg(server)]
fn add_payload_bytes(total: &mut usize, bytes: usize, limit: usize) -> bool {
	let Some(next) = total.checked_add(bytes) else {
		return false;
	};
	if next > limit {
		return false;
	}
	*total = next;
	true
}

#[cfg(server)]
fn inline_batch_fits_payload_limit(
	updates: &[crate::types::InlineEditMutation],
	limit: usize,
) -> bool {
	let mut total = 0;
	for update in updates {
		if !add_payload_bytes(&mut total, update.object_id.len(), limit) {
			return false;
		}
		for (field, value) in &update.changes {
			if !add_payload_bytes(&mut total, field.len(), limit)
				|| !add_payload_bytes(&mut total, value.to_string().len(), limit)
			{
				return false;
			}
		}
	}
	true
}

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
fn inline_value_is_empty(value: &serde_json::Value, field_type: &FieldType) -> bool {
	value.is_null()
		|| value.as_str().is_some_and(|value| value.trim().is_empty())
		|| (matches!(field_type, FieldType::MultiSelect { .. })
			&& value.as_array().is_some_and(Vec::is_empty))
}

#[cfg(server)]
fn validate_value_shape(
	object_id: &str,
	field: &str,
	value: &serde_json::Value,
	field_type: &FieldType,
	database_field_type: &DbFieldType,
	required: bool,
	nullable: bool,
) -> Option<InlineEditError> {
	let empty = inline_value_is_empty(value, field_type);
	if required && empty {
		return Some(inline_error(
			object_id,
			Some(field),
			"This field is required",
		));
	}
	if nullable && empty {
		return None;
	}
	if value.is_null() {
		return None;
	}

	let valid = match field_type {
		FieldType::Number => {
			value.is_number()
				|| (matches!(database_field_type, DbFieldType::Decimal { .. }) && value.is_string())
		}
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
			&metadata.field_type,
			infer_required(&metadata),
			metadata.nullable,
		) {
			errors.push(error);
		} else if let Err(error) =
			validate_admin_database_value(model_admin.table_name(), field, value)
		{
			errors.push(inline_error(
				&update.object_id,
				Some(field),
				error.to_string(),
			));
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
	#[cfg(feature = "file-uploads")]
	for update in &request.updates {
		super::multipart::reject_file_field_json_data(
			&update.changes,
			model_admin.as_ref(),
			site.as_ref(),
		)?;
	}
	let model_name = model_admin.model_name().to_string();
	let table_name = model_admin.table_name().to_string();
	let pk_field = model_admin.pk_field().to_string();
	let actor = user.get_username().to_string();

	let mut errors = Vec::new();
	if request.updates.is_empty() {
		return Ok(crate::types::InlineEditResponse {
			updated: 0,
			outcomes: Vec::new(),
			errors: vec![inline_error(
				"",
				None,
				"At least one row update is required",
			)],
		});
	}
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
	if !inline_batch_fits_payload_limit(&request.updates, super::validation::MAX_PAYLOAD_SIZE) {
		return Ok(crate::types::InlineEditResponse {
			updated: 0,
			outcomes: Vec::new(),
			errors: vec![inline_error(
				"",
				None,
				format!(
					"Payload too large (max {} bytes)",
					super::validation::MAX_PAYLOAD_SIZE
				),
			)],
		});
	}
	let mut object_ids = HashSet::new();
	let mut request_object_ids = Vec::with_capacity(request.updates.len());
	let mut prepared_updates = Vec::with_capacity(request.updates.len());
	for mut update in request.updates {
		let request_object_id = update.object_id.clone();
		errors.extend(validate_update(&update, model_admin.as_ref()));
		if !update.object_id.trim().is_empty() {
			match canonicalize_admin_primary_key(&table_name, &pk_field, &update.object_id) {
				Ok((canonical_id, _)) => {
					update.object_id = canonical_id;
					if !object_ids.insert(update.object_id.clone()) {
						errors.push(inline_error(
							&request_object_id,
							None,
							"Duplicate object ID",
						));
					}
				}
				Err(_) => {
					errors.push(inline_error(
						&request_object_id,
						None,
						"Invalid primary key value",
					));
				}
			}
		}
		request_object_ids.push(request_object_id);
		prepared_updates.push(update);
	}
	if !errors.is_empty() {
		return Ok(crate::types::InlineEditResponse {
			updated: 0,
			outcomes: Vec::new(),
			errors,
		});
	}
	let mutations = prepared_updates
		.into_iter()
		.map(|update| {
			let mut changes = update.changes;
			for (field, value) in &mut changes {
				if let Some(metadata) = get_field_metadata(&table_name, field)
					&& metadata.nullable
					&& inline_value_is_empty(value, &infer_admin_field_type(&metadata.field_type))
				{
					*value = serde_json::Value::Null;
				}
			}
			sanitize_mutation_values(&mut changes);
			super::create::inject_auto_now_timestamps(&mut changes, &table_name);
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
	let history_metadata = mutations
		.iter()
		.map(|mutation| {
			(
				mutation.object_id().to_owned(),
				mutation.changed_fields().to_vec(),
			)
		})
		.collect::<Vec<_>>();
	let history_table_name = table_name.clone();
	match db
		.update_batch_with(
			&table_name,
			&pk_field,
			mutations,
			async move |transaction| {
				for (object_id, changed_fields) in history_metadata {
					let event = audit::new_history_event(
						&actor,
						"UPDATE",
						&model_name,
						&history_table_name,
						&object_id,
						changed_fields,
						1,
					);
					insert_history_event(transaction, &event)
						.await
						.map_err(|error| AdminError::DatabaseError(error.to_string()))?;
				}
				Ok(())
			},
		)
		.await
	{
		Ok(updated) => Ok(crate::types::InlineEditResponse {
			updated,
			outcomes,
			errors: Vec::new(),
		}),
		Err(AdminBatchAtomicError::ZeroAffected {
			row_index,
			object_id,
		}) => {
			let request_object_id = request_object_ids
				.get(row_index)
				.map(String::as_str)
				.unwrap_or(&object_id);
			Ok(crate::types::InlineEditResponse {
				updated: 0,
				outcomes: Vec::new(),
				errors: vec![inline_error(
					request_object_id,
					None,
					"Object was not found",
				)],
			})
		}
		Err(AdminBatchAtomicError::Admin(error)) => Err(error.into_server_fn_error()),
		Err(AdminBatchAtomicError::Core(_)) => {
			Err(ServerFnError::server(500, "Database operation failed"))
		}
	}
}

#[cfg(all(test, server))]
mod tests {
	use super::{add_payload_bytes, inline_value_is_empty};
	use crate::types::FieldType;
	use rstest::rstest;

	#[rstest]
	fn payload_size_accepts_boundary_and_rejects_limit_or_usize_overflow() {
		let mut total = 6;
		assert!(add_payload_bytes(&mut total, 4, 10));
		assert_eq!(total, 10);
		assert!(!add_payload_bytes(&mut total, 1, 10));
		assert_eq!(total, 10);

		let mut overflow = usize::MAX;
		assert!(!add_payload_bytes(&mut overflow, 1, usize::MAX));
		assert_eq!(overflow, usize::MAX);
	}

	#[rstest]
	fn only_multi_select_treats_an_empty_array_as_an_empty_value() {
		let empty = serde_json::json!([]);

		assert!(inline_value_is_empty(
			&empty,
			&FieldType::MultiSelect {
				choices: Vec::new(),
			}
		));
		assert!(!inline_value_is_empty(&empty, &FieldType::Number));
	}
}
