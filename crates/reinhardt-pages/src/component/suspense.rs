//! Suspense boundary for lazy loading with resource-aware rendering.
//!
//! This module provides [`SuspenseBoundary`], a component that displays a fallback UI
//! while one or more [`Resource`]s are in the `ResourceState::Loading` state. Once all
//! tracked resources have resolved, the actual content is rendered.
//!
//! ## Features
//!
//! - **Fallback UI**: Configurable loading indicator shown while resources load
//! - **Multiple resources**: Track one or more resources simultaneously
//! - **Nesting**: SuspenseBoundary components can be nested; each boundary
//!   manages its own set of resources independently
//! - **SSR support**: During server-side rendering, the actual content is rendered
//!   (not the fallback), since the server can pre-fetch data synchronously
//!
//! ## Example
//!
//! ```ignore
//! use reinhardt_pages::component::suspense::SuspenseBoundary;
//! use reinhardt_pages::{Page, PageElement, IntoPage, Resource, ResourceState};
//!
//! fn user_profile(resource: Resource<String>) -> Page {
//!     let r = resource.clone();
//!     SuspenseBoundary::new()
//!         .fallback(|| Page::text("Loading user..."))
//!         .track(resource)
//!         .content(move || {
//!             match r.get() {
//!                 ResourceState::Success(name) => {
//!                     PageElement::new("div")
//!                         .child(name)
//!                         .into_page()
//!                 }
//!                 ResourceState::Error(err) => {
//!                     PageElement::new("span")
//!                         .attr("class", "error")
//!                         .child(err)
//!                         .into_page()
//!                 }
//!                 ResourceState::Loading => Page::Empty,
//!             }
//!         })
//!         .into_page()
//! }
//! ```

use crate::component::{IntoPage, Page, PageElement};
use crate::reactive::Resource;

/// Trait for checking whether a resource is in the loading state.
///
/// This trait abstracts over different `Resource` type parameters,
/// allowing [`SuspenseBoundary`] to track multiple resources with
/// heterogeneous types.
pub trait ResourceTracker {
	/// Returns `true` if the tracked resource is currently loading.
	fn is_loading(&self) -> bool;
}

impl<T: Clone + 'static, E: Clone + 'static> ResourceTracker for Resource<T, E> {
	fn is_loading(&self) -> bool {
		self.is_loading()
	}
}

/// A boxed resource tracker for dynamic dispatch over heterogeneous resources.
type BoxedTracker = Box<dyn ResourceTracker>;

/// Suspense boundary component for lazy loading.
///
/// `SuspenseBoundary` displays a configurable fallback UI while one or more
/// tracked [`Resource`]s are loading. Once all resources have resolved
/// (either `Success` or `Error`), the actual content closure is invoked
/// and its output is rendered.
///
/// # Builder Pattern
///
/// Use the builder methods to configure the boundary:
///
/// ```ignore
/// SuspenseBoundary::new()
///     .fallback(|| Page::text("Loading..."))
///     .content(|| PageElement::new("p").child("Content").into_page())
///     .into_page()
/// ```
///
/// # SSR Behavior
///
/// During server-side rendering (non-WASM target), `render()` always returns
/// the content (never the fallback), since the server can resolve data
/// synchronously before sending the response.
pub struct SuspenseBoundary {
	/// Fallback UI factory invoked while resources are loading.
	fallback_fn: Box<dyn Fn() -> Page>,
	/// Tracked resources to monitor for loading state.
	trackers: Vec<BoxedTracker>,
	/// Content closure invoked when all resources have resolved.
	content_fn: Box<dyn Fn() -> Page>,
}

impl SuspenseBoundary {
	/// Creates a new `SuspenseBoundary` with default settings.
	///
	/// The default fallback and content are empty views.
	/// Use [`fallback()`](Self::fallback) to provide a loading indicator
	/// and [`content()`](Self::content) to set the resolved view.
	pub fn new() -> Self {
		Self {
			fallback_fn: Box::new(|| Page::Empty),
			trackers: Vec::new(),
			content_fn: Box::new(|| Page::Empty),
		}
	}

	/// Sets the fallback UI factory displayed while resources are loading.
	///
	/// The closure is called each time the fallback needs to be rendered,
	/// producing a fresh [`Page`] each invocation.
	///
	/// # Arguments
	///
	/// * `f` - A closure that returns a [`Page`] to show during loading
	pub fn fallback(mut self, f: impl Fn() -> Page + 'static) -> Self {
		self.fallback_fn = Box::new(f);
		self
	}

	/// Tracks a resource for loading state detection.
	///
	/// Multiple resources can be tracked by chaining `.track()` calls.
	/// The fallback is shown until **all** tracked resources have resolved.
	///
	/// # Arguments
	///
	/// * `resource` - A [`Resource`] to monitor
	pub fn track<T, E>(mut self, resource: Resource<T, E>) -> Self
	where
		T: Clone + 'static,
		E: Clone + 'static,
	{
		self.trackers.push(Box::new(resource) as BoxedTracker);
		self
	}

