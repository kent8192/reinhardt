//! Layout components
//!
//! Provides layout components for the admin panel:
//! - `Header` - Top navigation bar
//! - `Sidebar` - Side navigation menu
//! - `Footer` - Footer component
//! - `MainLayout` - Main layout wrapper

use reinhardt_pages::component::{IntoPage, Page, PageElement};

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
	let user_display = user_name.unwrap_or("Guest");

	PageElement::new("nav")
		.attr("class", "navbar navbar-dark bg-dark")
		.child(
			PageElement::new("div")
				.attr("class", "container-fluid")
				.child(
					PageElement::new("a")
						.attr("class", "navbar-brand")
						.attr("href", "/admin/")
						.child(site_name.to_string()),
				)
				.child(
					PageElement::new("span")
						.attr("class", "navbar-text")
						.child(format!("User: {}", user_display)),
				),
		)
		.into_page()
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
			// Match both with and without trailing slash to handle browser URL normalization.
			// Sub-pages like "/admin/users/42/change/" are also matched while
			// similar prefixes like "/admin/usergroups/" are not.
			let is_active = is_active_path(&model.url, current_path);
			let item_class = if is_active {
				"nav-link active"
			} else {
				"nav-link"
			};

			PageElement::new("li")
				.attr("class", "nav-item")
				.child(
					Link::new(model.url.clone(), model.name.clone())
						.class(item_class)
						.render(),
				)
				.into_page()
		})
		.collect();

	PageElement::new("div")
		.attr("class", "sidebar bg-light border-end")
		.attr(
			"style",
			"width: 250px; height: 100vh; position: fixed; top: 56px; left: 0; overflow-y: auto;",
		)
		.child(
			PageElement::new("ul")
				.attr("class", "nav flex-column")
				.children(nav_items),
		)
		.into_page()
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
	PageElement::new("footer")
		.attr("class", "footer bg-light text-center py-3 border-top")
		.attr("style", "margin-left: 250px;")
		.child(
			PageElement::new("div")
				.attr("class", "container-fluid")
				.child(format!("Reinhardt Admin Panel v{}", version)),
		)
		.into_page()
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

	// Get the current path from the router for sidebar active-link highlighting
	let current_path = router.current_path().get();

	PageElement::new("div")
		.attr("class", "admin-layout")
		.child(header(site_name, user_name))
		.child(sidebar(models, Some(&current_path)))
		.child(
			PageElement::new("main")
				.attr("class", "main-content")
				.attr(
					"style",
					"margin-left: 250px; margin-top: 56px; padding: 20px; min-height: calc(100vh - 120px);",
				)
				.child(
					RouterOutlet::new(router)
						.id("admin-outlet")
						.class("router-content")
						.render(),
				),
		)
		.child(footer(version))
		.into_page()
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
