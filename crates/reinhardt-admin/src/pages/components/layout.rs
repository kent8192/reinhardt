//! Layout components
//!
//! Provides layout components for the admin panel:
//! - `Header` - Top navigation bar
//! - `Sidebar` - Side navigation menu
//! - `Footer` - Footer component
//! - `MainLayout` - Main layout wrapper

use reinhardt_pages::component::Page;
use reinhardt_pages::page;

/// Model information for navigation
#[derive(Debug, Clone)]
pub struct ModelInfo {
	/// Model name (display name)
	pub name: String,
	/// URL path for the model list view
	pub url: String,
}

/// Header component
///
/// Displays the top navigation bar with site title and user menu.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::pages::components::layout::header;
///
/// header("My Admin Panel", Some("john_doe"))
/// ```
pub fn header(site_name: &str, user_name: Option<&str>) -> Page {
	let site_name = site_name.to_string();
	let user_display = user_name.unwrap_or("Guest").to_string();

	page!(|| {
		nav {
			class: "flex items-center justify-between px-6 py-3 bg-slate-900 text-white animate__animated animate__fadeInDown",
			style: "position: fixed; top: 0; left: 0; right: 0; z-index: 50; height: 56px;",
			div {
				class: "flex items-center gap-3",
				a {
					class: "font-display text-lg font-bold tracking-tight text-white no-underline hover:text-amber-400",
					href: "/admin/",
					{ site_name }
				}
			}
			div {
				class: "flex items-center gap-2 text-sm text-slate-400",
				span {
					{ format!("User: {}", user_display) }
				}
			}
		}
	})()
}

/// Determines whether a nav item URL matches the current path.
///
/// Returns `true` when `current_path` equals the model URL exactly,
/// equals it without a trailing slash, or starts with the model URL
/// segment (to match sub-pages while avoiding similar-prefix collisions).
fn is_active_path(model_url: &str, current_path: Option<&str>) -> bool {
	current_path.is_some_and(|path| {
		let normalized_url = model_url.trim_end_matches('/');
		path == model_url
			|| path == normalized_url
			|| path.starts_with(&format!("{}/", normalized_url))
	})
}

/// Sidebar component
///
/// Displays the side navigation menu with model links.
/// Uses Link component for SPA navigation.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::pages::components::layout::{sidebar, ModelInfo};
///
/// let models = vec![
///     ModelInfo { name: "Users".to_string(), url: "/admin/users/".to_string() },
///     ModelInfo { name: "Posts".to_string(), url: "/admin/posts/".to_string() },
/// ];
/// sidebar(&models, Some("/admin/users/"))
/// ```
pub fn sidebar(models: &[ModelInfo], current_path: Option<&str>) -> Page {
	use reinhardt_pages::component::Component;
	use reinhardt_pages::router::Link;

	let nav_items: Vec<Page> = models
		.iter()
		.map(|model| {
			let is_active = is_active_path(&model.url, current_path);
			let item_class = if is_active {
				"block px-4 py-2.5 text-sm no-underline border-l-3 border-transparent admin-nav-active"
			} else {
				"block px-4 py-2.5 text-sm text-slate-400 no-underline border-l-3 border-transparent hover:text-white hover:bg-slate-800"
			};

			let link = Link::new(model.url.clone(), model.name.clone())
				.class(item_class)
				.render();

			page!(|| {
				li {
					class: "list-none",
					{ link }
				}
			})()
		})
		.collect();

	page!(|| {
		div {
			class: "admin-sidebar bg-slate-900 border-r border-slate-800 animate__animated animate__fadeInLeft",
			style: "width: 240px; height: 100vh; position: fixed; top: 56px; left: 0; overflow-y: auto; padding-top: 1rem;",
			div {
				class: "px-4 pb-3 mb-2 border-b border-slate-800",
				span {
					class: "text-xs font-semibold uppercase tracking-wider text-slate-500",
					"Models"
				}
			}
			ul {
				class: "flex flex-col gap-0.5 px-0 m-0",
				{ nav_items }
			}
		}
	})()
}

