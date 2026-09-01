//! Field definitions Server Function
//!
//! Provides field information for dynamic form generation.

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
use crate::adapters::{AdminDatabase, AdminSite, FieldInfo, FieldType};
#[cfg(server)]
use crate::core::inline::MAX_INLINE_ROWS;
#[cfg(server)]
use crate::core::{
	AdminDatabaseKey, AdminQuery, AdminRequestContext, AdminSiteKey, InlineModelAdmin,
};
use crate::types::{AdminError, FieldsResponse, ManyToManyLookupResponse};
#[cfg(server)]
use crate::types::{InlineFormInfo, InlineRowInfo};
#[cfg(server)]
use reinhardt_di::KeyedDepends;
#[cfg(server)]
use reinhardt_pages::server_fn::ServerFnRequest;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};

#[cfg(server)]
use super::error::{AdminAuth, MapServerFnError, ModelPermission};
#[cfg(server)]
use super::inline::map_inline_mutation_error;
#[cfg(server)]
use super::limits::RELATION_LOOKUP_PAGE_SIZE;
#[cfg(server)]
use super::relation::{current_relation_options, relation_options_with_executor, resolve_relation};
#[cfg(server)]
use super::validation::retain_allowed_fields;
#[cfg(server)]
use crate::server::form::resolve_admin_form;

#[cfg(server)]
use crate::server::relation::{
	relation_id_from_value, resolve_relation_configuration, resolve_relation_option,
};
#[cfg(server)]
use crate::server::type_inference::{
	get_field_metadata, infer_admin_field_type_from_metadata, infer_required,
	translate_physical_field_names_to_logical,
};
#[cfg(server)]
use reinhardt_utils::utils_core::text::humanize_field_name;
#[cfg(server)]
use std::collections::HashSet;

