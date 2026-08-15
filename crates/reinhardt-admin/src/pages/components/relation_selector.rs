//! Accessible two-panel selector for admin many-to-many fields.

#[cfg(any(client, test))]
use crate::types::ManyToManyLookupResponse;
use crate::types::{RelationOption, RelationSelectorLayout};
use reinhardt_pages::reactive::hooks::id::use_id_with_prefix;
use reinhardt_pages::{Page, Signal, page};
use std::collections::HashSet;
use std::{cell::Cell, rc::Rc};

#[cfg(any(client, test))]
#[derive(Clone, Debug, PartialEq, Eq)]
struct SearchState {
	available: Vec<RelationOption>,
	chosen: Vec<RelationOption>,
	status: String,
	page: u64,
	has_more: bool,
}

fn merge_search_results(
	results: Vec<RelationOption>,
	chosen: &[RelationOption],
) -> Vec<RelationOption> {
	let chosen_ids: HashSet<&str> = chosen.iter().map(|option| option.id.as_str()).collect();
	let mut seen = HashSet::new();
	results
		.into_iter()
		.filter(|option| !chosen_ids.contains(option.id.as_str()) && seen.insert(option.id.clone()))
		.collect()
}

#[cfg(any(client, test))]
fn reduce_search_result(
	mut state: SearchState,
	request_generation: u64,
	current_generation: u64,
	result: Result<ManyToManyLookupResponse, String>,
) -> Option<SearchState> {
	if request_generation != current_generation {
		return None;
	}

	match result {
		Ok(response) => {
			state.available = merge_search_results(response.options, &state.chosen);
			state.page = response.page;
			state.has_more = response.has_more;
			state.status = if response.has_more {
				"Showing the first 50 matches. Refine your search."
			} else {
				"Search results updated."
			}
			.to_string();
		}
		Err(error) => state.status = format!("Search failed: {error}"),
	}

	Some(state)
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
		if chosen_ids.insert(option.id.clone()) {
			next_chosen.push(option);
		}
	}

	let mut available_ids = HashSet::new();
	let mut next_available = Vec::new();
	for option in available {
		if selected_ids.contains(option.id.as_str()) {
			if chosen_ids.insert(option.id.clone()) {
				next_chosen.push(option);
			}
		} else if !chosen_ids.contains(option.id.as_str())
			&& available_ids.insert(option.id.clone())
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
		if available_ids.insert(option.id.clone()) {
			next_available.push(option);
		}
	}

	let mut chosen_ids = HashSet::new();
	let mut next_chosen = Vec::new();
	for option in chosen {
		if selected_ids.contains(option.id.as_str()) {
			if available_ids.insert(option.id.clone()) {
				next_available.push(option);
			}
		} else if chosen_ids.insert(option.id.clone()) {
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
				})(option.id, option.label)
			} else {
				page!(|value: String, label: String| {
					option {
						value: value,
						{ label }
					}
				})(option.id, option.label)
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

#[cfg(client)]
// The task needs the complete selector state so it can discard stale responses atomically.
#[allow(clippy::too_many_arguments)]
fn load_more(
	model_name: String,
	field_name: String,
	query: String,
	generation: Signal<u64>,
	page: Signal<u64>,
	has_more: Signal<bool>,
	available: Signal<Vec<RelationOption>>,
	chosen: Signal<Vec<RelationOption>>,
	highlighted: Signal<Vec<String>>,
	status: Signal<String>,
) {
	if !has_more.get_untracked() {
		return;
	}

	let request_generation = generation.get_untracked();
	let next_page = page.get_untracked().saturating_add(1);
	has_more.set(false);
	status.set("Loading more...".to_string());
	reinhardt_pages::platform::spawn_task(async move {
		let result =
			crate::server::lookup_relation_options(model_name, field_name, query, next_page).await;
		let Ok(current_generation) = generation.try_get_untracked() else {
			return;
		};
		if current_generation != request_generation {
			return;
		}
		let (Ok(current_available), Ok(current_chosen)) =
			(available.try_get_untracked(), chosen.try_get_untracked())
		else {
			return;
		};

		match result {
			Ok(response) => {
				let next_available = if response.options.is_empty() {
					current_available
				} else {
					merge_search_results(
						current_available
							.into_iter()
							.chain(response.options)
							.collect(),
						&current_chosen,
					)
				};
				let _ = available.try_set(next_available);
				let _ = page.try_set(response.page);
				has_more.set(response.has_more);
				status.set(
					if response.has_more {
						"More matches are available."
					} else {
						"Search results updated."
					}
					.to_string(),
				);
				let _ = highlighted.try_set(Vec::new());
			}
			Err(error) => {
				has_more.set(true);
				status.set(format!("Search failed: {error}"));
			}
		}
	});
}

#[cfg(client)]
const RELATION_QUERY_DEBOUNCE_MS: i32 = 150;

#[cfg(client)]
#[allow(
	clippy::too_many_arguments,
	reason = "The search keeps its signal state explicit."
)]
fn start_search(
	model_name: String,
	field_name: String,
	query: String,
	request_generation: u64,
	generation: Signal<u64>,
	status: Signal<String>,
	available: Signal<Vec<RelationOption>>,
	chosen: Signal<Vec<RelationOption>>,
	highlighted: Signal<Vec<String>>,
	page: Signal<u64>,
	has_more: Signal<bool>,
) {
	reinhardt_pages::platform::spawn_task(async move {
		let result = crate::server::lookup_relation_options(model_name, field_name, query, 1).await;
		let Ok(current_generation) = generation.try_get_untracked() else {
			return;
		};
		let (Ok(current_available), Ok(current_chosen), Ok(current_status)) = (
			available.try_get_untracked(),
			chosen.try_get_untracked(),
			status.try_get_untracked(),
		) else {
			return;
		};
		let clear_highlighted = result.is_ok();
		let Some(next) = reduce_search_result(
			SearchState {
				available: current_available,
				chosen: current_chosen,
				status: current_status,
				page: page.get_untracked(),
				has_more: has_more.get_untracked(),
			},
			request_generation,
			current_generation,
			result.map_err(|error| error.to_string()),
		) else {
			return;
		};
		let _ = available.try_set(next.available);
		let _ = chosen.try_set(next.chosen);
		let _ = status.try_set(next.status);
		let _ = page.try_set(next.page);
		let _ = has_more.try_set(next.has_more);
		if clear_highlighted {
			let _ = highlighted.try_set(Vec::new());
		}
	});
}