	/// Tracks a custom resource tracker for loading state detection.
	///
	/// This method accepts any type implementing [`ResourceTracker`],
	/// enabling integration with custom async data sources beyond
	/// the built-in `Resource<T>`.
	///
	/// # Arguments
	///
	/// * `tracker` - Any type implementing [`ResourceTracker`]
	pub fn track_custom(mut self, tracker: impl ResourceTracker + 'static) -> Self {
		self.trackers.push(Box::new(tracker) as BoxedTracker);
		self
	}

	/// Sets the content closure that produces the view when resources are ready.
	///
	/// The closure is called each time the boundary needs to render and all
	/// tracked resources have resolved.
	///
	/// # Arguments
	///
	/// * `f` - A closure that returns a [`Page`]
	pub fn content(mut self, f: impl Fn() -> Page + 'static) -> Self {
		self.content_fn = Box::new(f);
		self
	}

	/// Returns `true` if any tracked resource is currently loading.
	pub fn any_loading(&self) -> bool {
		self.trackers.iter().any(|t| t.is_loading())
	}

	/// Renders the suspense boundary.
	///
	/// On WASM targets, this checks the loading state of tracked resources
	/// and returns either the fallback or the content.
	///
	/// On non-WASM targets (SSR), this always returns the content,
	/// since the server can pre-fetch data before rendering.
	pub fn render(&self) -> Page {
		#[cfg(target_arch = "wasm32")]
		{
			if self.any_loading() {
				return self.render_fallback();
			}
			self.render_content()
		}

		#[cfg(not(target_arch = "wasm32"))]
		{
			// SSR: always render the actual content (server pre-fetches data)
			self.render_content()
		}
	}

	/// Renders the fallback wrapped in a marker div.
	#[cfg(target_arch = "wasm32")]
	fn render_fallback(&self) -> Page {
		let fallback = (self.fallback_fn)();
		PageElement::new("div")
			.attr("data-rh-suspense", "pending")
			.child(fallback)
			.into_page()
	}

	/// Renders the resolved content wrapped in a marker div.
	fn render_content(&self) -> Page {
		let content = (self.content_fn)();
		PageElement::new("div")
			.attr("data-rh-suspense", "resolved")
			.child(content)
			.into_page()
	}
}

impl Default for SuspenseBoundary {
	fn default() -> Self {
		Self::new()
	}
}

