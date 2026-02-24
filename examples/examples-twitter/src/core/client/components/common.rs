//! Common UI components
//!
//! Provides reusable UI components for the Twitter clone application:
//! - `ButtonVariant` - Button style variants
//! - `button` - Styled button with click handler
//! - `loading_spinner` - Loading indicator
//! - `error_alert` - Error message display
//! - `success_alert` - Success message display
//! - `text_input` - Form text input
//! - `textarea` - Form textarea with character count
//! - `avatar` - User avatar image
//! - `theme_toggle` - Dark/light mode toggle button
//!
//! ## Design Note
//!
//! These components use the `page!` macro for JSX-like syntax.
//! Interactive components with event handlers are hydrated on the client side.
//! UnoCSS shortcuts are defined in index.html for consistent styling.

use reinhardt::pages::Signal;
use reinhardt::pages::component::{ElementView, IntoView, View};
use reinhardt::pages::page;

#[cfg(client)]
use wasm_bindgen::JsCast;

/// Button variant styles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
	/// Primary action button (brand color)
	Primary,
	/// Secondary action button (gray)
	Secondary,
	/// Success action button (green)
	Success,
	/// Danger action button (red)
	Danger,
	/// Warning action button (yellow)
	Warning,
	/// Link style button (no background)
	Link,
	/// Outline primary button
	OutlinePrimary,
	/// Ghost button (transparent with hover)
	Ghost,
}

impl ButtonVariant {
	/// Get UnoCSS shortcut class for this variant
	pub fn class(&self) -> &'static str {
		match self {
			ButtonVariant::Primary => "btn-primary",
			ButtonVariant::Secondary => "btn-secondary",
			ButtonVariant::Success => "btn-success",
			ButtonVariant::Danger => "btn-danger",
			ButtonVariant::Warning => {
				"inline-flex items-center justify-center px-4 py-2 rounded-full font-semibold text-sm transition-all duration-200 cursor-pointer bg-warning text-black hover:opacity-90 active:scale-95"
			}
			ButtonVariant::Link => "btn-ghost text-brand hover:underline",
			ButtonVariant::OutlinePrimary => {
				"btn-outline text-brand border-brand hover:bg-brand-light"
			}
			ButtonVariant::Ghost => "btn-ghost",
		}
	}
}

/// Button size variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonSize {
	/// Small button
	Small,
	/// Medium button (default)
	#[default]
	Medium,
	/// Large button
	Large,
}

impl ButtonSize {
	/// Get UnoCSS class for this size
	pub fn class(&self) -> &'static str {
		match self {
			ButtonSize::Small => "btn-sm",
			ButtonSize::Medium => "",
			ButtonSize::Large => "btn-lg",
		}
	}
}

/// Button component
///
/// Displays a styled button with various variants.
/// When clicked, sets the provided Signal to true.
///
/// # Arguments
///
/// * `text` - Button label text
/// * `variant` - Visual style variant
/// * `disabled` - Whether the button is disabled
/// * `on_click` - Signal that will be set to true when clicked
pub fn button(text: &str, variant: ButtonVariant, disabled: bool, on_click: Signal<bool>) -> View {
	button_with_size(text, variant, ButtonSize::Medium, disabled, on_click)
}

/// Button component with size option
pub fn button_with_size(
	text: &str,
	variant: ButtonVariant,
	size: ButtonSize,
	disabled: bool,
	on_click: Signal<bool>,
) -> View {
	let class = if size.class().is_empty() {
		variant.class().to_string()
	} else {
		format!("{} {}", variant.class(), size.class())
	};
	let text = text.to_string();

	#[cfg(client)]
	{
		let on_click_clone = on_click.clone();
		page!(|class: String, text: String, disabled: bool| {
			button {
				class: class,
				r#type: "button",
				disabled: disabled,
				@click: {
							let on_click = on_click_clone.clone();
							move |_event| {
								on_click.set(true);
							}
						},
				{ text }
			}
		})(class, text, disabled)
	}

	#[cfg(server)]
	{
		let _ = on_click; // Suppress unused warning
		page!(|class: String, text: String, disabled: bool| {
			button {
				class: { class },
				r#type: "button",
				disabled: disabled,
				data_reactive: "true",
				{ text }
			}
		})(class, text, disabled)
	}
}

/// Loading spinner component
///
/// Displays a modern spinner animation while content is loading.
pub fn loading_spinner() -> View {
	page!(|| {
		div {
			class: "flex items-center justify-center py-8",
			div {
				class: "spinner-md",
				role: "status",
				span {
					class: "sr-only",
					"Loading..."
				}
			}
		}
	})()
}

