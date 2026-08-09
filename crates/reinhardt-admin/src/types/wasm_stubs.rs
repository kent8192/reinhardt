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
	use crate::types::{AdminResult, Fieldset, InlineStyle};

	/// Client-side shape of an inline model configuration.
	#[derive(Clone)]
	pub struct InlineModelAdmin;

	impl InlineModelAdmin {
		/// Preserve the native constructor shape for shared code.
		pub fn new<P, C>(
			_child_model: impl Into<String>,
			_foreign_key: impl Into<String>,
			_fields: &[&str],
		) -> AdminResult<Self> {
			let _ = std::marker::PhantomData::<(P, C)>;
			Ok(Self)
		}

		/// Preserve the native style builder shape for shared code.
		pub fn style(self, _style: InlineStyle) -> Self {
			self
		}

		/// Preserve the native extra-row builder shape for shared code.
		pub fn extra(self, _extra: usize) -> Self {
			self
		}

		/// Preserve the native delete builder shape for shared code.
		pub fn can_delete(self, _can_delete: bool) -> Self {
			self
		}
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

	/// Dummy AdminRecord type for WASM type checking
	///
	/// This type is never actually used in WASM code.
	pub struct AdminRecord;

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

		/// Ordering for list view.
		fn ordering(&self) -> Vec<&str> {
			vec!["-id"]
		}

		/// Number of items per page.
		fn list_per_page(&self) -> Option<usize> {
			None
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
	fn assert_admin_trait_shapes(_admin: &dyn ModelAdmin, _user: &dyn AdminUser) {}
}
