//! Reinhardt Pages - WASM-based Frontend Framework
//!
//! A Django-inspired frontend framework for Reinhardt that preserves the benefits of
//! Django templates while leveraging WebAssembly for modern interactivity.
//!
//! Use [`reactive::batch`] to group related reactive writes into one update
//! cycle. Async [`Action`] handles can be connected to [`OptimisticState`] with
//! [`Action::with_optimistic`] so failed mutations automatically roll back
//! optimistic UI state. [`Resource::latest_after`] and
//! [`use_latest_resource_value`] compose loaded resource state with action
//! success values so screens can render the latest loaded or mutated data.
//! [`use_query`] and [`use_action`] provide application-owned keyed reads and
//! explicit mutation workflows. A browser [`ClientLauncher`] owns one
//! [`QueryClient`], while SSR requests and native component-test screens use
//! isolated clients. Generated [`QueryFamily`], [`QueryKey`], and
//! [`QueryDescriptor`] helpers canonicalize JSON object arguments, hydrated
//! settled state is reused by the first client observer, and [`QuerySnapshot`]
//! distinguishes initial pending state from background fetching.
//!
//! ## Typed multipart server functions
//!
//! The function-like [`server_fn`] API infers multipart transport when a
//! client-visible argument is exactly
//! [`reinhardt_core::parsers::UploadedFile`] or
//! `Option<UploadedFile>`. Argument identifiers become multipart part names;
//! every other client-visible argument is encoded as a scalar JSON part.
//! Multipart is inferred request framing, not a selectable codec.
//!
//! ```rust,no_run
//! use reinhardt_core::parsers::UploadedFile;
//! use reinhardt_pages::server_fn::{server_fn, ServerFnError};
//!
//! #[server_fn]
//! async fn save(name: String, avatar: Option<UploadedFile>) -> Result<usize, ServerFnError> {
//!     let _ = name;
//!     Ok(avatar.as_ref().map_or(0, |file| file.size))
//! }
//!
//! async fn call_save() -> Result<usize, ServerFnError> {
//!     save(String::from("Ada"), None).await
//! }
//!
//! # fn main() {}
//! ```
//!
//! On the browser, an optional file input with an empty filename and no bytes
//! becomes `None`; a named zero-byte file remains a file. Required files reject
//! an empty browser file. Type aliases, `Vec<UploadedFile>`, nested `Option`,
//! and other wrappers are unsupported, as are destructured client arguments.
//! File arguments cannot be combined with an explicit `json`, `url`, or
//! `msgpack` codec. Use the database field descriptors for storage-backed
//! lifecycle coordination; the lower-level storage `store` API remains a
//! separate operation.
//!
//! ## Query client v2
//!
//! Configure application defaults on the launcher and observer behavior when
//! mounting a query:
//!
//! ```ignore
//! use std::time::Duration;
//! use reinhardt_pages::prelude::*;
//! use reinhardt_pages::server_fn::{ServerFnError, ServerFnErrorKind};
//! use reinhardt_pages::ClientLauncher;
//!
//! ClientLauncher::new("#root")
//!     .query_defaults(
//!         QueryDefaults::new()
//!             .stale_time(Duration::from_secs(30))
//!             .gc_time(Duration::from_secs(300)),
//!     );
//!
//! let jobs = use_query(
//!     list_project_jobs::query(project_id),
//!     QueryOptions::new().refetch_interval(Duration::from_secs(5)),
//! );
//!
//! let retrying_jobs = use_query(
//!     list_project_jobs::query(project_id),
//!     QueryOptions::new().retry(
//!         RetryPolicy::exponential()
//!             .max_attempts(3)
//!             .base_delay(Duration::from_millis(250))
//!             .max_delay(Duration::from_secs(5))
//!             .jitter(true)
//!             .when(|error: &ServerFnError| {
//!                 matches!(
//!                     error.kind(),
//!                     ServerFnErrorKind::Server | ServerFnErrorKind::Transport
//!                 )
//!             }),
//!     ),
//! );
//! ```
//!
//! The generated server-function module exposes `family()`, `key(args...)`,
//! and `query(args...)`. Non-server-function reads can use
//! [`QueryFamily::new`] and [`QueryFamily::query`] directly. Call
//! [`QueryClient::invalidate`] for one exact key or
//! [`QueryClient::invalidate_family`] after a successful [`use_action`]
//! mutation. Disabled uncached observers report [`QueryStatus::Idle`];
//! use [`QueryClient::remove`] or [`QueryClient::remove_family`] at an
//! authentication boundary when cached data must not cross principals.
//! Eviction clears the cached result, retry state, and active request so the
//! next observer starts from [`QueryStatus::Pending`] (or `Idle` when disabled).
//! enabled observers progress through [`QueryStatus::Pending`],
//! [`QueryStatus::Success`], or [`QueryStatus::Error`]. Successful data remains
//! visible if a background sequence ultimately fails, with the terminal error
//! available through the `QuerySnapshot::refetch_error` field. Three attempts
//! include the initial request. Intermediate errors are not published, equal
//! jitter never exceeds the nominal delay, and `QuerySnapshot::is_fetching` is
//! `false` while a retry waits in backoff.
//!
//! Observer polling suspends while the browser document is hidden and resumes
//! according to freshness. Retry attempts are shared by the cache entry across
//! observers. Hidden time does not consume retry backoff: stale data retries
//! immediately when visibility returns, while fresh data waits only the saved
//! remainder.
//!
//! SSR query state is request-local and is serialized for hydration before the
//! browser's first observer mounts. SSR retries require both an observer policy
//! and an explicit renderer gate, and the resource timeout covers fetches,
//! backoff, and jitter:
//!
//! ```ignore
//! let options = SsrOptions::new()
//!     .query_retries(true)
//!     .resource_timeout(Duration::from_secs(2));
//! ```
//!
//! Query client v2 removes `QueryKey::new`, query-handle policy builders,
//! `use_mutation`, and `Action::invalidates`. Normalized entities and retry
//! policy (#5844) are independent descriptor/request-level extensions.
//!
//! ## Normalized entity cache
//!
//! Opt into normalization by implementing [`Entity`] and calling
//! [`QueryDescriptor::with_entities`]. `Entity::TYPE` is a non-empty,
//! application-wide stable namespace, and `Entity::Id` is encoded as canonical
//! JSON. The standard [`EntityValue`] (required), [`OptionalEntity`] (optional),
//! and [`EntityVec`] (ordered vector) adapters cover the common result shapes:
//!
//! ```rust,no_run
//! use reinhardt_pages::{Entity, EntityValue, QueryFamily};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Clone, Debug, Deserialize, Serialize)]
//! struct Project {
//!     id: u64,
//!     name: String,
//! }
//! impl Entity for Project {
//!     type Id = u64;
//!     const TYPE: &'static str = "example.project";
//!     fn entity_id(&self) -> Self::Id { self.id }
//! }
//! #[derive(Clone, Debug, Deserialize, Serialize)]
//! struct LoadError;
//!
//! let family = QueryFamily::<u64, Project, LoadError>::new("projects.detail.v1");
//! let descriptor = family
//!     .query(7, || async {
//!         Ok::<_, LoadError>(Project {
//!             id: 7,
//!             name: String::from("Pages"),
//!         })
//!     })
//!     .with_entities(EntityValue::<Project>::new());
//! let _ = descriptor;
//! ```
//!
//! Use [`OptionalEntity`] when a missing entity should become `None`, and
//! [`EntityVec`] when removed IDs should disappear while the remaining order is
//! preserved. A custom [`EntityProjection`] is a zero-sized adapter with a
//! versioned, non-empty `SCHEMA`; its `dependencies` method must declare every
//! identity that `EntityReader` may access. The module-level
//! [`reactive::entity`] documentation contains a complete multi-entity custom
//! projection example.
//!
//! After a successful mutation, update the arena through the client. Upserts
//! are complete replacements: normalization never infers collection
//! membership, relationships, cascades, patches, or optimistic rollback.
//! `remove_entity` creates a tombstone. Required projections become internally
//! `MissingRequired`, mark their query stale (`normalization_missing`), and
//! always retain the last successful `T` with `QueryStatus::Success`, including
//! inactive and disabled handles. Only an active enabled `QueryHandle` observer
//! schedules at most one recovery refetch; inactive and disabled handles wait
//! for an enabled mount or an explicit refetch. Optional projections become
//! `None`, vectors drop the removed ID, and direct entity handles read `None`.
//! `update_entities` stages all writes and publishes dependent query snapshots
//! and handles atomically in one reactive batch:
//!
//! ```rust,no_run
//! use reinhardt_pages::{Entity, QueryClient, QueryDefaults};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Clone, Debug, Deserialize, Serialize)]
//! struct Project { id: u64, name: String }
//! impl Entity for Project {
//!     type Id = u64;
//!     const TYPE: &'static str = "example.project";
//!     fn entity_id(&self) -> Self::Id { self.id }
//! }
//!
//! let client = QueryClient::new(QueryDefaults::new());
//! client.upsert_entity(Project { id: 7, name: String::from("Pages") });
//! client.update_entities(|entities| {
//!     entities.upsert(Project { id: 7, name: String::from("Updated") });
//! });
//! client.remove_entity::<Project>(&7);
//! assert!(client.entity::<Project>(7).get().is_none());
//! ```
//!
//! `QueryClient::new_ssr` tracks identities read by normalized queries and
//! handles. SSR serializes one deduplicated `(TYPE, canonical ID)` table per
//! request; browser hydration consumes that table into the existing
//! application client before the first observer materializes its recipe, so a
//! normalized query can render without a duplicate fetch. Malformed tables,
//! incompatible types, duplicate identities, and missing required entities are
//! rejected. Plain query snapshots keep their existing serialization format.
//!
//! This extension preserves Query Client V2 migration compatibility. Existing
//! plain descriptors require no changes, `QueryHandle<T, E>` still exposes the
//! original `T` from `snapshot()` and `data()`, and only descriptors with
//! `.with_entities(...)` participate in normalization. Query family IDs,
//! `QueryOptions`, invalidation, polling, and the public `QueryStatus` are
//! unchanged.
//!
//! ## Features
//!
//! - **Fine-grained Reactivity**: Leptos/Solid.js-style Signal system with React-aligned hooks
//! - **Hybrid Rendering**: SSR + Client-side Hydration for optimal performance and SEO
//! - **Django-like API**: Familiar patterns for Reinhardt developers
//! - **Boundaries**: Suspense and error boundaries for async UI states
//!
//! ## React-aligned hook signatures (v0.4, Refs #5511 and #5577)
//!
//! `use_effect`, `use_layout_effect`, and `use_memo` accept either an explicit
//! `deps![...]` dependency list or `deps_auto!()`. Retained effects, callbacks,
//! and resources require an explicit list. Effect closures return `()` when no
//! cleanup is needed, or `Option<C>` when they register cleanup:
//!
//! ```ignore
//! use reinhardt_pages::prelude::*;
//! use reinhardt_pages::reactive::ReactiveScope;
//!
//! ReactiveScope::run(|| {
//!     let count = Signal::new(0_i32);
//!     use_retained_effect(
//!         {
//!             let count = count.clone();
//!             move || {
//!                 println!("count = {}", count.get());
//!             }
//!         },
//!         deps![count],
//!     );
//! });
//! ```
//!
//! In explicit dependency mode (`deps![...]`), effect, layout-effect, and memo
//! closures run with no active reactive Observer ("Option A"); `Signal::get`
//! inside does not auto-subscribe, and subscriptions derive exclusively from
//! the dependency list. Pass `deps![]` for a mount-only effect or memo.
//! Automatic tracking is available only for effects, layout effects, and memos;
//! pass `deps_auto!()` as their second argument to subscribe to signals read by
//! the closure. Retained effects, callbacks, and resources always use explicit
//! dependency lists.
//!
//! This is a breaking migration from the tuple and unit forms. Replace `()`
//! with `deps![]`, and replace `(signal.clone(), ...)` with `deps![signal, ...]`.
//! See `docs/migration/0.4.0-hook-dependency-modes.md` for the complete
//! migration guide and the relationship between #5511 and #5577.
//!
//! For a concept-by-concept mapping from React to Reinhardt Pages, see
//! `docs/react_to_reinhardt.md` in this crate.
//! - **Low-level Only**: Built on wasm-bindgen, web-sys, and js-sys (no high-level framework dependencies)
//! - **Security First**: Built-in CSRF protection, XSS prevention, and session management
//!
//! ## Architecture
//!
//! This framework consists of several key modules:
//!
//! - [`reactive`]: Fine-grained reactivity system (Signal, Effect, Memo)
//! - [`dom`]: DOM abstraction layer
//! - [`builder`]: HTML element builder API
//! - [`component`](mod@component): Component system with IntoPage trait, Head management
//! - [`ui`]: Headless asynchronous UI primitives ([`ui::ActionButton`],
//!   [`ui::ActionResultPanel`], and [`ui::ResourcePanel`])
//! - [`form`](mod@form): Django Form integration
//! - [`form_state`]: Typed `use_form` runtime state
//! - [`mod@client_form`]: Runtime support for DTO-derived client forms
//! - [`csrf`]: CSRF protection
//! - [`auth`]: Authentication integration
//! - [`api`]: API client with Django QuerySet-like interface
//! - [`server_fn`]: Server Functions (RPC)
//! - [`ssr`]: Server-side rendering with Head support
//! - [`hydration`]: Client-side hydration
//! - [`router`]: Client-side routing (reinhardt-urls compatible)
//! - [`portal`]: Explicit portal mounting into existing DOM targets
//! - `i18n`: Reactive page translations with SSR-resolved catalogs (requires the `i18n` feature)
//! - [`static_resolver`]: Static file URL resolution (collectstatic support)
//! - [`mod@style`]: Scoped class composition and typed runtime CSS values
//!
//! ## Structured server-function errors
//!
//! [`ServerFnError`] carries a versioned error envelope with a stable kind,
//! optional HTTP status, safe user message, and validation field errors. Match
//! on [`ServerFnErrorKind`] instead of parsing response JSON:
//!
//! ```no_run
//! use reinhardt_pages::{ServerFnError, ServerFnErrorKind};
//!
//! # fn log(_: &str, _: &str) {}
//! # fn redirect_to_login() {}
//! # fn show_message(_: &str) {}
//! # let error = ServerFnError::validation_with_message(
//! #     "Please correct the submitted values",
//! #     [("email", "Enter a valid email address")],
//! # );
//! match error.kind() {
//!     ServerFnErrorKind::Validation => {
//!         for field_error in error.field_errors() {
//!             log(field_error.field(), field_error.message());
//!         }
//!     }
//!     ServerFnErrorKind::Auth => redirect_to_login(),
//!     _ => show_message(error.user_message()),
//! }
//! ```
//!
//! With the native `model-server-fnset` feature,
//! [`ServerFnError::try_from_model_error_with`] maps only proven model
//! constraint violations. The optional callback may supply fixed,
//! client-safe text:
//!
//! ```rust,ignore
//! let server_error = ServerFnError::try_from_model_error_with::<User, _>(
//!     error,
//!     |database_error, _fields| {
//!         (database_error.constraint() == Some("users_email_unique"))
//!             .then(|| "This email is already registered".to_owned())
//!     },
//! )
//! .unwrap_or_else(|error| {
//!     tracing::error!(error = %error, "user write failed");
//!     ServerFnError::application("Failed to save user")
//! });
//! ```
//!
//! Callback code must not return `DatabaseError::message()`, rejected values,
//! table names, constraint names, or vendor diagnostics to the browser.
//! Single-field violations are field errors, while composite `UNIQUE` and
//! `CHECK` violations are form errors. Unmapped or unproven errors remain the
//! original framework error. The conversion helper is native-only; browser
//! code receives the resulting [`ServerFnError`].
//!
//! Conversion preserves the serialized [`ServerFnError`] wire shape and adds no
//! database metadata to the browser response. Generated client forms route
//! field errors by logical model field names. Composite `UNIQUE` and `CHECK`
//! violations have no single logical field, so they reach the form error.
//!
//! ## Typed server function sets
//!
//! [`server_fn::server_fnset`] groups existing server function markers into a
//! named, typed registration chain. Members retain their codec, CSRF,
//! dependency-injection, extractor, metadata, and mock contracts; applications
//! explicitly attach the completed set with
//! [`server_fn::ServerFnRouterExt::server_fnset`]. Mixed-codec sets are valid.
//!
//! The opt-in `model-server-fnset` feature generates exactly six standard POST
//! RPCs for a [`server_fn::ServerFnResource`]: `list`, `retrieve`, `create`,
//! `update`, `partial_update`, and `destroy`. Native resources select an
//! explicit `ServerFnSetPolicy`, provide model-to-DTO mappings, and
//! return a typed unique lookup. Pagination defaults to 25, accepts `1..=100`,
//! and reports the policy-scoped total before slicing. Checked standard
//! overrides and custom transactional actions share the same policy and
//! transaction runtime. Full and partial updates authorize the resulting object
//! again before read mapping and transaction commit.
//!
//! Wire contracts, structured errors, metadata, generated markers, and client
//! stubs are cross-target. ORM resources, policies, action contexts, database
//! executors, native CRUD handlers, and `ModelServerFnSet` are
//! native-only. Generated model failures map to stable 400/401/403/404/409/500
//! responses, and internal details are sanitized before serialization. Action
//! markers remain independent for component and MSW mocks.
//!
//! Model sets intentionally do not provide subsets, a read-only set type,
//! REST/OpenAPI generation, cursor pagination, bulk or nested actions,
//! composite lookups, global discovery, or automatic model-to-DTO derivation.
//! See `docs/server_fn_macro.md` for a complete resource and action example.
//!
//! ## Typed events
//!
//! Standard intrinsic `page!` events use one catalog-generated payload type per
//! event. Payloads expose common propagation and target snapshots, plus only
//! the capabilities assigned to that event, such as `InputEvent::value` or
//! `ChangeEvent::checked`. `current_target()` is captured while the listener is
//! active, so it remains available after an async handler yields.
//!
//! ```ignore
//! use reinhardt_pages::event::{ClickEvent, InputEvent};
//! use reinhardt_pages::prelude::*;
//!
//! page!({
//!     button { @click: |event: ClickEvent| { event.prevent_default(); }, "Run" }
//!     input { @input: |event: InputEvent| {
//!         if let Ok(value) = event.value() {
//!             info_log!("{value}");
//!         }
//!     } }
//! })
//! ```
//!
//! Arbitrary intrinsic events have adjacent raw and typed forms. Use
//! `@custom("name")` when the handler needs the unmodified
//! [`platform::Event`], or `@custom::<Detail>("name")` when a browser
//! `CustomEvent.detail` payload should deserialize into `Detail`.
//!
//! ```ignore
//! use reinhardt_pages::prelude::*;
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! struct ItemSelected {
//!     id: u64,
//! }
//!
//! page!({
//!     // Raw custom-event transport.
//!     button { @custom("item-selected"): |event: Event| { inspect(event); } }
//!
//!     // Typed custom-event detail, inferred from the DSL.
//!     button { @custom::<ItemSelected>("item-selected"): |event| {
//!         if let Ok(detail) = event.detail() {
//!             select(detail.id);
//!         }
//!     } }
//! })
//! ```
//!
//! Component `@event` props retain the type of their declared component prop
//! instead of using the intrinsic event catalog.
//!
//! ## Controlled form elements
//!
//! The `bind:` directive connects native form controls to typed [`Signal`]
//! values. Text and radio groups use `Signal<String>`, checkboxes use
//! `Signal<bool>`, numeric inputs use a primitive implementing [`NumberValue`],
//! and multiple selects use `Signal<Vec<String>>`. Numeric bindings may expose
//! a [`NumberParseError`] signal that retains recoverable invalid editor text.
//! Owned signal handles, shared references, and mutable references are supported,
//! including both signals passed to `number(value, error)`.
//! Only unmodified Arrow/Home/End keyboard moves are predicted; modifier-key
//! commands and already-canceled key events are treated as unknown. When a
//! pointer-positioned number edit is sanitized before its inaccessible selection
//! can be recovered, the error reports the browser's empty value.
//! Radio `value` expressions are evaluated once per rendered element. A bound
//! single select projects only its first matching option in tree order during
//! SSR, including options resolved inside a pending boundary; a multiple
//! select projects every match.
//!
//! ```rust
//! use reinhardt_pages::prelude::*;
//! use reinhardt_pages::reactive::ReactiveScope;
//!
//! ReactiveScope::run(|| {
//!     let query = Signal::new(String::new());
//!     let enabled = Signal::new(false);
//!     let mode = Signal::new("draft".to_owned());
//!     let amount = Signal::new(0_f64);
//!     let amount_error = Signal::new(None::<NumberParseError>);
//!     let targets = Signal::new(Vec::<String>::new());
//!
//!     let _form = page!({
//!         input { aria_label: "Search", bind: query }
//!         input { aria_label: "Enabled", type: "checkbox", bind: enabled }
//!         input {
//!             aria_label: "Draft",
//!             type: "radio",
//!             value: "draft",
//!             bind: mode,
//!         }
//!         input {
//!             aria_label: "Amount",
//!             type: "number",
//!             bind: number(amount, amount_error),
//!         }
//!         select {
//!             aria_label: "Targets",
//!             multiple: true,
//!             bind: targets,
//!             option { value: "native", "Native" }
//!             option { value: "wasm", "WebAssembly" }
//!         }
//!     });
//! });
//! ```
//!
//! ## Forms
//!
//! `form!` owns static form definition: field names, widgets, labels,
//! rendering metadata, and server function binding. [`use_form`] owns typed
//! runtime behavior: value signals, dirty/touched state, validation errors,
//! loading/success state, reset, and submit orchestration.
//!
//! Create a `form!` generated form and attach runtime behavior with
//! [`use_form`]:
//!
//! ```ignore
//! use reinhardt_pages::{form, use_form};
//!
//! let login_form = form! {
//!     name: LoginForm,
//!     action: "/login",
//!     fields: {
//!         username: CharField { initial: String::new() }
//!         password: CharField { initial: String::new() }
//!     }
//! };
//!
//! let runtime = use_form(&login_form).build();
//! runtime.set_value(login_form.username_field(), "ada".to_string());
//! ```
//!
//! ### Typed field bindings and reset ownership
//!
//! A generated form's runtime is the owner of values and mounted control
//! state. Call [`UseFormReturn::field`] with the generated token and pass the
//! opaque handle to `bind:`. The token spelling depends on the form source:
//!
//! - `ClientForm`: `runtime.field(LoginClientFormField::Email)` when the
//!   generated field enum is in scope;
//! - an ordinary `form!`: `runtime.field(login_form.email_field())`;
//! - a ModelForm using `fields: [email]`:
//!   `runtime.field(model_form.email_field())`.
//!
//! ModelForm field accessors are generated only for the explicit `fields: [...]`
//! selection. The accessor keeps the internal model token private while still
//! providing the same typed binding contract.
//!
//! ```rust,no_run
//! use reinhardt_pages::{form, page, use_form};
//! use reinhardt_pages::reactive::ReactiveScope;
//!
//! ReactiveScope::run(|| {
//!     let form = form! {
//!         name: LoginForm,
//!         action: "/login",
//!         fields: {
//!             email: EmailField { required },
//!         },
//!     };
//!     let runtime = use_form(&form).build();
//!     let _page = page!({
//!         input {
//!             aria_label: "Email",
//!             bind: runtime.field(form.email_field()),
//!         }
//!     });
//!     runtime.reset();
//! });
//! ```
//!
//! The supported value/control matrix is:
//!
//! | Generated value | Supported controls |
//! | --- | --- |
//! | `String` | text or textarea, radio, select-one |
//! | `T` implementing [`NumberValue`] | number (`input[type=number]`) |
//! | `bool` | checkbox |
//! | `Vec<String>` | select-many |
//!
//! `RangeInput` and `input[type=range]` are not currently compatible with a
//! typed runtime field binding; use an unbound or application-specific path
//! for range controls.
//!
//! A token from another form fails at the Rust type boundary. A token from the
//! right form paired with an incompatible control is valid Rust but panics at
//! page construction with a message naming the field and requested control.
//!
//! Reset is explicit: [`UseFormReturn::reset`] applies the current defaults
//! source-first, clears field/collection/path/form/submit errors and
//! touched/dirty/submitting/success state, and returns connected
//! [`FormAction`] handles to idle. It does not run automatically after a
//! successful submission and it is not wired to a native reset button or reset
//! event. [`UseFormReturn::sync_after_native_reset`] remains available when an
//! application deliberately handles the browser's native reset behavior.
//! A pending `use_form_action` request is not cancelled; its stale completion
//! cannot repopulate form-owned submit state. Standalone [`use_action`] handles
//! are not connected to this reset boundary.
//!
//! Fresh mount writes runtime values to the DOM. Hydration is normally
//! DOM-first, preserving an edit made after SSR and before hydration. If the
//! application calls `runtime.reset()` before hydration, runtime field
//! bindings become source-preferred and write the reset defaults instead of
//! adopting stale SSR properties. Existing direct `Signal` bindings retain
//! their normal DOM-first hydration behavior.
//!
//! Numeric bindings preserve invalid editor text and the last valid typed
//! value. The current [`NumberParseError`] retains the raw text and failure
//! kind; generated form runtimes expose it through field error state. A later
//! valid numeric write clears that tracked parse error, and `reset()` restores
//! the formatted default and clears it.
//!
//! Typed runtime bindings intentionally exclude file inputs, ModelForm
//! `exclude: [...]` declarations, and nested collection paths. Those forms
//! keep their existing generated/file/collection APIs. The binding categories
//! above reuse the existing `page!` classification and do not add a second DOM
//! registry or application-facing adapter type.
//!
//! DTO request types can opt in to generated client-form companions with
//! [`ClientForm`]. The generated form keeps enum choices and typed request
//! assembly tied to the request type while using the same [`use_form`] runtime.
//! Use the `#[client_form(...)]` attribute for the concise form; it uses the
//! same expansion logic as the derive and preserves container and field serde
//! metadata even without serde derives. Alternatively, keep
//! `#[derive(ClientForm)]` with its helper attribute for compatibility.
//! The `client_form` attribute macro must be imported explicitly; it is not re-exported by
//! `prelude::*` so legacy derive/helper declarations remain helper-only.
//! `#[client_form(...)]` before `#[serde(...)]` when no serde derive is present
//! so the attribute macro can consume those helper attributes.
//! Add `validate` when the DTO implements `Validate` and should feed those errors
//! into the generated form runtime:
//!
//! ```ignore
//! use reinhardt_pages::{ClientFormChoices, client_form, use_form};
//!
//! #[derive(Clone, Default, PartialEq, ClientFormChoices)]
//! #[serde(rename_all = "snake_case")]
//! enum ProviderMode {
//!     #[default]
//!     Fake,
//!     LiveApi,
//! }
//!
//! #[reinhardt::dto]
//! #[derive(Clone, serde::Serialize, serde::Deserialize)]
//! #[client_form(server_fn = crate::server::submit_project, validate)]
//! struct ProjectRequest {
//!     name: String,
//!     title: Option<String>,
//!     provider_mode: ProviderMode,
//! }
//!
//! let form = ProjectRequestClientForm::new();
//! let runtime = use_form(&form).build();
//! runtime.set_value(ProjectRequestClientFormField::Title, "  ".to_string());
//! let request = ProjectRequestClientForm::to_request(&runtime);
//! assert_eq!(request.title, None);
//! let outcome = form.submit(&runtime).await?;
//! ```
//!
//! Generated `submit` methods have the same signature on native and WASM
//! targets, so shared components can construct one action without
//! target-specific branches. Submission executes only on WASM; native SSR code
//! must not await or dispatch the generated method.
//!
//! [`ClientFormChoices`] mirrors serde's externally tagged string names for
//! unit variants, including matching `rename_all` and variant `rename`; tagged,
//! untagged, or directionally renamed enum representations are rejected because
//! form choices submit bare strings. DTO fields marked with serde skip
//! attributes are kept out of editable form fields and preserved through
//! generated request values. Exported DTOs cannot use private editable fields;
//! mark the field public or make it an explicit hidden field with
//! `#[client_form(skip)]` or a serde skip attribute. Forms with generated
//! `server_fn` submit helpers reject serde-skipped request fields because the
//! browser payload must match native request deserialization exactly.
//!
//! Compose validated submit flows with [`use_form_action`]:
//!
//! ```ignore
//! use reinhardt_pages::{form, use_form, use_form_action};
//!
//! let runtime = use_form(&login_form).build();
//! let save = use_form_action(&runtime, |values: LoginFormValues| async move {
//!     submit_login(values).await
//! });
//!
//! if !save.is_pending() {
//!     save.submit();
//! }
//! ```
//!
//! Use [`ui::FormActionButton`] with [`FormAction::submit_handler`] to preserve
//! native form submit semantics. [`ui::FormActionResultPanel`] renders
//! validation and typed mutation errors separately, while
//! [`Resource::latest_after_form`] composes successful validated mutations
//! without exposing the underlying dispatch handle.
//!
//! `FileField` and `ImageField` participate in this runtime contract as
//! `Option<web_sys::File>` values. File values are browser-owned and are
//! tracked for dirty/touched state without treating the file payload as a
//! serializable scalar.
//!
//! Stable native widget coverage includes the following `form!` DSL items:
//!
//! | DSL item | HTML output | Value state |
//! |---|---|---|
//! | `MonthInput` | `<input type="month">` | string field |
//! | `WeekInput` | `<input type="week">` | string field |
//! | `ResetButton` | `<button type="reset">` | none |
//! | `Button` | `<button type="button">` | none |
//! | `ImageInput` | `<input type="image">` | none |
//! | `Datalist` | `<datalist>` | option source only |
//! | `OptGroup` | `<optgroup>` | choice grouping only |
//! | `Output` | `<output>` | none |
//! | `Meter` | `<meter>` | none |
//! | `Progress` | `<progress>` | none |
//!
//! Typed native attributes are accepted for the controls that support them:
//!
//! | Attribute | Compatible controls |
//! |---|---|
//! | `min` / `max` / `step` | number, range, date, time, datetime-local, month, week |
//! | `size` | text-like inputs |
//! | `accept` / `capture` | file-like inputs |
//! | `multiple` | file-like inputs and multi-select |
//! | `list` | datalist-compatible text-like inputs |
//!
//! `FieldGroup` renders as semantic `<fieldset>` output. When `label` is
//! present, the label is rendered as a `<legend>` inside the fieldset.
//!
//! `CustomWidget` is experimental and must opt in explicitly:
//!
//! ```rust,ignore
//! date_range: CharField {
//!     widget: CustomWidget(crate::widgets::DateRangePicker) {
//!         experimental,
//!         adapter: crate::widgets::DateRangeAdapter,
//!     },
//! }
//! ```
//!
//! The adapter API may change in a minor release with a documented migration
//! path.
//!
//! Use `ambient_arguments` for non-field values supplied from surrounding
//! context. The old `strip_arguments` DSL name remains as a deprecated alias.
//! CSRF should be supplied by `#[server_fn]` client stubs through the
//! `X-CSRFToken` header rather than as a server function business argument.
//!
//! ## Macros
//!
//! - [`page!`]: JSX-like macro for defining view components
//! - [`head!`]: JSX-like macro for defining HTML head sections
//! - [`form!`]: Type-safe form component macro
//! - [`style!`]: Typed component-scoped style definition language
//! - [`style_def`]: Canonical static-item bridge for `style!`
//! - `t!`: Reactive page translation macro (requires the `i18n` feature)
//! - [`client_page`]: Client page function macro with native route-table stubs
//! - `#[component]`: Route-backed page component macro
//! - `#[layout]`: Route-backed layout component macro for nested SPA shells
//! - [`wasm_server_api`]: WASM/server API parity macro
//!
//! See `docs/wasm_server_api.md` for the target-specific API parity contract.
//!
//! ## Example
//!
//! ### Basic Component
//!
//! ```no_run
//! use reinhardt_pages::{Page, Signal, page};
//! use reinhardt_pages::reactive::ReactiveScope;
//!
//! fn counter(scope: &ReactiveScope) -> Page {
//!     scope.enter(|| {
//!         let count = Signal::new(0);
//!
//!         page!(|count: Signal<i32>| {
//!             div {
//!                 p { { format!("Count: {}", count.get()) } }
//!             }
//!         })(count)
//!     })
//! }
//! ```
//!
//! ### With Head Section
//!
//! ```ignore
//! use reinhardt_pages::{head, page, Page, resolve_static};
//!
//! fn home_page() -> Page {
//!     let page_head = head!(|| {
//!         title { "Home - My App" }
//!         meta { name: "description", content: "Welcome to my app" }
//!         link { rel: "stylesheet", href: resolve_static("css/main.css") }
//!     });
//!
//!     page! {
//!         #head: page_head,
//!         || {
//!             div { class: "container",
//!                 h1 { "Welcome Home" }
//!             }
//!         }
//!     }()
//! }
//! ```
//!
//! ### Lifecycle-managed document head
//!
//! `Head` declarations are composed in structural page order. A `page!`
//! `#head:` contribution is retained while its page is mounted, and its
//! parent contribution becomes visible again when the child is removed.
//! Route metadata contributes through the same model:
//!
//! ```ignore
//! use reinhardt_pages::{head, use_page_title, Head};
//! use reinhardt_pages::deps;
//! use reinhardt_urls::routers::RouteMetadata;
//!
//! let route_metadata = RouteMetadata::new().with_head(head!(|| {
//!     base { href: "/app/" }
//!     title { "Workspace" }
//! }));
//!
//! let project = Signal::new("Outline".to_owned());
//! use_page_title(
//!     {
//!         let project = project.clone();
//!         move || format!("{} · Cocrea", project.get())
//!     },
//!     deps![project.clone()],
//! );
//! ```
//!
//! Server rendering, hydration, and browser mounting resolve the same active
//! declarations. Hydration adopts framework-marked nodes, and browser
//! reconciliation touches only those marked nodes; unmanaged head elements
//! remain untouched. An unchanged script node is reused, but removing a
//! script cannot undo side effects that already ran in the browser.
//!
//! ### WebSocket Integration
//!
//! The `use_websocket` hook provides reactive WebSocket connections:
//!
//! ```ignore
//! use reinhardt_pages::reactive::hooks::{use_effect, use_websocket, UseWebSocketOptions};
//! use reinhardt_pages::reactive::hooks::{ConnectionState, WebSocketMessage};
//!
//! fn chat_component() -> Page {
//!     // Establish WebSocket connection
//!     let ws = use_websocket("ws://localhost:8000/ws/chat", UseWebSocketOptions::default());
//!
//!     // Monitor connection state reactively
//!     let connection_state = ws.connection_state().clone();
//!     use_effect(
//!         {
//!             let connection_state = connection_state.clone();
//!             move || {
//!                 match connection_state.get() {
//!                     ConnectionState::Open => log!("Connected to chat"),
//!                     ConnectionState::Closed => log!("Disconnected from chat"),
//!                     ConnectionState::Error(e) => log!("Connection error: {}", e),
//!                     _ => {}
//!                 }
//!             }
//!         },
//!         (connection_state,),
//!     );
//!
//!     // Handle incoming messages
//!     let latest_message = ws.latest_message().clone();
//!     use_effect(
//!         {
//!             let latest_message = latest_message.clone();
//!             move || {
//!                 if let Some(WebSocketMessage::Text(text)) = latest_message.get() {
//!                     log!("Received: {}", text);
//!                 }
//!             }
//!         },
//!         (latest_message,),
//!     );
//!
//!     page!(|| {
//!         div {
//!             button {
//!                 @click: move |_| {
//!                     ws.send_text("Hello, server!".to_string()).ok();
//!                 },
//!                 "Send Message"
//!             }
//!         }
//!     })()
//! }
//! ```
//!
//! **Note**: WebSocket functionality is WASM-only. On the server side (SSR),
//! `use_websocket` returns a no-op handle with connection state always set to `Closed`.

#![warn(missing_docs)]

extern crate self as reinhardt_pages;

// Re-export AST definitions from reinhardt-pages-ast
// This is deprecated but kept for backward compatibility
#[allow(deprecated)] // Intentional: maintaining backward compatibility with existing code
pub use reinhardt_pages_ast as ast;

// Core modules
pub mod builder;
pub mod callback;
// The cancellation substrate is introduced before its query/navigation
// consumers; this temporary allow keeps the foundational task warning-free.
#[allow(dead_code)]
mod cancellation;
pub use cancellation::{CancellationHandle, CancellationToken, Cancelled};
pub mod control_binding;
#[allow(dead_code)] // SSR and browser adapters consume this staged crate-private contract.
pub(crate) mod document_head;
pub mod dom;
pub mod event;
#[cfg(feature = "i18n")]
pub mod i18n;
pub mod logging;
pub mod reactive;
/// Typed runtime values generated by component style definitions.
pub mod style;

// Platform abstraction (unified types and task spawning for WASM and native)
pub mod platform;
pub mod portal;

/// Backward-compatibility re-export of task-spawning utilities.
///
/// Task spawning moved into `platform` (#4362). This deprecated module
/// keeps existing `reinhardt_pages::spawn::*` imports working for the
/// remainder of the 0.x line.
pub mod spawn {
	/// Deprecated: use `spawn_task` from `reinhardt_pages::prelude` instead.
	#[deprecated(
		note = "moved to reinhardt_pages::platform; use spawn_task from reinhardt_pages::prelude instead"
	)]
	pub use crate::platform::spawn_task;

	/// Deprecated: use `defer_yield` from `reinhardt_pages::prelude` instead.
	#[deprecated(
		note = "moved to reinhardt_pages::platform; use defer_yield from reinhardt_pages::prelude instead"
	)]
	pub use crate::platform::defer_yield;
}