#[cfg(client)]
#[allow(
	clippy::too_many_arguments,
	reason = "The debounce retains the complete search request state."
)]
fn schedule_search(
	model_name: String,
	field_name: String,
	query: String,
	request_generation: u64,
	debounce_generation: Rc<Cell<u64>>,
	generation: Signal<u64>,
	status: Signal<String>,
	available: Signal<Vec<RelationOption>>,
	chosen: Signal<Vec<RelationOption>>,
	highlighted: Signal<Vec<String>>,
	page: Signal<u64>,
	has_more: Signal<bool>,
) {
	use wasm_bindgen::JsCast;

	let timer_generation = debounce_generation.get().wrapping_add(1);
	debounce_generation.set(timer_generation);
	let callback_generation = debounce_generation.clone();
	let callback_model_name = model_name.clone();
	let callback_field_name = field_name.clone();
	let callback_query = query.clone();
	let callback_generation_signal = generation.clone();
	let callback_status = status.clone();
	let callback_available = available.clone();
	let callback_chosen = chosen.clone();
	let callback_highlighted = highlighted.clone();
	let callback_page = page.clone();
	let callback_has_more = has_more.clone();
	let callback = wasm_bindgen::closure::Closure::once_into_js(move || {
		if callback_generation.get() == timer_generation {
			start_search(
				callback_model_name,
				callback_field_name,
				callback_query,
				request_generation,
				callback_generation_signal,
				callback_status,
				callback_available,
				callback_chosen,
				callback_highlighted,
				callback_page,
				callback_has_more,
			);
		}
	});
	let Some(window) = web_sys::window() else {
		start_search(
			model_name,
			field_name,
			query,
			request_generation,
			generation,
			status,
			available,
			chosen,
			highlighted,
			page,
			has_more,
		);
		return;
	};
	if window
		.set_timeout_with_callback_and_timeout_and_arguments_0(
			callback.unchecked_ref(),
			RELATION_QUERY_DEBOUNCE_MS,
		)
		.is_err()
	{
		start_search(
			model_name,
			field_name,
			query,
			request_generation,
			generation,
			status,
			available,
			chosen,
			highlighted,
			page,
			has_more,
		);
	}
}

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
	let page = Signal::new(1_u64);
	let has_more_signal = Signal::new(has_more);
	let query = Signal::new(String::new());
	let debounce_generation = Rc::new(Cell::new(0_u64));
	let status = Signal::new(if has_more {
		"Showing the first 50 matches. Refine your search.".to_string()
	} else {
		String::new()
	});

	let input_id = use_id_with_prefix(&format!("field-{field_name}"));
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
	let search_page = page;
	let search_has_more = has_more_signal;
	let search_query = query;
	let search_debounce_generation = debounce_generation.clone();
	let load_more_content = Page::reactive({
		let load_more_has_more = has_more_signal;
		let load_more_page = page;
		let load_more_model = model_name.to_string();
		let load_more_field = field_name.to_string();
		let load_more_available_id = available_id.clone();
		move || {
			let disabled = !load_more_has_more.get();
			let model_name = load_more_model.clone();
			let field_name = load_more_field.clone();
			page!(|disabled: bool,
			 page: u64,
			 model_name: String,
			 field_name: String,
			 available_id: String,
			 query: Signal<String>,
			 generation: Signal<u64>,
			 page_signal: Signal<u64>,
			 has_more: Signal<bool>,
			 available: Signal<Vec<RelationOption>>,
			 chosen: Signal<Vec<RelationOption>>,
			 highlighted: Signal<Vec<String>>,
			 status: Signal<String>| {
				button {
					class: "admin-btn admin-btn-secondary",
					type: "button",
					disabled: disabled,
					data_relation_action: "load-more",
					aria_controls: available_id,
					aria_label: format!("Load more relation options after page {page}"),
					@click: move |_: reinhardt_pages::event::ClickEvent| {
						#[cfg(client)]
						crate::pages::components::relation_selector::load_more(
							model_name.clone(),
							field_name.clone(),
							query.get_untracked(),
							generation,
							page_signal,
							has_more,
							available,
							chosen,
							highlighted,
							status,
						);
					},
					"Load more"
				}
			})(
				disabled,
				load_more_page.get(),
				model_name,
				field_name,
				load_more_available_id.clone(),
				query,
				generation,
				page,
				has_more_signal,
				available,
				chosen,
				available_highlighted,
				status,
			)
		}
	});

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
	 search_page: Signal<u64>,
	 search_has_more: Signal<bool>,
	 search_query: Signal<String>,
	 debounce_generation: Rc<Cell<u64>>,
	 available: Signal<Vec<RelationOption>>,
	 chosen: Signal<Vec<RelationOption>>,
	 available_highlighted: Signal<Vec<String>>,
	 chosen_highlighted: Signal<Vec<String>>,
	 load_more_content: Page| {
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
							search_page.set(1);
							search_has_more.set(false);
							search_query.set(next_query.clone());
							search_status.set("Searching...".to_string());
							crate::pages::components::relation_selector::schedule_search(
								search_model.clone(),
								search_field.clone(),
								next_query,
								request_generation,
								debounce_generation.clone(),
								search_generation,
								search_status,
								search_available,
								search_chosen,
								search_highlighted,
								search_page,
								search_has_more,
							);
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
			{ load_more_content }
			select {
				id: input_id,
				name: field_name,
				multiple: true,
				hidden: true,
				data_relation_selector: "true",
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
		search_page,
		search_has_more,
		search_query,
		search_debounce_generation,
		available,
		chosen,
		available_highlighted,
		chosen_highlighted,
		load_more_content,
	)
}

