//! Admin panel configuration for examples-github-issues
//!
//! Configures and builds the admin interface with all app model admins.
//! Demonstrates both macro-based (`#[admin(model, ...)]`) and builder-based
//! (`ModelAdminConfigBuilder`) admin registration approaches.

use crate::apps::auth::admin::user_admin_config;
use crate::apps::issues::admin::IssueAdmin;
use crate::apps::projects::admin::{ProjectAdmin, ProjectMemberAdmin};
use reinhardt::admin::AdminSite;

/// Configure the admin site with all registered model admins
///
/// Creates an AdminSite and registers model admins from each app.
/// The returned AdminSite can be converted to routes via `get_urls()`.
pub fn configure_admin() -> AdminSite {
	let site = AdminSite::new("GitHub Issues Admin");

	// Customize admin site configuration
	site.configure(|config| {
		config.site_title = "GitHub Issues - Admin".into();
		config.site_header = "Issue Tracker Administration".into();
		config.list_per_page = 50;
	});

	// Register macro-based model admins from each app
	site.register("Project", ProjectAdmin)
		.expect("Failed to register ProjectAdmin");
	site.register("Project Member", ProjectMemberAdmin)
		.expect("Failed to register ProjectMemberAdmin");
	site.register("Issue", IssueAdmin)
		.expect("Failed to register IssueAdmin");

	// Register builder-based admin config (for models without #[model] macro)
	site.register("User", user_admin_config())
		.expect("Failed to register user admin config");

	site
}