// Unified prelude for simplified imports
pub mod prelude;

// Component system
pub mod component;

/// Headless UI primitives for common asynchronous screen states.
pub mod ui;

// Form and security
pub mod auth;
pub mod csrf;
#[doc(hidden)]
mod fetch;
// Static form metadata types for form! macro (WASM-compatible)
pub mod form_generated;
// Typed form runtime state (WASM-compatible)
pub mod form_state;
// Runtime support for DTO-derived client forms.
pub mod client_form;
// Model-backed form state is target-neutral. Legacy FormComponent support is
// gated inside the module because it still depends on reinhardt-forms.
pub mod form;

// API and communication
pub mod api;
pub mod server_fn;

// Server-side rendering
pub mod ssr;

// Client-side hydration
pub mod hydration;

// Client-side routing
pub mod router;

// WASM application launcher
pub mod app;

// Integration modules (runtime support for special macros)
pub mod integ;

// Testing utilities (available on both WASM and server)
// Layer 1: server_fn unit tests (server-side only)
// Layer 2: Component + server_fn mock tests (WASM)
// Layer 3: E2E tests (both platforms)
#[cfg(any(test, feature = "testing"))]
pub mod testing;

// Static file URL resolver
pub mod static_resolver;

// Hot Module Replacement (feature-gated with target-neutral protocol types)
#[cfg(feature = "hmr")]
pub mod hmr;