#[cfg(test)]
mod tests {
	use super::{
		SearchState, add_selected, merge_search_results, reduce_search_result, remove_selected,
	};
	use crate::types::{ManyToManyLookupResponse, RelationOption};
	use rstest::rstest;

	fn option(value: &str, label: &str) -> RelationOption {
		RelationOption::new(value, label)
	}

	#[rstest]
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

	#[rstest]
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

	#[rstest]
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

	#[rstest]
	fn retained_chosen_ids_survive_multiple_query_refreshes() {
		let chosen = vec![option("7", "Retained")];

		let first =
			merge_search_results(vec![option("7", "Retained"), option("8", "First")], &chosen);
		let second = merge_search_results(vec![option("9", "Second")], &chosen);

		assert_eq!(first, vec![option("8", "First")]);
		assert_eq!(second, vec![option("9", "Second")]);
		assert_eq!(chosen, vec![option("7", "Retained")]);
	}

	#[rstest]
	fn search_result_reducer_ignores_late_response_after_latest_response() {
		// Arrange
		let chosen = vec![option("7", "Retained")];
		let state = SearchState {
			available: vec![option("0", "Initial")],
			chosen: chosen.clone(),
			status: "Searching...".to_string(),
			page: 1,
			has_more: false,
		};
		let latest = ManyToManyLookupResponse {
			options: vec![
				option("2", "Generation two"),
				option("7", "Retained duplicate"),
			],
			page: 1,
			has_more: true,
		};
		let late = ManyToManyLookupResponse {
			options: vec![option("1", "Generation one")],
			page: 1,
			has_more: false,
		};

		// Act
		let state = reduce_search_result(state, 2, 2, Ok(latest)).expect("latest response");
		let stale = reduce_search_result(state.clone(), 1, 2, Ok(late));

		// Assert
		assert_eq!(stale, None);
		assert_eq!(state.available, vec![option("2", "Generation two")]);
		assert_eq!(state.chosen, chosen);
		assert_eq!(
			state.status,
			"Showing the first 50 matches. Refine your search."
		);
	}

	#[rstest]
	fn search_result_reducer_reports_latest_error_without_dropping_chosen() {
		// Arrange
		let available = vec![option("2", "WebAssembly")];
		let chosen = vec![option("7", "Retained")];
		let state = SearchState {
			available: available.clone(),
			chosen: chosen.clone(),
			status: "Searching...".to_string(),
			page: 1,
			has_more: false,
		};

		// Act
		let state = reduce_search_result(state, 3, 3, Err("network unavailable".to_string()))
			.expect("latest error");

		// Assert
		assert_eq!(state.available, available);
		assert_eq!(state.chosen, chosen);
		assert_eq!(state.status, "Search failed: network unavailable");
	}

	#[rstest]
	fn later_search_page_merges_unique_unchosen_options() {
		// Arrange
		let chosen = vec![option("7", "Retained")];
		let first_page = vec![option("1", "First"), option("7", "Chosen")];
		let second_page = vec![
			option("1", "Duplicate"),
			option("2", "Second"),
			option("7", "Chosen duplicate"),
		];

		// Act
		let available =
			merge_search_results(first_page.into_iter().chain(second_page).collect(), &chosen);

		// Assert
		assert_eq!(available, vec![option("1", "First"), option("2", "Second")]);
	}
}
