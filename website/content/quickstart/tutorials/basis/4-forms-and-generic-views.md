+++
title = "Part 4: Client-Side Forms and Component Patterns"
weight = 40

[extra]
sidebar_weight = 40
+++

# Part 4: Client-Side Forms and Component Patterns

In this tutorial, we'll implement form handling in reinhardt-pages using client-side components and server functions.

## Understanding Form Handling in reinhardt-pages

Unlike traditional server-rendered forms (templating engines like Tera), the reinhardt-pages stack handles forms on the client side with WASM components that call server functions. The tutorial uses **exactly one recommended path**: the `form!` macro.

> **📌 The `form!` macro is the single recommended path for forms in this tutorial.**
>
> Why:
> - ✅ **Automatic CSRF protection** — built-in token injection for POST/PUT/PATCH/DELETE
> - ✅ **Reactive state management** — `watch` blocks update the UI when form state changes
> - ✅ **Dynamic choices** — runtime population of select/radio options
> - ✅ **Zero boilerplate** — loading states, error handling, validation out of the box
> - ✅ **Type-safe** — the compiler checks that every field name matches the form definition
> - ✅ **Declarative** — you describe what the form does, not how it works
>
> The **reference implementation** of this section lives in
> [`examples/examples-tutorial-basis/src/shared/forms.rs`](https://github.com/kent8192/reinhardt-web/tree/main/examples/examples-tutorial-basis/src/shared/forms.rs)
> (server-side `Form` definition used to emit `FormMetadata` / CSRF tokens)
> and
> [`examples/examples-tutorial-basis/src/client/components/polls.rs`](https://github.com/kent8192/reinhardt-web/tree/main/examples/examples-tutorial-basis/src/client/components/polls.rs)
> (the `form!` macro + dynamic choices + `watch` blocks). Read them alongside the snippets below — they are the authoritative answer key.
>
> Low-level `use_state()` form handling is covered at the very end of this chapter under "Appendix: When You Need to Drop Below `form!`". Do not reach for it unless the `form!` macro cannot express your requirement — 90%+ of forms do not need it.

**Key Concepts:**

1. **One form type declared twice**: server-side `Form` in `src/shared/forms.rs` (produces `FormMetadata` and CSRF tokens) + client-side `form!` macro in `src/client/components/polls.rs` (produces the UI + action dispatch).
2. **`form!` macro as the primary API** — declarative fields, automatic state, reactive `watch` blocks.
3. **Server functions as form targets** — every submission lands on a `#[server_fn]` for persistence and validation.
4. **Error handling via `watch`** — validation errors and server errors display reactively.
5. **Navigation after success** — the `success_navigation` `watch` pattern redirects on successful submission.

## Example: Declarative Forms with form! Macro (Recommended)

The voting form in `src/client/components/polls.rs` demonstrates the recommended approach:

```rust
// Example from src/client/components/polls.rs
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
			choices_from: "choices",  // Dynamic choices
			choice_value: "id",
			choice_label: "choice_text",
		},
	},

	watch: {
		submit_button: |form| {
			let is_loading = form.loading().get();
			// Reactive UI updates when loading state changes
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
			// Show error messages reactively
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
						#[cfg(target_arch = "wasm32")]
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
```

**Key Benefits:**
- **Automatic CSRF Protection**: POST method automatically includes CSRF token
- **Reactive State Management**: `watch` blocks update UI when form state changes
- **Dynamic Choices**: `choices_from` populates select/radio options at runtime
- **Built-in Loading States**: `form.loading()` and `form.error()` Signals
- **Type-Safe**: Compiler ensures field names match form definition

**Navigation Patterns:**

The `form!` macro supports `watch` blocks for handling post-submission navigation. The `success_navigation` watch block pattern monitors loading and error state to detect successful form submissions:

**success_navigation watch block**: Monitors `form.loading()` and `form.error()` Signals to navigate after successful submission:
- When loading completes (`!is_loading`) and no error exists (`err.is_none()`), the form submission was successful
- Extract the question ID from the current URL path to construct the results URL
- Use `web_sys::window()` for client-side navigation

```rust
success_navigation: |form| {
	let is_loading = form.loading().get();
	let err = form.error().get();
	page!(|is_loading: bool, err: Option<String>| {
		watch {
			if ! is_loading &&err.is_none() {
				#[cfg(target_arch = "wasm32")]
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
```

**Note**: Errors are automatically stored in `form.error()` Signal and can be displayed using `watch` blocks (see `error_display` in the example above).

For complete implementation, see `examples/examples-tutorial-basis/src/client/components/polls.rs`.

### Understanding watch Blocks

The `watch` block in `form!` macro creates reactive UI components that automatically update when form state changes. This is similar to React's `useEffect` or Vue's `watch`.

**Key Concepts:**

1. **Reactive Updates**: UI re-renders automatically when referenced Signals change
2. **Direct Signal Access**: Access form state via `form.loading()`, `form.error()`, `form.{field_name}()`
3. **Type Safety**: Compiler ensures correct types in watch block closures

**Common Patterns:**

```rust
watch: {
	// Pattern 1: Submit button with loading state
	submit_button: |form| {
		let is_loading = form.loading().get();
		page!(|is_loading: bool| {
			div {
				class: "mt-3",
				button {
					type: "submit",
					class: if is_loading { "btn-primary opacity-50 cursor-not-allowed" } else { "btn-primary" },
					disabled: is_loading,
					{ if is_loading { "Submitting..." } else { "Submit" } }
				}
			}
		})(is_loading)
	},

	// Pattern 2: Field-specific error display
	username_error: |form| {
		let err = form.username_error().get();
		page!(|err: Option<String>| {
			watch {
				if let Some(e) = err.clone() {
					div { class: "text-danger small mt-1", { e } }
				}
			}
		})(err)
	},

	// Pattern 3: Field validation feedback (real-time)
	password_strength: |form| {
		let password = form.password().get();
		page!(|password: String| {
			let strength = if password.len() >= 12 { "Strong" }
				else if password.len() >= 8 { "Medium" }
				else { "Weak" };
			let color = if password.len() >= 12 { "text-success" }
				else if password.len() >= 8 { "text-warning" }
				else { "text-danger" };
			div {
				class: format!("small mt-1 {}", color),
				{ format!("Password Strength: {}", strength) }
			}
		})(password)
	},

	// Pattern 4: Conditional field visibility
	billing_address: |form| {
		let same_as_shipping = form.same_as_shipping().get();
		page!(|same_as_shipping: bool| {
			watch {
				if !same_as_shipping {
					div { class: "form-group",
						label { "Billing Address" }
						input { type: "text", class: "form-control" }
					}
				}
			}
		})(same_as_shipping)
	},
}
```

**Benefits:**
- ✅ **No manual DOM manipulation**: Framework handles updates
- ✅ **Reactive by default**: Changes propagate automatically
- ✅ **Type-safe**: Compiler catches type mismatches
- ✅ **Performance**: Only re-renders affected components

### Dynamic Choices Population

The `form!` macro supports runtime population of select/radio options via the `choices_from` field attribute.

**Workflow:**

1. Define field with `choices_from` attribute
2. Form macro generates a `{field_name}_choices()` Signal
3. Populate choices at runtime using `set()` method

**Complete Example:**

```rust
// Step 1: Define form with choices_from
let voting_form = form! {
	name: VotingForm,
	server_fn: submit_vote,
	method: Post,

	fields: {
		question_id: HiddenField {
			initial: qid.to_string(),
		},
		choice_id: ChoiceField {
			widget: RadioSelect,
			required,
			label: "Select your choice",
			choices_from: "choices",  // Indicates dynamic choices
			choice_value: "id",        // Which field is the value
			choice_label: "choice_text", // Which field is the label
		},
	},

	watch: {
		// Display loading state while fetching choices
		choices_loading: |form| {
			let choices = form.choice_id_choices().get();
			page!(|choices: Vec<(String, String)>| {
				watch {
					if choices.is_empty() {
						div { class: "spinner", "Loading choices..." }
					}
				}
			})(choices)
		},
	},
};

// Step 2: Create action for loading question detail
let load_detail =
	use_action(
		|qid: i64| async move { get_question_detail(qid).await.map_err(|e| e.to_string()) },
	);

// Step 3: Bridge load_detail results to form choices via use_effect
{
	let load_detail_for_effect = load_detail.clone();
	let voting_form_for_effect = voting_form.clone();
	use_effect(move || {
		if let Some((_, ref choices)) = load_detail_for_effect.result() {
			// Transform server data to (value, label) tuples
			let choice_options: Vec<(String, String)> = choices
				.iter()
				.map(|c| (c.id.to_string(), c.choice_text.clone()))
				.collect();

			// Populate choices - triggers UI update
			voting_form_for_effect
				.choice_id_choices()
				.set(choice_options);
		}
	});
}

// Dispatch the action to load question data
load_detail.dispatch(qid);
```

**Key Points:**

- **Type**: Choices are `Vec<(String, String)>` where first element is value, second is label
- **Generated Method**: `{field_name}_choices()` returns a Signal
- **Reactivity**: UI updates automatically when choices are set
- **Use Cases**: 
  - Country/state dropdowns based on user selection
  - Category filters based on search results
  - Dynamic form fields based on API responses

**Common Mistake:**

```rust
// ❌ Don't access choices directly
let choices = get_choices().await?;
// This won't update the UI

// ✅ Use the generated Signal setter
voting_form.choice_id_choices().set(choices);
// This triggers reactive UI update
```

## Event Handling Patterns

reinhardt-pages provides two main approaches for handling user interactions and events:

### @-Prefix Handlers (For Non-Form Interactions)

Use @-handlers for direct event bindings outside of forms, such as button clicks, modal toggles, and navigation:

#### Basic Button Click

```rust
use reinhardt::pages::prelude::*;

page!(|| {
	button {
		@click: move |_| {
			console_log!("Button clicked!");
		},
		"Click Me"
	}
})
```

#### Modal Toggle with State

```rust
use reinhardt::pages::prelude::*;
use reinhardt::pages::reactive::hooks::use_state;

let (show_modal, set_show_modal) = use_state(false);

page!(|show_modal: Signal<bool>| {
	button {
		@click: move |_| {
			set_show_modal(!show_modal.get());
		},
		"Toggle Modal"
	}

	watch {
		if show_modal.get() {
			div {
				class: "modal",
				@click: move |_| set_show_modal(false),
				"Click to close"
			}
		}
	}
})(show_modal)
```

**Supported Events**:
- `@click` - Mouse clicks
- `@change` - Input value changes
- `@input` - Input events (as user types)
- `@submit` - Form submission
- `@focus`, `@blur` - Focus events
- `@keydown`, `@keyup` - Keyboard events

### form! watch Blocks (For Form Interactions)

For form-related state and validation, use `form!` watch blocks instead of @-handlers:

```rust
form! {
	name: LoginForm,
	server_fn: login,
	method: Post,

	fields: {
		username: CharField { required },
	},

	watch: {
		username_feedback: |form| {
			let value = form.username().get();
			page!(|value: String| {
				watch {
					if value.len() < 3 {
						div {
							class: "text-danger",
							"Username must be at least 3 characters"
						}
					}
				}
			})(value)
		},
	},
}
```

**When to Use Which**:
- ✅ **@-handlers**: Buttons, modals, navigation, custom UI interactions (non-form)
- ✅ **form! watch**: Input validation, field-dependent UI, form state management

## Appendix: When You Need to Drop Below `form!`

Everything above — and the reference `examples-tutorial-basis` voting form — is built with the `form!` macro. That is the path we recommend for the entire tutorial and for nearly all production forms. This appendix documents the lower-level `use_state()` path **only** so that you recognize it when you see it in older code or in the `examples/` directory; it is *not* part of the main tutorial build.

### When `form!` Is Insufficient

Drop below `form!` only when you need:
- Multi-step wizard logic with complex branching that the macro cannot express
- Real-time collaborative editing with external CRDT libraries
- Drag-and-drop form builders where the field set itself is runtime-defined
- Integration with a third-party state management library the macro cannot plug into
- Highly unusual validation sequencing beyond `#[validate(...)]` and server-side validation

For a login form, CRUD create/update, search with filters, or the voting form in this tutorial, **use `form!`**.

### Quick Comparison

| Aspect | `form!` Macro | Manual `use_state()` |
|--------|---------------|-----------------------|
| **CSRF Protection** | Automatic (POST/PUT/PATCH/DELETE) | You must fetch and attach the token yourself |
| **State Management** | Built-in `watch` blocks, reactive Signals | One `use_state()` per field |
| **Boilerplate** | Minimal (declarative) | Extensive (imperative) |
| **Error Handling** | Built-in `form.error()` Signal | Your own error state |
| **Loading States** | Built-in `form.loading()` Signal | Your own loading state |
| **Validation** | Integrated with server_fn validation | Hand-written |
| **Type Safety** | Compiler-enforced field names | Whatever you write yourself |
| **Reactivity** | Automatic | Manual Signal updates |
| **Use it for** | The entire tutorial (100% of forms) | Only escape hatches from the list above |

Manual form code can be found in the Reinhardt repository under `examples/` for advanced scenarios; it is outside the scope of this tutorial.

## Server-Side Validation and Processing

The server function handles data persistence and server-side validation. Update `src/server_fn/polls.rs`:

```rust
// src/server_fn/polls.rs
use crate::shared::types::{ChoiceInfo, VoteRequest};
use reinhardt::pages::server_fn::{server_fn, ServerFnError};

// Server-only imports
#[cfg(native)]
use reinhardt::atomic;

/// Vote for a choice
///
/// Server-side validation and atomic database update.
#[server_fn]
pub async fn vote(
	request: VoteRequest,
	#[inject] db: reinhardt::DatabaseConnection,
) -> std::result::Result<ChoiceInfo, ServerFnError> {
	use crate::apps::polls::models::Choice;
	use reinhardt::Model;

	// Wrap read-modify-write in a transaction to prevent race conditions
	let updated_choice = atomic(&db, || async {
		let choice_manager = Choice::objects();

		// Get the choice
		let mut choice = choice_manager
			.get(request.choice_id)
			.first()
			.await
			.map_err(|e| anyhow::anyhow!(e.to_string()))?
			.ok_or_else(|| anyhow::anyhow!("Choice not found"))?;

		// Verify the choice belongs to the question
		if *choice.question_id() != request.question_id {
			return Err(anyhow::anyhow!("Choice does not belong to this question"));
		}

		// Increment vote count
		choice.vote();

		// Update in database
		let updated = choice_manager
			.update(&choice)
			.await
			.map_err(|e| anyhow::anyhow!(e.to_string()))?;

		Ok(updated)
	})
	.await
	.map_err(|e| ServerFnError::application(e.to_string()))?;

	Ok(ChoiceInfo::from(updated_choice))
}
```

**Server-Side Validation Benefits:**

1. **Security**: Never trust client-side validation alone
2. **Data Integrity**: Verify business rules at the server
3. **Consistency**: Centralized validation logic
4. **Error Messages**: Provide detailed feedback to clients

**Race Condition Prevention:**

For atomic updates, use database-level operations:

```rust
// Future enhancement: Use F expressions for atomic updates
use reinhardt::db::orm::F;

Choice::objects()
	.filter(Choice::field_id().eq(choice_id))
	.update()
	.set(Choice::field_votes(), F::new(Choice::field_votes()) + 1)
	.execute(&db)
	.await?;
```

**Why This Prevents Race Conditions:**

1. **Single UPDATE Query**: Database executes the increment atomically
2. **No SELECT Needed**: Avoids read-modify-write race condition
3. **Database Guarantees**: ACID properties ensure consistency

## CSRF Protection in reinhardt-pages

In traditional server-rendered applications, CSRF protection uses tokens embedded in templates:

```html
<!-- Old approach (Tera template) -->
<form method="post">
  {% csrf_token %}
  <!-- form fields -->
</form>
```

In reinhardt-pages, CSRF protection is handled differently:

## CSRF Protection in reinhardt-pages

### Automatic CSRF Protection

reinhardt-pages provides **automatic CSRF protection** for both forms and server functions. No manual token handling is required.

#### For Forms

The `form!` macro automatically injects CSRF tokens for POST/PUT/PATCH/DELETE methods:

```rust
// POST form automatically includes CSRF token
let contact_form = form! {
	name: ContactForm,
	server_fn: submit_contact,
	method: Post,

	fields: {
		message: CharField { required },
	},
};
```

The generated HTML includes a hidden CSRF token field:

```html
<form action="/api/submit_contact" method="post">
	<input type="hidden" name="csrfmiddlewaretoken" value="[token]">
	<!-- field elements -->
</form>
```

GET forms do NOT include CSRF tokens since they are safe methods that don't need CSRF protection:

```rust
// GET form does NOT include CSRF token (safe method)
let search_form = form! {
	name: SearchForm,
	server_fn: search,
	method: Get,

	fields: {
		query: CharField { required },
	},
};
```

#### For Server Functions

The `#[server_fn]` macro automatically includes CSRF headers in all requests by default:

```rust
#[server_fn]  // CSRF protection enabled by default
async fn vote(request: VoteRequest) -> Result<ChoiceInfo, ServerFnError> {
	// CSRF header automatically included: X-CSRFToken: [token]
	// No manual CSRF handling needed
	
	// ... business logic
}
```

**Important**: The `vote` function we created earlier in this tutorial is already CSRF-protected by default. No additional code is needed.

#### Disabling CSRF Protection (Optional)

For public APIs that don't require CSRF protection, you can disable it:

```rust
#[server_fn(no_csrf = true)]  // Disable CSRF for public APIs
async fn public_api() -> Result<Data, ServerFnError> {
	// No CSRF header - useful for public endpoints
	
	// ... business logic
}
```

### Token Retrieval

CSRF tokens are automatically retrieved from multiple sources in the following order of priority:

1. **Cookie**: `csrftoken`
2. **Meta tag**: `<meta name="csrf-token">`
3. **Hidden input**: `<input name="csrfmiddlewaretoken">`

For advanced use cases where you need manual control over CSRF tokens:

```rust
use reinhardt::pages::csrf::{get_csrf_token, csrf_headers};

// Get token (tries Cookie → Meta → Input in that order)
if let Some(token) = get_csrf_token() {
	// Use token for manual AJAX requests
}

// Get as HTTP header (for fetch API)
if let Some((header_name, header_value)) = csrf_headers() {
	// header_name: "X-CSRFToken"
	// header_value: token from browser
}
```

### Django Compatibility

reinhardt-pages uses Django-compatible CSRF conventions:

- **Cookie name**: `csrftoken`
- **Header name**: `X-CSRFToken`
- **Form field name**: `csrfmiddlewaretoken`

This ensures compatibility with Django backends and existing Django infrastructure.

## Component Patterns: Reusability Instead of Generic Views

In traditional server-rendered frameworks, "generic views" provide reusable patterns for common tasks (list views, detail views, etc.). In reinhardt-pages, we achieve similar reusability through **component composition**.

### Pattern 1: Reusable Loading Component

Extract common loading patterns:

```rust
// src/client/components/common.rs
use reinhardt::pages::prelude::*;

pub fn loading_spinner() -> Page {
	page!(|| {
		div {
			class: "spinner-border text-primary",
			role: "status",
			span {
				class: "visually-hidden",
				"Loading..."
			}
		}
	})()
}

pub fn error_alert(message: &str) -> Page {
	let msg = message.to_string();
	page!(|msg: String| {
		div {
			class: "alert alert-danger",
			{msg}
		}
	})(msg)
}
```

Usage:

```rust
use crate::client::components::common::{loading_spinner, error_alert};

pub fn polls_index() -> Page {
	// ... state management

	page!(|loading_state: bool, error_state: Option<String>| {
		div {
			class: "container mt-5",

			if let Some(ref err) = error_state {
				{error_alert(err)}
			} else if loading_state {
				{loading_spinner()}
			} else {
				// ... content
			}
		}
	})(loading_state, error_state)
}
```

### Pattern 2: Form Field Components

Create reusable form field components:

```rust
// src/client/components/forms.rs
use reinhardt::pages::prelude::*;

pub fn radio_choice(
	id: &str,
	name: &str,
	value: &str,
	label: &str,
	checked: bool,
	on_change: impl Fn(web_sys::Event) + 'static,
) -> Page {
	let id = id.to_string();
	let name = name.to_string();
	let value = value.to_string();
	let label = label.to_string();

	page!(|
		id: String,
		name: String,
		value: String,
		label: String,
		checked: bool,
		on_change: impl Fn(web_sys::Event) + 'static
	| {
		div {
			class: "form-check",
			input {
				class: "form-check-input",
				type: "radio",
				id: id.clone(),
				name: name,
				value: value,
				checked: checked,
				onchange: on_change
			}
			label {
				class: "form-check-label",
				for: id,
				{label}
			}
		}
	})(id, name, value, label, checked, on_change)
}

pub fn submit_button(
	label: &str,
	loading: bool,
	loading_label: &str,
) -> Page {
	let label = label.to_string();
	let loading_label = loading_label.to_string();

	page!(|label: String, loading: bool, loading_label: String| {
		button {
			class: "btn btn-primary",
			type: "submit",
			disabled: loading,
			if loading {
				{loading_label}
			} else {
				{label}
			}
		}
	})(label, loading, loading_label)
}
```

Usage in detail page:

```rust
use crate::client::components::forms::{radio_choice, submit_button};

pub fn polls_detail_page(question_id: i64) -> Page {
	// ... state and event handlers

	page!(|choices_data: Vec<ChoiceInfo>, ...| {
		form {
			onsubmit: handle_submit,

			div {
				class: "mb-3",
				for choice in &choices_data {
					{radio_choice(
						&format!("choice{}", choice.id),
						"choice",
						&choice.id.to_string(),
						&choice.choice_text,
						selected == Some(choice.id),
						handle_choice_change.clone()
					)}
				}
			}

			{submit_button("Vote", voting_state, "Voting...")}
		}
	})(choices_data, ...)
}
```

### Pattern 3: Custom Hooks for Form State

For complex forms, create custom hooks:

```rust
// src/client/hooks/form.rs
use reinhardt::pages::prelude::*;

pub struct FormState<T> {
	pub value: T,
	pub error: Option<String>,
	pub loading: bool,
}

pub fn use_form_field<T: Clone + 'static>(
	initial: T
) -> (FormState<T>, impl Fn(T), impl Fn(Option<String>), impl Fn(bool)) {
	let (value, set_value) = use_state(initial);
	let (error, set_error) = use_state(None::<String>);
	let (loading, set_loading) = use_state(false);

	let state = FormState {
		value: value.get().clone(),
		error: error.get().clone(),
		loading: loading.get(),
	};

	(state, set_value, set_error, set_loading)
}
```

Usage:

```rust
use crate::client::hooks::form::use_form_field;

pub fn polls_detail_page(question_id: i64) -> Page {
	let (choice_state, set_choice, set_choice_error, _) = use_form_field(None::<i64>);

	let handle_choice_change = {
		let set_choice = set_choice.clone();
		let set_choice_error = set_choice_error.clone();

		move |e: web_sys::Event| {
			if let Some(target) = e.target() {
				if let Some(input) = target.dyn_ref::<web_sys::HtmlInputElement>() {
					if let Ok(choice_id) = input.value().parse::<i64>() {
						set_choice(Some(choice_id));
						set_choice_error(None);
					}
				}
			}
		}
	};

	// ... rest of component
}
```

## Best Practices for Form Components

### 1. Controlled Components

Always use controlled components (state-driven):

```rust
// ✅ GOOD: Controlled component
input {
	value: email_state.clone(),
	oninput: handle_email_change
}

// ❌ BAD: Uncontrolled component
input {
	placeholder: "Email"
	// No value binding - state and UI can diverge
}
```

### 2. Immediate Validation Feedback

Clear errors when user starts typing:

```rust
let handle_email_change = {
	let set_email = set_email.clone();
	let set_email_error = set_email_error.clone();

	move |e: web_sys::Event| {
		if let Some(target) = e.target() {
			if let Some(input) = target.dyn_ref::<web_sys::HtmlInputElement>() {
				set_email(input.value());
				set_email_error(None);  // Clear error on change
			}
		}
	}
};
```

### 3. Disable Buttons During Submission

Prevent double submissions:

```rust
button {
	type: "submit",
	disabled: submitting_state || !is_valid_state,
	class: "btn btn-primary",
	if submitting_state {
		"Submitting..."
	} else {
		"Submit"
	}
}
```

### 4. Progressive Enhancement

Show loading states and optimistic updates:

```rust
let submit_action = use_action(|data: FormData| async move {
	submit_form(data).await.map_err(|e| e.to_string())
});

// Optimistic update (optional)
update_ui_optimistically();
submit_action.dispatch(data);

// Bridge results via use_effect
{
	let submit_action = submit_action.clone();
	use_effect(move || {
		match submit_action.result() {
			Some(Ok(result)) => {
				// Success
				navigate_to_success_page();
			}
			Some(Err(e)) => {
				// Rollback optimistic update
				rollback_ui();
				set_error(Some(e.clone()));
			}
			None => {} // Still loading
		}
	});
}
```

## Summary

In this tutorial, you learned:

- **Client-Side Form State**: Using `use_state()` for local form data and `use_action` for async operations
- **Event Handlers**: Attaching listeners to form elements
- **Client-Side Validation**: Immediate feedback before server submission
- **Server-Side Validation**: Security and data integrity at the server
- **CSRF Protection**: Current manual approach and future automatic integration
- **Component Patterns**: Reusable components instead of generic views
- **Custom Hooks**: Encapsulating form logic for reuse
- **Best Practices**: Controlled components, validation feedback, and progressive enhancement

**Key Differences from Traditional Approaches:**

| Aspect | Traditional (Tera) | reinhardt-pages |
|--------|-------------------|-----------------|
| Form Rendering | Server-side template | Client-side component |
| State Management | Server session | Client state (`use_state`, `use_action`) |
| Validation | Server-side only | Client + Server |
| CSRF Protection | Template tags (`{% csrf_token %}`) | Middleware integration (future) |
| Reusability | Generic views | Component composition |
| User Experience | Full page reload | Dynamic updates, no reload |

## What's Next?

In the next tutorial, we'll write automated tests for our reinhardt-pages application, including:

- Component testing
- Server function testing
- Integration testing with browser automation

Continue to [Part 5: Testing](../5-testing/).