// Table utilities (django-tables2 equivalent)
pub mod tables;

// Re-export commonly used types
pub use api::{ApiModel, ApiQuerySet, Filter, FilterOp};
pub use auth::{AuthData, AuthError, AuthState, auth_state};
pub use builder::{
	attributes::{AriaAttributes, BooleanAttributes},
	html::{
		a, button, div, form, h1, h2, h3, img, input, li, ol, option, p, select, span, textarea, ul,
	},
};
pub use callback::{
	Callback, IntoEventHandler, IntoTypedEventHandler, event_handler, into_event_handler,
	raw_async_event_handler, raw_event_handler, typed_async_custom_event_handler,
	typed_async_event_handler, typed_custom_event_handler, typed_event_handler,
};
pub use client_form::{ClientFormChoice, ClientFormChoiceSource};
#[cfg(native)]
pub use component::NativeEvent;
#[cfg(wasm)]
pub use component::cleanup_reactive_nodes;
pub use component::{
	ActivityBoundary, ActivityMode, BoundaryError, Component, ErrorBoundary, ErrorTracker, Head,
	IntoPage, LinkTag, MetaTag, Outlet, Page, PageElement, PageExt, Props, ResourceTracker,
	ScriptTag, StyleTag, SuspenseBoundary, ViewTransitionBoundary, ViewTransitionHandle,
	ViewTransitionStatus, start_view_transition,
};
pub use control_binding::{
	ControlBindingError, NumberParseError, NumberParseErrorKind, NumberValue,
};
pub use csrf::{CsrfManager, get_csrf_token};
pub use dom::{CustomEventOptions, Document, Element, EventHandle, EventType, document};
#[cfg(native)]
pub use form::{FormBinding, FormComponent};
pub use reinhardt_core::{deps, deps_auto};
// Static form metadata types (always available, used by form! macro)
pub use form_generated::{StaticFieldMetadata, StaticFormMetadata};
pub use form_state::{
	CollectionItem, CollectionItemKey, CollectionState, CustomWidgetContext, CustomWidgetRawValue,
	FieldError, FieldPathState, FieldState, FocusError, FormAction, FormCollectionRuntimeSource,
	FormEvent, FormRuntimeSource, FormState, FormSubscription, FormValidationError,
	FormWidgetAdapter, FormWidgetError, FormWidgetValueKind, NoDeps, ResetOnDeps, RevalidateOn,
	RuntimeControlBindingRequest, RuntimeFieldBinding, UseFormAsyncSubmitOutcome, UseFormBuilder,
	UseFormReturn, UseFormSubmitOutcome, use_form, use_form_action,
};
pub use hydration::{HydrationContext, HydrationError, hydrate};
pub use portal::{Portal, PortalError, PortalHandle, PortalTarget, mount_portal};
pub use reactive::{
	Effect, Entity, EntityArena, EntityDependencies, EntityHandle, EntityProjection, EntityReader,
	EntityValue, EntityVec, EntityWriter, ExplicitDeps, LatestResourceState, LatestResourceValue,
	LatestResourceValueBuilder, Memo, NoRetry, OptionalEntity, ProjectionMaterialization,
	ProjectionRemoval, QueryClient, QueryDefaults, QueryDescriptor, QueryFamily, QueryHandle,
	QueryKey, QueryOptions, QuerySnapshot, QueryStatus, ReactiveDeps, RemovedEntities, Resource,
	ResourceState, RetryPolicy, Signal, Trackable, queries, use_latest_resource_value,
	use_resource, use_resource_with_key,
};
// Re-export Context system
pub use reactive::{
	Context, ContextGuard, create_context, get_context, provide_context, remove_context,
};
// Re-export Hooks API
pub use app::{ClientLauncher, LaunchCtx, PathCtx, PathParams};
pub use reactive::use_query;
pub use reactive::{Action, ActionPhase, ActionStateBuilder, use_action, use_action_state};
pub use reactive::{
	Dispatch, EffectReturn, OptimisticState, Ref, SetState, SetStateExt, SharedSetState,
	SharedSignal, TransitionState, use_callback, use_context, use_debug_value, use_deferred_value,
	use_effect, use_head, use_id, use_layout_effect, use_memo, use_optimistic, use_page_title,
	use_reducer, use_ref, use_retained_effect, use_retained_layout_effect, use_shared_state,
	use_state, use_sync_external_store, use_transition,
};
#[cfg(native)]
pub use reinhardt_forms::{
	Widget,
	wasm_compat::{FieldMetadata, FormMetadata},
};
pub use router::Link;
// Imperative SPA navigation API (Issue #4610). `navigate` is the free
// function; `use_router` returns a `RouterHandle` for use inside hooks /
// components. `NavigateError` is the public error returned by both paths.
pub use reactive::hooks::router::{NavigateError, RouterHandle, use_router};
pub use router::loader::{
	Loader, LoaderInputError, LoaderInputKind, LoaderInputSpec, LoaderStore, LoaderStoreError,
	LoaderStoreScope, RouteLoader, RouteLoaderError, active_loader_store, canonical_loader_inputs,
	enter_loader_store, loader_cache_id, with_loader_store,
};
pub use router::{NavigationType, navigate, navigate_named, navigate_or_reload};
pub use router::{Path, Query, RouteLoaderId};
pub use server_fn::{
	ServerFn, ServerFnError, ServerFnErrorKind, ServerFnErrorPayload, ServerFnFieldError,
};
pub use ssr::SsrState;
#[cfg(native)]
pub use ssr::{SsrChunk, SsrOptions, SsrRenderer, SsrRouteOutput, SsrStream};
pub use static_resolver::{
	component_stylesheet_url, init_static_resolver, is_initialized, resolve_static,
};
pub use style::{
	ClassList, ClassToken, CssAngle, CssColor, CssInteger, CssLength, CssLengthPercentage,
	CssNumber, CssPercentage, CssTime, CssValueError, StyleValue, StyleVars,
};