#[cfg(server)]
fn consume_prefetched_relation_page(
	lookup: &mut ManyToManyLookupResponse,
	next: ManyToManyLookupResponse,
	selected_ids: &HashSet<&str>,
) {
	let remaining = RELATION_LOOKUP_PAGE_SIZE as usize - lookup.options.len();
	let mut options = next
		.options
		.into_iter()
		.filter(|option| !selected_ids.contains(option.id.as_str()));
	lookup.options.extend(options.by_ref().take(remaining));
	let has_unconsumed_options = options.next().is_some();
	lookup.has_more = next.has_more || has_unconsumed_options;
	lookup.page = if has_unconsumed_options {
		lookup.page
	} else {
		next.page
	};
}

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
	let request_context = AdminRequestContext::new(http_request.into_inner());
	let admin_query = model_admin
		.get_queryset(
			user.as_ref(),
			&request_context,
			AdminQuery::new(model_admin.table_name()),
		)
		.await
		.map_server_fn_error()?;
	let mut form = resolve_admin_form(&site, model_admin.as_ref()).map_server_fn_error()?;
	let table_name = model_admin.table_name();
	let relations =
		resolve_relation_configuration(&site, model_admin.as_ref()).map_server_fn_error()?;

	// Fetch existing values before resolving edit-form relation labels.
	let values = if let Some(id) = id.as_deref() {
		let mut values = db
			.get_admin_query(&admin_query, model_admin.pk_field(), id)
			.await
			.map_server_fn_error()?;
		if let Some(values) = values.as_mut() {
			translate_physical_field_names_to_logical(table_name, values).map_server_fn_error()?;
			let mut allowed_fields = form
				.fields
				.iter()
				.map(|field| field.name.clone())
				.collect::<Vec<_>>();
			allowed_fields.extend(
				form.aliases
					.iter()
					.filter(|&(_, physical)| {
						form.fields.iter().any(|field| field.name == *physical)
					})
					.map(|(logical, _)| logical.clone()),
			);
			retain_allowed_fields(values, &allowed_fields);
		}
		values
	} else {
		None
	};
	if id.is_some() && values.is_none() {
		return Err(ServerFnError::server(404, "Object not found"));
	}

	let mut connection = *db.connection();
	for field in &mut form.fields {
		if matches!(field.field_type, FieldType::ManyToManySelector { .. }) {
			let descriptor =
				resolve_relation(&site, model_admin.as_ref(), &field.name).map_server_fn_error()?;
			auth.require_model_permission(
				descriptor.target_admin.as_ref(),
				user.as_ref(),
				ModelPermission::View,
			)
			.await?;
			let target_query = descriptor
				.target_admin
				.get_queryset(
					user.as_ref(),
					&request_context,
					AdminQuery::new(descriptor.target_admin.table_name()),
				)
				.await
				.map_server_fn_error()?;
			let mut lookup =
				relation_options_with_executor(&descriptor, &target_query, "", 1, &mut connection)
					.await
					.map_server_fn_error()?;
			let selected = if let Some(source_id) = id.as_deref() {
				current_relation_options(&descriptor, &target_query, source_id, &mut connection)
					.await
					.map_server_fn_error()?
			} else {
				Vec::new()
			};
			let selected_ids = selected
				.iter()
				.map(|option| option.id.as_str())
				.collect::<HashSet<_>>();
			lookup
				.options
				.retain(|option| !selected_ids.contains(option.id.as_str()));
			let mut page = lookup.page.saturating_add(1);
			while lookup.options.len() < RELATION_LOOKUP_PAGE_SIZE as usize && lookup.has_more {
				let next = relation_options_with_executor(
					&descriptor,
					&target_query,
					"",
					page,
					&mut connection,
				)
				.await
				.map_server_fn_error()?;
				consume_prefetched_relation_page(&mut lookup, next, &selected_ids);
				page = lookup.page.saturating_add(1);
			}
			if let FieldType::ManyToManySelector {
				available,
				selected: current_selected,
				page: current_page,
				has_more,
				..
			} = &mut field.field_type
			{
				*available = lookup.options;
				*current_selected = selected;
				*current_page = lookup.page;
				*has_more = lookup.has_more;
			}
			continue;
		}
		let FieldType::Relation { field_name, .. } = &field.field_type else {
			continue;
		};
		if let Some(relation) = relations.iter().find(|relation| {
			relation.foreign_key.logical_name == *field_name
				|| relation.foreign_key.column_name == *field_name
		}) {
			let selected = match values.as_ref().and_then(|record| {
				record
					.get(&relation.foreign_key.logical_name)
					.or_else(|| record.get(&relation.foreign_key.column_name))
			}) {
				Some(value) => match relation_id_from_value(value).map_server_fn_error()? {
					Some(id) => Some(
						resolve_relation_option(
							&auth,
							user.as_ref(),
							&request_context,
							&db,
							relation,
							&id,
						)
						.await?,
					),
					None => None,
				},
				None => None,
			};
			if let FieldType::Relation {
				selected: current_selected,
				..
			} = &mut field.field_type
			{
				*current_selected = selected;
			}
		}
	}

	let inline_configs = model_admin.inlines();
	InlineModelAdmin::validate_resolved(&inline_configs).map_server_fn_error()?;
	let mut inlines = Vec::with_capacity(inline_configs.len());
	let mut remaining_loaded_rows = MAX_INLINE_ROWS;
	for inline in inline_configs {
		let child_admin = site
			.get_model_admin_by_table_name(inline.adapter().table_name())
			.map_server_fn_error()?;
		if id.is_some() {
			auth.require_model_permission(
				child_admin.as_ref(),
				user.as_ref(),
				ModelPermission::View,
			)
			.await?;
		}
		inline
			.validate_child_table(child_admin.table_name())
			.map_server_fn_error()?;
		let child_query = child_admin
			.get_queryset(
				user.as_ref(),
				&request_context,
				AdminQuery::new(child_admin.table_name()),
			)
			.await
			.map_server_fn_error()?;
		let can_add = child_admin.has_add_permission(user.as_ref()).await;
		let can_change = child_admin.has_change_permission(user.as_ref()).await;
		let can_delete =
			inline.delete_enabled() && child_admin.has_delete_permission(user.as_ref()).await;
		let child_readonly_fields = child_admin.readonly_fields();
		let inline_fields = inline
			.fields()
			.iter()
			.map(|name| {
				let metadata =
					get_field_metadata(child_admin.table_name(), name).ok_or_else(|| {
						AdminError::ValidationError(format!(
							"Inline field '{}' is not registered for model '{}'",
							name,
							inline.child_model()
						))
					})?;
				Ok(FieldInfo {
					name: name.clone(),
					label: humanize_field_name(name),
					field_type: infer_admin_field_type_from_metadata(&metadata),
					required: infer_required(&metadata),
					nullable: metadata.nullable,
					readonly: child_readonly_fields.contains(&name.as_str()),
					help_text: None,
					placeholder: None,
				})
			})
			.collect::<Result<Vec<_>, AdminError>>()
			.map_server_fn_error()?;
		let extra_row_count = if can_add { inline.extra_rows() } else { 0 };
		let available_loaded_rows = remaining_loaded_rows
			.checked_sub(extra_row_count)
			.ok_or_else(|| ServerFnError::application("Inline forms exceed 100 total rows"))?;
		let loaded_rows = if let Some(parent_id) = id.as_deref() {
			inline
				.adapter()
				.load_rows(
					parent_id,
					available_loaded_rows + 1,
					Some(&child_query),
					&mut connection,
				)
				.await
				.map_err(map_inline_mutation_error)?
		} else {
			Vec::new()
		};
		let mut rows = loaded_rows;
		if rows.len() > available_loaded_rows {
			return Err(ServerFnError::application(
				"Inline forms exceed 100 total rows",
			));
		}
		remaining_loaded_rows -= rows.len() + extra_row_count;
		rows.extend((0..extra_row_count).map(|_| InlineRowInfo {
			id: None,
			values: Default::default(),
		}));
		inlines.push(InlineFormInfo {
			key: inline.key().to_owned(),
			model_name: inline.child_model().to_owned(),
			style: inline.style_value(),
			fields: inline_fields,
			rows,
			can_change,
			can_delete,
		});
	}

	Ok(FieldsResponse {
		model_name,
		fields: form.fields,
		fieldsets: form.fieldsets,
		inlines,
		prepopulated_fields: form.prepopulated_fields,
		values,
	})
}

