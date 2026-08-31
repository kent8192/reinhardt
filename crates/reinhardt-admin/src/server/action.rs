//! Registered admin action Server Function.

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
#[cfg(server)]
use crate::adapters::{AdminDatabase, AdminSite, ModelAdmin};
#[cfg(server)]
use crate::core::history::insert_history_event;
#[cfg(server)]
use crate::core::{
	AdminDatabaseKey, AdminQuery, AdminRequestContext, AdminSiteKey, canonicalize_admin_primary_key,
};
#[cfg(server)]
use crate::types::{AdminAction, AdminError, AdminResult};
use crate::types::{AdminActionRequest, MutationResponse};
#[cfg(server)]
use reinhardt_di::KeyedDepends;
#[cfg(server)]
use reinhardt_pages::server_fn::ServerFnRequest;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};
#[cfg(server)]
use std::collections::HashSet;

#[cfg(server)]
use super::audit;
#[cfg(server)]
use super::error::{AdminAuth, IntoServerFnError};
#[cfg(server)]
use super::limits::MAX_BULK_DELETE_IDS;
#[cfg(server)]
use super::security::require_csrf_token;
#[cfg(server)]
pub(crate) fn registered_actions(model_admin: &dyn ModelAdmin) -> AdminResult<Vec<AdminAction>> {
	let actions = model_admin.actions();
	let mut names = HashSet::new();
	for action in &actions {
		if action.name.is_empty() || !names.insert(action.name.as_str()) {
			return Err(AdminError::ValidationError(
				"Admin action names must be nonempty and unique".to_string(),
			));
		}
	}
	Ok(actions)
}

/// Executes a registered action for the selected model records.
#[server_fn]
pub async fn execute_admin_action(
	model_name: String,
	request: AdminActionRequest,
	#[inject] site: KeyedDepends<AdminSiteKey, AdminSite>,
	#[inject] db: KeyedDepends<AdminDatabaseKey, AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] AdminAuthenticatedUser(user): AdminAuthenticatedUser,
) -> Result<MutationResponse, ServerFnError> {
	require_csrf_token(&request.csrf_token, &http_request.inner().headers)?;

	let auth = AdminAuth::from_request(&http_request);
	let user_id = auth.user_id().unwrap_or("unknown");
	let model_admin = match site.get_model_admin(&model_name) {
		Ok(model_admin) => model_admin,
		Err(error) => {
			audit::log_action(
				user_id,
				&model_name,
				&request.ids,
				&request.action,
				0,
				false,
			);
			return Err(error.into_server_fn_error());
		}
	};
	if request.action.is_empty() {
		audit::log_action(
			user_id,
			model_admin.model_name(),
			&request.ids,
			&request.action,
			0,
			false,
		);
		return Err(ServerFnError::server(400, "Action is required"));
	}
	let action = match registered_actions(model_admin.as_ref())
		.map_err(|error| error.into_server_fn_error())?
		.into_iter()
		.find(|action| action.name == request.action)
	{
		Some(action) => action,
		None => {
			audit::log_action(
				user_id,
				model_admin.model_name(),
				&request.ids,
				&request.action,
				0,
				false,
			);
			return Err(ServerFnError::server(400, "Unknown admin action"));
		}
	};

	if request.ids.is_empty() {
		audit::log_action(
			user_id,
			model_admin.model_name(),
			&request.ids,
			&action.name,
			0,
			false,
		);
		return Err(ServerFnError::server(400, "Select at least one record"));
	}
	if request.ids.len() > MAX_BULK_DELETE_IDS {
		audit::log_action(
			user_id,
			model_admin.model_name(),
			&request.ids,
			&action.name,
			0,
			false,
		);
		return Err(ServerFnError::server(400, "Too many records selected"));
	}
	let canonical_ids = request
		.ids
		.iter()
		.map(|id| {
			canonicalize_admin_primary_key(model_admin.table_name(), model_admin.pk_field(), id)
		})
		.collect::<Result<Vec<_>, _>>();
	let canonical_ids = match canonical_ids {
		Ok(ids) => ids.into_iter().map(|(id, _)| id).collect::<Vec<_>>(),
		Err(error) => {
			audit::log_action(
				user_id,
				model_admin.model_name(),
				&request.ids,
				&action.name,
				0,
				false,
			);
			return Err(error.into_server_fn_error());
		}
	};
	if canonical_ids.iter().collect::<HashSet<_>>().len() != canonical_ids.len() {
		audit::log_action(
			user_id,
			model_admin.model_name(),
			&request.ids,
			&action.name,
			0,
			false,
		);
		return Err(ServerFnError::application(
			"Duplicate record IDs are not allowed",
		));
	}

	if let Err(error) = auth
		.require_model_permission(model_admin.as_ref(), user.as_ref(), action.permission)
		.await
	{
		audit::log_action(
			user_id,
			model_admin.model_name(),
			&request.ids,
			&action.name,
			0,
			false,
		);
		return Err(error);
	}
	let request_context = AdminRequestContext::new(http_request.into_inner());
	let admin_query = model_admin
		.get_queryset(
			user.as_ref(),
			&request_context,
			AdminQuery::new(model_admin.table_name()),
		)
		.await
		.map_err(|error| error.into_server_fn_error())?;

	let canonical_model_name = model_admin.model_name().to_string();
	let table_name = model_admin.table_name().to_string();
	let pk_field = model_admin.pk_field().to_string();
	let actor = user.get_username().to_string();
	let connection = *db.connection();
	let result: Result<_, AdminError> = async {
		connection
			.atomic_write(async |transaction| {
				for id in &canonical_ids {
					if db
						.get_admin_query_with_executor_for_update(
							transaction,
							&admin_query,
							&pk_field,
							id,
						)
						.await?
						.is_none()
					{
						return Err(AdminError::ValidationError(
							"One or more selected records are unavailable".to_string(),
						));
					}
				}
				let outcome = model_admin
					.execute_action(&action.name, &canonical_ids, transaction, user.as_ref())
					.await?;
				let mut successful_objects = HashSet::new();
				let mut successful_ids = Vec::with_capacity(outcome.successful_ids.len());
				for successful_id in &outcome.successful_ids {
					let (object_id, _) =
						canonicalize_admin_primary_key(&table_name, &pk_field, successful_id)?;
					if !successful_objects.insert(object_id.clone()) {
						return Err(AdminError::ValidationError(format!(
							"Action returned duplicate successful ID '{object_id}'"
						)));
					}
					successful_ids.push(object_id.clone());
					let event = audit::new_history_event(
						&actor,
						&action.name,
						&canonical_model_name,
						&table_name,
						&object_id,
						Vec::new(),
						1,
					);
					insert_history_event(transaction, &event).await?;
				}
				Ok((outcome, successful_ids))
			})
			.await
	}
	.await;

	match result {
		Ok((outcome, successful_ids)) => {
			audit::log_action(
				user_id,
				model_admin.model_name(),
				&successful_ids,
				&action.name,
				outcome.affected,
				true,
			);
			Ok(MutationResponse {
				success: true,
				message: "Action completed successfully".to_string(),
				affected: Some(outcome.affected),
				data: None,
			})
		}
		Err(error) => {
			audit::log_action(
				user_id,
				model_admin.model_name(),
				&request.ids,
				&action.name,
				0,
				false,
			);
			Err(error.into_server_fn_error())
		}
	}
}
