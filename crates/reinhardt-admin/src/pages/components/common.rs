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
//! These components use the `page!` macro DSL for SSR compatibility and Router integration.
//! Interactive components with event handlers will be hydrated on the client side.

use std::sync::Arc;

use reinhardt_pages::Signal;
use reinhardt_pages::component::Page;
use reinhardt_pages::page;

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
	/// Warning action button (yellow/amber)
	Warning,
}

impl ButtonVariant {
	/// Get CSS class for this variant
	pub fn class(&self) -> &'static str {
		match self {
			ButtonVariant::Primary => "admin-btn-primary",
			ButtonVariant::Secondary => "admin-btn-secondary",
			ButtonVariant::Success => "admin-btn-success",
			ButtonVariant::Danger => "admin-btn-danger",
			ButtonVariant::Warning => "admin-btn-warning",
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
pub fn button(text: &str, variant: ButtonVariant, disabled: bool, on_click: Signal<bool>) -> Page {
	let classes = format!("admin-btn {}", variant.class());
	let text = text.to_string();

	if disabled {
		return page!(|| {
			button {
				class: classes,
				type: "button",
				disabled: true,
				{ text }
			}
		})();
	}

	page!(|_on_click: Signal<bool>| {
		button {
			class: classes,
			type: "button",
			@click: move |_| {
						_on_click.set(true);
					},
			{ text }
		}
	})(on_click)
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
	page!(|| {
		div {
			class: "flex justify-center items-center py-12",
			div {
				class: "admin-spinner",
				role: "status",
				span {
					class: "sr-only",
					"Loading..."
				}
			}
		}
	})()
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
	let message = message.to_string();

	if dismissible {
		page!(|| {
			div {
				class: "admin-alert admin-alert-danger flex items-start justify-between animate__animated animate__shakeX",
				role: "alert",
				span {
					{ message }
				}
				button {
					class: "ml-4 text-red-400 hover:text-red-600 cursor-pointer",
					type: "button",
					aria_label: "Close",
					"×"
				}
			}
		})()
	} else {
		page!(|| {
			div {
				class: "admin-alert admin-alert-danger animate__animated animate__shakeX",
				role: "alert",
				{ message }
			}
		})()
	}
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

	page!(|| {
		div {
			class: "flex justify-center gap-1 mt-6",
			{ nav_items }
		}
	})()
}

/// Helper function to create a pagination item with event handler
fn create_page_item<F>(
	text: &str,
	disabled: bool,
	active: bool,
	signal: Signal<u64>,
	handler: F,
) -> Page
where
	F: Fn(Signal<u64>) + 'static,
{
	let text = text.to_string();

	if disabled {
		page!(|| {
			span {
				class: "admin-page-link admin-page-link-disabled",
				aria_disabled: "true",
				tabindex: (- 1_i32).to_string(),
				{ text }
			}
		})()
	} else if active {
		page!(|| {
			span {
				class: "admin-page-link admin-page-link-active",
				aria_current: "page",
				{ text }
			}
		})()
	} else {
		let handler: Arc<dyn Fn(Signal<u64>)> = Arc::new(handler);
		page!(|_signal: Signal<u64>, _handler: Arc<dyn Fn(Signal<u64>)>| {
			a {
				class: "admin-page-link",
				href: "#",
				@click: move |_| {
							_handler(_signal.clone());
						},
				{ text }
			}
		})(signal, handler)
	}
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
	let placeholder = placeholder.to_string();

	page!(|| {
		div {
			class: "flex",
			span {
				class: "flex items-center px-3 bg-slate-100 border border-r-0 border-slate-200 rounded-l-lg text-slate-400 text-sm",
				"🔍"
			}
			input {
				class: "admin-input rounded-l-none border-l-0",
				type: "text",
				placeholder: placeholder,
				value: current_value,
			}
		}
	})()
}