#[cfg(all(test, server))]
mod tests {
	use super::*;
	use crate::core::{
		AdminDatabaseKey, AdminSiteKey, AdminUser, Fieldset, InlineModelAdmin, InlineStyle,
		ModelAdminConfig,
	};
	use crate::server::AdminAuthenticatedUser;
	use crate::types::{
		AdminWidget, FormFieldOverride, InlineRowInfo, PrepopulatedField, RelationSelectorLayout,
	};
	use reinhardt_db::associations::ForeignKeyField;
	use reinhardt_db::backends::DatabaseConnection as BackendsConnection;
	use reinhardt_db::migrations::{
		FieldMetadata, FieldType as DbFieldType, ManyToManyMetadata, ModelMetadata, global_registry,
	};
	use reinhardt_db::orm::DatabaseConnectionLease;
	use reinhardt_di::KeyedDepends;
	use reinhardt_http::AuthState;
	use reinhardt_macros::model;
	use reinhardt_pages::server_fn::{ServerFnErrorKind, ServerFnRequest};
	use rstest::{fixture, rstest};
	use serde::{Deserialize, Serialize};
	use serde_json::json;
	use serial_test::serial;
	use std::collections::HashMap;
	use std::sync::Arc;

	#[model(
		app_label = "admin",
		table_name = "fields_inline_parents",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	struct Parent {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[field(max_length = 100)]
		name: String,
	}

	#[model(
		app_label = "admin",
		table_name = "fields_inline_children",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	struct Child {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[rel(foreign_key, related_name = "children")]
		parent: ForeignKeyField<Parent>,
		#[field(max_length = 100)]
		display_name: String,
		#[field]
		position: i64,
	}

	#[model(
		app_label = "admin",
		table_name = "fields_inline_notes",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	struct Note {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[rel(foreign_key, related_name = "notes")]
		parent: ForeignKeyField<Parent>,
		#[field(max_length = 500)]
		body: String,
	}

	struct TestAdminUser;

	impl AdminUser for TestAdminUser {
		fn is_active(&self) -> bool {
			true
		}

		fn is_staff(&self) -> bool {
			true
		}

		fn is_superuser(&self) -> bool {
			false
		}

		fn get_username(&self) -> &str {
			"fields-test-user"
		}
	}

	struct FieldsContext {
		site: KeyedDepends<AdminSiteKey, AdminSite>,
		db: KeyedDepends<AdminDatabaseKey, AdminDatabase>,
		_lease: DatabaseConnectionLease,
	}

