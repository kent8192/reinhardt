//! Accessible two-panel selector for admin many-to-many fields.

use crate::types::RelationOption;
use crate::types::RelationSelectorLayout;
use reinhardt_pages::{Page, Signal, page};
use std::collections::HashSet;

fn merge_search_results(
	results: Vec<RelationOption>,
	chosen: &[RelationOption],
) -> Vec<RelationOption> {
	let chosen_ids: HashSet<&str> = chosen.iter().map(|option| option.value.as_str()).collect();
	let mut seen = HashSet::new();
	results
		.into_iter()
		.filter(|option| {
			!chosen_ids.contains(option.value.as_str()) && seen.insert(option.value.clone())
		})
		.collect()
}

fn add_selected(
	available: Vec<RelationOption>,
	chosen: Vec<RelationOption>,
	selected_ids: &[String],
) -> (Vec<RelationOption>, Vec<RelationOption>) {
	let selected_ids: HashSet<&str> = selected_ids.iter().map(String::as_str).collect();
	let mut chosen_ids = HashSet::new();
	let mut next_chosen = Vec::new();
	for option in chosen {
		if chosen_ids.insert(option.value.clone()) {
			next_chosen.push(option);
		}
	}

	let mut available_ids = HashSet::new();
	let mut next_available = Vec::new();
	for option in available {
		if selected_ids.contains(option.value.as_str()) {
			if chosen_ids.insert(option.value.clone()) {
				next_chosen.push(option);
			}
		} else if !chosen_ids.contains(option.value.as_str())
			&& available_ids.insert(option.value.clone())
		{
			next_available.push(option);
		}
	}

	(next_available, next_chosen)
}

fn remove_selected(
	available: Vec<RelationOption>,
	chosen: Vec<RelationOption>,
	selected_ids: &[String],
) -> (Vec<RelationOption>, Vec<RelationOption>) {
	let selected_ids: HashSet<&str> = selected_ids.iter().map(String::as_str).collect();
	let mut available_ids = HashSet::new();
	let mut next_available = Vec::new();
	for option in available {
		if available_ids.insert(option.value.clone()) {
			next_available.push(option);
		}
	}

	let mut chosen_ids = HashSet::new();
	let mut next_chosen = Vec::new();
	for option in chosen {
		if selected_ids.contains(option.value.as_str()) {
			if available_ids.insert(option.value.clone()) {
				next_available.push(option);
			}
		} else if chosen_ids.insert(option.value.clone()) {
			next_chosen.push(option);
		}
	}

	(next_available, next_chosen)
}

fn option_pages(options: Vec<RelationOption>, selected: bool) -> Vec<Page> {
	options
		.into_iter()
		.map(|option| {
			if selected {
				page!(|value: String, label: String| {
					option {
						value: value,
						selected: true,
						{ label }
					}
				})(option.value, option.label)
			} else {
				page!(|value: String, label: String| {
					option {
						value: value,
						{ label }
					}
				})(option.value, option.label)
			}
		})
		.collect()
}

fn add_highlighted(
	available: Signal<Vec<RelationOption>>,
	chosen: Signal<Vec<RelationOption>>,
	highlighted: Signal<Vec<String>>,
	focus_id: &str,
) {
	let (next_available, next_chosen) = add_selected(
		available.get_untracked(),
		chosen.get_untracked(),
		&highlighted.get_untracked(),
	);
	available.set(next_available);
	chosen.set(next_chosen);
	highlighted.set(Vec::new());
	focus_element(focus_id);
}

fn remove_highlighted(
	available: Signal<Vec<RelationOption>>,
	chosen: Signal<Vec<RelationOption>>,
	highlighted: Signal<Vec<String>>,
	focus_id: &str,
) {
	let (next_available, next_chosen) = remove_selected(
		available.get_untracked(),
		chosen.get_untracked(),
		&highlighted.get_untracked(),
	);
	available.set(next_available);
	chosen.set(next_chosen);
	highlighted.set(Vec::new());
	focus_element(focus_id);
}

#[cfg(client)]
fn focus_element(id: &str) {
	use wasm_bindgen::JsCast;

	if let Some(element) = web_sys::window()
		.and_then(|window| window.document())
		.and_then(|document| document.get_element_by_id(id))
		.and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
	{
		let _ = element.focus();
	}
}

#[cfg(not(client))]
fn focus_element(_id: &str) {}

