//! Router Components for navigation.
//!
//! This module provides Link and RouterOutlet components for
//! declarative navigation in component trees.

use crate::component::{Component, IntoPage, Page, PageElement};

/// A link component that navigates without full page reload.
///
/// Similar to HTML `<a>` but intercepts clicks to use the History API.
///
/// # Example
///
/// ```no_run
/// use reinhardt_pages::router::Link;
///
/// let link = Link::new("/users/42/", "View User");
/// ```
#[derive(Debug, Clone)]
pub struct Link {
	/// The destination path.
	to: String,
	/// The link text or content.
	content: String,
	/// Additional CSS classes.
	class: Option<String>,
	/// Whether to replace the current history entry.
	replace: bool,
	/// Whether to open in a new tab (disables SPA navigation).
	external: bool,
	/// Custom attributes.
	attrs: Vec<(String, String)>,
}

impl Link {
	/// Creates a new link.
	pub fn new(to: impl Into<String>, content: impl Into<String>) -> Self {
		Self {
			to: to.into(),
			content: content.into(),
			class: None,
			replace: false,
			external: false,
			attrs: Vec::new(),
		}
	}

	/// Sets the CSS class.
	pub fn class(mut self, class: impl Into<String>) -> Self {
		self.class = Some(class.into());
		self
	}

	/// Sets whether to replace the current history entry.
	pub fn replace(mut self, replace: bool) -> Self {
		self.replace = replace;
		self
	}

	/// Sets whether this is an external link.
	pub fn external(mut self, external: bool) -> Self {
		self.external = external;
		self
	}

	/// Adds a custom attribute.
	pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
		self.attrs.push((name.into(), value.into()));
		self
	}

	/// Returns the destination path.
	pub fn to(&self) -> &str {
		&self.to
	}

	/// Returns the content.
	pub fn content(&self) -> &str {
		&self.content
	}

	/// Returns whether this is a replace navigation.
	pub fn is_replace(&self) -> bool {
		self.replace
	}

	/// Returns whether this is an external link.
	pub fn is_external(&self) -> bool {
		self.external
	}
}

impl Component for Link {
	fn render(&self) -> Page {
		let mut el = PageElement::new("a").attr("href", self.to.clone());

		if let Some(ref class) = self.class {
			el = el.attr("class", class.clone());
		}

		// Add data attributes for JS handling
		if !self.external {
			el = el.attr("data-link", "true");
			if self.replace {
				el = el.attr("data-replace", "true");
			}
		} else {
			el = el.attr("target", "_blank");
			el = el.attr("rel", "noopener noreferrer");
		}

		// Add custom attributes
		for (name, value) in &self.attrs {
			el = el.attr(name.clone(), value.clone());
		}

		el.child(self.content.clone()).into_page()
	}

	fn name() -> &'static str {
		"Link"
	}
}

/// A component that renders the matched route's content.
///
/// Place this where you want route content to appear.
///
/// # Example
///
/// ```no_run
/// use reinhardt_pages::router::{Router, RouterOutlet};
/// use reinhardt_pages::component::Page;
/// use std::sync::Arc;
///
/// # fn home_view() -> Page { Page::text("Home") }
/// let router = Arc::new(Router::new().route("/", home_view));
/// let outlet = RouterOutlet::new(router);
/// ```
#[derive(Debug, Clone)]
pub struct RouterOutlet {
	/// Reference to the router.
	router: std::sync::Arc<super::Router>,
	/// The ID attribute for the outlet element.
	id: Option<String>,
	/// CSS class for the outlet element.
	class: Option<String>,
}

impl RouterOutlet {
	/// Creates a new router outlet with a router reference.
	///
	/// ## Arguments
	///
	/// * `router` - An Arc reference to the Router instance
	pub fn new(router: std::sync::Arc<super::Router>) -> Self {
		Self {
			router,
			id: None,
			class: None,
		}
	}

	/// Sets the ID attribute.
	pub fn id(mut self, id: impl Into<String>) -> Self {
		self.id = Some(id.into());
		self
	}

	/// Sets the CSS class.
	pub fn class(mut self, class: impl Into<String>) -> Self {
		self.class = Some(class.into());
		self
	}
}

impl Component for RouterOutlet {
	fn render(&self) -> Page {
		let mut el = PageElement::new("div").attr("data-router-outlet", "true");

		if let Some(ref id) = self.id {
			el = el.attr("id", id.clone());
		}

		if let Some(ref class) = self.class {
			el = el.attr("class", class.clone());
		}

		// Render current route inside the outlet container
		el.child(self.router.render_current()).into_page()
	}

	fn name() -> &'static str {
		"RouterOutlet"
	}
}

/// A redirect component that immediately navigates to another path.
///
/// The redirect URL is validated at construction time to prevent open redirect
/// attacks. Relative URLs (starting with `/`) are always allowed. Absolute URLs
/// must have their host in the provided `allowed_hosts` set.
#[derive(Debug, Clone)]
pub struct Redirect {
	/// The destination path.
	to: String,
	/// Whether to replace the current history entry.
	replace: bool,
}

impl Redirect {
	/// Creates a new redirect with URL validation.
	///
	/// Validates the redirect URL against the provided allowed hosts to prevent
	/// open redirect attacks. Relative URLs (starting with `/`) are always safe.
	///
	/// # Errors
	///
	/// Returns `RedirectValidationError` if the URL fails validation.
	pub fn validated(
		to: impl Into<String>,
		allowed_hosts: &std::collections::HashSet<String>,
	) -> Result<Self, reinhardt_core::security::redirect::RedirectValidationError> {
		let url = to.into();
		reinhardt_core::security::redirect::validate_redirect_url(&url, allowed_hosts)?;
		Ok(Self {
			to: url,
			replace: true,
		})
	}