	fn configured_inlines() -> Vec<InlineModelAdmin> {
		vec![
			InlineModelAdmin::new::<Parent, Note>("Note", "parent_id", &["body"])
				.unwrap()
				.style(InlineStyle::Stacked)
				.extra(1),
			InlineModelAdmin::new::<Parent, Child>(
				"Child",
				"parent_id",
				&["position", "display_name"],
			)
			.unwrap()
			.style(InlineStyle::Tabular)
			.extra(2)
			.can_delete(true),
		]
	}

	fn register_field_metadata() {
		let mut parent = ModelMetadata::new("admin", "Parent", "fields_inline_parents");
		parent
			.fields
			.insert("id".to_owned(), FieldMetadata::new(DbFieldType::Integer));
		parent.fields.insert(
			"name".to_owned(),
			FieldMetadata::new(DbFieldType::VarChar(100)),
		);
		global_registry().register_model(parent);

		let mut child = ModelMetadata::new("admin", "Child", "fields_inline_children");
		child.fields.insert(
			"parent_id".to_owned(),
			FieldMetadata::new(DbFieldType::Integer)
				.with_param("fk_target", "Parent")
				.with_param("fk_target_app", "admin"),
		);
		child.fields.insert(
			"display_name".to_owned(),
			FieldMetadata::new(DbFieldType::VarChar(100)),
		);
		child.fields.insert(
			"position".to_owned(),
			FieldMetadata::new(DbFieldType::BigInteger),
		);
		global_registry().register_model(child);

		let mut note = ModelMetadata::new("admin", "Note", "fields_inline_notes");
		note.fields.insert(
			"body".to_owned(),
			FieldMetadata::new(DbFieldType::VarChar(500)),
		);
		global_registry().register_model(note);
	}

	async fn fields_context_with(
		inlines: Vec<InlineModelAdmin>,
		register_children: bool,
		allow_children: bool,
		child_fields: Option<Vec<&str>>,
	) -> FieldsContext {
		register_field_metadata();
		let owner = BackendsConnection::connect_sqlite("sqlite::memory:")
			.await
			.unwrap();
		let lease = DatabaseConnectionLease::register(owner).unwrap();
		let connection = lease.handle();
		for statement in [
			"CREATE TABLE fields_inline_parents (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
			"CREATE TABLE fields_inline_children (id INTEGER PRIMARY KEY AUTOINCREMENT, parent_id INTEGER NOT NULL, display_name TEXT NOT NULL, position BIGINT NOT NULL)",
			"CREATE TABLE fields_inline_notes (id INTEGER PRIMARY KEY AUTOINCREMENT, parent_id INTEGER NOT NULL, body TEXT NOT NULL)",
		] {
			connection.execute(statement, vec![]).await.unwrap();
		}
		for statement in [
			"INSERT INTO fields_inline_parents (id, name) VALUES (1, 'first'), (2, 'second')",
			"INSERT INTO fields_inline_children (id, parent_id, display_name, position) VALUES (10, 1, 'owned child', 7), (11, 2, 'other child', 8)",
			"INSERT INTO fields_inline_notes (id, parent_id, body) VALUES (20, 1, 'owned note'), (21, 2, 'other note')",
		] {
			connection.execute(statement, vec![]).await.unwrap();
		}

		let site = AdminSite::new("Inline fields test");
		let parent_admin = ModelAdminConfig::builder()
			.model_name("Parent")
			.table_name("fields_inline_parents")
			.fieldsets(vec![Fieldset::new(Some("Main"), &["name"])])
			.search_fields(vec!["name"])
			.inlines(inlines)
			.allow_all(true)
			.build()
			.unwrap();
		site.register("Parent", parent_admin).unwrap();
		if register_children {
			let mut child_builder = ModelAdminConfig::builder()
				.model_name("Child")
				.table_name("fields_inline_children")
				.readonly_fields(vec!["position"])
				.allow_all(allow_children);
			if let Some(fields) = child_fields {
				child_builder = child_builder
					.fields(fields)
					.autocomplete_fields(vec!["parent"])
					.formfield_overrides(vec![FormFieldOverride::new("parent").label("Owner")]);
			} else {
				child_builder = child_builder.fields(vec!["id", "display_name"]);
			}
			let child_admin = child_builder.build().unwrap();
			site.register("Child", child_admin).unwrap();
			let note_admin = ModelAdminConfig::builder()
				.model_name("Note")
				.table_name("fields_inline_notes")
				.fields(vec!["body"])
				.allow_all(allow_children)
				.build()
				.unwrap();
			site.register("Note", note_admin).unwrap();
		}

		FieldsContext {
			site: KeyedDepends::from_value(site),
			db: KeyedDepends::from_value(AdminDatabase::new(connection)),
			_lease: lease,
		}
	}