/// Render an accessible, searchable two-panel many-to-many selector.
pub fn relation_selector(
	model_name: &str,
	field_name: &str,
	label: &str,
	layout: RelationSelectorLayout,
	available: Vec<RelationOption>,
	selected: Vec<RelationOption>,
	has_more: bool,
) -> Page {
	let chosen_initial = merge_search_results(selected, &[]);
	let available_initial = merge_search_results(available, &chosen_initial);
	let available = Signal::new(available_initial);
	let chosen = Signal::new(chosen_initial);
	let available_highlighted = Signal::new(Vec::<String>::new());
	let chosen_highlighted = Signal::new(Vec::<String>::new());
	let generation = Signal::new(0_u64);
	let status = Signal::new(if has_more {
		"Showing the first 50 matches. Refine your search.".to_string()
	} else {
		String::new()
	});

	let input_id = format!("field-{field_name}");
	let search_id = format!("{input_id}-search");
	let available_id = format!("{input_id}-available");
	let chosen_id = format!("{input_id}-chosen");
	let status_id = format!("{input_id}-status");
	let available_label_id = format!("{input_id}-available-label");
	let chosen_label_id = format!("{input_id}-chosen-label");
	let layout_class = match layout {
		RelationSelectorLayout::Horizontal => "relation-selector relation-selector--horizontal",
		RelationSelectorLayout::Vertical => "relation-selector relation-selector--vertical",
	}
	.to_string();

	let available_options =
		Page::reactive(move || Page::fragment(option_pages(available.get(), false)));
	let chosen_options = Page::reactive(move || Page::fragment(option_pages(chosen.get(), false)));
	let submitted_options =
		Page::reactive(move || Page::fragment(option_pages(chosen.get(), true)));
	let status_content = Page::reactive(move || Page::text(status.get()));

	let search_model = model_name.to_string();
	let search_field = field_name.to_string();
	let search_generation = generation;
	let search_status = status;
	let search_available = available;
	let search_chosen = chosen;
	let search_highlighted = available_highlighted;

	page!(|layout_class: String,
	 label: String,
	 input_id: String,
	 field_name: String,
	 search_id: String,
	 available_id: String,
	 chosen_id: String,
	 status_id: String,
	 available_label_id: String,
	 chosen_label_id: String,
	 available_options: Page,
	 chosen_options: Page,
	 submitted_options: Page,
	 status_content: Page,
	 search_model: String,
	 search_field: String,
	 search_generation: Signal<u64>,
	 search_status: Signal<String>,
	 search_available: Signal<Vec<RelationOption>>,
	 search_chosen: Signal<Vec<RelationOption>>,
	 search_highlighted: Signal<Vec<String>>,
	 available: Signal<Vec<RelationOption>>,
	 chosen: Signal<Vec<RelationOption>>,
	 available_highlighted: Signal<Vec<String>>,
	 chosen_highlighted: Signal<Vec<String>>| {
		fieldset {
			class: layout_class,
			legend {
				class: "admin-label relation-selector__legend",
				{ label }
			}
			div {
				class: "relation-selector__search",
				label {
					class: "admin-label",
					for: search_id.clone(),
					"Search"
				}
				input {
					class: "admin-input",
					type: "search",
					id: search_id,
					autocomplete: "off",
					@input: move |event| {
						#[cfg(client)]
						{
						let Ok(next_query) = event.value() else {
							return;
						};
						let request_generation = search_generation.get_untracked() + 1;
						search_generation.set(request_generation);
						search_status.set("Searching...".to_string());
						let model_name = search_model.clone();
						let field_name = search_field.clone();
						reinhardt_pages::platform::spawn_task(async move {
							let result = crate::server::lookup_relation_options(
								model_name,
								field_name,
								next_query,
								1,
							)
							.await;
							if search_generation.try_get_untracked().ok() != Some(request_generation) {
								return;
							}
							match result {
								Ok(response) => {
									let Ok(chosen) = search_chosen.try_get_untracked() else {
										return;
									};
									let _ = search_available.try_set(
										crate::pages::components::relation_selector::merge_search_results(
											response.options,
											&chosen,
										),
									);
									let _ = search_highlighted.try_set(Vec::new());
									let message = if response.has_more {
										"Showing the first 50 matches. Refine your search."
									} else {
										"Search results updated."
									};
									let _ = search_status.try_set(message.to_string());
								}
								Err(error) => {
									let _ = search_status.try_set(format!("Search failed: {error}"));
								}
							}
						});
						}
						#[cfg(not(client))]
						let _ = event;
					},
				}
			}
			div {
				class: "relation-selector__grid",
				div {
					class: "relation-selector__panel",
					label {
						class: "admin-label",
						id: available_label_id.clone(),
						for: available_id.clone(),
						"Available"
					}
					select {
						class: "admin-select relation-selector__select",
						id: available_id.clone(),
						aria_labelledby: available_label_id,
						aria_describedby: status_id.clone(),
						multiple: true,
						bind: available_highlighted,
						@change: move |event| {
							if let Ok(values) = event.selected_values() {
								available_highlighted.set(values);
							}
						},
						@keydown: {
							let focus_id = chosen_id.clone();
							move |event: reinhardt_pages::event::KeyDownEvent| {
								if event.key() == "Enter" {
									event.prevent_default();
									crate::pages::components::relation_selector::add_highlighted(
										available,
										chosen,
										available_highlighted,
										&focus_id,
									);
								}
							}
						},
						{ available_options }
					}
				}
				div {
					class: "relation-selector__actions",
					button {
						class: "admin-btn admin-btn-secondary",
						type: "button",
						data_relation_action: "add",
						@click: {
							let focus_id = chosen_id.clone();
							move |_: reinhardt_pages::event::ClickEvent| {
								crate::pages::components::relation_selector::add_highlighted(
									available,
									chosen,
									available_highlighted,
									&focus_id,
								);
							}
						},
						"Add"
					}
					button {
						class: "admin-btn admin-btn-secondary",
						type: "button",
						data_relation_action: "remove",
						@click: {
							let focus_id = available_id.clone();
							move |_: reinhardt_pages::event::ClickEvent| {
								crate::pages::components::relation_selector::remove_highlighted(
									available,
									chosen,
									chosen_highlighted,
									&focus_id,
								);
							}
						},
						"Remove"
					}
				}
				div {
					class: "relation-selector__panel",
					label {
						class: "admin-label",
						id: chosen_label_id.clone(),
						for: chosen_id.clone(),
						"Chosen"
					}
					select {
						class: "admin-select relation-selector__select",
						id: chosen_id.clone(),
						aria_labelledby: chosen_label_id,
						aria_describedby: status_id.clone(),
						multiple: true,
						bind: chosen_highlighted,
						@change: move |event| {
							if let Ok(values) = event.selected_values() {
								chosen_highlighted.set(values);
							}
						},
						@keydown: {
							let focus_id = available_id.clone();
							move |event: reinhardt_pages::event::KeyDownEvent| {
								if matches!(event.key().as_str(), "Delete" | "Backspace") {
									event.prevent_default();
									crate::pages::components::relation_selector::remove_highlighted(
										available,
										chosen,
										chosen_highlighted,
										&focus_id,
									);
								}
							}
						},
						{ chosen_options }
					}
				}
			}
			select {
				class: "sr-only",
				id: input_id,
				name: field_name,
				multiple: true,
				tabindex: -1,
				{ submitted_options }
			}
			div {
				class: "relation-selector__status",
				id: status_id,
				role: "status",
				aria_live: "polite",
				aria_atomic: "true",
				{ status_content }
			}
		}
	})(
		layout_class,
		label.to_string(),
		input_id,
		field_name.to_string(),
		search_id,
		available_id,
		chosen_id,
		status_id,
		available_label_id,
		chosen_label_id,
		available_options,
		chosen_options,
		submitted_options,
		status_content,
		search_model,
		search_field,
		search_generation,
		search_status,
		search_available,
		search_chosen,
		search_highlighted,
		available,
		chosen,
		available_highlighted,
		chosen_highlighted,
	)
}

