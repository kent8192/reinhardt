//! Permission-aware foreign-key relation lookups.

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
#[cfg(server)]
use super::error::{AdminAuth, IntoServerFnError, MapServerFnError, ModelPermission};
#[cfg(server)]
use super::limits::{
	DEFAULT_RELATION_PAGE_SIZE, MAX_RELATION_PAGE, MAX_RELATION_PAGE_SIZE,
	MAX_RELATION_QUERY_LENGTH,
};
use crate::adapters::{
	AdminDatabase, AdminRecord, AdminSite, RelationLookupRequest, RelationLookupResponse,
	RelationOption,
};
#[cfg(server)]
use crate::core::{AdminDatabaseKey, AdminSiteKey, AdminUser, ModelAdmin};
#[cfg(server)]
use crate::server::type_inference::{
	ForeignKeyFieldMetadata, find_model_by_table_name, resolve_foreign_key_field_metadata,
};
#[cfg(server)]
use crate::types::{AdminError, AdminResult, RelationWidget};
#[cfg(server)]
use reinhardt_apps::{RelationshipMetadata, get_relationships_for_model};
#[cfg(server)]
use reinhardt_db::migrations::{ModelMetadata, ModelRegistry, global_registry};
#[cfg(server)]
use reinhardt_db::orm::{Filter, FilterCondition, FilterOperator, FilterValue};
#[cfg(server)]
use reinhardt_di::KeyedDepends;
#[cfg(server)]
use reinhardt_pages::server_fn::ServerFnRequest;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};
#[cfg(server)]
use std::collections::HashMap;
#[cfg(server)]
use std::sync::Arc;

#[cfg(server)]
pub(crate) struct ResolvedRelationField {
	pub(crate) foreign_key: ForeignKeyFieldMetadata,
	pub(crate) widget: RelationWidget,
	pub(crate) target_admin: Arc<dyn ModelAdmin>,
}

#[cfg(server)]
pub(crate) fn validate_relation_configuration(
	site: &AdminSite,
	source_admin: &Arc<dyn ModelAdmin>,
	source_model: &ModelMetadata,
	relationships: &[&RelationshipMetadata],
	registry: &ModelRegistry,
) -> AdminResult<Vec<ResolvedRelationField>> {
	let configured_fields = source_admin
		.autocomplete_fields()
		.into_iter()
		.map(|field| (field, RelationWidget::Autocomplete))
		.chain(
			source_admin
				.raw_id_fields()
				.into_iter()
				.map(|field| (field, RelationWidget::RawId)),
		);
	let mut seen_columns = HashMap::new();
	let mut resolved_fields = Vec::new();

	for (configured_name, widget) in configured_fields {
		let foreign_key = resolve_foreign_key_field_metadata(
			source_model,
			configured_name,
			relationships,
			registry,
		)?;
		if let Some(previous_name) =
			seen_columns.insert(foreign_key.column_name.clone(), configured_name)
		{
			return Err(AdminError::ValidationError(format!(
				"Relation fields '{}' and '{}' both resolve to column '{}'",
				previous_name, configured_name, foreign_key.column_name
			)));
		}

		let target_name = foreign_key.target_model.model_name.as_str();
		let target_admin = site.get_model_admin(target_name).map_err(|_| {
			AdminError::ValidationError(format!(
				"Related admin '{}' for field '{}' is not registered",
				target_name, foreign_key.logical_name
			))
		})?;
		if target_admin.table_name() != foreign_key.target_model.table_name {
			return Err(AdminError::ValidationError(format!(
				"Related admin '{}' uses table '{}', expected '{}'",
				target_name,
				target_admin.table_name(),
				foreign_key.target_model.table_name
			)));
		}
		if widget == RelationWidget::Autocomplete && target_admin.search_fields().is_empty() {
			return Err(AdminError::ValidationError(format!(
				"Related admin '{}' for field '{}' must configure search_fields for autocomplete",
				target_name, foreign_key.logical_name
			)));
		}

		resolved_fields.push(ResolvedRelationField {
			foreign_key,
			widget,
			target_admin,
		});
	}

	Ok(resolved_fields)
}

#[cfg(server)]
pub(crate) fn resolve_relation_configuration(
	site: &AdminSite,
	source_admin: &Arc<dyn ModelAdmin>,
) -> AdminResult<Vec<ResolvedRelationField>> {
	if source_admin.autocomplete_fields().is_empty() && source_admin.raw_id_fields().is_empty() {
		return Ok(Vec::new());
	}

	let source_model = find_model_by_table_name(source_admin.table_name()).ok_or_else(|| {
		AdminError::ValidationError(format!(
			"Model metadata for admin '{}' is not registered",
			source_admin.model_name()
		))
	})?;
	let qualified_source_name = format!("{}.{}", source_model.app_label, source_model.model_name);
	let relationships = get_relationships_for_model(&qualified_source_name);

	validate_relation_configuration(
		site,
		source_admin,
		&source_model,
		&relationships,
		global_registry(),
	)
}