	async fn customized_fields_context() -> FieldsContext {
		let mut model = ModelMetadata::new(
			"admin",
			"FieldsCustomizedParent",
			"fields_customized_parents",
		);
		for field in ["title", "slug", "seo_slug"] {
			model.fields.insert(
				field.to_owned(),
				FieldMetadata::new(DbFieldType::VarChar(200)),
			);
		}
		global_registry().register_model(model);

		let owner = BackendsConnection::connect_sqlite("sqlite::memory:")
			.await
			.unwrap();
		let lease = DatabaseConnectionLease::register(owner).unwrap();
		let connection = lease.handle();
		connection
			.execute(
				"CREATE TABLE fields_customized_parents (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, slug TEXT NOT NULL, seo_slug TEXT NOT NULL)",
				vec![],
			)
			.await
			.unwrap();

		let site = AdminSite::new("Customized fields test");
		let parent_admin = ModelAdminConfig::builder()
			.model_name("FieldsCustomizedParent")
			.table_name("fields_customized_parents")
			.fields(vec!["title", "slug", "seo_slug"])
			.formfield_overrides(vec![
				FormFieldOverride::new("title")
					.label("Headline")
					.help_text("Shown in the page title")
					.placeholder("Write a headline")
					.required(true),
				FormFieldOverride::new("slug").widget(AdminWidget::TextArea { rows: Some(7) }),
			])
			.prepopulated_fields(vec![
				PrepopulatedField::new("seo_slug", ["slug"]),
				PrepopulatedField::new("slug", ["title"]),
			])
			.allow_all(true)
			.build()
			.unwrap();
		site.register("FieldsCustomizedParent", parent_admin)
			.unwrap();

		FieldsContext {
			site: KeyedDepends::from_value(site),
			db: KeyedDepends::from_value(AdminDatabase::new(connection)),
			_lease: lease,
		}
	}

	async fn many_to_many_fields_context(target_view: bool) -> FieldsContext {
		let mut source = ModelMetadata::new("admin", "FieldsM2mSource", "fields_m2m_sources");
		source
			.fields
			.insert("id".to_owned(), FieldMetadata::new(DbFieldType::Integer));
		source.fields.insert(
			"title".to_owned(),
			FieldMetadata::new(DbFieldType::VarChar(200)),
		);
		source.add_many_to_many(ManyToManyMetadata::new("tags", "FieldsM2mTarget"));
		global_registry().register_model(source);

		let mut target = ModelMetadata::new("admin", "FieldsM2mTarget", "fields_m2m_targets");
		target
			.fields
			.insert("id".to_owned(), FieldMetadata::new(DbFieldType::Integer));
		target.fields.insert(
			"name".to_owned(),
			FieldMetadata::new(DbFieldType::VarChar(100)),
		);
		global_registry().register_model(target);

		let owner = BackendsConnection::connect_sqlite("sqlite::memory:")
			.await
			.unwrap();
		let lease = DatabaseConnectionLease::register(owner).unwrap();
		let connection = lease.handle();
		for statement in [
			"CREATE TABLE fields_m2m_sources (id INTEGER PRIMARY KEY, title TEXT NOT NULL)",
			"CREATE TABLE fields_m2m_targets (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
			"CREATE TABLE fields_m2m_sources_tags (fields_m2m_sources_id INTEGER NOT NULL, fields_m2m_targets_id INTEGER NOT NULL)",
			"INSERT INTO fields_m2m_sources (id, title) VALUES (1, 'Selectors')",
			"INSERT INTO fields_m2m_targets (id, name) VALUES (1, 'Tag 001'), (2, 'Tag 002'), (3, 'Tag 003')",
			"INSERT INTO fields_m2m_sources_tags (fields_m2m_sources_id, fields_m2m_targets_id) VALUES (1, 2), (1, 3)",
		] {
			connection.execute(statement, vec![]).await.unwrap();
		}

		let site = AdminSite::new("Many-to-many fields test");
		let source_admin = ModelAdminConfig::builder()
			.model_name("FieldsM2mSource")
			.table_name("fields_m2m_sources")
			.fields(vec!["id", "title"])
			.filter_horizontal(vec!["tags"])
			.formfield_overrides(vec![
				FormFieldOverride::new("tags")
					.widget(AdminWidget::ManyToMany {
						layout: RelationSelectorLayout::Vertical,
					})
					.required(true),
			])
			.allow_all(true)
			.build()
			.unwrap();
		site.register("FieldsM2mSource", source_admin).unwrap();
		let target_admin = ModelAdminConfig::builder()
			.model_name("FieldsM2mTarget")
			.table_name("fields_m2m_targets")
			.fields(vec!["id", "name"])
			.search_fields(vec!["name"])
			.allow_all(target_view)
			.build()
			.unwrap();
		site.register("FieldsM2mTarget", target_admin).unwrap();

		FieldsContext {
			site: KeyedDepends::from_value(site),
			db: KeyedDepends::from_value(AdminDatabase::new(connection)),
			_lease: lease,
		}
	}

