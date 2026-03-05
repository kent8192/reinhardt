//! Polling application components
//!
//! Provides UI components for the polling application including
//! the index page, detail page with voting form, and results page.

use crate::shared::types::{ChoiceInfo, QuestionInfo};
use reinhardt::pages::Signal;
use reinhardt::pages::component::View;
use reinhardt::pages::form;
use reinhardt::pages::page;
use reinhardt::pages::reactive::hooks::use_state;

use {
	crate::server_fn::polls::{
		get_question_detail, get_question_results, get_questions, submit_vote,
	},
	reinhardt::pages::spawn::spawn_task,
};

/// Polls index page - List all polls
///
/// Displays a list of available polls with links to vote.
/// Uses watch blocks for reactive UI updates when async data loads.
pub fn polls_index() -> View {
	let (questions, set_questions) = use_state(Vec::<QuestionInfo>::new());
	let (loading, set_loading) = use_state(true);
	let (error, set_error) = use_state(None::<String>);

	{
		let set_questions = set_questions.clone();
		let set_loading = set_loading.clone();
		let set_error = set_error.clone();

		spawn_task(async move {
			match get_questions().await {
				Ok(qs) => {
					set_questions(qs);
					set_loading(false);
				}
				Err(e) => {
					set_error(Some(e.to_string()));
					set_loading(false);
				}
			}
		});
	}

	// Clone signals for passing to page! macro (NOT extracting values)
	let questions_signal = questions.clone();
	let loading_signal = loading.clone();
	let error_signal = error.clone();

	page!(|questions_signal: Signal<Vec<QuestionInfo>>, loading_signal: Signal<bool>, error_signal: Signal<Option<String>>| {
		div {
			class: "max-w-4xl mx-auto px-4 mt-12",
			h1 {
				class: "mb-4",
				"Polls"
			}
			watch {
				if error_signal.get().is_some() {
					div {
						class: "alert-danger",
						{ error_signal.get().unwrap_or_default() }
					}
				}
			}
			watch {
				if loading_signal.get() {
					div {
						class: "text-center",
						div {
							class: "spinner w-8 h-8",
							role: "status",
							span {
								class: "sr-only",
								"Loading..."
							}
						}
					}
				} else if questions_signal.get().is_empty() {
					p {
						class: "text-gray-500",
						"No polls are available."
					}
				} else {
					div {
						class: "space-y-2",
						{ View::fragment(questions_signal.get().iter().map(|question| { let href = format!("/polls/{}/", question.id); let question_text = question.question_text.clone(); let pub_date = question.pub_date.format("%Y-%m-%d %H:%M").to_string(); page!(|href : String, question_text : String, pub_date : String| { a { href : href, class : "block p-4 border rounded hover:bg-gray-50 transition-colors", div { class : "flex w-full justify-between", h5 { class : "mb-1", { question_text } } small { { pub_date } } } } }) (href, question_text, pub_date) }).collect ::<Vec<_>>()) }
					}
				}
			}
		}
	})(questions_signal, loading_signal, error_signal)
}

