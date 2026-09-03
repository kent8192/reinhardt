//! Server-side projection of controlled form-element state.

use std::cell::Cell;
use std::rc::Rc;

use crate::component::{ControlBinding, ControlKind, ControlValue, Page, PageElement};

pub(crate) struct SsrControlProjection {
	pub value: Option<String>,
	pub checked: bool,
	pub textarea_text: Option<String>,
	pub selected_values: Vec<String>,
}

pub(crate) fn project(binding: Option<&ControlBinding>) -> SsrControlProjection {
	let mut projection = SsrControlProjection {
		value: None,
		checked: false,
		textarea_text: None,
		selected_values: Vec::new(),
	};

	let Some(binding) = binding else {
		return projection;
	};

	match (binding.kind(), binding.read()) {
		(ControlKind::Text | ControlKind::Number, ControlValue::Text(value)) => {
			projection.value = Some(value.clone());
			projection.textarea_text = Some(value);
		}
		(ControlKind::Checkbox | ControlKind::Radio, ControlValue::Checked(checked)) => {
			projection.checked = checked;
			if binding.kind() == ControlKind::Radio {
				projection.value = binding.radio_value().map(str::to_owned);
			}
		}
		(ControlKind::SelectOne, ControlValue::Text(value)) => {
			projection.selected_values.push(value);
		}
		(ControlKind::SelectMany, ControlValue::SelectedValues(values)) => {
			projection.selected_values = values;
		}
		_ => {}
	}

	projection
}

#[derive(Clone)]
pub(crate) struct SsrSelectionState {
	selected_values: Rc<[String]>,
	selects_many: bool,
	first_match_claimed: Rc<Cell<bool>>,
}

impl SsrSelectionState {
	pub(crate) fn new(binding: &ControlBinding, selected_values: Vec<String>) -> Self {
		Self {
			selected_values: Rc::from(selected_values),
			selects_many: binding.kind() == ControlKind::SelectMany,
			first_match_claimed: Rc::new(Cell::new(false)),
		}
	}

	pub(crate) fn fork(&self) -> Self {
		Self {
			selected_values: Rc::clone(&self.selected_values),
			selects_many: self.selects_many,
			first_match_claimed: Rc::new(Cell::new(self.first_match_claimed.get())),
		}
	}

	pub(crate) fn selects_one(&self) -> bool {
		!self.selects_many
	}

	pub(crate) fn commit_from(&self, rendered: &Self) {
		if !self.selects_many {
			self.first_match_claimed
				.set(rendered.first_match_claimed.get());
		}
	}

	pub(crate) fn reserve_pending_match(&self) {
		if !self.selects_many {
			self.first_match_claimed.set(true);
		}
	}

	pub(crate) fn fork_after_pending_match(&self) -> Self {
		let fork = self.fork();
		fork.reserve_pending_match();
		fork
	}

	pub(crate) fn option_selected(&self, element: &PageElement) -> bool {
		let matches = option_matches(element, &self.selected_values);
		if self.selects_many {
			return matches;
		}
		if !matches || self.first_match_claimed.replace(true) {
			return false;
		}
		true
	}
}

fn option_matches(element: &PageElement, selected_values: &[String]) -> bool {
	let effective_value = option_value(element);
	selected_values
		.iter()
		.any(|selected| selected == &effective_value)
}

pub(crate) fn option_value(element: &PageElement) -> String {
	element
		.attrs()
		.iter()
		.find_map(|(name, value)| name.eq_ignore_ascii_case("value").then_some(value.as_ref()))
		.map(str::to_owned)
		.unwrap_or_else(|| normalize_option_text(&collect_option_text(element.child_views())))
}

fn collect_option_text(pages: &[Page]) -> String {
	pages.iter().map(page_option_text).collect()
}

fn page_option_text(page: &Page) -> String {
	match page {
		Page::Element(element) if is_script(element.tag_name()) => String::new(),
		Page::Element(element) => collect_option_text(element.child_views()),
		Page::Text(text) => text.clone().into_owned(),
		Page::Fragment(children) => collect_option_text(children),
		Page::KeyedFragment(children) => children
			.iter()
			.map(|(_, child)| page_option_text(child))
			.collect(),
		Page::Outlet(outlet) => outlet.child().map(page_option_text).unwrap_or_default(),
		Page::Empty => String::new(),
		Page::WithHead { view, .. } => page_option_text(view),
		#[cfg(feature = "hmr")]
		Page::DevTemplate { view, .. } | Page::DevSlot { view, .. } => page_option_text(view),
		Page::ReactiveIf(_) | Page::Reactive(_) | Page::Suspense(_) | Page::Deferred(_) => {
			String::new()
		}
	}
}

fn is_script(tag: &str) -> bool {
	tag.eq_ignore_ascii_case("script") || tag.eq_ignore_ascii_case("svg:script")
}

fn normalize_option_text(text: &str) -> String {
	let mut normalized = String::with_capacity(text.len());
	let mut pending_space = false;

	for character in text.chars() {
		if matches!(character, '\t' | '\n' | '\x0c' | '\r' | ' ') {
			pending_space = !normalized.is_empty();
		} else {
			if pending_space {
				normalized.push(' ');
				pending_space = false;
			}
			normalized.push(character);
		}
	}

	normalized
}

#[cfg(test)]
mod tests {
	use std::cell::Cell;
	use std::rc::Rc;

	use super::*;
	use crate::component::IntoPage;

	#[test]
	fn inferred_value_does_not_invoke_reactive_factory() {
		// Arrange
		let renders = Rc::new(Cell::new(0));
		let render_count = Rc::clone(&renders);
		let option = PageElement::new("option")
			.child("Static")
			.child(Page::reactive(move || {
				render_count.set(render_count.get() + 1);
				Page::text(" Dynamic")
			}));

		// Act
		let value = option_value(&option);

		// Assert
		assert_eq!(value, "Static");
		assert_eq!(renders.get(), 0);
	}

	#[test]
	fn inferred_value_flattens_sanitized_elements_and_normalizes_ascii_whitespace() {
		// Arrange
		let option = PageElement::new("option")
			.child(" \tAlpha\n")
			.child(PageElement::new("script").child("html ignored").into_page())
			.child(PageElement::new("svg:script").child("ignored").into_page())
			.child(PageElement::new("math:script").child(" Gamma ").into_page())
			.child("\u{a0} Beta\x0c ");

		// Act
		let value = option_value(&option);

		// Assert
		assert_eq!(value, "Alpha html ignoredignored Gamma \u{a0} Beta");
	}
}