	#[fixture]
	async fn inline_fields_context() -> FieldsContext {
		fields_context_with(configured_inlines(), true, true, None).await
	}

	#[fixture]
	async fn unregistered_inline_context() -> FieldsContext {
		let inline =
			InlineModelAdmin::new::<Parent, Child>("Child", "parent_id", &["display_name"])
				.unwrap()
				.extra(0);
		fields_context_with(vec![inline], false, false, None).await
	}

	fn request() -> ServerFnRequest {
		let request = reinhardt_http::Request::builder()
			.uri("/admin/parent")
			.build()
			.unwrap();
		request
			.extensions
			.insert(AuthState::authenticated("fields-test-user", true, true));
		ServerFnRequest(Arc::new(request))
	}

	fn user() -> AdminAuthenticatedUser {
		AdminAuthenticatedUser(Arc::new(TestAdminUser))
	}

	fn blank_rows(count: usize) -> Vec<InlineRowInfo> {
		vec![
			InlineRowInfo {
				id: None,
				values: HashMap::new(),
			};
			count
		]
	}

	#[rstest]
	#[serial(admin_inline_fields)]
	#[tokio::test]
	async fn create_response_preserves_parent_fieldsets_and_exact_inline_configuration(
		#[future] inline_fields_context: FieldsContext,
	) {
		// Arrange
		let FieldsContext {
			site, db, _lease, ..
		} = inline_fields_context.await;

		// Act
		let response = get_fields("Parent".to_owned(), None, site, db, request(), user())
			.await
			.unwrap();

		// Assert
		assert_eq!(
			response.fieldsets,
			Some(vec![Fieldset::new(Some("Main"), &["name"])])
		);
		assert_eq!(response.values, None);
		assert!(response.prepopulated_fields.is_empty());
		assert_eq!(response.inlines.len(), 2);

		let note = &response.inlines[0];
		assert_eq!(note.key, "fields_inline_notes-parent_id");
		assert_eq!(note.model_name, "Note");
		assert_eq!(note.style, InlineStyle::Stacked);
		assert!(!note.can_delete);
		assert_eq!(note.fields.len(), 1);
		assert_eq!(note.fields[0].name, "body");
		assert_eq!(note.fields[0].label, "Body");
		assert_eq!(note.fields[0].field_type, FieldType::Text);
		assert!(note.fields[0].required);
		assert!(!note.fields[0].readonly);
		assert_eq!(note.rows, blank_rows(1));

		let child = &response.inlines[1];
		assert_eq!(child.key, "fields_inline_children-parent_id");
		assert_eq!(child.model_name, "Child");
		assert_eq!(child.style, InlineStyle::Tabular);
		assert!(child.can_delete);
		assert_eq!(
			child
				.fields
				.iter()
				.map(|field| field.name.as_str())
				.collect::<Vec<_>>(),
			vec!["position", "display_name"]
		);
		assert_eq!(child.fields[0].label, "Position");
		assert_eq!(child.fields[0].field_type, FieldType::Number);
		assert!(child.fields[0].required);
		assert!(child.fields[0].readonly);
		assert_eq!(child.fields[1].label, "Display Name");
		assert_eq!(child.fields[1].field_type, FieldType::Text);
		assert!(child.fields[1].required);
		assert!(!child.fields[1].readonly);
		assert_eq!(child.rows, blank_rows(2));
	}