#[cfg(test)]
mod tests {
	use super::{add_selected, merge_search_results, remove_selected};
	use crate::types::RelationOption;

	fn option(value: &str, label: &str) -> RelationOption {
		RelationOption::new(value, label)
	}

	#[test]
	fn add_moves_highlighted_available_options_once_in_stable_order() {
		let available = vec![
			option("1", "Rust"),
			option("2", "WebAssembly"),
			option("2", "Duplicate"),
		];
		let chosen = vec![option("3", "Serde"), option("1", "Rust")];

		let (available, chosen) =
			add_selected(available, chosen, &["1".into(), "2".into(), "2".into()]);

		assert_eq!(available, Vec::<RelationOption>::new());
		assert_eq!(
			chosen,
			vec![
				option("3", "Serde"),
				option("1", "Rust"),
				option("2", "WebAssembly"),
			]
		);
	}

	#[test]
	fn remove_moves_highlighted_chosen_options_back_once_in_stable_order() {
		let available = vec![option("1", "Rust"), option("3", "Old label")];
		let chosen = vec![option("2", "WebAssembly"), option("3", "Serde")];

		let (available, chosen) = remove_selected(available, chosen, &["3".into(), "3".into()]);

		assert_eq!(chosen, vec![option("2", "WebAssembly")]);
		assert_eq!(
			available,
			vec![option("1", "Rust"), option("3", "Old label")]
		);
	}

	#[test]
	fn search_refresh_filters_chosen_and_deduplicates_without_mutating_chosen() {
		let chosen = vec![option("3", "Serde"), option("1", "Rust")];
		let results = vec![
			option("2", "WebAssembly"),
			option("3", "Serde"),
			option("2", "Duplicate"),
			option("4", "Tokio"),
		];

		let available = merge_search_results(results, &chosen);

		assert_eq!(
			available,
			vec![option("2", "WebAssembly"), option("4", "Tokio")]
		);
		assert_eq!(chosen, vec![option("3", "Serde"), option("1", "Rust")]);
	}

	#[test]
	fn retained_chosen_ids_survive_multiple_query_refreshes() {
		let chosen = vec![option("7", "Retained")];

		let first =
			merge_search_results(vec![option("7", "Retained"), option("8", "First")], &chosen);
		let second = merge_search_results(vec![option("9", "Second")], &chosen);

		assert_eq!(first, vec![option("8", "First")]);
		assert_eq!(second, vec![option("9", "Second")]);
		assert_eq!(chosen, vec![option("7", "Retained")]);
	}
}
