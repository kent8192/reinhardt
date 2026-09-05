# reinhardt-pages

WASM-based reactive frontend framework for Reinhardt with Django-like API.

## Component-scoped styles

Component styles use the canonical `#[style_def] static ... = style! { ... };`
envelope. Selectors and properties remain CSS-shaped, while `globals` and
defaulted `vars` provide checked references and typed runtime overrides:

```rust,ignore
use reinhardt_pages::{CssColor, page, style, style_def};

#[style_def]
static STYLES: CardStyles = style! {
	globals { border: Color; }
	vars { accent: Color = red; }

	.card {
		border-color: globals.border;
		color: vars.accent;
		.label { color: vars.accent; }
	}
};

let accent = CssColor::parse("blue")?;
let card = page!({
	article {
		class: STYLES.card() + "legacy-card",
		style: STYLES.vars().accent(accent),
		"Card"
	}
});
# Ok::<_, reinhardt_pages::CssValueError>(card)
```

The generated stylesheet is a static asset; applications must link it once per
document. Plain string `class:` and `style:` values remain supported for gradual
migration. Descendants use nested rules because Rust token streams do not retain
selector whitespace.

## Features

- **Fine-grained Reactivity**: Leptos/Solid.js-style Signal system with automatic dependency tracking
- **Hybrid Rendering**: SSR + Client-side Hydration for optimal performance and SEO
- **Django-like API**: Familiar patterns for Reinhardt developers
- **Low-level Only**: Built on wasm-bindgen, web-sys, and js-sys (no high-level framework dependencies)
- **Security First**: Built-in CSRF protection, XSS prevention, and session management
- **Simplified Conditional Compilation**: `cfg_aliases` integration and automatic event handler handling
- **Action State Helpers**: `use_action_state` and `Action::dispatching*` reduce async mutation boilerplate
- **Headless UI Primitives**: `reinhardt_pages::ui::{ActionButton, ActionResultPanel, ResourcePanel}` compose typed action and resource states without imposing visual styles
- **Controlled Form Elements**: `bind:` synchronizes typed signals with text, checkbox, radio, numeric, and select controls
- **Model-backed Forms**: `#[model(form = true)]` supplies typed fields and one
  policy-safe payload to `form!` on native and WASM targets

For a React concept mapping, see
[Reinhardt Pages for React developers](docs/react_to_reinhardt.md).

For route-level loaders, prepare/commit navigation, prefetch, cancellation, and
SSR hydration, see [Route-level data loaders](docs/route_loaders.md).

## Headless UI primitives

The `reinhardt_pages::ui` module provides small, headless building blocks for
action-heavy screens. `ActionButton` renders a semantic
`<button type="button">`, dispatches an `Action`, and manages `disabled` plus
`aria-busy="true"` while the action is pending. `ActionResultPanel` selects
idle, pending, success, or error content. `ResourcePanel` selects loading,
empty, success, or error content and can also consume a
`Resource::latest_after(action)` value so a successful mutation can take
precedence over stale resource data.

```rust,ignore
use reinhardt_pages::component::Page;
use reinhardt_pages::ui::{ActionButton, ActionResultPanel, ResourcePanel};

let save_button = ActionButton::new(save, project_id, Page::text("Save"));
let save_result = ActionResultPanel::new(save)
    .pending(|| Page::text("Saving"))
    .success(|value| Page::text(value.clone()));
let project_view = ResourcePanel::new(project)
    .loading(|| Page::text("Loading"))
    .success(|value| Page::text(value.clone()));
```

The examples use application-defined `save`, `project_id`, and `project`
handles. Slot closures are repeatable: idle and pending/loading slots use
`Fn() -> Page`, value slots use `Fn(&T) -> Page`, and error slots use
`Fn(&E) -> Page`. `ResourcePanel::empty_if` takes `Fn(&T) -> bool` and runs
before the success slot. Unconfigured slots render an empty page.

These primitives deliberately own behavior rather than presentation. CSS,
themes, live-region roles and announcements, localization, and error
redaction remain application-owned. Error slots receive the typed `&E` value;
the UI primitives do not stringify or log errors automatically.

For lifecycle-managed `Head` declarations across SSR, hydration, and SPA
navigation, see [Document head management](docs/document_head_management.md).

## Development template hot reload

Enable the `hmr` feature when developing a Pages WASM client and start the
existing development server with `runserver --with-pages`. Literal text and
literal attribute edits inside a WASM-owned `page!` template are classified as
state-preserving template patches. The browser validates each registered
template key and dynamic ABI, applies the DOM update transactionally, and
keeps retained dynamic ranges, event handlers, keyed instances, and bound
elements alive. Patches for templates that have not mounted yet are retained
until their first mount and validated against that descriptor then.

Edits to Rust expressions, event handlers, bindings, control flow, components,
the page callsite set, or shared/SSR-visible code are outside the safe static
boundary and use the normal WASM/server rebuild path. Failed builds and patch
rejections retain the last successful application, show normalized diagnostics
in the development overlay, and recover through the existing reload path.

The current static boundary is intentionally conservative: a literal edit
inside an element with a direct dynamic attribute, control binding, or event
handler, and any nested template edit, uses the normal rebuild path until that
subtree has an independently mountable runtime range.

The patch contract does not guarantee preservation of focus, selection, scroll
position, or uncontrolled input state when those nodes are replaced. A
representative development command is:

```bash
cargo run --bin manage -- runserver --with-pages
```

## Quick Start

### Using the Prelude (Recommended)

The prelude provides all commonly used types with a single import:

```rust
// Instead of multiple scattered imports:
// use reinhardt_pages::{Signal, View, use_state, ...};
// use reinhardt_pages::component::{ElementView, IntoView};
// use reinhardt_pages::reactive::{Effect, Memo};

// Use the unified prelude:
use reinhardt_pages::prelude::*;
// or via reinhardt crate:
use reinhardt::pages::prelude::*;
```

### Typed standard events and custom-event detail

Standard intrinsic events select an exact payload from the authoritative event
catalog. The payload is inferred in `page!`, or can be named explicitly for an
external function or `Callback`:

```rust,ignore
use reinhardt_pages::event::{ClickEvent, InputEvent};
use reinhardt_pages::prelude::*;

fn inspect_click(event: ClickEvent) {
    let _origin = event.target();
    let _listener = event.current_target();
}

page!({
    button { @click: inspect_click, "Inspect" }
    input { @input: |event: InputEvent| {
        match event.value() {
            Ok(value) => info_log!("value={value}"),
            Err(error) => warn_log!("input extraction failed: {error}"),
        }
    } }
})
```

