//! Form Widgets and HTML Rendering
//!
//! This module provides HTML rendering utilities for form fields.
//! Widgets handle rendering of form fields as HTML, supporting
//! various CSS frameworks like Bootstrap and Tailwind.
//!
//! ## Server-side only
//!
//! This module is only available on server-side (non-WASM) targets.
//! For client-side (WASM) rendering, use DOM APIs directly through
//! the `FormComponent` in this crate.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Widget type enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WidgetType {
	/// Text input field
	TextInput,
	/// Password input field
	PasswordInput,
	/// Email input field
	EmailInput,
	/// Number input field
	NumberInput,
	/// Date input field
	DateInput,
	/// Time input field
	TimeInput,
	/// DateTime input field
	DateTimeInput,
	/// Textarea (multiline text)
	Textarea,
	/// Checkbox input
	Checkbox,
	/// Select dropdown
	Select,
	/// Multiple select dropdown
	SelectMultiple,
	/// Radio button
	Radio,
	/// Radio button group
	RadioSelect,
	/// Checkbox group for multiple selection
	CheckboxSelectMultiple,
	/// File input
	FileInput,
	/// Hidden input
	HiddenInput,
	/// Split date/time inputs
	SplitDateTime,
	/// Date select with year/month/day dropdowns
	SelectDate,
}

/// Base widget trait
pub trait Widget: Send + Sync {
	/// Get the widget type
	fn widget_type(&self) -> WidgetType;

	/// Render the widget as HTML
	fn render(&self, name: &str, value: Option<&str>, attrs: &HashMap<String, String>) -> String;

	/// Render the widget with choices (for select widgets)
	fn render_with_choices(
		&self,
		name: &str,
		value: Option<&str>,
		attrs: &HashMap<String, String>,
		_choices: &[(String, String)],
	) -> String {
		// Default implementation for non-select widgets
		self.render(name, value, attrs)
	}
}

/// Text input widget
#[derive(Debug, Clone)]
pub struct TextInput {
	input_type: String,
}

impl TextInput {
	/// Create a new text input widget
	pub fn new() -> Self {
		Self {
			input_type: "text".to_string(),
		}
	}

	/// Create a password input widget
	pub fn password() -> Self {
		Self {
			input_type: "password".to_string(),
		}
	}

	/// Create an email input widget
	pub fn email() -> Self {
		Self {
			input_type: "email".to_string(),
		}
	}

	/// Create a number input widget
	pub fn number() -> Self {
		Self {
			input_type: "number".to_string(),
		}
	}
}

impl Default for TextInput {
	fn default() -> Self {
		Self::new()
	}
}

impl Widget for TextInput {
	fn widget_type(&self) -> WidgetType {
		WidgetType::TextInput
	}