/// Large loading spinner with text
pub fn loading_spinner_large(message: &str) -> View {
	let message = message.to_string();
	page!(|message: String| {
		div {
			class: "flex flex-col items-center justify-center py-12 gap-4",
			div {
				class: "spinner-lg",
				role: "status",
			}
			p {
				class: "text-content-secondary text-sm",
				{ message }
			}
		}
	})(message)
}

/// Error alert component
///
/// Displays an error message in a styled alert box.
///
/// # Arguments
///
/// * `message` - Error message to display
/// * `dismissible` - Whether the alert can be dismissed
pub fn error_alert(message: &str, dismissible: bool) -> View {
	let message = message.to_string();
	if dismissible {
		page!(|message: String| {
			div {
				class: "alert-danger animate-fade-in",
				role: "alert",
				div {
					class: "flex items-start gap-3",
					svg {
						class: "w-5 h-5 flex-shrink-0 mt-0.5",
						fill: "currentColor",
						viewBox: "0 0 20 20",
						path {
							fill_rule: "evenodd",
							d: "M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z",
						}
					}
					span {
						class: "flex-1",
						{ message }
					}
					button {
						r#type: "button",
						class: "btn-icon text-danger hover:bg-red-100 dark:hover:bg-red-900/30 -mr-2 -mt-1",
						aria_label: "Close",
						svg {
							class: "w-4 h-4",
							fill: "none",
							stroke: "currentColor",
							viewBox: "0 0 24 24",
							path {
								stroke_linecap: "round",
								stroke_linejoin: "round",
								stroke_width: "2",
								d: "M6 18L18 6M6 6l12 12",
							}
						}
					}
				}
			}
		})(message)
	} else {
		page!(|message: String| {
			div {
				class: "alert-danger animate-fade-in",
				role: "alert",
				div {
					class: "flex items-start gap-3",
					svg {
						class: "w-5 h-5 flex-shrink-0 mt-0.5",
						fill: "currentColor",
						viewBox: "0 0 20 20",
						path {
							fill_rule: "evenodd",
							d: "M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z",
						}
					}
					span {
						{ message }
					}
				}
			}
		})(message)
	}
}

/// Success alert component
///
/// Displays a success message in a styled alert box.
///
/// # Arguments
///
/// * `message` - Success message to display
pub fn success_alert(message: &str) -> View {
	let message = message.to_string();
	page!(|message: String| {
		div {
			class: "alert-success animate-fade-in",
			role: "alert",
			div {
				class: "flex items-start gap-3",
				svg {
					class: "w-5 h-5 flex-shrink-0 mt-0.5",
					fill: "currentColor",
					viewBox: "0 0 20 20",
					path {
						fill_rule: "evenodd",
						d: "M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z",
					}
				}
				span {
					{ message }
				}
			}
		}
	})(message)
}

/// Warning alert component
pub fn warning_alert(message: &str) -> View {
	let message = message.to_string();
	page!(|message: String| {
		div {
			class: "alert-warning animate-fade-in",
			role: "alert",
			div {
				class: "flex items-start gap-3",
				svg {
					class: "w-5 h-5 flex-shrink-0 mt-0.5",
					fill: "currentColor",
					viewBox: "0 0 20 20",
					path {
						fill_rule: "evenodd",
						d: "M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z",
					}
				}
				span {
					{ message }
				}
			}
		}
	})(message)
}