For arbitrary custom events, choose the raw or typed DSL explicitly:

```rust,ignore
use reinhardt_pages::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct ItemSelected {
    id: u64,
}

page!({
    // Raw DOM event transport.
    button { @custom("item-selected"): |event: Event| { inspect(event); } }

    // Typed CustomEvent.detail transport.
    button { @custom::<ItemSelected>("item-selected"): |event| {
        if let Ok(detail) = event.detail() {
            select(detail.id);
        }
    } }
})
```

Native component tests can dispatch the same typed custom event:

```rust,ignore
button.dispatch(
    EventFixture::custom("item-selected")
        .custom_detail(&ItemSelected { id: 42 }),
)?;
```

`EventFixture::custom("item-selected")` by itself creates a plain named event,
not a browser `CustomEvent`. Use `.custom_detail_value(Value::Null)` to model
the browser `CustomEvent` default detail, and use a pre-serialized malformed
value for decoding-error tests:

```rust,ignore
use serde_json::{Value, json};

let default_detail = EventFixture::custom("item-selected")
    .custom_detail_value(Value::Null);
let malformed_detail = EventFixture::custom("item-selected")
    .custom_detail_value(json!({ "id": "not-a-number" }));
```

Component `@event` props remain typed by the component's declared prop type;
the DOM event catalog applies only to intrinsic elements.

### Lifecycle-managed document head

Head declarations are resolved from the active page and route tree. Use
`head!` for structural values, attach them with `#head:`/`Page::with_head`, or
provide route metadata with `RouteMetadata::with_head`:

```rust,ignore
let metadata = RouteMetadata::new().with_head(head!(|| {
    base { href: "/app/" }
    title { "Workspace" }
}));
```

Use `use_head` or `use_page_title` for retained reactive contributions. These
hooks require explicit dependencies, for example `deps![project.clone()]`, and
their registrations are removed with the owning reactive scope. A persistent
layout keeps its contribution while sibling routes change, and removing a
child reveals the previous parent or layout value.

The same resolution model is used by buffered/streaming SSR, hydration, and
browser mounting. Hydration adopts framework-marked SSR nodes before the body
pass. Browser reconciliation manages only `data-reinhardt-head` nodes, so
third-party head elements remain untouched. Unchanged scripts reuse their DOM
node; removing a script cannot undo side effects that already executed.

### Controlled form elements

Use `bind:` when a signal should own a native control after hydration. The
control shape determines the signal type: `String` for text, radio, and
single-select controls; `bool` for checkboxes; a supported numeric primitive
for number inputs; and `Vec<String>` for multiple selects.

```rust
use reinhardt_pages::prelude::*;

let query = Signal::new(String::new());
let parse_error = Signal::new(None::<NumberParseError>);
let amount = Signal::new(0_f64);

let _controls = page!({
    input { aria_label: "Search", bind: query, placeholder: "Search" }
    input {
        aria_label: "Amount",
        type: "number",
        bind: number(amount, parse_error),
    }
});
```

