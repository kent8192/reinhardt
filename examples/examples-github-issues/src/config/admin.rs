//! Admin panel configuration for examples-github-issues
//!
//! Configures and builds the admin interface with all app model admins.
//! Uses the `#[admin(model, ...)]` macro for admin registration.

use crate::apps::auth::admin::UserAdmin;
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
	site.register("User", UserAdmin)
		.expect("Failed to register UserAdmin");
	site.register("Project", ProjectAdmin)
		.expect("Failed to register ProjectAdmin");
	site.register("Project Member", ProjectMemberAdmin)
		.expect("Failed to register ProjectMemberAdmin");
	site.register("Issue", IssueAdmin)
		.expect("Failed to register IssueAdmin");

	site
}
