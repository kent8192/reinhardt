//! Polling application components
//!
//! Provides UI components for the polling application including
//! the index page, detail page with voting form, and results page.

use crate::shared::types::{ChoiceInfo, QuestionInfo};
use reinhardt::pages::component::Page;
use reinhardt::pages::form;
use reinhardt::pages::page;
use reinhardt::pages::reactive::hooks::{Action, use_action, use_effect};

use crate::server_fn::polls::{
	get_question_detail, get_question_results, get_questions, submit_vote,
};

/// Polls index page - List all polls
///
/// Displays a list of available polls with links to vote.
/// Uses watch blocks for reactive UI updates when async data loads.
pub fn polls_index() -> Page {
	let load_questions =
		use_action(|_: ()| async move { get_questions().await.map_err(|e| e.to_string()) });
	load_questions.dispatch(());

	let load_questions_error = load_questions.clone();
	let load_questions_signal = load_questions.clone();

	page!(|load_questions_error: Action<Vec<QuestionInfo>, String>, load_questions_signal: Action<Vec<QuestionInfo>, String>| {
		div {
			class: "max-w-4xl mx-auto px-4 mt-12",
			h1 {
				class: "mb-4",
				"Polls"
			}
			watch {
				if load_questions_error.error().is_some() {
					div {
						class: "alert-danger",
						{ load_questions_error.error().unwrap_or_default() }
					}
				}
			}
			watch {
				if load_questions_signal.is_pending() {
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
				} else if load_questions_signal.result().unwrap_or_default().is_empty() {
					p {
						class: "text-gray-500",
						"No polls are available."
					}
				} else {
					div {
						class: "space-y-2",
						{
							Page::Fragment(
									load_questions_signal
										.result()
										.unwrap_or_default()
										.iter()
										.map(|question| {
											let href = format!("/polls/{}/", question.id);
											let question_text = question.question_text.clone();
											let pub_date = question.pub_date.format("%Y-%m-%d %H:%M").to_string();
											page!(
												| href : String, question_text : String, pub_date : String | { a {
												href : href, class :
												"block p-4 border rounded hover:bg-gray-50 transition-colors", div {
												class : "flex w-full justify-between", h5 { class : "mb-1", {
												question_text } } small { { pub_date } } } } }
											)(href, question_text, pub_date)
										})
										.collect::<Vec<_>>(),
								)
						}
					}
				}
			}
		}
	})(load_questions_error, load_questions_signal)
}