	#[rstest]
	#[serial(admin_customized_fields)]
	#[tokio::test]
	async fn response_returns_resolved_form_metadata_and_ordered_prepopulation_rules() {
		// Arrange
		let FieldsContext {
			site, db, _lease, ..
		} = customized_fields_context().await;

		// Act
		let response = get_fields(
			"FieldsCustomizedParent".to_owned(),
			None,
			site,
			db,
			request(),
			user(),
		)
		.await
		.unwrap();

		// Assert
		assert_eq!(
			response
				.fields
				.iter()
				.map(|field| field.name.as_str())
				.collect::<Vec<_>>(),
			vec!["title", "slug", "seo_slug"]
		);
		let title = &response.fields[0];
		assert_eq!(title.label, "Headline");
		assert_eq!(title.help_text.as_deref(), Some("Shown in the page title"));
		assert_eq!(title.placeholder.as_deref(), Some("Write a headline"));
		assert!(title.required);
		assert_eq!(
			response.fields[1].field_type,
			FieldType::TextAreaWithRows { rows: Some(7) }
		);
		assert_eq!(
			response.prepopulated_fields,
			vec![
				PrepopulatedField::new("slug", ["title"]),
				PrepopulatedField::new("seo_slug", ["slug"]),
			]
		);
	}

	#[rstest]
	#[serial(admin_inline_fields)]
	#[tokio::test]
	async fn edit_response_keeps_foreign_key_selection_after_presentation_overlay() {
		// Arrange
		let FieldsContext {
			site, db, _lease, ..
		} = fields_context_with(
			Vec::new(),
			true,
			true,
			Some(vec!["id", "parent", "display_name"]),
		)
		.await;

		// Act
		let response = get_fields(
			"Child".to_owned(),
			Some("10".to_owned()),
			site,
			db,
			request(),
			user(),
		)
		.await
		.unwrap();

		// Assert
		let parent = response
			.fields
			.iter()
			.find(|field| field.name == "parent_id")
			.expect("the configured foreign-key field must be returned");
		assert_eq!(parent.label, "Owner");
		let FieldType::Relation {
			selected: Some(selected),
			..
		} = &parent.field_type
		else {
			panic!("the edit response must hydrate the selected foreign-key option")
		};
		assert_eq!(selected, &crate::types::RelationOption::new("1", "1"));
	}

	#[rstest]
	#[serial(admin_inline_fields)]
	#[tokio::test]
	async fn edit_response_keeps_many_to_many_state_after_widget_overlay_and_permission_checks() {
		// Arrange
		let FieldsContext {
			site, db, _lease, ..
		} = many_to_many_fields_context(true).await;

		// Act
		let response = get_fields(
			"FieldsM2mSource".to_owned(),
			Some("1".to_owned()),
			site,
			db,
			request(),
			user(),
		)
		.await
		.unwrap();

		// Assert
		let tags = response
			.fields
			.iter()
			.find(|field| field.name == "tags")
			.expect("the configured many-to-many field must be returned");
		let FieldType::ManyToManySelector {
			layout,
			available,
			selected,
			page,
			has_more,
		} = &tags.field_type
		else {
			panic!("tags must remain a many-to-many selector after overlays")
		};
		assert_eq!(*layout, RelationSelectorLayout::Vertical);
		assert_eq!(
			selected,
			&vec![
				crate::types::RelationOption::new("2", "2"),
				crate::types::RelationOption::new("3", "3"),
			]
		);
		assert_eq!(
			available,
			&vec![crate::types::RelationOption::new("1", "1")]
		);
		assert_eq!(*page, 1);
		assert!(!has_more);

		let FieldsContext {
			site, db, _lease, ..
		} = many_to_many_fields_context(false).await;
		let denied = get_fields(
			"FieldsM2mSource".to_owned(),
			Some("1".to_owned()),
			site,
			db,
			request(),
			user(),
		)
		.await
		.expect_err("target view permission must still guard many-to-many labels");
		assert_eq!(denied.status(), Some(403));
		assert_eq!(denied.user_message(), "Permission denied");
	}

