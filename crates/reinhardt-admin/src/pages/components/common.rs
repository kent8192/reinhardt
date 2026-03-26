//! Common reusable components
//!
//! Provides common UI components:
//! - `Button` - Button component
//! - `LoadingSpinner` - Loading indicator
//! - `ErrorDisplay` - Error message display
//! - `Pagination` - Pagination component
//! - `SearchBar` - Search input component
//!
//! ## Design Note
//!
//! These components use PageElement for SSR compatibility and Router integration.
//! Interactive components with event handlers will be hydrated on the client side.

use reinhardt_pages::Signal;
use reinhardt_pages::component::{IntoPage, Page, PageElement};

#[cfg(target_arch = "wasm32")]
use reinhardt_pages::dom::EventType;

#[cfg(target_arch = "wasm32")]
use std::sync::Arc;

/// Button variant styles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
	/// Primary action button (blue)
	Primary,
	/// Secondary action button (gray)
	Secondary,
	/// Success action button (green)
	Success,
	/// Danger action button (red)
	Danger,
	/// Warning action button (yellow)
	Warning,
}

impl ButtonVariant {
	/// Get CSS class for this variant
	pub fn class(&self) -> &'static str {
		match self {
			ButtonVariant::Primary => "btn-primary",
			ButtonVariant::Secondary => "btn-secondary",
			ButtonVariant::Success => "btn-success",
			ButtonVariant::Danger => "btn-danger",
			ButtonVariant::Warning => "btn-warning",
		}
	}
}

/// Button component
///
/// Displays a styled button with various variants.
/// When clicked, sets the provided Signal to true.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::pages::components::common::*;
/// use reinhardt_pages::Signal;
///
/// let clicked = Signal::new(false);
/// button("Click me", ButtonVariant::Primary, false, clicked)
/// ```
pub fn button(text: &str, variant: ButtonVariant, disabled: bool, _on_click: Signal<bool>) -> Page {
	let classes = format!("btn {}", variant.class());

	#[cfg(target_arch = "wasm32")]
	let button_view = {
		let mut element = PageElement::new("button")
			.attr("class", classes.clone())
			.attr("type", "button")
			.on(
				EventType::Click,
				Arc::new(move |_event: web_sys::Event| {
					_on_click.set(true);
				}),
			)
			.child(text.to_string());
		// Only add disabled attribute when actually disabled
		// (in HTML, disabled="false" is still truthy)
		if disabled {
			element = element.attr("disabled", "");
		}
		element
	};

	#[cfg(not(target_arch = "wasm32"))]
	let button_view = {
		// SSR: No event handler needed (will be hydrated on client)
		let mut element = PageElement::new("button")
			.attr("class", classes)
			.attr("type", "button")
			.attr("data-reactive", "true") // Marker for client-side hydration
			.child(text.to_string());
		// Only add disabled attribute when actually disabled
		// (in HTML, disabled="false" is still truthy)
		if disabled {
			element = element.attr("disabled", "");
		}
		element
	};

	button_view.into_page()
}

/// Loading spinner component
///
/// Displays a loading spinner while data is being fetched.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::pages::components::common::loading_spinner;
///
/// loading_spinner()
/// ```
pub fn loading_spinner() -> Page {
	PageElement::new("div")
		.attr("class", "loading-spinner")
		.child(
			PageElement::new("div")
				.attr("class", "spinner-border")
				.attr("role", "status")
				.child(
					PageElement::new("span")
						.attr("class", "visually-hidden")
						.child("Loading..."),
				),
		)
		.into_page()
}

/// Error display component
///
/// Displays error messages in a styled container.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::pages::components::common::error_display;
///
/// error_display("An error occurred", true)
/// ```
pub fn error_display(message: &str, dismissible: bool) -> Page {
	let mut container = PageElement::new("div")
		.attr("class", "alert alert-danger")
		.attr("role", "alert");

	if dismissible {
		container = container.child(
			PageElement::new("button")
				.attr("class", "btn-close")
				.attr("type", "button")
				.attr("data-bs-dismiss", "alert")
				.attr("aria-label", "Close"),
		);
	}

	container.child(message.to_string()).into_page()
}