Hydration first adopts the live DOM value, preserving browser restoration and
edits made before hydration. The adopted value also becomes the browser reset
default, so a later form reset preserves the pre-hydration control state. Later
signal changes update the control. See the
[React migration guide](docs/react_to_reinhardt.md#controlled-and-uncontrolled-form-controls)
for event ordering, IME, numeric-error, and low-level escape-hatch details.
For `input[type=number]`, the binding combines `beforeinput` metadata with the
browser value so parse errors retain incomplete editor states when their edit
position is known. Only unmodified Arrow/Home/End keyboard moves are predicted;
modifier-key commands and already-canceled key events are treated as unknown. Browsers
do not expose number-input selection ranges; after
a pointer move followed immediately by sanitization, the error safely reports
the browser's empty value instead of inventing raw text.

### Simplified cfg Attributes with cfg_aliases

Configure `cfg_aliases` in your project's `build.rs`:

```rust
// build.rs
use cfg_aliases::cfg_aliases;

fn main() {
    // Rust 2024 edition requires explicit check-cfg declarations
    println!("cargo::rustc-check-cfg=cfg(wasm)");
    println!("cargo::rustc-check-cfg=cfg(native)");

    cfg_aliases! {
        // Browser-WASM only (wasm32-unknown-unknown); excludes WASI / emscripten.
        wasm: { all(target_family = "wasm", target_os = "unknown") },
        native: { not(all(target_family = "wasm", target_os = "unknown")) },
    }
}
```

Add to `Cargo.toml`:

```toml
[build-dependencies]
cfg_aliases = "0.2"
```

Now you can use shorter cfg attributes:

```rust
// Before:
#[cfg(target_arch = "wasm32")]
// After:
#[cfg(wasm)]

// Before:
#[cfg(not(target_arch = "wasm32"))]
// After:
#[cfg(native)]
```

### Automatic Event Handler Handling

The `page!` macro automatically handles event handlers for server-side rendering. You no longer need to write duplicate conditional blocks:

```rust
use reinhardt_pages::prelude::*;

// This works on both WASM and native targets!
// On WASM: Event handlers are bound to DOM events
// On native: Event handlers are stored and can be dispatched by component tests
fn my_button(on_click: Signal<bool>) -> View {
    page!({
        button {
            @click: move |_| { on_click.set(true); },
            "Click me"
        }
    })
}
```

**Before** (manual conditional compilation):
```rust
#[cfg(target_arch = "wasm32")]
{
    page!({
        button {
            @click: move |_| { on_click.set(true); },
            "Click me"
        }
    })
}
#[cfg(not(target_arch = "wasm32"))]
{
    let _ = on_click; // suppress warning
    page!({
        button { "Click me" }
    })
}
```

**After** (automatic handling):
```rust
// Just write once - the macro handles everything!
page!({
    button {
        @click: move |_| { on_click.set(true); },
        "Click me"
    }
})
```

### `page!` Body Forms

Use `page!({ ... })` for app screens and ordinary functions that return a
`Page`. Free value identifiers from the surrounding Rust scope are treated as
implicit captures and cloned into generated reactive/event closures. Captured
values must implement `Clone`; `Signal<T>`, `Callback`, `Page`, `String`, and
most application handles are intended to be cheap to clone.

Use `page!(|| { ... })` or `page!(|props: Props| { ... })` when you want a
reusable factory that is called later. Closure-form pages keep strict capture
discipline: every value used in the body must be listed as a closure parameter.
Existing body-only pages that relied on surrounding values should migrate to
`page!({ ... })`. Use `page!(|| { ... })` for no-argument factories that must
remain callable, and use `page!(|value: Value| { ... })` when a factory needs
caller-supplied state.

### Reactive I18n

Enable the `i18n` feature to use `reinhardt-i18n` catalogs directly from
`page!`. `t!` returns lazily translated page text, so SSR renders the current
locale and later locale switches update reactive snapshots without explicitly
threading a resource through each component.

```rust,ignore
use reinhardt_pages::i18n::{I18nContext, MessageCatalog, TranslationContext};
use reinhardt_pages::prelude::*;

let mut translations = TranslationContext::new("ja", "en-US");
let mut ja = MessageCatalog::new("ja");
ja.add_translation("dashboard.title", "ダッシュボード");
translations.add_catalog("ja", ja)?;

let i18n = I18nContext::new(translations);
let mut renderer = SsrRenderer::with_options(SsrOptions::new().i18n_context(i18n));

let html = renderer.render_page_with_view_head(page!(|| {
    h1 { { t!("dashboard.title") } }
})());
```

The SSR renderer serializes resolved catalogs into the hydration state under
`pages.i18n`, so client hydration can restore translations without refetching
the catalog.

### Forms: Static Definition and Dynamic Behavior

`form!` defines static form structure: field names, widgets, labels,
validation metadata, server function binding, and rendering. `use_form` owns
typed runtime behavior: values, field signals, dirty/touched state, validation
errors, loading, success, reset, and submit orchestration.

Async submit lifecycle callbacks re-enter the form's owning reactive scope
after the submit future resolves, so callbacks may safely create scoped
reactive handles even when the submit was started outside the render turn.

Model-backed browser submits retain the server function's typed response;
`submit_server_fn` exposes it through `UseFormAsyncSubmitOutcome` and routes
structured field errors into the same runtime state.

Pages validates an owned raw snapshot before generated submission, but the
client is not a trust boundary. The server must repeat the generated pipeline,
run any async application validator explicitly, and construct from the cleaned
payload with server-owned context:

```rust,ignore
use reinhardt_core::model_form::{
    ModelFormUpdatingPayload,
    ModelFormValidatingPayload,
};

let cleaned = payload.clean_and_validate()?;
ensure_cluster_name_available(&cleaned).await?;
let cluster = cleaned.into_model(
    ClusterModelFormServerContext::new().organization_id(organization_id),
)?;

let cleaned = update_payload.clean_and_validate_for_update(&existing)?;
ensure_cluster_name_available(&cleaned).await?;
let updated = cleaned.apply_to(existing)?;
```

Cleaned payloads are not deserializable. For partial updates, first call
`update_payload.clean_and_validate_for_update(&existing)`. This validates the
post-merge candidate, including synchronous cross-field rules, while returning
a partial cleaned payload. Then `cleaned.apply_to(existing)` changes only
supplied public fields and preserves primary keys, server-owned values, and
omitted fields. Bring `ModelFormUpdatingPayload` into scope for the update
method; `clean_and_validate()` remains the strict create boundary.
Use `#[form(validate = path)]` for synchronous cross-field validation and
`#[form(trim)]` for opt-in generated text, email, or URL trimming;
`#[field(...)]` remains database and model metadata. Database failures remain
the persistence boundary rather than form validation errors.

For model-derived controls, explicit field allowlists, display overrides,
trusted server setters, and native async persistence, see
[Model-backed Pages forms](docs/model_forms.md). Model mode submits one
model-generated generic payload. The form-specific policy and data alias remain
internal to the `form!` expression and cannot be named by callers.

Create the form with `form!`, then attach runtime behavior to that generated
form:

```rust
use reinhardt_pages::{form, use_form};

let login_form = form! {
    name: LoginForm,
    action: "/login",
    fields: {
        username: CharField { initial: String::new() }
        password: CharField { initial: String::new() }
    }
};

let runtime = use_form(&login_form).build();
runtime.set_value(login_form.username_field(), "ada".to_string());
```

DTO request types can opt in to generated client-form companions with
`ClientForm`. This keeps request field names, enum choices, and typed request
assembly tied to the DTO while still using the same `use_form` runtime. The
`#[client_form(...)]` attribute uses the same expansion logic as the derive;
the paired `#[derive(ClientForm)]` and helper attribute remain supported for
compatibility. Container and field-level serde metadata remains available even
when the DTO does not derive `Serialize` or `Deserialize`.
Import the attribute macro explicitly; it is intentionally not part of
`prelude::*` so legacy derive/helper declarations remain helper-only.
When no serde derive is present, place `#[client_form(...)]` before the
DTO's `#[serde(...)]` attributes so the attribute macro can consume them.
Add `validate` when the DTO implements `Validate` and should feed those errors
into the generated form runtime:

```rust,ignore
use reinhardt_pages::{ClientFormChoices, client_form, use_form};

#[derive(Clone, Default, PartialEq, ClientFormChoices)]
#[serde(rename_all = "snake_case")]
enum ProviderMode {
    #[default]
    Fake,
    LiveApi,
}

#[reinhardt::dto]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[client_form(server_fn = crate::server::submit_project, validate)]
struct ProjectRequest {
    name: String,
    title: Option<String>,
    provider_mode: ProviderMode,
}

let form = ProjectRequestClientForm::new();
let runtime = use_form(&form).build();
runtime.set_value(ProjectRequestClientFormField::Title, "  ".to_string());
let request = ProjectRequestClientForm::to_request(&runtime);
assert_eq!(request.title, None);
let outcome = form.submit(&runtime).await?;
```

Generated `submit` methods have the same signature on native and WASM targets,
so shared components can construct one action without target-specific branches.
Submission executes only on WASM; native SSR code must not await or dispatch the
generated method.

`ClientFormChoices` mirrors serde's externally tagged string names for unit
variants, including matching `rename_all` and variant `rename`; tagged,
untagged, or directionally renamed enum representations are rejected because
form choices submit bare strings. DTO fields marked with serde skip attributes
are kept out of editable form fields and preserved through generated request
values. Exported DTOs cannot use private editable fields; mark the field public
or make it an explicit hidden field with `#[client_form(skip)]` or a serde skip
attribute. Forms with generated `server_fn` submit helpers reject serde-skipped
request fields because the browser payload must match native request
deserialization exactly.

Use `use_form_action` when a validated form should dispatch a typed async
mutation:

```rust,ignore
use reinhardt_pages::{form, use_form, use_form_action};

let runtime = use_form(&login_form).build();
let save = use_form_action(&runtime, |values: LoginFormValues| async move {
    submit_login(values).await
})
.on_success(|runtime, _result| {
    runtime.reset_default_values();
});

if !save.is_pending() {
    save.submit();
}
```

For native form submission and headless status UI, attach the form action's
submit handler to the containing form and use the form-aware primitives:

```rust,ignore
use reinhardt_pages::ui::{FormActionButton, FormActionResultPanel};

let submit = save.submit_handler();
let button = FormActionButton::new(save.clone(), "Save");
let result = FormActionResultPanel::new(save.clone())
    .pending(|| Page::text("Saving"))
    .validation_error(|message| Page::text(message.to_string()))
    .success(|_| Page::text("Saved"))
    .error(|error| Page::text(error.to_string()));
```

Bind `submit` to the containing element's `Submit` event. The button remains
`type="submit"`, so pointer activation and Enter-key submission use the same
validated `FormAction::submit()` path. `Resource::latest_after_form(&save)`
reconciles successful form mutations without exposing payload dispatch.

`FileField` and `ImageField` also participate in the generated runtime
contract as `Option<web_sys::File>` values. File values are browser-owned and
are tracked for dirty/touched state without treating the file payload as a
serializable scalar.

Stable native widget coverage includes the following `form!` DSL items:

| DSL item | HTML output | Value state |
|---|---|---|
| `MonthInput` | `<input type="month">` | string field |
| `WeekInput` | `<input type="week">` | string field |
| `ResetButton` | `<button type="reset">` | none |
| `Button` | `<button type="button">` | none |
| `ImageInput` | `<input type="image">` | none |
| `Datalist` | `<datalist>` | option source only |
| `OptGroup` | `<optgroup>` | choice grouping only |
| `Output` | `<output>` | none |
| `Meter` | `<meter>` | none |
| `Progress` | `<progress>` | none |

Typed native attributes are accepted for the controls that support them:

| Attribute | Compatible controls |
|---|---|
| `min` / `max` / `step` | number, range, date, time, datetime-local, month, week |
| `size` | text-like inputs |
| `accept` / `capture` | file-like inputs |
| `multiple` | file-like inputs and multi-select |
| `list` | datalist-compatible text-like inputs |

`FieldGroup` renders as semantic `<fieldset>` output. When `label` is
present, the label is rendered as a `<legend>` inside the fieldset.

`CustomWidget` is experimental and must opt in explicitly:

```rust,ignore
date_range: CharField {
    widget: CustomWidget(crate::widgets::DateRangePicker) {
        experimental,
        adapter: crate::widgets::DateRangeAdapter,
    },
}
```

The adapter API may change in a minor release with a documented migration path.

Arguments supplied from ambient context use `ambient_arguments`. The old
`strip_arguments` name remains as a deprecated alias. CSRF should stay at the
transport layer: `#[server_fn]` client stubs attach `X-CSRFToken`, while
non-WASM forms still render the hidden CSRF input for traditional posts.

### Structured Server-Function Errors

The native `model-server-fnset` feature can turn a proven model constraint
violation into a safe validation response. Use the optional callback only for
fixed, client-safe text:

```rust,ignore
let server_error = ServerFnError::try_from_model_error_with::<User, _>(
	error,
	|database_error, _fields| {
		(database_error.constraint() == Some("users_email_unique"))
			.then(|| "This email is already registered".to_owned())
	},
)
.unwrap_or_else(|error| {
	tracing::error!(error = %error, "user write failed");
	ServerFnError::application("Failed to save user")
});
```

Callback code must not return `DatabaseError::message()`, rejected values,
table names, constraint names, or vendor diagnostics to the browser. A known
single-field violation maps to that field; composite `UNIQUE` and `CHECK`
violations map to the form. Unmapped or unproven errors remain the original
framework error, so the caller chooses the safe fallback above. This helper is
native-only; browser code consumes the resulting `ServerFnError` instead.

This conversion preserves the existing serialized `ServerFnError` wire shape:
it adds no database metadata to the browser response. Generated client forms
route field errors by logical model field names. Composite `UNIQUE` and `CHECK`
violations have no single logical field, so they reach the form error instead.

### Typed multipart server functions

The function-like `#[server_fn]` API infers multipart transport when a
client-visible argument is exactly `UploadedFile` or `Option<UploadedFile>`.
Argument identifiers become multipart part names. All other client-visible
arguments remain scalar JSON parts, and the response codec remains JSON; do
not add a multipart codec option.

```rust,no_run
use reinhardt_core::parsers::UploadedFile;
use reinhardt_pages::server_fn::{server_fn, ServerFnError};

#[server_fn]
async fn save(name: String, avatar: Option<UploadedFile>) -> Result<usize, ServerFnError> {
    let _ = name;
    Ok(avatar.as_ref().map_or(0, |file| file.size))
}

async fn call_save() -> Result<usize, ServerFnError> {
    save(String::from("Ada"), None).await
}

# fn main() {}
```

On the browser, an optional file input with an empty filename and no bytes is
decoded as `None`; a named zero-byte file remains a file. Required files reject
an empty browser file. Unsupported client-visible file shapes include type
aliases, `Vec<UploadedFile>`, nested `Option`, and other wrappers. Destructured
client arguments are not multipart names. File arguments cannot be combined
with an explicit `json`, `url`, or `msgpack` codec.

For storage-backed model values, use the database field descriptor lifecycle
methods described by `reinhardt-db`; the lower-level storage `store` operation
does not belong in a server-function argument decoder.

### Server-function injection

Injected server-function parameters support mutable bindings and destructuring
patterns while preserving those bindings in the server implementation.

```rust,ignore
#[inject] mut db: DatabaseConnection
#[inject] Wrapper(mut value): Wrapper<Data>
```

Mutability applies only to the server function's internal binding; it does not
change resolver ownership or caching.

### Structured server-function errors

`ServerFnError` is a typed, versioned error contract shared by server handlers,
client stubs, `Action<T, ServerFnError>`, and `Resource<T, ServerFnError>`.
Use `kind()`, `status()`, `user_message()`, and `field_errors()` rather than
parsing a server response:

```rust,ignore
use reinhardt_pages::{ServerFnError, ServerFnErrorKind};

fn render_error(error: &ServerFnError) {
    match error.kind() {
        ServerFnErrorKind::Validation => {
            for field_error in error.field_errors() {
                show_field_error(field_error.field(), field_error.message());
            }
        }
        ServerFnErrorKind::Auth => redirect_to_login(),
        _ => show_message(error.user_message()),
    }
}
```

Construct server failures with `validation(field_errors)`, `validation_with_message(...)`,
`auth(status, message)`, `application(message)`, `server(status, message)`,
`transport(message)`, or `deserialization(message)`. A validation error has
status `422`; converting `ValidationErrors` with `?` preserves every field
message for generated forms.

Failure responses use a version 1 JSON envelope with `version`, lowercase
`kind`, nullable `status`, safe `message`, and a `field_errors` array. This is
a breaking 0.4 API and wire-format change: enum variant matching and legacy
externally tagged JSON parsing are no longer supported. Deploy server and WASM
client artifacts together because the legacy error envelope is not accepted at
runtime. See the [Server Function Macro Guide](docs/server_fn_macro.md) and
the [0.4 migration guide](../../instructions/MIGRATION_0.4.md) for examples.

### Reactive Conditional Rendering

`page!` wraps expression, `if`, and `for` nodes in reactive render scopes. When
those nodes read `Signal` values, they re-evaluate as the signals change.

#### Why Signal Reads Belong Inside `page!`

When you extract Signal values before the `page!` macro, they become static:

```rust
// Problem: Static values don't update when Signal changes
let has_error = error.get().is_some(); // Static bool captured at render time
page!({
    if has_error {
        div { "Error occurred" }
    }
})
```

Read the signal inside the page body instead:

```rust
page!({
    if error.get().is_some() {
        div { { error.get().unwrap_or_default() } }
    }
})
```

#### Signal-first Pattern

For reactive UIs, pass Signals directly to the `page!` macro instead of extracting values:

```rust
use reinhardt_pages::prelude::*;

fn error_display() -> View {
    let (error, set_error) = use_state(None::<String>);

    // Read the Signal inside page! (not before it)
    let error_signal = error;

    page!({
        if error_signal.get().is_some() {
            div {
                class: "alert-danger",
                { error_signal.get().unwrap_or_default() }
            }
        }
    })
}
```

#### Reactive vs Static Values

| Syntax | Use Case | Behavior |
|--------|----------|----------|
| `if signal.get().is_some() { ... }` | Signal-dependent branches | Re-evaluates when the signal changes |
| `for item in items.get() { ... }` | Signal-dependent lists | Rebuilds the list when the signal changes |
| `if precomputed_bool { ... }` | Static values | Uses the value captured when the page is built |

#### Best Practices

1. **Pass Signals directly**: Use `Signal<T>` parameters instead of extracting values
2. **Copy reactive handles**: Pass `Signal`, `Memo`, `Action`, `Resource`, and `Callback` handles directly
3. **Clone captured handles**: direct `page!({ ... })` clones captured values into generated closures
4. **Use closure form for factories**: keep `page!(|props: Props| { ... })` when the page must be called later

## Typed Server Function Sets

`#[server_fnset]` groups existing `#[server_fn]` markers into a named, typed
registration chain. Members retain their individual codecs, CSRF behavior,
extractors, injected parameters, metadata, and mock identity. Registration stays
explicit:

```rust,ignore
#[server_fnset(name = "admin")]
pub fn admin_fns() -> impl ServerFnSetRegistration {
    ServerFnSet::new()
        .server_fn(load_dashboard::marker)
        .server_fn(export_data::marker)
}

let router = ServerRouter::new().server_fnset(admin_fns());
```

The opt-in `model-server-fnset` feature generates exactly six typed POST RPCs
for a resource: `list`, `retrieve`, `create`, `update`, `partial_update`, and
`destroy`. Resources use explicit wire DTO mappings, a typed unique lookup, and
a mandatory policy; unrestricted access requires choosing `AllowAllPolicy`.
Standard methods can be replaced with checked overrides, while additional
methods use `#[action(detail = ..., transactional = ...)]`. Action underscores
normalize to hyphens under `/api/server_fn/<set-name>/<action>`.

Offset pagination defaults to 25, accepts limits from 1 through 100, and returns
the policy-scoped total before slicing. Structured model errors map to stable
400, 401, 403, 404, 409, and 500 responses, with internal failures sanitized on
the wire. Each generated action has its own marker for component and MSW mocks;
`reinhardt-test` model-action mocks use `model-server-fnset-msw`.

Full and partial model updates authorize the loaded object before mutation and
authorize the resulting object again before its read mapping and transaction
commit, so ownership or tenant changes cannot bypass object policy.

WASM builds retain wire contracts, metadata, markers, and client stubs. ORM
resources, policies, action contexts, database executors, native handlers, and
the `ModelServerFnSet` constructor remain native-only. Model sets do not provide
action subsets, a read-only set type, REST/OpenAPI generation, cursor
pagination, bulk or nested actions, composite lookups, global discovery, or
automatic model-to-DTO derivation. See the
[Server Function Macro Guide](docs/server_fn_macro.md#typed-server-function-sets)
for the complete `ArticleResource` example and target boundary.

## Testing

### Native Component Tests

Use `reinhardt_pages::testing::component::render` for fast interaction tests
that do not need a browser:

```rust
use reinhardt_pages::testing::component::{Role, render};

#[tokio::test]
async fn refresh_loads_jobs() {
    let screen = render(jobs_page);
    screen.mock_server_fn::<load_jobs::marker>(|_args| Ok(vec!["Index job".to_string()]));

    screen.get_by_role(Role::Button, "Refresh").click();
    screen.settle().await;

    assert!(screen.query_by_text("Index job").is_some());
}
```

The mock API uses `MockableServerFn` markers and therefore requires the
`msw` feature. Use direct `server_fn` calls for business logic tests and
WASM/browser tests for hydration or browser API coverage. The native renderer
resolves reactive views, active suspense branches, and deferred content branches
before exposing queryable text and roles.

## Architecture

### Fresh CSR root contexts

Install application-wide contexts on `ClientLauncher` so their RAII guards
remain live for the initial render and later SPA navigations:

```rust,ignore
let i18n = I18nContext::empty("en-US", "en-US"); // Requires the `i18n` feature.

ClientLauncher::new("#root")
    .i18n_context(i18n)
    .register_routes_from_inventory()
    .launch()
```

For other context keys, use
`ClientLauncher::provide_context(&context, value)`. The launcher installs all
root contexts before lifecycle callbacks and router construction. A failed
launch drops the guards automatically.

This framework consists of several key modules:

- **`reactive`**: Fine-grained reactivity system (Signal, Effect, Memo)
- **`dom`**: DOM abstraction layer
- **`builder`**: HTML element builder API
- **`component`**: Component system with IntoView trait
- **`form`**: Cross-target model-form state plus native Django Form rendering
- **`csrf`**: CSRF protection
- **`auth`**: Authentication integration
- **`api`**: API client with Django QuerySet-like interface
- **`server_fn`**: Server Functions (RPC)
- **`ssr`**: Server-side rendering
- **`hydration`**: Client-side hydration
- **`router`**: Client-side routing (reinhardt-urls compatible)
- **`platform`**: Platform abstraction types
- **`prelude`**: Unified imports

## Prelude Contents

The prelude includes:

### Reactive System
- `Signal`, `Effect`, `Memo`, `Resource`, `ResourceState`
- `LatestResourceValue`, `LatestResourceState`, `use_latest_resource_value`
- Context: `Context`, `ContextGuard`, `create_context`, `get_context`, `provide_context`, `remove_context`
- Explicit batching: `reinhardt_pages::reactive::batch`

### Hooks
- `use_state`, `use_effect`, `use_memo`, `use_callback`, `use_context`
- `use_ref`, `use_reducer`, `use_transition`, `use_deferred_value`
- `use_id`, `use_layout_effect`, `use_debug_value`
- `use_optimistic`, `use_action`, `Action::with_optimistic`, `use_shared_state`, `use_sync_external_store`
- `use_resource` (async data fetching; `use_resource(fetcher, deps![...])` uses an explicit dependency list, while SSR registers the fetcher in the request context, awaits it up to `SsrOptions::resource_timeout`, and serializes resolved state for hydration)
- `use_query`, `use_action`, `QueryClient`, `QueryFamily`, `QueryOptions` (application-owned keyed async data with shared requests, observer policies, and explicit mutation invalidation)

### Headless UI primitives

The three public components are also re-exported by the prelude. Prefer the
explicit `reinhardt_pages::ui::{ActionButton, ActionResultPanel, ResourcePanel}`
path when documenting component boundaries or when a narrower import is useful.

`Resource::latest_after(&action)` and `use_latest_resource_value(resource)` compose loaded resource state with one or more `Action` success values. Later actions have higher priority, and `refetch_on_success()` can automatically refresh the resource after a mutation succeeds.

Action invocation and each deferred future poll run in the action's owning
reactive scope. A disposed owner cancels the pending future before another
poll, preventing stale mutation work from allocating into a removed scope.

For cross-component reads, prefer `use_query` with the descriptor generated by
`#[server_fn]`. A launched browser application owns one `QueryClient`; each SSR
request and each native component-test screen receives an isolated client. Set
application defaults when constructing the launcher:

```rust,ignore
ClientLauncher::new("#root")
    .query_defaults(
        QueryDefaults::new()
            .stale_time(Duration::from_secs(30))
            .gc_time(Duration::from_secs(300)),
    )
    .router(app_router)
    .launch()?;
```

The macro emits `family()`, `key(args...)`, and `query(args...)`. `query`
combines the exact key with its fetcher, while `family` selects every cached
argument set for that endpoint:

```rust,ignore
use reinhardt_pages::server_fn::{ServerFnError, ServerFnErrorKind};

let jobs = use_query(
    list_project_jobs::query(project_id),
    QueryOptions::new().refetch_interval(Duration::from_secs(5)),
);

let retrying_jobs = use_query(
    list_project_jobs::query(project_id),
    QueryOptions::new().retry(
        RetryPolicy::exponential()
            .max_attempts(3)
            .base_delay(Duration::from_millis(250))
            .max_delay(Duration::from_secs(5))
            .jitter(true)
            .when(|error: &ServerFnError| {
                matches!(
                    error.kind(),
                    ServerFnErrorKind::Server | ServerFnErrorKind::Transport
                )
            }),
    ),
);

let client = queries();
let retry = use_action(move |job_id| {
    let client = client.clone();
    async move {
        let result = retry_job(project_id, job_id).await?;
        client.invalidate_family(list_project_jobs::family());
        Ok::<_, ServerFnError>(result)
    }
});
```

Use `client.invalidate(&list_project_jobs::key(project_id))` when only one
argument set changed. Use `invalidate_family(list_project_jobs::family())` when
a mutation may affect every cached argument set. Invalidation is an explicit
success-path effect of `use_action`; failed mutations leave the cache unchanged.

Use `client.remove(&list_project_jobs::key(project_id))` or
`client.remove_family(list_project_jobs::family())` at an authentication
boundary when cached data must not cross principals. Eviction physically drops
the cached result, retry state, and active request; existing handles are reset,
and the next observer starts from `Pending` (or `Idle` when disabled) instead of
seeing the previous principal's success.

For non-server-function data, define a manual typed family and provide the
fetcher when building each descriptor:

```rust,ignore
const PROJECTS: QueryFamily<u64, Vec<Project>, ApiError> =
    QueryFamily::new("projects.by-organization");

let projects = use_query(
    PROJECTS.query(organization_id, move || fetch_projects(organization_id)),
    QueryOptions::new().stale_time(Duration::from_secs(60)),
);
```

Treat a manual family's ID and `Args` encoding as a persistent cache contract.
Every descriptor that reuses the same family ID with the same argument type
must represent the same semantic operation and produce the same canonical JSON
argument shape. If the operation meaning or canonical encoding changes, use a
new versioned ID such as `projects.by-organization.v2`, even when the Rust types
stay unchanged.

`QueryOptions` are fixed when an observer mounts. They control `enabled`,
`stale_time`, `gc_time`, `refetch_interval`, and retry policy for that observer.
Three retry attempts include the initial request. Intermediate errors remain
private until the shared sequence is exhausted, equal jitter stays between half
and all of the nominal delay, and `is_fetching` is false during backoff. Disabled
observers without cached data report `QueryStatus::Idle`; enabled initial loads
report `Pending`; resolved reads report `Success` or `Error`. During a
background refetch, successful data remains available, `is_fetching` is true,
and a failure appears in `refetch_error` instead of replacing the stale data.
Polling is owned by the query observer, suspends while the browser document is
hidden, and resumes according to whether the cached value is stale.

Mounted observers for the same key share entry-level attempt numbers even when
their retry policies differ. Browser retry backoff also pauses while the
document is hidden. When visibility returns, stale data retries immediately;
fresh data waits only the remaining visible-time delay.

The cache canonicalizes JSON object arguments, hashes the canonical payload in
the generated key ID, and deduplicates mounted queries with the same key. Raw
server-function arguments are therefore not written into SSR hydration keys.
SSR serializes settled query snapshots into the request hydration payload, and
the browser seeds its application client before the first observer mounts, so
hydrated data is reused without a duplicate initial request. Server-function
descriptors that depend on request extractors or injected parameters skip
native SSR prefetching and are left for the browser fetch path or native
component-test mocks. Query handles can also be tracked by
`SuspenseBoundary::track(...)`.

### Normalized entity cache

Normalized caching is an opt-in extension to Query Client V2. Implement
`Entity` with a non-empty, application-wide stable `TYPE` and a serializable
`Id`; the `TYPE` is part of the cache identity, so two entity types may safely
share the same raw ID. Add a projection to a descriptor with
`QueryDescriptor::with_entities`:

```rust,ignore
use reinhardt_pages::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Project {
    id: u64,
    name: String,
}

impl Entity for Project {
    type Id = u64;
    const TYPE: &'static str = "example.project";

    fn entity_id(&self) -> Self::Id {
        self.id
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct LoadError;

const PROJECTS: QueryFamily<u64, Project, LoadError> =
    QueryFamily::new("projects.detail.v1");

let project = use_query(
    PROJECTS
        .query(7, || async {
            Ok::<_, LoadError>(Project {
                id: 7,
                name: String::from("Pages"),
            })
        })
        .with_entities(EntityValue::<Project>::new()),
    QueryOptions::new(),
);
assert_eq!(project.data().map(|value| value.id), None);
```

Use `EntityValue<E>` for a required entity, `OptionalEntity<E>` for an
optional entity, and `EntityVec<E>` for an ordered vector. Required removal
makes the internal query materialization `MissingRequired`, marks the query
stale (`normalization_missing` internally), and always retains its last
successful `T` with `QueryStatus::Success`, even for inactive or disabled
handles. Only an active enabled `QueryHandle` automatically schedules at most
one recovery refetch; inactive or disabled handles wait for an enabled mount
or an explicit refetch. Optional removal yields `None`; vector removal drops
only the removed ID and preserves the remaining order. A direct
`client.entity::<E>(id)` handle reads `None` for a vacant or tombstoned record.
Entity handles and query dependencies hold leases, and the arena retains
unleased records and tombstones until the client's default `QueryDefaults::gc_time`
deadline.

For combined results, define a zero-sized custom `EntityProjection`. Give its
recipe a versioned, non-empty `SCHEMA`, write complete entity values in
`normalize`, declare every identity that `EntityReader` may read in
`dependencies`, and handle tombstones in `apply_removals`:

```rust,ignore
#[derive(Clone, Copy)]
struct ProjectCardProjection;

impl EntityProjection<ProjectCard> for ProjectCardProjection {
    type Recipe = (u64, Option<u64>);
    const SCHEMA: &'static str = "project-card.v1";

    // normalize writes Project/User records and returns only serializable IDs.
    // dependencies declares both IDs; materialize uses required/optional reads.
    // apply_removals returns MissingRequired or Updated for tombstones.
    # /* Implement the four EntityProjection methods. */
}

let descriptor = PROJECT_CARD.query(7, fetch_project_card)
    .with_entities(ProjectCardProjection);
```

`upsert_entity` and `remove_entity` are convenience wrappers around
`update_entities`. Call them after a successful mutation; each upsert is a
complete replacement and normalization never infers collection membership,
relationships, cascades, patches, or optimistic rollback. A multi-entity
transaction stages all writes and publishes dependent query snapshots and
entity signals atomically:

```rust,ignore
let client = queries();
client.update_entities(|entities| {
    entities.upsert(updated_project);
    entities.upsert(updated_owner);
    entities.remove::<Project>(&removed_project_id);
});
```

SSR requests use an isolated `QueryClient::new_ssr` arena. Reads from
normalized queries and entity handles mark reachable identities; SSR emits one
deduplicated `(TYPE, canonical ID)` table per request. Browser hydration
consumes that table into the existing application client before the first
observer materializes its recipe, avoiding a duplicate initial fetch. Invalid
versions, duplicate identities, mismatched types, or missing required entities
are rejected. Plain query snapshots retain their existing serialization and
hydration path.

Existing Query Client V2 families need no migration. `QueryHandle<T, E>` still
returns the original `T` from `snapshot()` and `data()`, while normalization is
enabled only on descriptors that call `.with_entities(...)`. Family IDs,
`QueryOptions`, invalidation, polling, and `QueryStatus` remain unchanged.
SSR retry is a double opt-in: the query needs a `RetryPolicy` and the renderer
must enable request-owned retries. The resource timeout is one budget for all
fetch attempts, backoff, and jitter:

```rust,ignore
let options = SsrOptions::new()
    .query_retries(true)
    .resource_timeout(Duration::from_secs(2));
```

### Query client v2 migration

Query client v2 moves fetchers and policies out of keys and handles:

```rust,ignore
// Before
let jobs = use_query(list_project_jobs::key(project_id)).poll(Duration::from_secs(5));

// After
let jobs = use_query(
    list_project_jobs::query(project_id),
    QueryOptions::new().refetch_interval(Duration::from_secs(5)),
);
```

Replace `QueryKey::new` with a generated or manual `QueryFamily`, handle
`.poll(...)`, `.stale_time(...)`, and `.gc_time(...)` with mount-time
`QueryOptions`, and `use_mutation` with `use_action`. `Action::invalidates` was
removed; invalidate an exact key or family explicitly after the mutation
succeeds. Install retry behavior with `QueryOptions::retry`; entity
normalization (#5843) remains a separate non-goal.

### Component System
- `Component`, `ElementView`, `IntoView`, `View`, `Props`, `ViewEventHandler`
- `SuspenseBoundary`, `ErrorBoundary`, `BoundaryError`, `ErrorTracker`
  - `ErrorBoundary::new()` uses a generic default error message so tracked resource
    errors do not expose internal diagnostics. Provide a custom fallback only for
    messages that are safe to show to clients.

### Events and Callbacks
- `EventPayload`, catalog-generated payloads such as `ClickEvent` and `InputEvent`
- `EventTarget`, `EventTargetError`, `EventFile`, `Modifiers`, `Point`
- `Callback`, `IntoTypedEventHandler`, `typed_event_handler`; `Callback::new` requires an active
  scope, while `Callback::new_in_scope` creates external callbacks under an explicit disposable
  `ReactiveScope`
- `raw_event_handler` and `platform::Event` for explicit raw custom events
- `ControlBindingError`, `NumberParseError`, `NumberParseErrorKind`, `NumberValue`
- [Native component testing](docs/native_component_testing.md)

### DOM
- `Document`, `Element`, `EventHandle`, `EventType`, `document`

### Routing

```rust,no_run
use reinhardt_pages::{NavigationType, navigate_named, route_params};

fn navigate_to_document() -> Result<(), reinhardt_pages::NavigateError> {
    navigate_named(
        "workspace-document",
        route_params! {
            "workspace_id" => 42_i64,
            "slug" => "draft",
        },
        NavigationType::Push,
    )
}
```

`navigate_named()` requires an active SPA router and resolves registered routes
by name without a hard reload. Pass homogeneous parameter arrays directly, or
use `route_params!` to format mixed `Display` values into owned parameters.

Applications can replace local `navigate_to` wrappers with the framework-owned
path fallback:

```rust,no_run
use reinhardt_pages::{NavigationType, navigate_or_reload};

fn navigate_to_login() -> Result<(), reinhardt_pages::NavigateError> {
    navigate_or_reload("/login/", NavigationType::Push)
}
```

On browser WASM, `navigate_or_reload()` falls back to a hard browser navigation
only after SPA navigation returns `RouterNotInstalled`; rejected routes and
route-resolution errors are returned without retrying. Cross-origin HTTP(S) and
browser-safe non-HTTP destinations such as `blob:` use hard navigation directly.
Same-origin HTTP(S) absolute URLs are normalized to their path and query for SPA
navigation, while destinations containing a fragment use hard navigation so the
browser performs its native anchor scroll. Unsupported schemes such as
`javascript:` and `data:` return `HardNavigationFailed`. Native and SSR callers
receive `RouterNotInstalled` when no router is installed.

- `Link`, `Router`, `Route`, `RouterOutlet`, `PathPattern`

### API and Server Functions
- `ApiModel`, `ApiQuerySet`, `Filter`, `FilterOp`
- `ServerFn`, `ServerFnError`, `ServerFnErrorPayload`, `ServerFnErrorKind`, `ServerFnFieldError`
- See [Server Function Macro Guide](docs/server_fn_macro.md) for detailed usage and migration information
- Use `#[client_page]` for client page functions that must also compile as native route-table stubs
- See [WASM/server API Parity Macro](docs/wasm_server_api.md) for APIs that need matching public surfaces with target-specific implementations
- See [React-to-Reinhardt Guide](docs/react_to_reinhardt.md) for React hooks, JSX, actions, routing, SSR, and hydration mappings

### Authentication and Security
- `AuthData`, `AuthError`, `AuthState`, `auth_state`
- `CsrfManager`, `get_csrf_token`

### SSR and Hydration
- `HydrationContext`, `HydrationError`, `hydrate`
- `SsrOptions`, `SsrRenderer`, `SsrStream`, `SsrState`

SSR rendering APIs are async. Use `render_page(...).await` for streamed output
or `render_page_to_string(...).await` when a buffered string is needed:

```rust,no_run
use reinhardt_pages::component::{Component, Page};
use reinhardt_pages::ssr::{SsrOptions, SsrRenderer};
use std::time::Duration;

struct App;

impl Component for App {
    fn render(&self) -> Page {
        Page::text("Hello")
    }

    fn name() -> &'static str {
        "App"
    }
}

async fn render_app() {
    let app = App;
    let mut renderer = SsrRenderer::new();
    let stream = renderer.render_page(&app).await;

    let mut renderer = SsrRenderer::with_options(
        SsrOptions::new().resource_timeout(Duration::from_secs(1)),
    );
    let html = renderer.render_page_to_string(&app).await;
    let _ = (stream, html);
}
```

Resources created with `use_resource` during SSR are keyed deterministically,
resolved on the server, and embedded in the hydration payload. Use
`use_resource_with_key` when a resource hook is conditionally rendered and needs
a stable explicit hydration key. Implicit resource keys are allocated at the
document level so marker-rendered islands and their hydration replays preserve
the same key order. Suspense boundaries keep fallback and content roots
transparent; streaming metadata is emitted outside the branch DOM.

### I18n
- Reactive Pages API: `I18nContext`, `I18nStateError`, `TranslatedText`, `tr`, `tn`, `tp`, `tnp`
- Catalog and global API: `MessageCatalog`, `TranslationContext`, `I18nError`, `LazyString`, `TranslationGuard`, and the functions under `reinhardt_pages::i18n`

### Forms
- Cross-target: `ModelFormState`, `ModelFormPolicy`, `ModelFormSchema`, and
  generated model payload contracts; wasm browser file selections remain outside JSON payloads
- Native: `FormBinding`, `FormComponent`, `Widget`, `FieldMetadata`, and
  `FormMetadata`
- [Model-backed Pages forms](docs/model_forms.md)

### Macros
- `page!`
- `head!`
- `form!`
- `t!` (with the `i18n` feature)
- `client_page`
- `wasm_server_api`

### Task spawning (cross-target)
- `spawn_task`, `defer_yield` (no-op on native)

### WASM-specific
- `spawn_local` (re-exported from wasm_bindgen_futures; **deprecated** — use `spawn_task`)

## Example

```rust
use reinhardt_pages::prelude::*;

fn counter() -> View {
    let (count, set_count) = use_state(0);

    page!({
        div {
            p { { format!("Count: {}", count.get()) } }
            button {
                @click: move |_| set_count.update(|current| current + 1),
                "Increment"
            }
        }
    })
}
```

## Feature Flags

| Feature | Description |
|---------|-------------|
| `msgpack` | MessagePack serialization support |
| `model-server-fnset` | Native model-backed typed CRUD server function sets plus cross-target wire/client generation |
| `msw` | Typed marker metadata for component and MSW server function mocks |
| `testing` | Cross-target testing support; combine with `msw` for native component mocks |
| `pages-full` | Browser-oriented bundle (`msgpack` + `web-sys-full`); enable `model-server-fnset`, `msw`, and `testing` separately |
| `static` | Static file serving |
| `urls` | URL routing integration |
| `debug-hooks` | Debug hooks for development |
| `uuid` | UUID type support |
| `chrono` | Chrono date/time type support |
| `ast` | AST processing support |
| `web-sys-full` | All required web-sys features for WASM applications |

## License

Licensed under the BSD 3-Clause License.
For SSR documents, include generated component styles explicitly with
`component_stylesheet_url()`. The helper resolves the stable
`__reinhardt__/components.css` logical path through the active development URL
or production manifest; it does not inject a `<link>` element automatically.
