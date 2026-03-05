//! Admin configurations for auth app
//!
//! DefaultUser is defined without `#[model(...)]` macro, so it lacks
//! the `field_*()` accessors required by `#[admin(model, ...)]` macro.
//! Instead, we use ModelAdminConfigBuilder for explicit configuration.

use reinhardt::admin::{ModelAdminConfig, ModelAdminConfigBuilder};

/// Creates admin configuration for DefaultUser
///
/// This function returns a ModelAdminConfig that can be registered
/// with AdminSite for managing user records in the admin panel.
///
/// # Example
///
/// ```rust,ignore
/// let site = AdminSite::new("Admin");
/// site.register_config(user_admin_config());
/// ```
pub fn user_admin_config() -> ModelAdminConfig {
	ModelAdminConfigBuilder::default()
		.model_name("User")
		.table_name("auth_user")
		.pk_field("id")
		.list_display(vec![
			"id".to_string(),
			"username".to_string(),
			"email".to_string(),
			"is_active".to_string(),
			"is_staff".to_string(),
			"date_joined".to_string(),
		])
		.list_filter(vec![
			"is_active".to_string(),
			"is_staff".to_string(),
			"is_superuser".to_string(),
			"date_joined".to_string(),
		])
		.search_fields(vec![
			"username".to_string(),
			"email".to_string(),
			"first_name".to_string(),
			"last_name".to_string(),
		])
		.ordering(vec!["-date_joined".to_string()])
		.readonly_fields(vec![
			"id".to_string(),
			"date_joined".to_string(),
			"last_login".to_string(),
		])
		.list_per_page(25)
		.build()
		.expect("valid user admin config")
}
