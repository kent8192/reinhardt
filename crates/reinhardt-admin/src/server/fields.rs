//! Field definitions Server Function
//!
//! Provides field information for dynamic form generation.

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
use crate::adapters::{AdminDatabase, AdminRecord, AdminSite, FieldInfo, FieldType};
#[cfg(server)]
use crate::core::inline::MAX_INLINE_ROWS;
#[cfg(server)]
use crate::core::{AdminDatabaseKey, AdminSiteKey, InlineModelAdmin, resolve_form_fields};
use crate::types::{AdminError, FieldsResponse};
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
	let (field_names, fieldsets) =
		resolve_form_fields(model_admin.as_ref()).map_server_fn_error()?;
	let has_fieldsets = fieldsets.is_some();
	let readonly_fields = model_admin.readonly_fields();

	// Build field metadata with type inference from global registry
	let table_name = model_admin.table_name();
	let fields = field_names
		.iter()
		.map(|name| {
			let is_readonly = readonly_fields.contains(&name.as_str());

			// Try to get field metadata from the global model registry
			let metadata = get_field_metadata(table_name, name);
			let (field_type, required) = if has_fieldsets {
				let metadata = metadata.ok_or_else(|| {
					AdminError::ValidationError(format!(
						"Fieldset field '{}' is not registered for model '{}'",
						name, model_name
					))
				})?;
				(
					infer_admin_field_type(&metadata.field_type),
					infer_required(&metadata),
				)
			} else {
				metadata
					.map(|meta| {
						let admin_type = infer_admin_field_type(&meta.field_type);
						let is_required = infer_required(&meta);
						(admin_type, is_required)
					})
					.unwrap_or((FieldType::Text, false))
			};

			Ok(FieldInfo {
				name: name.clone(),
				label: humanize_field_name(name),
				field_type,
				required,
				readonly: is_readonly,
				help_text: None,
				placeholder: None,
			})
		})
		.collect::<Result<Vec<_>, AdminError>>()
		.map_server_fn_error()?;

	let inline_configs = model_admin.inlines();
	InlineModelAdmin::validate_resolved(&inline_configs).map_server_fn_error()?;
	let mut connection = *db.connection();
	let mut inlines = Vec::with_capacity(inline_configs.len());
	let mut remaining_loaded_rows = MAX_INLINE_ROWS
		- inline_configs
			.iter()
			.map(InlineModelAdmin::extra_rows)
			.sum::<usize>();
	for inline in inline_configs {
		let child_admin = site
			.get_model_admin(inline.child_model())
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
		let editing = id.is_some();
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
					field_type: infer_admin_field_type(&metadata.field_type),
					required: infer_required(&metadata),
					readonly: child_readonly_fields.contains(&name.as_str())
						|| (editing && !can_change),
					help_text: None,
					placeholder: None,
				})
			})
			.collect::<Result<Vec<_>, AdminError>>()
			.map_server_fn_error()?;
		let mut rows = if let Some(parent_id) = id.as_deref() {
			inline
				.adapter()
				.load_rows(parent_id, remaining_loaded_rows + 1, &mut connection)
				.await
				.map_err(map_inline_mutation_error)?
		} else {
			Vec::new()
		};
		if rows.len() > remaining_loaded_rows {
			return Err(ServerFnError::application(
				"Inline forms exceed 100 total rows",
			));
		}
		remaining_loaded_rows -= rows.len();
		let extra_row_count = if can_add && (!editing || can_change) {
			inline.extra_rows()
		} else {
			0
		};
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
			can_delete,
		});
	}

	// Fetch existing values if editing
	let values = if let Some(id) = id.as_deref() {
		db.get::<AdminRecord>(model_admin.table_name(), model_admin.pk_field(), id)
			.await
			.map_server_fn_error()?
	} else {
		None
	};

	Ok(FieldsResponse {
		model_name,
		fields,
		fieldsets,
		inlines,
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
	use crate::types::InlineRowInfo;
	use reinhardt_db::associations::ForeignKeyField;
	use reinhardt_db::backends::DatabaseConnection as BackendsConnection;
	use reinhardt_db::migrations::{
		FieldMetadata, FieldType as DbFieldType, ModelMetadata, global_registry,
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
		let mut parent = ModelMetadata::new("admin", "FieldsParent", "fields_inline_parents");
		parent.fields.insert(
			"name".to_owned(),
			FieldMetadata::new(DbFieldType::VarChar(100)),
		);
		global_registry().register_model(parent);

		let mut child = ModelMetadata::new("admin", "FieldsChild", "fields_inline_children");
		child.fields.insert(
			"display_name".to_owned(),
			FieldMetadata::new(DbFieldType::VarChar(100)),
		);
		child.fields.insert(
			"position".to_owned(),
			FieldMetadata::new(DbFieldType::BigInteger),
		);
		global_registry().register_model(child);

		let mut note = ModelMetadata::new("admin", "FieldsNote", "fields_inline_notes");
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
			.inlines(inlines)
			.allow_all(true)
			.build()
			.unwrap();
		site.register("Parent", parent_admin).unwrap();
		if register_children {
			let child_admin = ModelAdminConfig::builder()
				.model_name("Child")
				.table_name("fields_inline_children")
				.fields(vec!["id", "display_name"])
				.readonly_fields(vec!["position"])
				.allow_all(allow_children)
				.build()
				.unwrap();
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

	#[fixture]
	async fn inline_fields_context() -> FieldsContext {
		fields_context_with(configured_inlines(), true, true).await
	}

	#[fixture]
	async fn unregistered_inline_context() -> FieldsContext {
		let inline =
			InlineModelAdmin::new::<Parent, Child>("Child", "parent_id", &["display_name"])
				.unwrap()
				.extra(0);
		fields_context_with(vec![inline], false, false).await
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
			Some(HashMap::from([
				("id".to_owned(), json!(1)),
				("name".to_owned(), json!("first")),
			]))
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
		assert_eq!(error.user_message(), "Child");
	}

	#[rstest]
	#[serial(admin_inline_fields)]
	#[tokio::test]
	async fn edit_response_requires_view_permission_for_each_child_admin() {
		let FieldsContext {
			site, db, _lease, ..
		} = fields_context_with(configured_inlines(), true, false).await;

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
		} = fields_context_with(vec![inline], true, true).await;

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
}
