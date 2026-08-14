//! WASM stub types for dependency injection
//!
//! These types are only used for type checking on WASM targets.
//! They provide dummy implementations of server-side types that appear
//! in Server Function signatures but are automatically injected and
//! filtered out by the `#[server_fn]` macro on the client side.

#[cfg(client)]
pub use wasm_only::*;

#[cfg(client)]
mod wasm_only {
	use std::collections::HashMap;

	use crate::types::{
		AdminAction, AdminActionOutcome, AdminError, AdminResult, Fieldset, InlineStyle,
	};
	use reinhardt_core::model_form::ModelFormTableName;
	use std::collections::HashMap;

	/// Client-side P1 symbol-parity shape of an inline model configuration.
	///
	/// The WASM side is inert metadata: constructing and reading this value has
	/// no network, database, filesystem, or registration side effects.
	#[derive(Clone, Debug)]
	pub struct InlineModelAdmin {
		key: String,
		child_model: String,
		foreign_key: String,
		fields: Vec<String>,
		style: InlineStyle,
		extra: usize,
		can_delete: bool,
	}

	impl InlineModelAdmin {
		/// Preserve the native constructor shape for shared code.
		///
		/// This is a P1 parity constructor; it records metadata only and does not
		/// perform native validation, persistence, or registration.
		pub fn new<P, C>(
			child_model: impl Into<String>,
			foreign_key: impl Into<String>,
			fields: &[&str],
		) -> AdminResult<Self>
		where
			C: ModelFormTableName,
		{
			let _ = std::marker::PhantomData::<(P, C)>;
			let child_model = child_model.into();
			let foreign_key = foreign_key.into();
			Ok(Self {
				key: format!(
					"{}-{}",
					identifier_part(<C as ModelFormTableName>::table_name()),
					identifier_part(&foreign_key)
				),
				child_model,
				foreign_key,
				fields: fields.iter().map(|field| (*field).to_owned()).collect(),
				style: InlineStyle::Tabular,
				extra: 0,
				can_delete: false,
			})
		}

		/// Preserve the native style builder shape for shared code.
		pub fn style(mut self, style: InlineStyle) -> Self {
			self.style = style;
			self
		}

		/// Preserve the native extra-row builder shape for shared code.
		pub fn extra(mut self, extra: usize) -> Self {
			self.extra = extra.min(100);
			self
		}

		/// Preserve the native delete builder shape for shared code.
		pub fn can_delete(mut self, can_delete: bool) -> Self {
			self.can_delete = can_delete;
			self
		}

		/// Stable key used by flat inline control names.
		pub fn key(&self) -> &str {
			&self.key
		}

		/// Child model display name.
		pub fn child_model(&self) -> &str {
			&self.child_model
		}

		/// Generated relationship identifier on the child model.
		pub fn foreign_key(&self) -> &str {
			&self.foreign_key
		}

		/// Editable child fields.
		pub fn fields(&self) -> &[String] {
			&self.fields
		}

		/// Configured presentation style.
		pub fn style_value(&self) -> InlineStyle {
			self.style
		}

		/// Number of blank rows appended to loaded children.
		pub fn extra_rows(&self) -> usize {
			self.extra
		}

		/// Whether explicit child deletion is enabled.
		pub fn delete_enabled(&self) -> bool {
			self.can_delete
		}
	}