#[cfg(feature = "i18n")]
pub use i18n::{
	I18nContext, I18nError, I18nStateError, LazyString, MessageCatalog, TranslatedText,
	TranslationContext, TranslationGuard, locale, provide_i18n_context, set_locale, tn, tnp, tp,
	tr, use_i18n_context, with_i18n_context,
};

// Re-export procedural macros
pub use reinhardt_pages_macros::form;
pub use reinhardt_pages_macros::head;
pub use reinhardt_pages_macros::layout;
pub use reinhardt_pages_macros::loader;
pub use reinhardt_pages_macros::page;
pub use reinhardt_pages_macros::style;
pub use reinhardt_pages_macros::style_def;
pub use reinhardt_pages_macros::wasm_server_api;
pub use reinhardt_pages_macros::{
	ClientForm, ClientFormChoices, FromRequest, client_form, client_page, component, page_props,
};

// Private re-exports used by macro-generated code. Not part of the public API.
#[doc(hidden)]
pub mod __private {
	pub mod client_form {
		pub use crate::client_form::__private::*;
	}

	pub fn capture<T: Clone>(value: &T) -> T {
		value.clone()
	}

	pub mod fetch {
		pub use crate::fetch::{
			FetchCredentials, FetchResponse, request, request_with_credentials,
		};
	}
	pub use bon;
	pub use bytes;
	#[cfg(native)]
	pub use hyper;
	pub use inventory;
	pub use reinhardt_urls;
	pub use serde;
	pub use serde_json;
	#[cfg(wasm)]
	pub use wasm_bindgen;
	#[cfg(wasm)]
	pub use web_sys;

