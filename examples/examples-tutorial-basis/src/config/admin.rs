//! Admin panel configuration for examples-tutorial-basis.
//!
//! Builds an `AdminSite` and registers per-app `ModelAdmin` configurations
//! so the Django-style auto-generated admin UI is reachable at `/admin/`.
//! Mounting and DI wiring happen in `crate::config::urls`.

use crate::apps::polls::server::admin::{ChoiceAdmin, QuestionAdmin};
use crate::config::settings::get_settings;
use reinhardt::HasCoreSettings;
use reinhardt::admin::AdminSite;

/// Configure the admin site and register all polls-app model admins.
///
/// The database connection is supplied later via DI (see
/// `admin_routes_with_di` in `crate::config::urls`), so this function
/// only handles registration metadata.
pub fn configure_admin() -> AdminSite {
	let mut site = AdminSite::new("Polls Tutorial Admin");
	let settings = get_settings()
		.expect("tutorial settings sources must load for admin configuration")
		.resolve()
		.expect("tutorial settings must resolve for admin configuration")
		.into_parts()
		.0;

	site.configure(|config| {
		config.site_title = "Polls Tutorial - Admin".into();
		config.site_header = "Polls Administration".into();
		config.list_per_page = 25;
	});
	site.set_jwt_secret(settings.core().secret_key.as_bytes());

	site.register("Question", QuestionAdmin)
		.expect("Failed to register QuestionAdmin");
	site.register("Choice", ChoiceAdmin)
		.expect("Failed to register ChoiceAdmin");

	site
}