/// Text input component
///
/// Displays a labeled text input field.
///
/// # Arguments
///
/// * `id` - Input element ID
/// * `label` - Label text
/// * `placeholder` - Placeholder text
/// * `input_type` - HTML input type (e.g., "text", "email", "password")
/// * `value` - Reactive value signal
/// * `required` - Whether the field is required
pub fn text_input(
	id: &str,
	label: &str,
	placeholder: &str,
	input_type: &str,
	value: Signal<String>,
	required: bool,
) -> View {
	let id_owned = id.to_string();
	let placeholder_owned = placeholder.to_string();
	let input_type_owned = input_type.to_string();
	let label_owned = label.to_string();

	// Clone Signal for passing to page! macro (NOT extracting values)
	let value_signal = value.clone();

	#[cfg(client)]
	{
		let value_clone = value.clone();
		page!(|id_owned: String, label_owned: String, input_type_owned: String, placeholder_owned: String, value_signal: Signal<String>, required: bool| {
			div {
				class: "mb-4",
				label {
					r#for: id_owned.clone(),
					class: "form-label",
					{ label_owned }
				}
				watch {
					input {
						r#type: input_type_owned.clone(),
						class: "form-input",
						id: id_owned.clone(),
						name: id_owned.clone(),
						placeholder: placeholder_owned.clone(),
						value: value_signal.get(),
						required: required,
						@input: {
									let value = value_clone.clone();
									move |event: web_sys::Event| {
										if let Some(target) = event.target() {
											if let Ok(input_el) = target.dyn_into::<web_sys::HtmlInputElement>() {
												value.set(input_el.value());
											}
										}
									}
								},
					}
				}
			}
		})(
			id_owned,
			label_owned,
			input_type_owned,
			placeholder_owned,
			value_signal,
			required,
		)
	}

	#[cfg(server)]
	{
		page!(|id_owned: String, label_owned: String, input_type_owned: String, placeholder_owned: String, value_signal: Signal<String>, required: bool| {
			div {
				class: "mb-4",
				label {
					r#for: { id_owned.clone() },
					class: "form-label",
					{ label_owned }
				}
				watch {
					input {
						r#type: { input_type_owned.clone() },
						class: "form-input",
						id: { id_owned.clone() },
						name: { id_owned.clone() },
						placeholder: { placeholder_owned.clone() },
						value: { value_signal.get() },
						required: required,
						data_reactive: "true",
					}
				}
			}
		})(
			id_owned,
			label_owned,
			input_type_owned,
			placeholder_owned,
			value_signal,
			required,
		)
	}
}

/// Textarea component with character count
///
/// Displays a labeled textarea with optional character limit display.
///
/// # Arguments
///
/// * `id` - Textarea element ID
/// * `label` - Label text
/// * `placeholder` - Placeholder text
/// * `rows` - Number of visible rows
/// * `max_length` - Maximum character length (0 for no limit)
/// * `value` - Reactive value signal
pub fn textarea(
	id: &str,
	label: &str,
	placeholder: &str,
	rows: u32,
	max_length: usize,
	value: Signal<String>,
) -> View {
	let id_owned = id.to_string();
	let placeholder_owned = placeholder.to_string();
	let label_owned = label.to_string();
	let rows_str = rows.to_string();
	let maxlength_attr = if max_length > 0 {
		max_length.to_string()
	} else {
		String::new()
	};
	let show_count = max_length > 0;

	// Clone Signal for passing to page! macro (NOT extracting values)
	// Two clones needed because each watch block captures the signal by move
	let value_signal = value.clone();
	let value_signal_for_count = value.clone();

	#[cfg(client)]
	{
		let value_clone = value.clone();
		page!(|id_owned: String, label_owned: String, rows_str: String, placeholder_owned: String, value_signal: Signal<String>, value_signal_for_count: Signal<String>, maxlength_attr: String, show_count: bool, max_length: usize| {
			div {
				class: "mb-4",
				label {
					r#for: id_owned.clone(),
					class: "form-label",
					{ label_owned }
				}
				watch {
					textarea {
						class: "form-textarea",
						id: id_owned.clone(),
						name: id_owned.clone(),
						rows: rows_str.clone(),
						placeholder: placeholder_owned.clone(),
						maxlength: maxlength_attr.clone(),
						@input: {
									let value = value_clone.clone();
									move |event: web_sys::Event| {
										if let Some(target) = event.target() {
											if let Ok(textarea_el) = target.dyn_into::<web_sys::HtmlTextAreaElement>() {
												value.set(textarea_el.value());
											}
										}
									}
								},
						{ value_signal.get() }
					}
				}
				watch {
					if show_count {
						div {
							class: "flex justify-end mt-1",
							span {
								class: if value_signal_for_count.get().len()> max_length { "text-danger font-medium" } else if value_signal_for_count.get().len()> max_length * 9 / 10 { "text-warning font-medium" } else { "text-content-tertiary" },
								{ format!("{}/{}", value_signal_for_count.get().len(), max_length) }
							}
						}
					}
				}
			}
		})(
			id_owned,
			label_owned,
			rows_str,
			placeholder_owned,
			value_signal,
			value_signal_for_count,
			maxlength_attr,
			show_count,
			max_length,
		)
	}

	#[cfg(server)]
	{
		page!(|id_owned: String, label_owned: String, rows_str: String, placeholder_owned: String, value_signal: Signal<String>, maxlength_attr: String, show_count: bool, max_length: usize| {
			div {
				class: "mb-4",
				label {
					r#for: { id_owned.clone() },
					class: "form-label",
					{ label_owned }
				}
				watch {
					textarea {
						class: "form-textarea",
						id: { id_owned.clone() },
						name: { id_owned.clone() },
						rows: { rows_str.clone() },
						placeholder: { placeholder_owned.clone() },
						maxlength: { maxlength_attr.clone() },
						data_reactive: "true",
						{ value_signal.get() }
					}
				}
				watch {
					if show_count {
						let char_count = value_signal.get().len();
						let count_class = if char_count > max_length {
							"text-danger font-medium"
						} else if char_count > max_length * 9 / 10 {
							"text-warning font-medium"
						} else {
							"text-content-tertiary"
						};
						let count_text = format!("{}/{}", char_count, max_length);
						div {
							class: "flex justify-end mt-1",
							span {
								class: { count_class },
								{ count_text }
							}
						}
					}
				}
			}
		})(
			id_owned,
			label_owned,
			rows_str,
			placeholder_owned,
			value_signal,
			maxlength_attr,
			show_count,
			max_length,
		)
	}
}