	// `tracing` is enabled for all targets *except* browser wasm (wasm32-unknown-unknown).
	// Browser wasm uses a different logging mechanism, so tracing is intentionally excluded there.
	// The `native` cfg alias (defined in build.rs as
	// `not(all(target_family = "wasm", target_os = "unknown"))`) precisely targets every
	// non-browser-wasm platform.
	#[cfg(native)]
	pub use tracing;
}

// Logging macros are automatically exported via #[macro_export]
// Users can access them as: reinhardt_pages::debug_log!, reinhardt_pages::info_log!, etc.

#[cfg(all(test, feature = "hmr"))]
mod hmr_feature_tests {
	use super::{Page, page};
	use crate::hmr::protocol::{SourceId, TemplateKey};

	#[test]
	fn hmr_feature_enables_page_metadata_and_page_macro() {
		// Arrange
		let key = TemplateKey {
			source_id: SourceId("src/app.rs".to_owned()),
			line: 12,
			column: 4,
			nested_template_index: 0,
		};

		// Act
		let view = page!({ "body" })
			.with_dev_slot(3)
			.with_dev_template_metadata(key.clone());
		let (metadata, slot) = view.into_dev_template_parts().expect("metadata");

		// Assert
		assert_eq!(metadata.downcast_ref::<TemplateKey>(), Some(&key));
		assert_eq!(slot.dev_slot_id(), Some(3));
		assert!(matches!(slot, Page::DevSlot { .. }));
	}
}