	#[rstest]
	#[serial(admin_inline_fields)]
	#[tokio::test]
	async fn edit_response_loads_only_owned_rows_before_exact_extras(
		#[future] inline_fields_context: FieldsContext,
	) {
		// Arrange
		let FieldsContext {
			site, db, _lease, ..
		} = inline_fields_context.await;

		// Act
		let response = get_fields(
			"Parent".to_owned(),
			Some("1".to_owned()),
			site,
			db,
			request(),
			user(),
		)
		.await
		.unwrap();

		// Assert
		assert_eq!(
			response.values,
			Some(HashMap::from([("name".to_owned(), json!("first"))]))
		);
		assert_eq!(
			response.inlines[0].rows,
			vec![
				InlineRowInfo {
					id: Some("20".to_owned()),
					values: HashMap::from([("body".to_owned(), json!("owned note"))]),
				},
				InlineRowInfo {
					id: None,
					values: HashMap::new(),
				},
			]
		);
		assert_eq!(
			response.inlines[1].rows,
			vec![
				InlineRowInfo {
					id: Some("10".to_owned()),
					values: HashMap::from([
						("position".to_owned(), json!(7)),
						("display_name".to_owned(), json!("owned child")),
					]),
				},
				InlineRowInfo {
					id: None,
					values: HashMap::new(),
				},
				InlineRowInfo {
					id: None,
					values: HashMap::new(),
				},
			]
		);
	}

	#[rstest]
	#[serial(admin_inline_fields)]
	#[tokio::test]
	async fn create_response_rejects_unregistered_child_with_zero_extras(
		#[future] unregistered_inline_context: FieldsContext,
	) {
		// Arrange
		let FieldsContext {
			site, db, _lease, ..
		} = unregistered_inline_context.await;

		// Act
		let error = get_fields("Parent".to_owned(), None, site, db, request(), user())
			.await
			.unwrap_err();

		// Assert
		assert_eq!(error.kind(), ServerFnErrorKind::Server);
		assert_eq!(error.status(), Some(404));
		assert_eq!(error.user_message(), "fields_inline_children");
	}

	#[rstest]
	#[serial(admin_inline_fields)]
	#[tokio::test]
	async fn edit_response_requires_view_permission_for_each_child_admin() {
		let FieldsContext {
			site, db, _lease, ..
		} = fields_context_with(configured_inlines(), true, false, None).await;

		let error = get_fields(
			"Parent".to_owned(),
			Some("1".to_owned()),
			site,
			db,
			request(),
			user(),
		)
		.await
		.unwrap_err();

		assert_eq!(error.kind(), ServerFnErrorKind::Server);
		assert_eq!(error.status(), Some(403));
		assert_eq!(error.user_message(), "Permission denied");
	}

	#[rstest]
	#[serial(admin_inline_fields)]
	#[tokio::test]
	async fn edit_response_rejects_more_than_one_hundred_total_inline_rows() {
		let inline =
			InlineModelAdmin::new::<Parent, Child>("Child", "parent_id", &["display_name"])
				.unwrap()
				.extra(MAX_INLINE_ROWS);
		let FieldsContext {
			site, db, _lease, ..
		} = fields_context_with(vec![inline], true, true, None).await;

		let error = get_fields(
			"Parent".to_owned(),
			Some("1".to_owned()),
			site,
			db,
			request(),
			user(),
		)
		.await
		.unwrap_err();

		assert_eq!(error.kind(), ServerFnErrorKind::Application);
		assert_eq!(error.user_message(), "Inline forms exceed 100 total rows");
	}

	#[test]
	fn relation_prefetch_keeps_cursor_before_a_partially_consumed_page() {
		let mut lookup = ManyToManyLookupResponse {
			options: (0..49)
				.map(|id| crate::types::RelationOption::new(id.to_string(), id.to_string()))
				.collect(),
			page: 1,
			has_more: true,
		};
		let next = ManyToManyLookupResponse {
			options: vec![
				crate::types::RelationOption::new("49", "49"),
				crate::types::RelationOption::new("50", "50"),
			],
			page: 2,
			has_more: false,
		};
		let selected_ids = HashSet::new();

		consume_prefetched_relation_page(&mut lookup, next, &selected_ids);

		assert_eq!(lookup.options.len(), 50);
		assert_eq!(lookup.page, 1);
		assert!(lookup.has_more);
	}
}