/// Poll detail page - Show question and voting form
///
/// Displays a question with its choices and allows the user to vote.
/// Uses form! macro with Dynamic ChoiceField for declarative form handling.
/// CSRF protection is automatically injected for POST method.
pub fn polls_detail(question_id: i64) -> View {
	// State for question data and loading status
	let (question, set_question) = use_state(None::<QuestionInfo>);
	let (data_loading, set_data_loading) = use_state(true);
	let (data_error, set_data_error) = use_state(None::<String>);

	// Capture question_id for use in on_success closure
	let qid = question_id;

	// Create the voting form using form! macro
	// - server_fn: submit_vote accepts (question_id: String, choice_id: String)
	// - method: Post enables automatic CSRF token injection
	// - state: loading/error signals for form submission feedback
	// - watch blocks for reactive UI updates
	let voting_form = form! {
		name: VotingForm,
		server_fn: submit_vote,
		method: Post,
		state: { loading, error },

		fields: {
			question_id: HiddenField {
				initial: qid.to_string(),
			},
			choice_id: ChoiceField {
				widget: RadioSelect,
				required,
				label: "Select your choice",
				class: "form-check",
				choices_from: "choices",
				choice_value: "id",
				choice_label: "choice_text",
			},
		},

		watch: {
			submit_button: |form| {
				let is_loading = form.loading().get();
				page!(|is_loading: bool| {
					div {
						class: "mt-3",
						button {
							r#type: "submit",
							class: if is_loading { "btn-primary opacity-50 cursor-not-allowed" } else { "btn-primary" },
							disabled: is_loading,
							{ if is_loading { "Voting..." } else { "Vote" } }
						}
						a {
							href: "/",
							class: "btn-secondary ml-2",
							"Back to Polls"
						}
					}
				})(is_loading)
			},
			error_display: |form| {
				let err = form.error().get();
				page!(|err: Option<String>| {
					watch {
						if let Some(e) = err.clone() {
							div {
								class: "alert-danger mt-3",
								{ e }
							}
						}
					}
				})(err)
			},
			success_navigation: |form| {
				let is_loading = form.loading().get();
				let err = form.error().get();
				page!(|is_loading: bool, err: Option<String>| {
					watch {
						if ! is_loading &&err.is_none() {
							# [cfg(target_arch = "wasm32")] { if let Some(window) = web_sys::window() { let pathname = window.location().pathname().ok(); if let Some(path) = pathname { let parts : Vec<&str> = path.split('/').collect(); if parts.len()>= 3 &&parts [1] == "polls" { if let Ok(question_id) = parts [2].parse ::<i64>() { let results_url = format!("/polls/{}/results/", question_id); let _ = window.location().set_href(&results_url); } } } } }
						}
					}
				})(is_loading, err)
			},
		},
	};

	// Load question data and populate choice options
	{
		let set_question = set_question.clone();
		let set_data_loading = set_data_loading.clone();
		let set_data_error = set_data_error.clone();
		let voting_form_clone = voting_form.clone();

		spawn_task(async move {
			match get_question_detail(qid).await {
				Ok((q, choices)) => {
					set_question(Some(q));

					// Populate choice options in the form
					// choice_id_choices Signal accepts Vec<(String, String)> as (value, label)
					let choice_options: Vec<(String, String)> = choices
						.iter()
						.map(|c| (c.id.to_string(), c.choice_text.clone()))
						.collect();
					voting_form_clone.choice_id_choices().set(choice_options);

					set_data_loading(false);
				}
				Err(e) => {
					set_data_error(Some(e.to_string()));
					set_data_loading(false);
				}
			}
		});
	}

	// Clone signals for page! macro
	let question_signal = question.clone();
	let loading_signal = data_loading.clone();
	let error_signal = data_error.clone();

	// Loading state
	if loading_signal.get() {
		return page!(|| {
			div {
				class: "max-w-4xl mx-auto px-4 mt-12 text-center",
				div {
					class: "spinner w-8 h-8",
					role: "status",
					span {
						class: "sr-only",
						"Loading..."
					}
				}
			}
		})();
	}

	// Error state
	if let Some(err) = error_signal.get() {
		return page!(|err: String, question_id: i64| {
			div {
				class: "max-w-4xl mx-auto px-4 mt-12",
				div {
					class: "alert-danger",
					{ err }
				}
				a {
					href: format!("/polls/{}/", question_id),
					class: "btn-secondary",
					"Try Again"
				}
				a {
					href: "/",
					class: "btn-primary ml-2",
					"Back to Polls"
				}
			}
		})(err, question_id);
	}

	// Question found - render voting form
	if let Some(q) = question_signal.get() {
		let question_text = q.question_text.clone();
		let form_view = voting_form.into_view();

		page!(|question_text: String, form_view: View| {
			div {
				class: "max-w-4xl mx-auto px-4 mt-12",
				h1 {
					class: "mb-4",
					{ question_text }
				}
				{ form_view }
			}
		})(question_text, form_view)
	} else {
		// Question not found
		page!(|| {
			div {
				class: "max-w-4xl mx-auto px-4 mt-12",
				div {
					class: "alert-warning",
					"Question not found"
				}
				a {
					href: "/",
					class: "btn-primary",
					"Back to Polls"
				}
			}
		})()
	}
}