#[cfg(server)]
fn find_configured_relation<'a>(
	relations: &'a [ResolvedRelationField],
	field_name: &str,
) -> AdminResult<&'a ResolvedRelationField> {
	relations
		.iter()
		.find(|relation| {
			relation.foreign_key.logical_name == field_name
				|| relation.foreign_key.column_name == field_name
		})
		.ok_or_else(|| {
			AdminError::ValidationError(format!(
				"Field '{field_name}' is not configured as an admin relation"
			))
		})
}

#[cfg(server)]
async fn require_related_view_permission(
	auth: &AdminAuth,
	user: &dyn AdminUser,
	relation: &ResolvedRelationField,
) -> Result<(), ServerFnError> {
	auth.require_model_permission(relation.target_admin.as_ref(), user, ModelPermission::View)
		.await
}

#[cfg(server)]
async fn fetch_related_record(
	db: &AdminDatabase,
	relation: &ResolvedRelationField,
	id: &str,
) -> Result<HashMap<String, serde_json::Value>, ServerFnError> {
	db.get::<AdminRecord>(
		relation.target_admin.table_name(),
		relation.target_admin.pk_field(),
		id,
	)
	.await
	.map_server_fn_error()?
	.ok_or_else(|| {
		AdminError::ValidationError(format!(
			"Related object '{}' with id '{}' does not exist",
			relation.target_admin.model_name(),
			id
		))
	})
	.map_server_fn_error()
}

#[cfg(server)]
pub(crate) fn relation_id_from_value(value: &serde_json::Value) -> AdminResult<Option<String>> {
	match value {
		serde_json::Value::Null => Ok(None),
		serde_json::Value::String(value) => Ok(Some(value.clone())),
		serde_json::Value::Number(value) => Ok(Some(value.to_string())),
		serde_json::Value::Bool(value) => Ok(Some(value.to_string())),
		serde_json::Value::Array(_) | serde_json::Value::Object(_) => Err(
			AdminError::ValidationError("Relation primary keys must be scalar values".to_string()),
		),
	}
}

#[cfg(server)]
fn relation_option_from_record(
	relation: &ResolvedRelationField,
	record: &HashMap<String, serde_json::Value>,
) -> AdminResult<RelationOption> {
	let pk_field = relation.target_admin.pk_field();
	let id = record
		.get(pk_field)
		.ok_or_else(|| {
			AdminError::ValidationError(format!(
				"Related object is missing primary key field '{pk_field}'"
			))
		})
		.and_then(relation_id_from_value)?
		.ok_or_else(|| {
			AdminError::ValidationError(format!(
				"Related object primary key field '{pk_field}' cannot be null"
			))
		})?;
	let label = relation
		.target_admin
		.object_label(record)
		.unwrap_or_else(|| id.clone());

	Ok(RelationOption { id, label })
}

#[cfg(server)]
pub(crate) async fn resolve_relation_option(
	auth: &AdminAuth,
	user: &dyn AdminUser,
	db: &AdminDatabase,
	relation: &ResolvedRelationField,
	id: &str,
) -> Result<RelationOption, ServerFnError> {
	require_related_view_permission(auth, user, relation).await?;
	let record = fetch_related_record(db, relation, id).await?;

	relation_option_from_record(relation, &record).map_server_fn_error()
}

#[cfg(server)]
pub(crate) async fn validate_relation_values(
	auth: &AdminAuth,
	user: &dyn AdminUser,
	site: &AdminSite,
	db: &AdminDatabase,
	source_admin: &Arc<dyn ModelAdmin>,
	data: &mut HashMap<String, serde_json::Value>,
) -> Result<(), ServerFnError> {
	let relations = resolve_relation_configuration(site, source_admin).map_server_fn_error()?;

	for relation in &relations {
		let logical_name = relation.foreign_key.logical_name.as_str();
		let column_name = relation.foreign_key.column_name.as_str();
		let value = if logical_name == column_name {
			data.remove(column_name)
		} else {
			match (data.remove(logical_name), data.remove(column_name)) {
				(Some(_), Some(_)) => {
					return Err(AdminError::ValidationError(format!(
						"Relation field '{logical_name}' was submitted using both '{logical_name}' and '{column_name}'"
					))
					.into_server_fn_error());
				}
				(Some(value), None) | (None, Some(value)) => Some(value),
				(None, None) => None,
			}
		};
		let Some(value) = value else {
			continue;
		};

		require_related_view_permission(auth, user, relation).await?;
		let normalized = match relation_id_from_value(&value).map_server_fn_error()? {
			None if relation.foreign_key.field_metadata.is_nullable() => serde_json::Value::Null,
			None => {
				return Err(AdminError::ValidationError(format!(
					"Relation field '{logical_name}' cannot be null"
				))
				.into_server_fn_error());
			}
			Some(id) => {
				let record = fetch_related_record(db, relation, &id).await?;
				let pk_field = relation.target_admin.pk_field();
				let pk_value = record
					.get(pk_field)
					.cloned()
					.ok_or_else(|| {
						AdminError::ValidationError(format!(
							"Related object is missing primary key field '{pk_field}'"
						))
					})
					.map_server_fn_error()?;
				if relation_id_from_value(&pk_value)
					.map_server_fn_error()?
					.is_none()
				{
					return Err(AdminError::ValidationError(format!(
						"Related object primary key field '{pk_field}' cannot be null"
					))
					.into_server_fn_error());
				}
				pk_value
			}
		};

		data.insert(column_name.to_string(), normalized);
	}

	Ok(())
}