	fn render(&self, name: &str, value: Option<&str>, attrs: &HashMap<String, String>) -> String {
		let mut html = format!(
			r#"<input type="{}" name="{}""#,
			self.input_type,
			html_escape(name)
		);

		if let Some(v) = value {
			html.push_str(&format!(r#" value="{}""#, html_escape(v)));
		}

		for (key, val) in attrs {
			html.push_str(&format!(r#" {}="{}""#, key, html_escape(val)));
		}

		html.push_str(" />");
		html
	}
}

/// Date input widget
#[derive(Debug, Clone)]
pub struct DateInput;

impl DateInput {
	/// Create a new date input widget
	pub fn new() -> Self {
		Self
	}
}

impl Default for DateInput {
	fn default() -> Self {
		Self::new()
	}
}

impl Widget for DateInput {
	fn widget_type(&self) -> WidgetType {
		WidgetType::DateInput
	}

	fn render(&self, name: &str, value: Option<&str>, attrs: &HashMap<String, String>) -> String {
		let mut html = format!(r#"<input type="date" name="{}""#, html_escape(name));

		if let Some(v) = value {
			html.push_str(&format!(r#" value="{}""#, html_escape(v)));
		}

		for (key, val) in attrs {
			html.push_str(&format!(r#" {}="{}""#, key, html_escape(val)));
		}

		html.push_str(" />");
		html
	}
}

/// Checkbox input widget
#[derive(Debug, Clone)]
pub struct CheckboxInput;

impl CheckboxInput {
	/// Create a new checkbox input widget
	pub fn new() -> Self {
		Self
	}
}

impl Default for CheckboxInput {
	fn default() -> Self {
		Self::new()
	}
}

impl Widget for CheckboxInput {
	fn widget_type(&self) -> WidgetType {
		WidgetType::Checkbox
	}

	fn render(&self, name: &str, value: Option<&str>, attrs: &HashMap<String, String>) -> String {
		let mut html = format!(r#"<input type="checkbox" name="{}""#, html_escape(name));

		if value == Some("true") || value == Some("1") || value == Some("on") {
			html.push_str(" checked");
		}

		for (key, val) in attrs {
			html.push_str(&format!(r#" {}="{}""#, key, html_escape(val)));
		}

		html.push_str(" />");
		html
	}
}

/// Select widget
#[derive(Debug, Clone)]
pub struct Select;

impl Select {
	/// Create a new select widget
	pub fn new() -> Self {
		Self
	}
}

impl Default for Select {
	fn default() -> Self {
		Self::new()
	}
}

impl Widget for Select {
	fn widget_type(&self) -> WidgetType {
		WidgetType::Select
	}

	fn render(&self, name: &str, value: Option<&str>, attrs: &HashMap<String, String>) -> String {
		self.render_with_choices(name, value, attrs, &[])
	}

	fn render_with_choices(
		&self,
		name: &str,
		value: Option<&str>,
		attrs: &HashMap<String, String>,
		choices: &[(String, String)],
	) -> String {
		let mut html = format!(r#"<select name="{}""#, html_escape(name));

		for (key, val) in attrs {
			html.push_str(&format!(r#" {}="{}""#, key, html_escape(val)));
		}

		html.push('>');

		for (choice_value, choice_label) in choices {
			html.push_str("<option");
			html.push_str(&format!(r#" value="{}""#, html_escape(choice_value)));

			if Some(choice_value.as_str()) == value {
				html.push_str(" selected");
			}

			html.push('>');
			html.push_str(&html_escape(choice_label));
			html.push_str("</option>");
		}

		html.push_str("</select>");
		html
	}
}

/// Select multiple widget
#[derive(Debug, Clone)]
pub struct SelectMultiple;

impl SelectMultiple {
	/// Create a new multiple select widget
	pub fn new() -> Self {
		Self
	}
}

impl Default for SelectMultiple {
	fn default() -> Self {
		Self::new()
	}
}

impl Widget for SelectMultiple {
	fn widget_type(&self) -> WidgetType {
		WidgetType::SelectMultiple
	}

	fn render(&self, name: &str, value: Option<&str>, attrs: &HashMap<String, String>) -> String {
		self.render_with_choices(name, value, attrs, &[])
	}

	fn render_with_choices(
		&self,
		name: &str,
		value: Option<&str>,
		attrs: &HashMap<String, String>,
		choices: &[(String, String)],
	) -> String {
		let selected_values: Vec<&str> = value.map(|v| v.split(',').collect()).unwrap_or_default();

		let mut html = format!(r#"<select name="{}" multiple"#, html_escape(name));

		for (key, val) in attrs {
			html.push_str(&format!(r#" {}="{}""#, key, html_escape(val)));
		}

		html.push('>');

		for (choice_value, choice_label) in choices {
			html.push_str("<option");
			html.push_str(&format!(r#" value="{}""#, html_escape(choice_value)));

			if selected_values.contains(&choice_value.as_str()) {
				html.push_str(" selected");
			}

			html.push('>');
			html.push_str(&html_escape(choice_label));
			html.push_str("</option>");
		}

		html.push_str("</select>");
		html
	}
}

/// Radio select widget
#[derive(Debug, Clone)]
pub struct RadioSelect;

impl RadioSelect {
	/// Create a new radio select widget
	pub fn new() -> Self {
		Self
	}
}

impl Default for RadioSelect {
	fn default() -> Self {
		Self::new()
	}
}

impl Widget for RadioSelect {
	fn widget_type(&self) -> WidgetType {
		WidgetType::RadioSelect
	}

	fn render(&self, name: &str, value: Option<&str>, attrs: &HashMap<String, String>) -> String {
		self.render_with_choices(name, value, attrs, &[])
	}

	fn render_with_choices(
		&self,
		name: &str,
		value: Option<&str>,
		attrs: &HashMap<String, String>,
		choices: &[(String, String)],
	) -> String {
		let mut html = String::new();
		let escaped_name = html_escape(name);

		for (i, (choice_value, choice_label)) in choices.iter().enumerate() {
			let input_id = format!("{}_{}", escaped_name, i);

			html.push_str(&format!(
				r#"<label for="{}"><input type="radio" name="{}" id="{}" value="{}""#,
				input_id,
				escaped_name,
				input_id,
				html_escape(choice_value)
			));

			if Some(choice_value.as_str()) == value {
				html.push_str(" checked");
			}

			for (key, val) in attrs {
				html.push_str(&format!(r#" {}="{}""#, key, html_escape(val)));
			}

			html.push_str(" /> ");
			html.push_str(&html_escape(choice_label));
			html.push_str("</label>");
		}

		html
	}
}

/// Checkbox select multiple widget
#[derive(Debug, Clone)]
pub struct CheckboxSelectMultiple;

impl CheckboxSelectMultiple {
	/// Create a new checkbox select multiple widget
	pub fn new() -> Self {
		Self
	}
}

impl Default for CheckboxSelectMultiple {
	fn default() -> Self {
		Self::new()
	}
}

impl Widget for CheckboxSelectMultiple {
	fn widget_type(&self) -> WidgetType {
		WidgetType::CheckboxSelectMultiple
	}

	fn render(&self, name: &str, value: Option<&str>, attrs: &HashMap<String, String>) -> String {
		self.render_with_choices(name, value, attrs, &[])
	}

	fn render_with_choices(
		&self,
		name: &str,
		value: Option<&str>,
		attrs: &HashMap<String, String>,
		choices: &[(String, String)],
	) -> String {
		let selected_values: Vec<&str> = value.map(|v| v.split(',').collect()).unwrap_or_default();

		let mut html = String::new();
		let escaped_name = html_escape(name);

		for (i, (choice_value, choice_label)) in choices.iter().enumerate() {
			let input_id = format!("{}_{}", escaped_name, i);

			html.push_str(&format!(
				r#"<label for="{}"><input type="checkbox" name="{}" id="{}" value="{}""#,
				input_id,
				escaped_name,
				input_id,
				html_escape(choice_value)
			));

			if selected_values.contains(&choice_value.as_str()) {
				html.push_str(" checked");
			}

			for (key, val) in attrs {
				html.push_str(&format!(r#" {}="{}""#, key, html_escape(val)));
			}

			html.push_str(" /> ");
			html.push_str(&html_escape(choice_label));
			html.push_str("</label>");
		}

		html
	}
}

/// File input widget
#[derive(Debug, Clone)]
pub struct FileInput;

impl FileInput {
	/// Create a new file input widget
	pub fn new() -> Self {
		Self
	}
}

impl Default for FileInput {
	fn default() -> Self {
		Self::new()
	}
}

impl Widget for FileInput {
	fn widget_type(&self) -> WidgetType {
		WidgetType::FileInput
	}

	fn render(&self, name: &str, _value: Option<&str>, attrs: &HashMap<String, String>) -> String {
		let mut html = format!(r#"<input type="file" name="{}""#, html_escape(name));

		for (key, val) in attrs {
			html.push_str(&format!(r#" {}="{}""#, key, html_escape(val)));
		}

		html.push_str(" />");
		html
	}
}

/// Split date time widget (separate inputs for date and time)
#[derive(Debug, Clone)]
pub struct SplitDateTimeWidget;

impl SplitDateTimeWidget {
	/// Create a new split date/time widget
	pub fn new() -> Self {
		Self
	}
}

impl Default for SplitDateTimeWidget {
	fn default() -> Self {
		Self::new()
	}
}

impl Widget for SplitDateTimeWidget {
	fn widget_type(&self) -> WidgetType {
		WidgetType::SplitDateTime
	}

	fn render(&self, name: &str, value: Option<&str>, attrs: &HashMap<String, String>) -> String {
		let (date_value, time_value) = value.and_then(|v| v.split_once('T')).unwrap_or(("", ""));

		let mut html = String::new();
		let escaped_name = html_escape(name);

		// Date input
		html.push_str(&format!(
			r#"<input type="date" name="{}_0" value="{}""#,
			escaped_name,
			html_escape(date_value)
		));
		for (key, val) in attrs {
			if key.starts_with("date_") {
				let date_attr = key.strip_prefix("date_").unwrap();
				html.push_str(&format!(r#" {}="{}""#, date_attr, html_escape(val)));
			}
		}
		html.push_str(" /> ");

		// Time input
		html.push_str(&format!(
			r#"<input type="time" name="{}_1" value="{}""#,
			escaped_name,
			html_escape(time_value)
		));
		for (key, val) in attrs {
			if key.starts_with("time_") {
				let time_attr = key.strip_prefix("time_").unwrap();
				html.push_str(&format!(r#" {}="{}""#, time_attr, html_escape(val)));
			}
		}
		html.push_str(" />");

		html
	}
}

/// Select date widget (separate selects for year, month, day)
#[derive(Debug, Clone)]
pub struct SelectDateWidget {
	years: Vec<i32>,
}

impl SelectDateWidget {
	/// Create a new select date widget with default year range
	pub fn new() -> Self {
		let current_year = 2025;
		let years = (current_year - 100..=current_year + 10).collect();
		Self { years }
	}

	/// Create with custom year range
	pub fn with_years(years: Vec<i32>) -> Self {
		Self { years }
	}
}

impl Default for SelectDateWidget {
	fn default() -> Self {
		Self::new()
	}
}

impl Widget for SelectDateWidget {
	fn widget_type(&self) -> WidgetType {
		WidgetType::SelectDate
	}

	fn render(&self, name: &str, value: Option<&str>, attrs: &HashMap<String, String>) -> String {
		let (year, month, day) = value
			.and_then(|v| {
				let parts: Vec<&str> = v.split('-').collect();
				if parts.len() == 3 {
					Some((parts[0], parts[1], parts[2]))
				} else {
					None
				}
			})
			.unwrap_or(("", "", ""));

		let mut html = String::new();
		let escaped_name = html_escape(name);

		// Year select
		html.push_str(&format!(r#"<select name="{}_year""#, escaped_name));
		for (key, val) in attrs {
			if key.starts_with("year_") {
				let year_attr = key.strip_prefix("year_").unwrap();
				html.push_str(&format!(r#" {}="{}""#, year_attr, html_escape(val)));
			}
		}
		html.push('>');
		for y in &self.years {
			html.push_str(&format!(r#"<option value="{}""#, y));
			if year == y.to_string() {
				html.push_str(" selected");
			}
			html.push('>');
			html.push_str(&y.to_string());
			html.push_str("</option>");
		}
		html.push_str("</select> ");

		// Month select
		html.push_str(&format!(r#"<select name="{}_month""#, escaped_name));
		for (key, val) in attrs {
			if key.starts_with("month_") {
				let month_attr = key.strip_prefix("month_").unwrap();
				html.push_str(&format!(r#" {}="{}""#, month_attr, html_escape(val)));
			}
		}
		html.push('>');
		for m in 1..=12 {
			html.push_str(&format!(r#"<option value="{:02}""#, m));
			if month == format!("{:02}", m) {
				html.push_str(" selected");
			}
			html.push('>');
			html.push_str(&format!("{:02}", m));
			html.push_str("</option>");
		}
		html.push_str("</select> ");

		// Day select
		html.push_str(&format!(r#"<select name="{}_day""#, escaped_name));
		for (key, val) in attrs {
			if key.starts_with("day_") {
				let day_attr = key.strip_prefix("day_").unwrap();
				html.push_str(&format!(r#" {}="{}""#, day_attr, html_escape(val)));
			}
		}
		html.push('>');
		for d in 1..=31 {
			html.push_str(&format!(r#"<option value="{:02}""#, d));
			if day == format!("{:02}", d) {
				html.push_str(" selected");
			}
			html.push('>');
			html.push_str(&format!("{:02}", d));
			html.push_str("</option>");
		}
		html.push_str("</select>");

		html
	}
}

/// HTML escape utility
pub fn html_escape(s: &str) -> String {
	s.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
		.replace('"', "&quot;")
		.replace('\'', "&#x27;")
}

/// CSS Framework rendering styles
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CssFramework {
	/// Bootstrap 5 CSS framework
	Bootstrap5,
	/// Tailwind CSS framework
	TailwindCSS,
	/// No CSS framework (plain HTML)
	None,
}

/// Widget attribute builder for data-* and ARIA attributes
#[derive(Debug, Clone, Default)]
pub struct WidgetAttrs {
	attrs: HashMap<String, String>,
}

impl WidgetAttrs {
	/// Create a new empty attribute builder
	pub fn new() -> Self {
		Self {
			attrs: HashMap::new(),
		}
	}

	/// Add a custom attribute
	pub fn attr(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
		self.attrs.insert(key.into(), value.into());
		self
	}

	/// Add a data-* attribute
	pub fn data(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
		self.attrs
			.insert(format!("data-{}", key.into()), value.into());
		self
	}

	/// Add an ARIA attribute
	pub fn aria(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
		self.attrs
			.insert(format!("aria-{}", key.into()), value.into());
		self
	}

	/// Add a CSS class
	pub fn class(mut self, value: impl Into<String>) -> Self {
		let class_value = value.into();
		if let Some(existing) = self.attrs.get_mut("class") {
			existing.push(' ');
			existing.push_str(&class_value);
		} else {
			self.attrs.insert("class".to_string(), class_value);
		}
		self
	}

	/// Add an ID attribute
	pub fn id(mut self, value: impl Into<String>) -> Self {
		self.attrs.insert("id".to_string(), value.into());
		self
	}

	/// Add a placeholder attribute
	pub fn placeholder(mut self, value: impl Into<String>) -> Self {
		self.attrs.insert("placeholder".to_string(), value.into());
		self
	}

	/// Add a required attribute
	pub fn required(mut self) -> Self {
		self.attrs
			.insert("required".to_string(), "required".to_string());
		self
	}

	/// Add a disabled attribute
	pub fn disabled(mut self) -> Self {
		self.attrs
			.insert("disabled".to_string(), "disabled".to_string());
		self
	}

	/// Add a readonly attribute
	pub fn readonly(mut self) -> Self {
		self.attrs
			.insert("readonly".to_string(), "readonly".to_string());
		self
	}

	/// Build the attributes map
	pub fn build(self) -> HashMap<String, String> {
		self.attrs
	}
}

/// Bootstrap 5 widget renderer
pub struct BootstrapRenderer;

impl BootstrapRenderer {
	/// Get Bootstrap 5 CSS classes for form controls
	pub fn form_control_class() -> &'static str {
		"form-control"
	}

	/// Get Bootstrap 5 CSS classes for form check (checkbox/radio)
	pub fn form_check_class() -> &'static str {
		"form-check"
	}

	/// Get Bootstrap 5 CSS classes for form check input
	pub fn form_check_input_class() -> &'static str {
		"form-check-input"
	}

	/// Get Bootstrap 5 CSS classes for form select
	pub fn form_select_class() -> &'static str {
		"form-select"
	}

	/// Render a text input with Bootstrap 5 classes
	pub fn render_text_input(
		name: &str,
		value: Option<&str>,
		mut attrs: HashMap<String, String>,
	) -> String {
		Self::add_class(&mut attrs, Self::form_control_class());
		TextInput::new().render(name, value, &attrs)
	}

	/// Render a select with Bootstrap 5 classes
	pub fn render_select(
		name: &str,
		value: Option<&str>,
		mut attrs: HashMap<String, String>,
		choices: &[(String, String)],
	) -> String {
		Self::add_class(&mut attrs, Self::form_select_class());
		Select::new().render_with_choices(name, value, &attrs, choices)
	}

	/// Render a checkbox with Bootstrap 5 classes
	pub fn render_checkbox(
		name: &str,
		value: Option<&str>,
		mut attrs: HashMap<String, String>,
	) -> String {
		Self::add_class(&mut attrs, Self::form_check_input_class());
		let input_html = CheckboxInput::new().render(name, value, &attrs);
		format!(r#"<div class="form-check">{}</div>"#, input_html)
	}

	fn add_class(attrs: &mut HashMap<String, String>, class: &str) {
		if let Some(existing) = attrs.get_mut("class") {
			existing.push(' ');
			existing.push_str(class);
		} else {
			attrs.insert("class".to_string(), class.to_string());
		}
	}
}

/// Tailwind CSS widget renderer
pub struct TailwindRenderer;

impl TailwindRenderer {
	/// Get Tailwind CSS classes for form controls
	pub fn form_control_class() -> &'static str {
		"block w-full rounded-md border-gray-300 shadow-sm focus:border-indigo-500 focus:ring-indigo-500 sm:text-sm"
	}

	/// Get Tailwind CSS classes for form check (checkbox/radio)
	pub fn form_check_class() -> &'static str {
		"h-4 w-4 rounded border-gray-300 text-indigo-600 focus:ring-indigo-500"
	}

	/// Get Tailwind CSS classes for form select
	pub fn form_select_class() -> &'static str {
		"block w-full rounded-md border-gray-300 py-2 pl-3 pr-10 text-base focus:border-indigo-500 focus:outline-none focus:ring-indigo-500 sm:text-sm"
	}

	/// Render a text input with Tailwind CSS classes
	pub fn render_text_input(
		name: &str,
		value: Option<&str>,
		mut attrs: HashMap<String, String>,
	) -> String {
		Self::add_class(&mut attrs, Self::form_control_class());
		TextInput::new().render(name, value, &attrs)
	}

	/// Render a select with Tailwind CSS classes
	pub fn render_select(
		name: &str,
		value: Option<&str>,
		mut attrs: HashMap<String, String>,
		choices: &[(String, String)],
	) -> String {
		Self::add_class(&mut attrs, Self::form_select_class());
		Select::new().render_with_choices(name, value, &attrs, choices)
	}

	/// Render a checkbox with Tailwind CSS classes
	pub fn render_checkbox(
		name: &str,
		value: Option<&str>,
		mut attrs: HashMap<String, String>,
	) -> String {
		Self::add_class(&mut attrs, Self::form_check_class());
		CheckboxInput::new().render(name, value, &attrs)
	}

	fn add_class(attrs: &mut HashMap<String, String>, class: &str) {
		if let Some(existing) = attrs.get_mut("class") {
			existing.push(' ');
			existing.push_str(class);
		} else {
			attrs.insert("class".to_string(), class.to_string());
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_text_input_render() {
		let widget = TextInput::new();
		let html = widget.render("username", Some("john"), &HashMap::new());
		assert!(html.contains(r#"type="text""#));
		assert!(html.contains(r#"name="username""#));
		assert!(html.contains(r#"value="john""#));
	}

	#[test]
	fn test_select_render() {
		let widget = Select::new();
		let choices = vec![
			("1".to_string(), "Option 1".to_string()),
			("2".to_string(), "Option 2".to_string()),
		];
		let html = widget.render_with_choices("choice", Some("2"), &HashMap::new(), &choices);
		assert!(html.contains(r#"<select name="choice""#));
		assert!(html.contains(r#"value="1""#));
		assert!(html.contains(r#"value="2" selected"#));
	}

	#[test]
	fn test_checkbox_input_render() {
		let widget = CheckboxInput::new();
		let html = widget.render("agree", Some("true"), &HashMap::new());
		assert!(html.contains(r#"type="checkbox""#));
		assert!(html.contains("checked"));
	}

	#[test]
	fn test_radio_select_render() {
		let widget = RadioSelect::new();
		let choices = vec![
			("male".to_string(), "Male".to_string()),
			("female".to_string(), "Female".to_string()),
		];
		let html = widget.render_with_choices("gender", Some("female"), &HashMap::new(), &choices);
		assert!(html.contains(r#"type="radio""#));
		assert!(html.contains(r#"value="female" checked"#));
	}

	#[test]
	fn test_forms_widgets_html_escape() {
		assert_eq!(html_escape("<script>"), "&lt;script&gt;");
		assert_eq!(html_escape("A & B"), "A &amp; B");
		assert_eq!(html_escape(r#"He said "hi""#), "He said &quot;hi&quot;");
	}

	#[test]
	fn test_widget_attrs_builder() {
		let attrs = WidgetAttrs::new()
			.class("form-control")
			.data("id", "123")
			.aria("label", "Username")
			.placeholder("Enter username")
			.required()
			.build();

		assert_eq!(attrs.get("class"), Some(&"form-control".to_string()));
		assert_eq!(attrs.get("data-id"), Some(&"123".to_string()));
		assert_eq!(attrs.get("aria-label"), Some(&"Username".to_string()));
		assert_eq!(
			attrs.get("placeholder"),
			Some(&"Enter username".to_string())
		);
		assert_eq!(attrs.get("required"), Some(&"required".to_string()));
	}

	#[test]
	fn test_widget_attrs_multiple_classes() {
		let attrs = WidgetAttrs::new()
			.class("form-control")
			.class("is-valid")
			.build();

		assert_eq!(
			attrs.get("class"),
			Some(&"form-control is-valid".to_string())
		);
	}

	#[test]
	fn test_bootstrap_renderer() {
		let html = BootstrapRenderer::render_text_input("username", Some("john"), HashMap::new());
		assert!(html.contains("form-control"));
		assert!(html.contains(r#"name="username""#));
		assert!(html.contains(r#"value="john""#));
	}

	#[test]
	fn test_tailwind_renderer() {
		let html = TailwindRenderer::render_text_input("email", None, HashMap::new());
		assert!(html.contains("rounded-md"));
		assert!(html.contains("border-gray-300"));
		assert!(html.contains(r#"name="email""#));
	}

	#[test]
	fn test_split_datetime_widget() {
		let widget = SplitDateTimeWidget::new();
		let html = widget.render("created_at", Some("2025-10-10T14:30:00"), &HashMap::new());
		assert!(html.contains(r#"type="date""#));
		assert!(html.contains(r#"type="time""#));
		assert!(html.contains(r#"value="2025-10-10""#));
		assert!(html.contains(r#"value="14:30:00""#));
	}

	#[test]
	fn test_select_date_widget() {
		let widget = SelectDateWidget::new();
		let html = widget.render("birthday", Some("1990-05-15"), &HashMap::new());
		assert!(html.contains(r#"<select name="birthday_year""#));
		assert!(html.contains(r#"<select name="birthday_month""#));
		assert!(html.contains(r#"<select name="birthday_day""#));
		assert!(html.contains(r#"<option value="1990" selected"#));
	}

	// ============================================================================
	// XSS Prevention Tests (Issue #594)
	// ============================================================================

	#[test]
	fn test_text_input_escapes_name() {
		let widget = TextInput::new();
		// Malicious name that could break out of the name attribute
		let xss_name = "field\"><script>alert('xss')</script>";
		let html = widget.render(xss_name, Some("value"), &HashMap::new());

		// Should NOT contain raw script tag
		assert!(!html.contains("<script>"));
		// Should contain escaped version
		assert!(html.contains("&lt;script&gt;"));
		assert!(html.contains("&quot;"));
	}

	#[test]
	fn test_date_input_escapes_name() {
		let widget = DateInput::new();
		let xss_name = "date\"><script>alert('xss')</script>";
		let html = widget.render(xss_name, None, &HashMap::new());

		assert!(!html.contains("<script>"));
		assert!(html.contains("&lt;script&gt;"));
	}

	#[test]
	fn test_checkbox_input_escapes_name() {
		let widget = CheckboxInput::new();
		let xss_name = "agree\"><script>alert('xss')</script>";
		let html = widget.render(xss_name, Some("true"), &HashMap::new());

		assert!(!html.contains("<script>"));
		assert!(html.contains("&lt;script&gt;"));
	}

	#[test]
	fn test_select_escapes_name() {
		let widget = Select::new();
		let choices = vec![("1".to_string(), "Option 1".to_string())];
		let xss_name = "choice\"><script>alert('xss')</script>";
		let html = widget.render_with_choices(xss_name, None, &HashMap::new(), &choices);

		assert!(!html.contains("<script>"));
		assert!(html.contains("&lt;script&gt;"));
	}

	#[test]
	fn test_radio_select_escapes_name() {
		let widget = RadioSelect::new();
		let choices = vec![("male".to_string(), "Male".to_string())];
		let xss_name = "gender\"><script>alert('xss')</script>";
		let html = widget.render_with_choices(xss_name, None, &HashMap::new(), &choices);

		assert!(!html.contains("<script>"));
		assert!(html.contains("&lt;script&gt;"));
	}

	#[test]
	fn test_file_input_escapes_name() {
		let widget = FileInput::new();
		let xss_name = "upload\"><script>alert('xss')</script>";
		let html = widget.render(xss_name, None, &HashMap::new());

		assert!(!html.contains("<script>"));
		assert!(html.contains("&lt;script&gt;"));
	}

	#[test]
	fn test_split_datetime_escapes_name() {
		let widget = SplitDateTimeWidget::new();
		let xss_name = "date\"><script>alert('xss')</script>";
		let html = widget.render(xss_name, Some("2025-10-10T14:30:00"), &HashMap::new());

		assert!(!html.contains("<script>"));
		assert!(html.contains("&lt;script&gt;"));
	}

	#[test]
	fn test_select_date_escapes_name() {
		let widget = SelectDateWidget::new();
		let xss_name = "birthday\"><script>alert('xss')</script>";
		let html = widget.render(xss_name, Some("1990-05-15"), &HashMap::new());

		assert!(!html.contains("<script>"));
		assert!(html.contains("&lt;script&gt;"));
	}

	#[test]
	fn test_normal_names_preserved() {
		let widget = TextInput::new();
		let html = widget.render("username", Some("john"), &HashMap::new());

		// Normal names should work correctly
		assert!(html.contains(r#"name="username""#));
		assert!(html.contains(r#"value="john""#));
	}
}