/// Poll detail page - Show question and voting form
///
/// Displays a question with its choices and allows the user to vote.
/// Uses form! macro with Dynamic ChoiceField for declarative form handling.
/// CSRF protection is automatically injected for POST method.
pub fn polls_detail(question_id: i64) -> Page {
	let qid = question_id;

	// Create action for loading question detail
	let load_detail =
		use_action(
			|qid: i64| async move { get_question_detail(qid).await.map_err(|e| e.to_string()) },
		);

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
							type: "submit",
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
							#[cfg(wasm)]
									{
										if let Some(window) = web_sys::window() {
											let pathname = window.location().pathname().ok();
											if let Some(path) = pathname {
												let parts: Vec<&str> = path.split('/').collect();
												if parts.len() >= 3 && parts[1] == "polls" {
													if let Ok(question_id) = parts[2].parse::<i64>() {
														let results_url = format!("/polls/{}/results/", question_id);
														let _ = window.location().set_href(&results_url);
													}
												}
											}
										}
									}
						}
					}
				})(is_loading, err)
			},
		},
	};

	// Bridge load_detail results to form choices via use_effect
	{
		let load_detail_for_effect = load_detail.clone();
		let voting_form_for_effect = voting_form.clone();
		use_effect(move || {
			if let Some((_, ref choices)) = load_detail_for_effect.result() {
				let choice_options: Vec<(String, String)> = choices
					.iter()
					.map(|c| (c.id.to_string(), c.choice_text.clone()))
					.collect();
				voting_form_for_effect
					.choice_id_choices()
					.set(choice_options);
			}
		});
	}

	// Dispatch the action to load question data
	load_detail.dispatch(qid);

	let load_detail_signal = load_detail.clone();

	// Loading state
	if load_detail_signal.is_pending() {
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
	if let Some(err) = load_detail_signal.error() {
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
	if let Some((ref q, _)) = load_detail_signal.result() {
		let question_text = q.question_text.clone();
		let form_view = voting_form.into_page();

		page!(|question_text: String, form_view: Page| {
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
pub fn polls_results(question_id: i64) -> Page {
	let load_results =
		use_action(
			|qid: i64| async move { get_question_results(qid).await.map_err(|e| e.to_string()) },
		);
	load_results.dispatch(question_id);

	let load_results_signal = load_results.clone();

	page!(|load_results_signal: Action<(QuestionInfo, Vec<ChoiceInfo>, i32), String>, question_id: i64| {
		div {
			watch {
				if load_results_signal.is_pending() {
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
				} else if load_results_signal.error().is_some() {
					div {
						class: "max-w-4xl mx-auto px-4 mt-12",
						div {
							class: "alert-danger",
							{ load_results_signal.error().unwrap_or_default() }
						}
						a {
							href: "/",
							class: "btn-primary",
							"Back to Polls"
						}
					}
				} else if load_results_signal.result().is_some() {
					div {
						class: "max-w-4xl mx-auto px-4 mt-12",
						h1 {
							class: "mb-4",
							{
								load_results_signal
										.result()
										.map(|(q, _, _)| q.question_text.clone())
										.unwrap_or_default()
							}
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
									{
										Page::Fragment(
										        load_results_signal
										            .result()
										            .map(|(_, choices, total)| {
										                choices
										                    .iter()
										                    .map(|choice| {
										                        let percentage = if total > 0 {
										                            (choice.votes as f64 / total as f64 * 100.0) as i32
										                        } else {
										                            0
										                        };
										                        let choice_text = choice.choice_text.clone();
										                        let votes = choice.votes;
										                        page!(
										                            | choice_text : String, votes : i32, percentage : i32 | { div
										                            { class : "py-4", div { class :
										                            "flex justify-between items-center mb-2", strong { {
										                            choice_text } } span { class :
										                            "inline-flex items-center bg-brand rounded-full px-2.5 py-0.5 text-xs font-medium text-white",
										                            { format!("{} votes", votes) } } } div { class :
										                            "w-full bg-gray-200 rounded-full h-2.5", div { class :
										                            "bg-brand h-2.5 rounded-full", role : "progressbar", style :
										                            format!("width: {}%", percentage), aria_valuenow : percentage
										                            .to_string(), aria_valuemin : "0", aria_valuemax : "100", {
										                            format!("{}%", percentage) } } } } }
										                        )(choice_text, votes, percentage)
										                    })
										                    .collect::<Vec<_>>()
										            })
										            .unwrap_or_default(),
										    )
									}
								}
								div {
									class: "mt-3",
									p {
										class: "text-gray-500",
										{
											format!(
													"Total votes: {}",
													load_results_signal
														.result()
														.map(|(_, _, total)| total)
														.unwrap_or(0)
												)
										}
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
	})(load_results_signal, question_id)
}

/// Example component demonstrating static URL resolution
///
/// This shows how to use resolve_static() for images in page! macros.
/// This function is identical to polls_index() but adds poll icons using
/// static URL resolution.
pub fn polls_index_with_logo() -> Page {
	let load_questions =
		use_action(|_: ()| async move { get_questions().await.map_err(|e| e.to_string()) });
	load_questions.dispatch(());

	let load_questions_error = load_questions.clone();
	let load_questions_signal = load_questions.clone();

	page!(|load_questions_error: Action<Vec<QuestionInfo>, String>, load_questions_signal: Action<Vec<QuestionInfo>, String>| {
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
				if load_questions_error.error().is_some() {
					div {
						class: "alert-danger",
						{ load_questions_error.error().unwrap_or_default() }
					}
				}
			}
			watch {
				if load_questions_signal.is_pending() {
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
				} else if load_questions_signal.result().unwrap_or_default().is_empty() {
					p {
						class: "text-gray-500",
						"No polls are available."
					}
				} else {
					div {
						class: "space-y-2",
						{
							Page::Fragment(
									load_questions_signal
										.result()
										.unwrap_or_default()
										.iter()
										.map(|question| {
											let href = format!("/polls/{}/", question.id);
											let question_text = question.question_text.clone();
											let pub_date = question.pub_date.format("%Y-%m-%d %H:%M").to_string();
											page!(
												| href : String, question_text : String, pub_date : String | { a {
												href : href, class :
												"block p-4 border rounded hover:bg-gray-50 transition-colors", div {
												class : "flex w-full justify-between items-center", img { src :
												"/static/images/poll-icon.svg", alt : "Poll", class : "w-8 h-8 mr-3",
												} div { class : "flex-1", h5 { class : "mb-1", { question_text } } }
												small { { pub_date } } } } }
											)(href, question_text, pub_date)
										})
										.collect::<Vec<_>>(),
								)
						}
					}
				}
			}
		}
	})(load_questions_error, load_questions_signal)
}