/// Search or resolve related objects for one configured foreign-key field.
#[server_fn]
pub async fn get_relation_options(
	model_name: String,
	field_name: String,
	request: RelationLookupRequest,
	#[inject] site: KeyedDepends<AdminSiteKey, AdminSite>,
	#[inject] db: KeyedDepends<AdminDatabaseKey, AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] AdminAuthenticatedUser(user): AdminAuthenticatedUser,
) -> Result<RelationLookupResponse, ServerFnError> {
	let auth = AdminAuth::from_request(&http_request);
	let source_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	auth.require_model_permission(source_admin.as_ref(), user.as_ref(), ModelPermission::View)
		.await?;

	let relations = resolve_relation_configuration(&site, &source_admin).map_server_fn_error()?;
	let relation = find_configured_relation(&relations, &field_name).map_server_fn_error()?;

	match request {
		RelationLookupRequest::Search {
			query,
			page,
			page_size,
		} => {
			require_related_view_permission(&auth, user.as_ref(), relation).await?;
			if relation.widget != RelationWidget::Autocomplete {
				return Err(AdminError::ValidationError(format!(
					"Field '{}' does not support relation search",
					relation.foreign_key.logical_name
				))
				.into_server_fn_error());
			}
			if query.len() > MAX_RELATION_QUERY_LENGTH {
				return Err(AdminError::ValidationError(format!(
					"Relation query exceeds maximum length of {MAX_RELATION_QUERY_LENGTH} bytes"
				))
				.into_server_fn_error());
			}

			let page = page.unwrap_or(1).max(1);
			if page > MAX_RELATION_PAGE {
				return Err(AdminError::ValidationError(format!(
					"Relation page exceeds maximum of {MAX_RELATION_PAGE}"
				))
				.into_server_fn_error());
			}
			let page_size = page_size
				.unwrap_or(DEFAULT_RELATION_PAGE_SIZE)
				.clamp(1, MAX_RELATION_PAGE_SIZE);
			let offset = (page - 1) * page_size;
			let filter_condition = if query.is_empty() {
				None
			} else {
				Some(FilterCondition::Or(
					relation
						.target_admin
						.search_fields()
						.into_iter()
						.map(|field| {
							FilterCondition::Single(Filter::new(
								field.to_string(),
								FilterOperator::Contains,
								FilterValue::String(query.clone()),
							))
						})
						.collect(),
				))
			};
			let ordering = relation.target_admin.ordering();
			let mut records = db
				.list_with_condition::<AdminRecord>(
					relation.target_admin.table_name(),
					filter_condition.as_ref(),
					Vec::new(),
					ordering.first().copied(),
					offset,
					page_size + 1,
				)
				.await
				.map_server_fn_error()?;
			let has_next = records.len() > page_size as usize;
			records.truncate(page_size as usize);
			let results = records
				.iter()
				.map(|record| relation_option_from_record(relation, record))
				.collect::<AdminResult<Vec<_>>>()
				.map_server_fn_error()?;

			Ok(RelationLookupResponse {
				results,
				page,
				has_next,
			})
		}
		RelationLookupRequest::Resolve { id } => {
			let option = resolve_relation_option(&auth, user.as_ref(), &db, relation, &id).await?;
			Ok(RelationLookupResponse {
				results: vec![option],
				page: 1,
				has_next: false,
			})
		}
	}
}

#[cfg(all(test, server))]
mod tests {
	use super::*;
	use crate::core::{AdminSite, ModelAdmin, ModelAdminConfig};
	use reinhardt_apps::{RelationshipMetadata, RelationshipType};
	use reinhardt_db::migrations::{FieldMetadata, FieldType, ModelMetadata, ModelRegistry};
	use rstest::rstest;
	use std::sync::Arc;