/// Pagination component
///
/// Displays pagination controls for navigating through pages.
/// Updates the provided Signal when page navigation occurs.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::pages::components::common::pagination;
/// use reinhardt_pages::Signal;
///
/// let current_page = Signal::new(1u64);
/// pagination(current_page, 10)
/// ```
pub fn pagination(current_page: Signal<u64>, total_pages: u64) -> Page {
	let current_val = current_page.get();
	let mut nav_items = Vec::new();

	// Previous button
	let prev_disabled = current_val <= 1;
	nav_items.push(create_page_item(
		"Previous",
		prev_disabled,
		false,
		current_page.clone(),
		move |page: Signal<u64>| {
			let current = page.get();
			if current > 1 {
				page.set(current - 1);
			}
		},
	));

	// Page numbers (show up to 5 pages around current)
	let start = current_val.saturating_sub(2).max(1);
	let end = (current_val + 2).min(total_pages);

	for page_num in start..=end {
		let is_current = page_num == current_val;
		let page_num_str = page_num.to_string();
		nav_items.push(create_page_item(
			&page_num_str,
			false,
			is_current,
			current_page.clone(),
			move |page: Signal<u64>| {
				page.set(page_num);
			},
		));
	}

	// Next button
	let next_disabled = current_val >= total_pages;
	nav_items.push(create_page_item(
		"Next",
		next_disabled,
		false,
		current_page,
		move |page: Signal<u64>| {
			let current = page.get();
			if current < total_pages {
				page.set(current + 1);
			}
		},
	));

	PageElement::new("div")
		.attr("class", "d-flex justify-content-center")
		.child(
			PageElement::new("ul")
				.attr("class", "pagination")
				.children(nav_items),
		)
		.into_page()
}

/// Helper function to create a pagination item with event handler
fn create_page_item<F>(
	text: &str,
	disabled: bool,
	active: bool,
	_signal: Signal<u64>,
	_handler: F,
) -> Page
where
	F: Fn(Signal<u64>) + 'static,
{
	let class_name = if active {
		"page-item active"
	} else if disabled {
		"page-item disabled"
	} else {
		"page-item"
	};

	#[cfg(target_arch = "wasm32")]
	let link = {
		if disabled {
			// Disabled items use <span> to prevent click interaction and navigation
			PageElement::new("span")
				.attr("class", "page-link")
				.attr("aria-disabled", "true")
				.attr("tabindex", "-1")
				.child(text.to_string())
		} else {
			PageElement::new("a")
				.attr("class", "page-link")
				.attr("href", "#")
				.child(text.to_string())
				.on(
					EventType::Click,
					Arc::new(move |_event: web_sys::Event| {
						_handler(_signal.clone());
					}),
				)
		}
	};

	#[cfg(not(target_arch = "wasm32"))]
	let link = {
		if disabled {
			// Disabled items use <span> to prevent click interaction and navigation
			PageElement::new("span")
				.attr("class", "page-link")
				.attr("aria-disabled", "true")
				.attr("tabindex", "-1")
				.child(text.to_string())
		} else {
			// SSR: No event handler needed (will be hydrated on client)
			PageElement::new("a")
				.attr("class", "page-link")
				.attr("href", "#")
				.attr("data-reactive", "true") // Marker for client-side hydration
				.child(text.to_string())
		}
	};

	PageElement::new("li")
		.attr("class", class_name)
		.child(link)
		.into_page()
}

/// Search bar component
///
/// Displays a search input with icon.
/// The current value is displayed from the Signal.
///
/// Note: Input value updates must be handled via form binding or external mechanisms.
/// This component only displays the current Signal value.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::pages::components::common::search_bar;
/// use reinhardt_pages::Signal;
///
/// let search_value = Signal::new(String::new());
/// search_bar(search_value, "Search...")
/// ```
pub fn search_bar(value: Signal<String>, placeholder: &str) -> Page {
	let current_value = value.get();

	PageElement::new("div")
		.attr("class", "input-group")
		.child(
			PageElement::new("span")
				.attr("class", "input-group-text")
				.child(PageElement::new("i").attr("class", "bi bi-search")),
		)
		.child(
			PageElement::new("input")
				.attr("class", "form-control")
				.attr("type", "text")
				.attr("placeholder", placeholder.to_string())
				.attr("value", current_value),
		)
		.into_page()
}