	fn identifier_part(value: &str) -> String {
		value
			.chars()
			.map(|character| {
				if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
					character.to_ascii_lowercase()
				} else {
					'_'
				}
			})
			.collect::<String>()
			.trim_matches('_')
			.to_owned()
	}
	/// Dummy AdminSite type for WASM type checking
	///
	/// This type is never actually used in WASM code, as the `#[server_fn]`
	/// macro removes all dependency injection parameters from client stubs.
	/// It exists purely for type checking purposes.
	pub struct AdminSite;

	/// Dummy AdminDatabase type for WASM type checking
	///
	/// This type is never actually used in WASM code, as the `#[server_fn]`
	/// macro removes all dependency injection parameters from client stubs.
	/// It exists purely for type checking purposes.
	pub struct AdminDatabase;

	/// Dummy admin action transaction type for WASM type checking.
	///
	/// This type is never actually used in WASM code because the server owns
	/// action transactions.
	pub struct AdminActionTransaction;

	/// Dummy AdminRecord type for WASM type checking
	///
	/// This type is never actually used in WASM code.
	pub struct AdminRecord;

	/// Dummy admin query type for WASM type checking.
	///
	/// This type is never actually used in WASM code.
	pub struct AdminQuery;

	/// Dummy admin request context type for WASM type checking.
	///
	/// This type is never actually used in WASM code.
	pub struct AdminRequestContext;

	/// Admin user trait stub for WASM type checking.
	///
	/// This trait is never actually used in WASM code.
	pub trait AdminUser: Send + Sync {
		/// Whether the user account is active.
		fn is_active(&self) -> bool;

		/// Whether the user is a staff member.
		fn is_staff(&self) -> bool;

		/// Whether the user is a superuser.
		fn is_superuser(&self) -> bool;

		/// The username for audit logging.
		fn get_username(&self) -> &str;
	}

	/// Changelist column descriptor stub for WASM type checking.
	#[derive(Debug, Clone, PartialEq, Eq)]
	pub enum ListColumn {
		/// A database-backed field column.
		Field {
			/// Field name to read from the result row.
			field: String,
			/// Display label for the column header.
			label: String,
		},
		/// A value computed after the result row is fetched.
		Computed {
			/// Stable key used in responses and computed-value lookup.
			key: String,
			/// Display label for the column header.
			label: String,
			/// Database field used when this computed column is sorted.
			sort_field: Option<String>,
		},
	}

	/// Model admin trait stub for WASM type checking.
	///
	/// This trait is never actually used in WASM code.
	#[async_trait::async_trait]
	pub trait ModelAdmin: Send + Sync {
		/// Get the model name.
		fn model_name(&self) -> &str;

		/// Get the database table name.
		fn table_name(&self) -> &str {
			""
		}

		/// Get the primary key field name.
		fn pk_field(&self) -> &str {
			"id"
		}

		/// Fields to display in list view.
		fn list_display(&self) -> Vec<&str> {
			vec!["id"]
		}

		/// Owned descriptors for columns displayed in list view.
		fn list_columns(&self) -> Vec<ListColumn> {
			self.list_display()
				.into_iter()
				.map(|field| ListColumn::Field {
					field: field.to_string(),
					label: field.to_string(),
				})
				.collect()
		}

		/// Resolve a computed changelist column for a fetched result row.
		fn computed_list_value(
			&self,
			key: &str,
			_row: &HashMap<String, serde_json::Value>,
		) -> crate::types::AdminResult<serde_json::Value> {
			Err(crate::types::AdminError::TemplateError(format!(
				"No computed list column is configured for key '{key}'"
			)))
		}

		/// Date or datetime field used for hierarchical changelist navigation.
		fn date_hierarchy(&self) -> Option<&str> {
			None
		}

		/// Fields that can be edited directly in list view.
		fn list_editable(&self) -> Vec<&str> {
			vec![]
		}

		/// Fields that can be used for filtering.
		fn list_filter(&self) -> Vec<&str> {
			vec![]
		}

		/// Fields that can be searched.
		fn search_fields(&self) -> Vec<&str> {
			vec![]
		}

		/// Many-to-many fields rendered with a horizontal selector.
		fn filter_horizontal(&self) -> Vec<&str> {
			vec![]
		}

		/// Many-to-many fields rendered with a vertical selector.
		fn filter_vertical(&self) -> Vec<&str> {
			vec![]
		}

		/// Fields to display in forms.
		fn fields(&self) -> Option<Vec<&str>> {
			None
		}

		/// Fieldsets to display in forms.
		fn fieldsets(&self) -> Option<Vec<Fieldset>> {
			None
		}

		/// Related child model configurations.
		fn inlines(&self) -> Vec<InlineModelAdmin> {
			Vec::new()
		}

		/// Read-only fields.
		fn readonly_fields(&self) -> Vec<&str> {
			vec![]
		}

		/// Relation fields rendered with autocomplete controls.
		fn autocomplete_fields(&self) -> Vec<&str> {
			vec![]
		}

		/// Relation fields rendered as raw ID inputs.
		fn raw_id_fields(&self) -> Vec<&str> {
			vec![]
		}

		/// Return a display label for an object represented by field values.
		fn object_label(&self, _values: &HashMap<String, serde_json::Value>) -> Option<String> {
			None
		}

		/// Ordering for list view.
		fn ordering(&self) -> Vec<&str> {
			vec!["-id"]
		}

		/// Number of items per page.
		fn list_per_page(&self) -> Option<usize> {
			None
		}

		/// One-level forward foreign keys to select with each changelist row.
		fn list_select_related(&self) -> Vec<&str> {
			vec![]
		}

		/// Customize the changelist query for a request.
		async fn get_queryset(
			&self,
			_user: &dyn AdminUser,
			_request: &AdminRequestContext,
			query: AdminQuery,
		) -> crate::types::AdminResult<AdminQuery> {
			Ok(query)
		}

		/// Actions available for this model.
		fn actions(&self) -> Vec<AdminAction> {
			Vec::new()
		}

		/// Executes an action for the selected model instances.
		async fn execute_action(
			&self,
			action: &str,
			_ids: &[String],
			_transaction: &mut AdminActionTransaction,
			_user: &dyn AdminUser,
		) -> AdminResult<AdminActionOutcome> {
			Err(AdminError::ValidationError(format!(
				"Invalid action: {action}"
			)))
		}

		/// Check if user has permission to view this model.
		async fn has_view_permission(&self, _user: &dyn AdminUser) -> bool {
			false
		}

		/// Check if user has permission to add records for this model.
		async fn has_add_permission(&self, _user: &dyn AdminUser) -> bool {
			false
		}

		/// Check if user has permission to change records for this model.
		async fn has_change_permission(&self, _user: &dyn AdminUser) -> bool {
			false
		}

		/// Check if user has permission to delete records for this model.
		async fn has_delete_permission(&self, _user: &dyn AdminUser) -> bool {
			false
		}
	}

	/// Dummy ModelAdminConfig type for WASM type checking
	///
	/// This type is never actually used in WASM code.
	pub struct ModelAdminConfig;

	/// Dummy ModelAdminConfigBuilder type for WASM type checking
	///
	/// This type is never actually used in WASM code.
	pub struct ModelAdminConfigBuilder;

	/// Dummy ExportFormat type for WASM type checking
	///
	/// This type is never actually used in WASM code.
	#[derive(serde::Serialize, serde::Deserialize)]
	pub struct ExportFormat;

	/// Dummy ImportBuilder type for WASM type checking
	///
	/// This type is never actually used in WASM code.
	pub struct ImportBuilder;

	/// Dummy ImportError type for WASM type checking
	///
	/// This type is never actually used in WASM code.
	pub struct ImportError;

	/// Dummy ImportFormat type for WASM type checking
	///
	/// This type is never actually used in WASM code.
	#[derive(serde::Serialize, serde::Deserialize)]
	pub struct ImportFormat;

	/// Dummy ImportResult type for WASM type checking
	///
	/// This type is never actually used in WASM code.
	pub struct ImportResult;

	// The assertion function is intentionally never called; compiling its
	// signature keeps the WASM trait-object shapes in sync with the native API.
	#[allow(dead_code)]
	fn assert_admin_trait_shapes(
		admin: &dyn ModelAdmin,
		_user: &dyn AdminUser,
		_query: AdminQuery,
		_request: &AdminRequestContext,
		record: &std::collections::HashMap<String, serde_json::Value>,
	) {
		let _: Option<String> = admin.object_label(record);
	}
}