	fn source_metadata() -> ModelMetadata {
		let mut source = ModelMetadata::new(
			"admin_relation_config_source",
			"ResolverSource",
			"resolver_sources",
		);
		source.fields.insert(
			"author_id".to_string(),
			FieldMetadata::new(FieldType::Uuid)
				.with_param("fk_target", "ResolverTarget")
				.with_param("fk_target_app", "admin_relation_config_target"),
		);
		source
	}

	fn relationship() -> RelationshipMetadata {
		RelationshipMetadata::new(
			"admin_relation_config_source.ResolverSource",
			"ResolverTarget",
			RelationshipType::ForeignKey,
			"author",
			None,
			Some("author_id"),
			None,
		)
	}

	fn target_registry() -> ModelRegistry {
		let registry = ModelRegistry::new();
		registry.register_model(ModelMetadata::new(
			"admin_relation_config_target",
			"ResolverTarget",
			"resolver_targets",
		));
		registry
	}

	fn register_source(
		site: &AdminSite,
		autocomplete_fields: Vec<&str>,
		raw_id_fields: Vec<&str>,
	) -> Arc<dyn ModelAdmin> {
		let source = ModelAdminConfig::builder()
			.model_name("ResolverSource")
			.table_name("resolver_sources")
			.autocomplete_fields(autocomplete_fields)
			.raw_id_fields(raw_id_fields)
			.build()
			.expect("source admin should build");
		site.register("ResolverSource", source)
			.expect("source admin should register");
		site.get_model_admin("ResolverSource")
			.expect("source admin should be available")
	}

	fn register_target(site: &AdminSite, table_name: &str, search_fields: Vec<&str>) {
		let target = ModelAdminConfig::builder()
			.model_name("ResolverTarget")
			.table_name(table_name)
			.search_fields(search_fields)
			.build()
			.expect("target admin should build");
		site.register("ResolverTarget", target)
			.expect("target admin should register");
	}

	#[rstest]
	fn relation_configuration_rejects_normalized_duplicates() {
		// Arrange
		let site = AdminSite::new("Relation configuration test");
		let source = register_source(&site, vec!["author"], vec!["author_id"]);
		register_target(&site, "resolver_targets", vec!["name"]);
		let source_metadata = source_metadata();
		let relationship = relationship();
		let relationships = [&relationship];
		let registry = target_registry();

		// Act
		let error = validate_relation_configuration(
			&site,
			&source,
			&source_metadata,
			&relationships,
			&registry,
		)
		.err()
		.expect("logical and physical duplicates must be rejected");

		// Assert
		assert_eq!(
			error.to_string(),
			"Validation error: Relation fields 'author' and 'author_id' both resolve to column 'author_id'"
		);
	}

	#[rstest]
	fn relation_configuration_requires_related_admin() {
		// Arrange
		let site = AdminSite::new("Relation configuration test");
		let source = register_source(&site, vec!["author"], vec![]);
		let source_metadata = source_metadata();
		let relationship = relationship();
		let relationships = [&relationship];
		let registry = target_registry();

		// Act
		let error = validate_relation_configuration(
			&site,
			&source,
			&source_metadata,
			&relationships,
			&registry,
		)
		.err()
		.expect("missing related admin must be rejected");

		// Assert
		assert_eq!(
			error.to_string(),
			"Validation error: Related admin 'ResolverTarget' for field 'author' is not registered"
		);
	}

	#[rstest]
	fn autocomplete_configuration_requires_related_search_fields() {
		// Arrange
		let site = AdminSite::new("Relation configuration test");
		let source = register_source(&site, vec!["author"], vec![]);
		register_target(&site, "resolver_targets", vec![]);
		let source_metadata = source_metadata();
		let relationship = relationship();
		let relationships = [&relationship];
		let registry = target_registry();

		// Act
		let error = validate_relation_configuration(
			&site,
			&source,
			&source_metadata,
			&relationships,
			&registry,
		)
		.err()
		.expect("autocomplete without search fields must be rejected");

		// Assert
		assert_eq!(
			error.to_string(),
			"Validation error: Related admin 'ResolverTarget' for field 'author' must configure search_fields for autocomplete"
		);
	}

	#[rstest]
	fn relation_configuration_rejects_related_admin_table_mismatch() {
		// Arrange
		let site = AdminSite::new("Relation configuration test");
		let source = register_source(&site, vec!["author"], vec![]);
		register_target(&site, "wrong_targets", vec!["name"]);
		let source_metadata = source_metadata();
		let relationship = relationship();
		let relationships = [&relationship];
		let registry = target_registry();

		// Act
		let error = validate_relation_configuration(
			&site,
			&source,
			&source_metadata,
			&relationships,
			&registry,
		)
		.err()
		.expect("related admin table mismatch must be rejected");

		// Assert
		assert_eq!(
			error.to_string(),
			"Validation error: Related admin 'ResolverTarget' uses table 'wrong_targets', expected 'resolver_targets'"
		);
	}
}