/// Avatar size variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AvatarSize {
	/// Small avatar (32px)
	Small,
	/// Medium avatar (48px) - default
	#[default]
	Medium,
	/// Large avatar (64px)
	Large,
	/// Extra large avatar (96px)
	ExtraLarge,
}

impl AvatarSize {
	/// Get UnoCSS class for this size
	pub fn class(&self) -> &'static str {
		match self {
			AvatarSize::Small => "avatar-sm",
			AvatarSize::Medium => "avatar-md",
			AvatarSize::Large => "avatar-lg",
			AvatarSize::ExtraLarge => "avatar-xl",
		}
	}

	/// Get size in pixels
	pub fn pixels(&self) -> u32 {
		match self {
			AvatarSize::Small => 32,
			AvatarSize::Medium => 48,
			AvatarSize::Large => 64,
			AvatarSize::ExtraLarge => 96,
		}
	}
}

/// Avatar component
///
/// Displays a user avatar image with fallback.
///
/// # Arguments
///
/// * `url` - Avatar image URL (None for default avatar)
/// * `alt` - Alt text for the image
/// * `size` - Size in pixels
pub fn avatar(url: Option<&str>, alt: &str, size: u32) -> View {
	let src = url
		.map(|s| s.to_string())
		.unwrap_or_else(|| "https://via.placeholder.com/150?text=User".to_string());
	let alt_owned = alt.to_string();
	let size_str = format!("{}px", size);

	// Use ElementView instead of page! macro for dynamic src attribute
	ElementView::new("img")
		.attr("src", src)
		.attr("alt", alt_owned)
		.attr("class", "rounded-full object-cover bg-surface-tertiary")
		.attr("width", size_str.clone())
		.attr("height", size_str)
		.into_view()
}

/// Avatar component with size enum
pub fn avatar_sized(url: Option<&str>, alt: &str, size: AvatarSize) -> View {
	let src = url
		.map(|s| s.to_string())
		.unwrap_or_else(|| "https://via.placeholder.com/150?text=User".to_string());
	let alt_owned = alt.to_string();

	ElementView::new("img")
		.attr("src", src)
		.attr("alt", alt_owned)
		.attr("class", format!("{} bg-surface-tertiary", size.class()))
		.into_view()
}

/// Theme toggle button component
///
/// Displays a button to toggle between light and dark mode.
/// Uses JavaScript to toggle the theme and persist to localStorage.
/// The click event is attached via JavaScript in index.html.
pub fn theme_toggle() -> View {
	page!(|| {
		button {
			class: "theme-toggle",
			r#type: "button",
			id: "theme-toggle-btn",
			aria_label: "Toggle theme",
			svg {
				class: "icon-sun w-5 h-5",
				fill: "none",
				stroke: "currentColor",
				viewBox: "0 0 24 24",
				path {
					stroke_linecap: "round",
					stroke_linejoin: "round",
					stroke_width: "2",
					d: "M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z",
				}
			}
			svg {
				class: "icon-moon w-5 h-5",
				fill: "none",
				stroke: "currentColor",
				viewBox: "0 0 24 24",
				path {
					stroke_linecap: "round",
					stroke_linejoin: "round",
					stroke_width: "2",
					d: "M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z",
				}
			}
		}
	})()
}

/// Empty placeholder component
///
/// Displays an empty div (useful for conditional rendering).
pub fn empty() -> View {
	page!(|| { div {} })()
}

/// Divider component
pub fn divider() -> View {
	page!(|| {
		div {
			class: "divider",
		}
	})()
}

/// Badge component
pub fn badge(text: &str, primary: bool) -> View {
	let text = text.to_string();
	let class = if primary {
		"badge-primary"
	} else {
		"badge-secondary"
	}
	.to_string();

	page!(|text: String, class: String| {
		span {
			class: class,
			{ text }
		}
	})(text, class)
}
