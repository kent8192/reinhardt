//! Admin panel configuration
//!
//! Configures and builds the admin interface for examples-twitter.
//!
//! Demonstrates:
//! - `AdminSiteConfig` customization via `configure()`
//! - `#[admin(model, ...)]` macro-based registration
//! - `fields` attribute for controlling form fields

use crate::apps::dm::admin::{DMMessageAdmin, DMRoomAdmin};
use crate::apps::profile::admin::ProfileAdmin;
use crate::apps::tweet::admin::TweetAdmin;
use reinhardt::admin::AdminSite;

/// Configure the admin site
///
/// Creates an AdminSite and registers all model admins from each app.
/// Database connection will be configured via DI container in urls.rs.
///
/// # Endpoints (via `AdminSite::get_urls()` in urls.rs)
///
/// - `GET /admin/api/` - Dashboard (list of registered models)
/// - `GET /admin/api/{model}/` - List model instances
/// - `GET /admin/api/{model}/{id}/` - Get model instance detail
/// - `POST /admin/api/{model}/` - Create model instance
/// - `PUT /admin/api/{model}/{id}/` - Update model instance
/// - `DELETE /admin/api/{model}/{id}/` - Delete model instance
/// - `GET /admin/api/{model}/export/` - Export model data
/// - `POST /admin/api/{model}/import/` - Import model data
pub fn configure_admin() -> AdminSite {
	let site = AdminSite::new("Twitter Admin");

	// Customize admin site configuration
	site.configure(|config| {
		config.site_title = "Twitter Clone - Admin".into();
		config.site_header = "Twitter Administration".into();
		config.list_per_page = 50;
	});

	// Register admin configurations from each app
	site.register("Tweet", TweetAdmin)
		.expect("Failed to register TweetAdmin");
	site.register("Profile", ProfileAdmin)
		.expect("Failed to register ProfileAdmin");
	site.register("DM Room", DMRoomAdmin)
		.expect("Failed to register DMRoomAdmin");
	site.register("DM Message", DMMessageAdmin)
		.expect("Failed to register DMMessageAdmin");

	site
}