	/// Creates a new redirect without external URL validation.
	///
	/// Only allows relative URLs (starting with `/`). Rejects any URL that
	/// could be an absolute URL or uses dangerous protocols.
	///
	/// # Panics
	///
	/// Panics if the URL does not start with `/` (use `validated` for absolute URLs).
	pub fn new(to: impl Into<String>) -> Self {
		let url = to.into();
		assert!(
			url.starts_with('/') && !url.starts_with("//"),
			"Redirect::new only accepts relative URLs starting with '/'. Use Redirect::validated for absolute URLs."
		);
		Self {
			to: url,
			replace: true,
		}
	}

	/// Sets whether to use replace navigation.
	pub fn replace(mut self, replace: bool) -> Self {
		self.replace = replace;
		self
	}

	/// Returns the destination path.
	pub fn to(&self) -> &str {
		&self.to
	}
}

impl Component for Redirect {
	fn render(&self) -> Page {
		// Render a meta refresh as fallback, actual redirect handled by JS
		PageElement::new("meta")
			.attr("http-equiv", "refresh")
			.attr("content", format!("0;url={}", self.to))
			.attr("data-redirect", self.to.clone())
			.attr("data-replace", if self.replace { "true" } else { "false" })
			.into_page()
	}

	fn name() -> &'static str {
		"Redirect"
	}
}

/// A navigation guard that conditionally renders content.
///
/// This is a function that wraps content rendering with a condition check.
pub fn guard<F, V>(condition: F, content: V) -> impl FnOnce() -> Page
where
	F: FnOnce() -> bool,
	V: IntoPage,
{
	move || {
		if condition() {
			content.into_page()
		} else {
			Page::Empty
		}
	}
}

/// A navigation guard with fallback that conditionally renders content.
pub fn guard_or<F, V, U>(condition: F, content: V, fallback: U) -> impl FnOnce() -> Page
where
	F: FnOnce() -> bool,
	V: IntoPage,
	U: IntoPage,
{
	move || {
		if condition() {
			content.into_page()
		} else {
			fallback.into_page()
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_link_new() {
		let link = Link::new("/users/", "Users");
		assert_eq!(link.to(), "/users/");
		assert_eq!(link.content(), "Users");
		assert!(!link.is_replace());
		assert!(!link.is_external());
	}

	#[test]
	fn test_link_builder() {
		let link = Link::new("/admin/", "Admin")
			.class("nav-link")
			.replace(true)
			.attr("aria-label", "Admin Panel");

		let html = link.render().render_to_string();
		assert!(html.contains("href=\"/admin/\""));
		assert!(html.contains("class=\"nav-link\""));
		assert!(html.contains("data-replace=\"true\""));
		assert!(html.contains("aria-label=\"Admin Panel\""));
	}

	#[test]
	fn test_link_external() {
		let link = Link::new("https://example.com", "Example").external(true);

		let html = link.render().render_to_string();
		assert!(html.contains("target=\"_blank\""));
		assert!(html.contains("rel=\"noopener noreferrer\""));
		assert!(!html.contains("data-link"));
	}

	#[test]
	fn test_router_outlet() {
		use crate::router::Router;
		use std::sync::Arc;

		fn test_view() -> Page {
			Page::text("Test Route")
		}

		let router = Arc::new(Router::new().route("/", test_view));
		let outlet = RouterOutlet::new(router).id("main-outlet").class("content");

		let html = outlet.render().render_to_string();
		assert!(html.contains("data-router-outlet=\"true\""));
		assert!(html.contains("id=\"main-outlet\""));
		assert!(html.contains("class=\"content\""));
		// Should render the current route content
		assert!(html.contains("Test Route"));
	}

	#[test]
	fn test_redirect() {
		let redirect = Redirect::new("/login/");
		assert_eq!(redirect.to(), "/login/");

		let html = redirect.render().render_to_string();
		assert!(html.contains("url=/login/"));
		assert!(html.contains("data-redirect=\"/login/\""));
	}

	#[test]
	#[should_panic(expected = "Redirect::new only accepts relative URLs")]
	fn test_redirect_rejects_absolute_url() {
		Redirect::new("https://evil.com/phish");
	}

	#[test]
	#[should_panic(expected = "Redirect::new only accepts relative URLs")]
	fn test_redirect_rejects_protocol_relative() {
		Redirect::new("//evil.com/path");
	}

	#[test]
	fn test_redirect_validated_allows_trusted_host() {
		let mut hosts = std::collections::HashSet::new();
		hosts.insert("example.com".to_string());
		let redirect = Redirect::validated("https://example.com/page", &hosts).unwrap();
		assert_eq!(redirect.to(), "https://example.com/page");
	}

	#[test]
	fn test_redirect_validated_rejects_untrusted_host() {
		let hosts = std::collections::HashSet::new();
		let result = Redirect::validated("https://evil.com/phish", &hosts);
		assert!(result.is_err());
	}

	#[test]
	fn test_guard_true() {
		let view = guard(|| true, "Allowed")();
		assert_eq!(view.render_to_string(), "Allowed");
	}

	#[test]
	fn test_guard_false() {
		let view = guard(|| false, "Allowed")();
		assert_eq!(view.render_to_string(), "");
	}

	#[test]
	fn test_guard_with_fallback() {
		let view = guard_or(|| false, "Allowed", "Denied")();
		assert_eq!(view.render_to_string(), "Denied");
	}

	#[test]
	fn test_link_component_name() {
		assert_eq!(Link::name(), "Link");
	}

	#[test]
	fn test_router_outlet_component_name() {
		assert_eq!(RouterOutlet::name(), "RouterOutlet");
	}

	#[test]
	fn test_redirect_component_name() {
		assert_eq!(Redirect::name(), "Redirect");
	}
}