impl IntoPage for SuspenseBoundary {
	fn into_page(self) -> Page {
		self.render()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::reactive::{ResourceState, Signal};
	use rstest::rstest;

	/// Mock resource tracker for testing without WASM.
	///
	/// Uses a `Signal<ResourceState>` internally so the loading state
	/// can be observed in a non-WASM test environment.
	struct MockResource {
		state: Signal<ResourceState<String, String>>,
	}

	impl MockResource {
		fn new(state: ResourceState<String, String>) -> Self {
			Self {
				state: Signal::new(state),
			}
		}
	}

	impl ResourceTracker for MockResource {
		fn is_loading(&self) -> bool {
			self.state.get().is_loading()
		}
	}

	#[rstest]
	fn suspense_boundary_defaults_to_empty() {
		// Arrange
		let boundary = SuspenseBoundary::new();

		// Act
		let page = boundary.render();

		// Assert
		let html = page.render_to_string();
		assert!(html.contains("data-rh-suspense=\"resolved\""));
	}

	#[rstest]
	fn suspense_boundary_renders_content_when_resource_ready() {
		// Arrange
		let mock = MockResource::new(ResourceState::Success("loaded".to_string()));
		let boundary = SuspenseBoundary::new()
			.fallback(|| Page::text("Loading..."))
			.track_custom(mock)
			.content(|| PageElement::new("p").child("Hello").into_page());

		// Act
		let page = boundary.render();
		let html = page.render_to_string();

		// Assert
		assert!(html.contains("<p>Hello</p>"));
		assert!(html.contains("data-rh-suspense=\"resolved\""));
		assert!(!html.contains("Loading..."));
	}

	#[rstest]
	fn suspense_boundary_renders_content_when_resource_error() {
		// Arrange
		let mock = MockResource::new(ResourceState::Error("failed".to_string()));
		let boundary = SuspenseBoundary::new()
			.fallback(|| Page::text("Loading..."))
			.track_custom(mock)
			.content(|| PageElement::new("p").child("Error view").into_page());

		// Act
		let page = boundary.render();
		let html = page.render_to_string();

		// Assert
		assert!(html.contains("<p>Error view</p>"));
		assert!(html.contains("data-rh-suspense=\"resolved\""));
	}

	#[rstest]
	fn suspense_boundary_any_loading_with_no_trackers() {
		// Arrange
		let boundary = SuspenseBoundary::new();

		// Act & Assert
		assert!(!boundary.any_loading());
	}

	#[rstest]
	fn suspense_boundary_any_loading_with_mixed_states() {
		// Arrange
		let ready = MockResource::new(ResourceState::Success("ok".to_string()));
		let loading = MockResource::new(ResourceState::Loading);
		let boundary = SuspenseBoundary::new()
			.track_custom(ready)
			.track_custom(loading);

		// Act & Assert
		assert!(boundary.any_loading());
	}

	#[rstest]
	fn suspense_boundary_any_loading_all_resolved() {
		// Arrange
		let r1 = MockResource::new(ResourceState::Success("a".to_string()));
		let r2 = MockResource::new(ResourceState::Error("b".to_string()));
		let boundary = SuspenseBoundary::new().track_custom(r1).track_custom(r2);

		// Act & Assert
		assert!(!boundary.any_loading());
	}

	#[rstest]
	fn nested_suspense_boundaries_render_independently() {
		// Arrange
		let inner_mock = MockResource::new(ResourceState::Success("inner data".to_string()));
		let inner_boundary = SuspenseBoundary::new()
			.fallback(|| Page::text("Inner loading..."))
			.track_custom(inner_mock)
			.content(|| PageElement::new("span").child("Inner content").into_page());
		let inner_html = inner_boundary.render().render_to_string();

		let outer_mock = MockResource::new(ResourceState::Success("outer data".to_string()));
		let outer_boundary = SuspenseBoundary::new()
			.fallback(|| Page::text("Outer loading..."))
			.track_custom(outer_mock)
			.content(move || Page::text(inner_html.clone()));

		// Act
		let page = outer_boundary.render();
		let html = page.render_to_string();

		// Assert
		assert!(html.contains("Inner content"));
		assert!(!html.contains("Outer loading..."));
		assert!(!html.contains("Inner loading..."));
	}

	#[rstest]
	fn ssr_renders_content_not_fallback() {
		// Arrange
		// On non-WASM (where tests run), SSR mode always renders content.
		// Even if resources report loading, SSR should skip the fallback.
		let mock = MockResource::new(ResourceState::Loading);
		let boundary = SuspenseBoundary::new()
			.fallback(|| Page::text("Should not appear in SSR"))
			.track_custom(mock)
			.content(|| PageElement::new("div").child("SSR content").into_page());

		// Act
		let page = boundary.render();
		let html = page.render_to_string();

		// Assert
		assert!(html.contains("SSR content"));
		assert!(html.contains("data-rh-suspense=\"resolved\""));
		assert!(!html.contains("Should not appear in SSR"));
	}

	#[rstest]
	fn suspense_boundary_into_page() {
		// Arrange
		let mock = MockResource::new(ResourceState::Success("data".to_string()));
		let boundary = SuspenseBoundary::new()
			.fallback(|| Page::text("Loading..."))
			.track_custom(mock)
			.content(|| PageElement::new("p").child("Done").into_page());

		// Act
		let page: Page = boundary.into_page();
		let html = page.render_to_string();

		// Assert
		assert!(html.contains("<p>Done</p>"));
	}

	#[rstest]
	fn suspense_boundary_with_custom_fallback_element() {
		// Arrange
		let mock = MockResource::new(ResourceState::Success("ok".to_string()));
		let boundary = SuspenseBoundary::new()
			.fallback(|| {
				PageElement::new("div")
					.attr("class", "spinner")
					.child("Loading...")
					.into_page()
			})
			.track_custom(mock)
			.content(|| PageElement::new("main").child("Loaded").into_page());

		// Act
		let page = boundary.render();
		let html = page.render_to_string();

		// Assert
		assert!(html.contains("<main>Loaded</main>"));
	}

	#[rstest]
	fn suspense_boundary_builder_api() {
		// Arrange & Act
		let boundary = SuspenseBoundary::new()
			.fallback(|| Page::text("Loading..."))
			.content(|| PageElement::new("p").child("Content").into_page());

		// Assert
		let html = boundary.render().render_to_string();
		assert!(html.contains("<p>Content</p>"));
	}

	#[rstest]
	fn suspense_boundary_multiple_resources_all_loading() {
		// Arrange
		let r1 = MockResource::new(ResourceState::Loading);
		let r2 = MockResource::new(ResourceState::Loading);
		let r3 = MockResource::new(ResourceState::Loading);
		let boundary = SuspenseBoundary::new()
			.track_custom(r1)
			.track_custom(r2)
			.track_custom(r3);

		// Act & Assert
		assert!(boundary.any_loading());
	}

	#[rstest]
	fn suspense_boundary_marker_attributes() {
		// Arrange
		let boundary = SuspenseBoundary::new().content(|| Page::text("test"));

		// Act
		let html = boundary.render().render_to_string();

		// Assert
		assert!(html.contains("data-rh-suspense=\"resolved\""));
	}
}