/// Poll results page - Show voting results
///
/// Displays the question with vote counts for each choice.
/// Uses watch blocks for reactive UI updates when async data loads.
pub fn polls_results(question_id: i64) -> View {
	let (question, set_question) = use_state(None::<QuestionInfo>);
	let (choices, set_choices) = use_state(Vec::<ChoiceInfo>::new());
	let (total_votes, set_total_votes) = use_state(0);
	let (loading, set_loading) = use_state(true);
	let (error, set_error) = use_state(None::<String>);

	{
		let set_question = set_question.clone();
		let set_choices = set_choices.clone();
		let set_total_votes = set_total_votes.clone();
		let set_loading = set_loading.clone();
		let set_error = set_error.clone();

		spawn_task(async move {
			match get_question_results(question_id).await {
				Ok((q, cs, total)) => {
					set_question(Some(q));
					set_choices(cs);
					set_total_votes(total);
					set_loading(false);
				}
				Err(e) => {
					set_error(Some(e.to_string()));
					set_loading(false);
				}
			}
		});
	}

	// Clone signals for passing to page! macro (NOT extracting values)
	let question_signal = question.clone();
	let choices_signal = choices.clone();
	let total_signal = total_votes.clone();
	let loading_signal = loading.clone();
	let error_signal = error.clone();

	page!(|question_signal: Signal<Option<QuestionInfo>>, choices_signal: Signal<Vec<ChoiceInfo>>, total_signal: Signal<i32>, loading_signal: Signal<bool>, error_signal: Signal<Option<String>>, question_id: i64| {
		div {
			watch {
				if loading_signal.get() {
					div {
						class: "max-w-4xl mx-auto px-4 mt-12 text-center",
						div {
							class: "spinner w-8 h-8",
							role: "status",
							span {
								class: "sr-only",
								"Loading..."
							}
						}
					}
				} else if error_signal.get().is_some() {
					div {
						class: "max-w-4xl mx-auto px-4 mt-12",
						div {
							class: "alert-danger",
							{ error_signal.get().unwrap_or_default() }
						}
						a {
							href: "/",
							class: "btn-primary",
							"Back to Polls"
						}
					}
				} else if question_signal.get().is_some() {
					div {
						class: "max-w-4xl mx-auto px-4 mt-12",
						h1 {
							class: "mb-4",
							{ question_signal.get().map(|q| q.question_text.clone()).unwrap_or_default() }
						}
						div {
							class: "card",
							div {
								class: "card-body",
								h5 {
									class: "text-xl font-bold",
									"Results"
								}
								div {
									class: "divide-y divide-gray-200",
									{ View::fragment(choices_signal.get().iter().map(|choice| { let total = total_signal.get(); let percentage = if total> 0 { (choice.votes as f64 / total as f64 * 100.0) as i32 } else { 0 }; let choice_text = choice.choice_text.clone(); let votes = choice.votes; page!(|choice_text : String, votes : i32, percentage : i32| { div { class : "py-4", div { class : "flex justify-between items-center mb-2", strong { { choice_text } } span { class : "inline-flex items-center bg-brand rounded-full px-2.5 py-0.5 text-xs font-medium text-white", { format!("{} votes", votes) } } } div { class : "w-full bg-gray-200 rounded-full h-2.5", div { class : "bg-brand h-2.5 rounded-full", role : "progressbar", style : format!("width: {}%", percentage), aria_valuenow : percentage.to_string(), aria_valuemin : "0", aria_valuemax : "100", { format!("{}%", percentage) } } } } }) (choice_text, votes, percentage) }).collect ::<Vec<_>>()) }
								}
								div {
									class: "mt-3",
									p {
										class: "text-gray-500",
										{ format!("Total votes: {}", total_signal.get()) }
									}
								}
							}
						}
						div {
							class: "mt-3",
							a {
								href: format!("/polls/{}/", question_id),
								class: "btn-primary",
								"Vote Again"
							}
							a {
								href: "/",
								class: "btn-secondary ml-2",
								"Back to Polls"
							}
						}
					}
				} else {
					div {
						class: "max-w-4xl mx-auto px-4 mt-12",
						div {
							class: "alert-warning",
							"Question not found"
						}
						a {
							href: "/",
							class: "btn-primary",
							"Back to Polls"
						}
					}
				}
			}
		}
	})(
		question_signal,
		choices_signal,
		total_signal,
		loading_signal,
		error_signal,
		question_id,
	)
}