/// Footer component
///
/// Displays the footer with copyright and version information.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::pages::components::layout::footer;
///
/// footer("0.1.0")
/// ```
pub fn footer(version: &str) -> Page {
	let version = version.to_string();

	page!(|| {
		footer {
			class: "text-center py-4 text-xs text-slate-400 border-t border-slate-200 animate__animated animate__fadeIn",
			style: "margin-left: 240px;",
			{ format!("Reinhardt Admin v{}", version) }
		}
	})()
}

/// Main layout wrapper
///
/// Wraps the main content area with header, sidebar, and footer.
/// Uses RouterOutlet for dynamic content rendering.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::pages::components::layout::{main_layout, ModelInfo};
/// use reinhardt_pages::router::Router;
/// use std::sync::Arc;
///
/// let models = vec![
///     ModelInfo { name: "Users".to_string(), url: "/admin/users/".to_string() },
/// ];
/// let router = Arc::new(Router::new());
/// main_layout("My Admin", &models, None, "0.1.0", router)
/// ```
pub fn main_layout(
	site_name: &str,
	models: &[ModelInfo],
	user_name: Option<&str>,
	version: &str,
	router: std::sync::Arc<reinhardt_pages::router::Router>,
) -> Page {
	use reinhardt_pages::component::Component;
	use reinhardt_pages::router::RouterOutlet;

	let current_path = router.current_path().get();
	let header_page = header(site_name, user_name);
	let sidebar_page = sidebar(models, Some(&current_path));
	let footer_page = footer(version);
	let outlet = RouterOutlet::new(router)
		.id("admin-outlet")
		.class("router-content")
		.render();

	page!(|| {
		div {
			class: "admin-layout min-h-screen bg-slate-50",
			{ header_page }
			{ sidebar_page }
			main {
				class: "bg-slate-50",
				style: "margin-left: 240px; margin-top: 56px; padding: 1.5rem 2rem; min-height: calc(100vh - 120px);",
				{ outlet }
			}
			{ footer_page }
		}
	})()
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::is_active_path;

	// ==================== is_active_path tests ====================

	#[rstest]
	fn test_exact_match_with_trailing_slash() {
		// Arrange
		let model_url = "/admin/users/";
		let current_path = Some("/admin/users/");

		// Act
		let result = is_active_path(model_url, current_path);

		// Assert
		assert!(result);
	}

	#[rstest]
	fn test_match_without_trailing_slash() {
		// Arrange
		let model_url = "/admin/users/";
		let current_path = Some("/admin/users");

		// Act
		let result = is_active_path(model_url, current_path);

		// Assert
		assert!(result);
	}

	#[rstest]
	fn test_sub_page_matches() {
		// Arrange
		let model_url = "/admin/users/";
		let current_path = Some("/admin/users/42/change/");

		// Act
		let result = is_active_path(model_url, current_path);

		// Assert
		assert!(result);
	}

	#[rstest]
	fn test_similar_prefix_does_not_match() {
		// Arrange
		let model_url = "/admin/users/";
		let current_path = Some("/admin/usergroups/");

		// Act
		let result = is_active_path(model_url, current_path);

		// Assert
		assert!(!result);
	}

	#[rstest]
	fn test_root_admin_path_matches() {
		// Arrange
		let model_url = "/admin/";
		let current_path = Some("/admin/");

		// Act
		let result = is_active_path(model_url, current_path);

		// Assert
		assert!(result);
	}

	#[rstest]
	fn test_none_current_path_does_not_match() {
		// Arrange
		let model_url = "/admin/users/";
		let current_path = None;

		// Act
		let result = is_active_path(model_url, current_path);

		// Assert
		assert!(!result);
	}

	#[rstest]
	fn test_different_path_does_not_match() {
		// Arrange
		let model_url = "/admin/users/";
		let current_path = Some("/admin/posts/");

		// Act
		let result = is_active_path(model_url, current_path);

		// Assert
		assert!(!result);
	}
}
