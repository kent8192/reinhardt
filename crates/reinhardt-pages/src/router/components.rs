//! Router Components for navigation.
//!
//! This module provides Link and RouterOutlet components for
//! declarative navigation in component trees.

use crate::component::{Component, Head, IntoPage, MetaTag, Page, PageElement};
use crate::router::loader::{LoaderStore, RouteLoaderError, with_loader_store};
use reinhardt_urls::routers::ClientRouter;
use std::rc::Rc;

/// Controls when a link prepares matched route loaders in the background.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PrefetchMode {
	/// Do not prefetch route data.
	#[default]
	None,
	/// Prefetch on pointer intent or keyboard focus.
	Hover,
	/// Prefetch when the link enters the viewport.
	Viewport,
}

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
	/// Background route-loader preparation policy.
	prefetch: PrefetchMode,
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
			prefetch: PrefetchMode::None,
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

	/// Sets the route-loader prefetch policy for this link.
	pub fn prefetch(mut self, mode: PrefetchMode) -> Self {
		self.prefetch = mode;
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

	/// Returns the configured prefetch mode.
	pub fn prefetch_mode(&self) -> PrefetchMode {
		self.prefetch
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
			match self.prefetch {
				PrefetchMode::None => {}
				PrefetchMode::Hover => el = el.attr("data-prefetch", "hover"),
				PrefetchMode::Viewport => el = el.attr("data-prefetch", "viewport"),
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

type NavigationErrorFallback = Rc<dyn Fn(&RouteLoaderError) -> Page>;

/// Renders the current route from a [`ClientRouter`].
///
/// `RouterOutlet` is a component-level adapter for embedding the canonical
/// `reinhardt-urls` client router in a pages component tree.
#[derive(Clone)]
pub struct RouterOutlet {
	/// Router used to resolve and render the current client-side route.
	router: ClientRouter,
	/// Optional sibling fallback for a failed route-loader navigation.
	navigation_error_fallback: Option<NavigationErrorFallback>,
}

impl std::fmt::Debug for RouterOutlet {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("RouterOutlet")
			.field("router", &self.router)
			.field(
				"has_navigation_error_fallback",
				&self.navigation_error_fallback.is_some(),
			)
			.finish()
	}
}

impl RouterOutlet {
	/// Creates a new router outlet backed by `router`.
	pub fn new(router: ClientRouter) -> Self {
		Self {
			router,
			navigation_error_fallback: None,
		}
	}

	/// Adds a sibling boundary for errors raised while preparing a route.
	///
	/// The current route remains mounted; the fallback is rendered alongside it
	/// until a subsequent navigation clears the coordinator error.
	pub fn navigation_error_fallback<F>(mut self, fallback: F) -> Self
	where
		F: Fn(&RouteLoaderError) -> Page + 'static,
	{
		self.navigation_error_fallback = Some(Rc::new(fallback));
		self
	}

	/// Returns the router backing this outlet.
	pub fn router(&self) -> &ClientRouter {
		&self.router
	}
}

impl Component for RouterOutlet {
	fn render(&self) -> Page {
		let router = self.router.clone();
		let fallback = self.navigation_error_fallback.clone();
		Page::reactive(move || {
			let mounted_store = crate::app::try_with_navigation_coordinator(|coordinator| {
				coordinator.mounted_store()
			})
			.flatten();
			let current = render_current_with_mounted_loader_store(&router, mounted_store);
			if let Some(fallback) = &fallback
				&& let Some(Some(error)) =
					crate::app::try_with_navigation_coordinator(|coordinator| {
						coordinator.error().get()
					}) {
				return Page::Fragment(vec![current, fallback(&error)]);
			}
			current
		})
	}

	fn name() -> &'static str {
		"RouterOutlet"
	}
}

fn render_current_with_mounted_loader_store(
	router: &ClientRouter,
	mounted_store: Option<LoaderStore>,
) -> Page {
	if let Some(store) = mounted_store {
		return with_loader_store(&store, || router.render_current());
	}
	router.render_current()
}

impl IntoPage for RouterOutlet {
	fn into_page(self) -> Page {
		self.render()
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
		let head = Head::new().meta(MetaTag::http_equiv("refresh", format!("0;url={}", self.to)));
		PageElement::new("span")
			.attr("data-redirect", self.to.clone())
			.attr("data-replace", if self.replace { "true" } else { "false" })
			.attr("hidden", "hidden")
			.into_page()
			.with_head(head)
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
	use crate::router::loader::active_loader_store;
	use reinhardt_core::reactive::ReactiveScope;
	use reinhardt_urls::routers::ClientRouter;
	use serial_test::serial;

	#[test]
	#[serial]
	fn router_outlet_reacts_to_loader_errors_after_its_initial_render() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new().route("home", "/", || {
				assert!(
					active_loader_store().is_some(),
					"loader-backed rerenders must retain their mounted store"
				);
				Page::text("HOME")
			});
			crate::app::__install_client_router_for_test(router.clone());
			crate::app::try_with_navigation_coordinator(|coordinator| {
				coordinator.set_mounted_store_for_test(LoaderStore::new());
			})
			.expect("test router installs a navigation coordinator");
			let outlet = RouterOutlet::new(router).navigation_error_fallback(|error| {
				Page::text(format!("FAILED: {}", error.public_message()))
			});
			let page = outlet.render();

			assert_eq!(page.render_to_string(), "HOME");
			crate::app::try_with_navigation_coordinator(|coordinator| {
				coordinator
					.error()
					.set(Some(RouteLoaderError::new("loader failed")));
			})
			.expect("test router installs a navigation coordinator");
			assert_eq!(page.render_to_string(), "HOMEFAILED: loader failed");

			crate::app::__clear_spa_router_for_test();
		});
	}

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
	fn test_redirect() {
		let redirect = Redirect::new("/login/");
		assert_eq!(redirect.to(), "/login/");

		let html = redirect.render().render_to_string();
		assert!(html.contains("data-redirect=\"/login/\""));
		let page = redirect.render();
		assert!(
			page.extract_head()
				.expect("redirect fallback head")
				.to_html()
				.contains("url=/login/")
		);
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
	fn test_link_prefetch_attribute() {
		let html = Link::new("/jobs/", "Jobs")
			.prefetch(PrefetchMode::Hover)
			.render()
			.render_to_string();
		assert!(html.contains("data-prefetch=\"hover\""));
	}

	#[test]
	fn test_redirect_component_name() {
		assert_eq!(Redirect::name(), "Redirect");
	}
}