/// Example component demonstrating static URL resolution
///
/// This shows how to use resolve_static() for images in page! macros.
/// This function is identical to polls_index() but adds poll icons using
/// static URL resolution.
pub fn polls_index_with_logo() -> View {
	let (questions, set_questions) = use_state(Vec::<QuestionInfo>::new());
	let (loading, set_loading) = use_state(true);
	let (error, set_error) = use_state(None::<String>);

	{
		let set_questions = set_questions.clone();
		let set_loading = set_loading.clone();
		let set_error = set_error.clone();

		spawn_task(async move {
			match get_questions().await {
				Ok(qs) => {
					set_questions(qs);
					set_loading(false);
				}
				Err(e) => {
					set_error(Some(e.to_string()));
					set_loading(false);
				}
			}
		});
	}

	let questions_signal = questions.clone();
	let loading_signal = loading.clone();
	let error_signal = error.clone();

	page!(|questions_signal: Signal<Vec<QuestionInfo>>, loading_signal: Signal<bool>, error_signal: Signal<Option<String>>| {
		div {
			class: "max-w-4xl mx-auto px-4 mt-12",
			div {
				class: "text-center mb-6",
				img {
					src: "/static/images/poll-icon.svg",
					alt: "Polls App",
					class: "mx-auto w-16 h-16",
				}
			}
			h1 {
				class: "mb-4 text-center",
				"Polls"
			}
			watch {
				if error_signal.get().is_some() {
					div {
						class: "alert-danger",
						{ error_signal.get().unwrap_or_default() }
					}
				}
			}
			watch {
				if loading_signal.get() {
					div {
						class: "text-center",
						div {
							class: "spinner w-8 h-8",
							role: "status",
							span {
								class: "sr-only",
								"Loading..."
							}
						}
					}
				} else if questions_signal.get().is_empty() {
					p {
						class: "text-gray-500",
						"No polls are available."
					}
				} else {
					div {
						class: "space-y-2",
						{ View::fragment(questions_signal.get().iter().map(|question| { let href = format!("/polls/{}/", question.id); let question_text = question.question_text.clone(); let pub_date = question.pub_date.format("%Y-%m-%d %H:%M").to_string(); page!(|href : String, question_text : String, pub_date : String| { a { href : href, class : "block p-4 border rounded hover:bg-gray-50 transition-colors", div { class : "flex w-full justify-between items-center", img { src : "/static/images/poll-icon.svg", alt : "Poll", class : "w-8 h-8 mr-3" } div { class : "flex-1", h5 { class : "mb-1", { question_text } } } small { { pub_date } } } } }) (href, question_text, pub_date) }).collect ::<Vec<_>>()) }
					}
				}
			}
		}
	})(questions_signal, loading_signal, error_signal)
}
