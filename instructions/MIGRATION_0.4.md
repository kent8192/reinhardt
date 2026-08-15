# Migration Guide: 0.3.x to 0.4.0

This guide covers the Rust management-shell migration, breaking Reinhardt Pages
event API, closure-scoped ORM transaction API, and typed manager upsert API
introduced for 0.4.

For the complete `get_or_create` and `update_or_create` migration, including
transaction, uniqueness, race, and custom-manager hook semantics, see
[`0.4.0-typed-manager-upserts.md`](../docs/migration/0.4.0-typed-manager-upserts.md).

## Storage-backed `FileField` source migration

The 0.4 storage foundation separates the old synchronous field descriptors
from the typed value used by model instances. Update imports as follows:

| 0.3 source | 0.4 source | Purpose |
| --- | --- | --- |
| `orm::file_fields::FileField` | `orm::legacy_file_fields::LegacyFileField` | Deprecated synchronous descriptor |
| `orm::file_fields::ImageField` | `orm::legacy_file_fields::LegacyImageField` | Deprecated synchronous image descriptor |
| `orm::file_fields::FileFieldError` | `orm::legacy_file_fields::LegacyFileFieldError` | Legacy descriptor errors |
| — | `orm::FileField` (with `file-storage`) | Typed storage-backed model value |

The legacy descriptors are also available through explicit `Legacy*` top-level
ORM re-exports and remain deprecated for compatibility. The unprefixed
`FileField` is now the typed value with async `open`, `size`, and URL methods;
it is not a synchronous descriptor. The unprefixed `ImageField` name remains
reserved for the Phase B image API. Existing image descriptor code should use
`LegacyImageField` until that API is delivered.

Enable `file-storage` plus exactly one provider facade feature in a new model
module. A field declaration supplies `upload_to` and optionally
`file_storage = "private_uploads"`; the generated `file_avatar()` descriptor
stores an upload before the model is saved:

```rust,ignore
let avatar = Profile::file_avatar().store(upload).await?;
let mut profile = Profile::build().avatar(avatar).finish();
profile.save().await?;

let bytes = profile.avatar.open().await?;
let size = profile.avatar.size().await?;
let url = profile.avatar.url().await?;
```

The database column contains only the validated logical path. Hydration uses
the model field's `file_storage` metadata to restore the storage alias; the
alias is not stored per row and a provider prefix is never serialized into the
column.

### Existing rows and alias changes

Changing `file_storage` changes where future operations resolve an existing
logical path; it does not move the object. For rows already in production,
perform an object/data migration that copies each object to the destination
alias and verifies the copy before removing the source. Alternatively, keep a
stable old alias and redirect that alias to the new backend. A migration that
only edits the model attribute or TOML can leave every existing row pointing
at a missing object.

### Storage lifecycle and admin integration

The low-level `store` operation remains eager. Model mutation coordinators
stage uploads before persistence, compensate staged objects when the database
operation fails, and attempt replacement, clear, or delete cleanup after a
successful database commit. Cleanup failures are logged without changing the
database result, and `cleanup = false` preserves old objects. When the admin
`file-uploads` feature is enabled, multipart form binding uses these same
policies and validates ImageField uploads before persistence.

## Validated session authentication

`SessionMiddleware` now manages session storage and DI registration only. A
`USER_ID_SESSION_KEY` value is an identity reference, not proof that the
account remains active or authorized. Projects using cookie-backed sessions
must follow it with authentication middleware that loads the current account
record and publishes `AuthState` before using `CurrentUser<U>` or
authorization guards. This prevents deactivated accounts with unexpired
sessions from being authorized.

`CookieSessionAuthMiddleware` remains available for custom
`AsyncSessionBackend` integrations, but it only restores flags stored in the
session and does not validate the current account. Follow it with
account-resolving middleware, or use an authentication backend that performs
that validation.

## Rust management shell

The former Rhai evaluator was replaced by a stateful Rust evaluator backed by
`evcxr`. Remove `shell-rhai`; it has no compatibility alias, and old Rhai
snippets are not supported Rust syntax. Projects that do not use
`manage shell` can keep the settings-only command entry point for every
non-shell command.

To opt in, declare a local feature that forwards to the facade feature. Keep it
out of `default` so projects pay the evaluator dependency and build cost only
when requested:

```toml
[features]
# Keep the project's existing default list unchanged.
commands-shell = ["reinhardt/commands-shell"]
```

Add `config::shell::get_shell_config()` with the explicit aliases that preserve
bindings across evaluator state transitions:

```rust,ignore
use crate::config::apps::InstalledApp;
use crate::config::settings::ProjectSettings;
use reinhardt::commands::ShellConfig;

pub use reinhardt as framework;

pub type ShellSettings = ProjectSettings;
pub type ProjectShellEnvironment =
	framework::commands::ShellEnvironment<ShellSettings>;
pub type ShellDatabase = framework::db::orm::DatabaseConnection;
pub type ShellDi = std::sync::Arc<framework::di::InjectionContext>;

pub fn get_shell_config() -> ShellConfig {
	ShellConfig::new(
		env!("CARGO_PKG_NAME"),
		"my_project",
		env!("CARGO_MANIFEST_DIR"),
		"my_project::config::settings::get_settings",
		InstalledApp::all_labels().iter().copied(),
	)
	.with_dependency_features(["commands-shell"])
}
```

Export it from `config.rs` only when enabled:

```rust
#[cfg(feature = "commands-shell")]
pub mod shell;
```

Then update `manage.rs`. The outer native `main` must call the runtime hook
before `#[tokio::main]` constructs Tokio, and the native module must force-link
the project crate so model, route, and DI-provider inventory registrations
remain available:

```rust,ignore
#[cfg(not(target_arch = "wasm32"))]
mod native {
	use my_project as _;
	#[cfg(feature = "commands-shell")]
	use my_project::config::shell::get_shell_config;
	use my_project::config::settings::get_settings;
	#[cfg(feature = "commands-shell")]
	use reinhardt::commands::execute_from_command_line_with_settings_and_shell;
	#[cfg(not(feature = "commands-shell"))]
	use reinhardt::commands::execute_from_command_line_with_settings;

	#[tokio::main]
	pub(super) async fn main() {
		// Preserve the project's existing settings-module initialization.
		// SAFETY: Called at program start before any spawned tasks.
		unsafe {
			std::env::set_var(
				"REINHARDT_SETTINGS_MODULE",
				"my_project.config.settings",
			);
		}

		#[cfg(feature = "commands-shell")]
		let result = execute_from_command_line_with_settings_and_shell(
			get_settings(),
			get_shell_config(),
		)
		.await;
		#[cfg(not(feature = "commands-shell"))]
		let result = execute_from_command_line_with_settings(get_settings()).await;

		if let Err(error) = result {
			eprintln!("Error: {error}");
			std::process::exit(1);
		}
	}
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
	reinhardt::commands::shell_runtime_hook();
	native::main();
}

#[cfg(target_arch = "wasm32")]
fn main() {}
```

Preserve the project's existing `REINHARDT_SETTINGS_MODULE` initialization and
its project-specific module string. `my_project.config.settings` is a
placeholder for that existing value, not a replacement required by the shell
migration.

Start the shell with
`cargo run --bin manage --features commands-shell -- shell`. The following are
ordinary Rust expressions in the evaluator:

```rust,ignore
println!("{}", settings.core.debug);
println!("{:?}", db.backend());
println!(
	"{}",
	di.get_singleton::<framework::db::orm::DatabaseConnection>()
		.is_some()
);
println!("{}", std::any::type_name::<User>());
println!(
	"{}",
	std::any::type_name::<project_crate::apps::billing::models::Record>()
);
```

`settings`, `db`, and `di` are the concrete project settings, copyable ORM
handle, and `Arc<InjectionContext>`. A uniquely named installed model such as
`User` is imported by its short name. A colliding name such as `Record` is not
imported; the deterministic startup warning lists concrete registered crate
paths such as `my_project::apps::billing::models::Record`. The evaluator's
stable `project_crate::...` alias can reference the same registered type. A
project can append Rust with `ShellConfig::with_prelude(...)`.

The first cold start may compile the project crate and evaluator support.
Unchanged warm starts reuse normal Cargo artifacts. History uses
`<platform local data directory>/reinhardt/shell/<package-name>.history`: a
missing file is a silent first run, while directory-resolution, read, or write
failures warn and leave the shell usable.

`shell -c SOURCE` returns zero only when bootstrap and the one evaluation
succeed. Reinhardt does not echo raw `SOURCE` in its own framework diagnostics,
but arbitrary Rust, compiler diagnostics, panics, and user code can print
literals or sensitive values. The shell runs with the invoking user's
permissions and is not a sandbox.

## Structured server-function errors

`ServerFnError` is now a structured type with `kind()`, `status()`,
`user_message()`, and `field_errors()` accessors. Replace enum constructors,
pattern matches, and raw response JSON parsing with the typed API:

| Previous API | New API |
| --- | --- |
| `ServerFnError::Network(message)` | `ServerFnError::transport(message)` |
| `ServerFnError::Serialization(message)` | `ServerFnError::serialization(message)` or `ServerFnError::transport(message)` |
| `ServerFnError::Deserialization(message)` | `ServerFnError::deserialization(message)` |
| `ServerFnError::Server { status, message }` | `ServerFnError::server(status, message)` |
| `ServerFnError::Application(message)` | `ServerFnError::application(message)` |
| enum pattern matching | `kind()`, `status()`, `user_message()`, and `field_errors()` |
| raw error JSON parsing | typed `ServerFnError` accessors |

Server-function failures now use the version 1 JSON envelope with lowercase
`kind`, nullable `status`, a safe `message`, and `field_errors`. Legacy
externally tagged JSON is not accepted as a runtime fallback. Deploy server and
WASM client artifacts together so both sides use the version 1 envelope.

## Closure-scoped ORM transactions

ORM transactions are now exclusively closure-scoped. `DatabaseConnection::atomic`
opens the outer transaction and lends its executor to the callback. Call
`AtomicTransaction::atomic` from that callback to create a nested savepoint.
The executor is mutable and cannot be used outside its callback, so all ORM
operations in the scope must use `*_with_conn(transaction, ...)` or
`*_with_db(transaction)` methods.

```rust,ignore
// Before
let mut transaction = connection.begin().await?;
let user = User::objects()
    .create_with_conn(&mut transaction, &new_user)
    .await?;
transaction.commit().await?;

// After: the nested callback stays inside the outer callback's scope.
let user = connection.atomic(async |transaction| {
    let user = User::objects()
        .create_with_conn(transaction, &new_user)
        .await?;

    // A nested callback is a savepoint on the same executor.
    transaction.atomic(async |nested_transaction| {
        audit_manager
            .create_with_conn(nested_transaction, &audit_log)
            .await
    }).await?;

    Ok(user)
}).await?;
```

Outside an atomic block, acquire and pass a mutable connection directly rather
than starting a manual transaction:

```rust,ignore
let mut connection = get_connection().await?;
let user = User::objects()
    .create_with_conn(&mut connection, &new_user)
    .await?;
```

`Session` remains a unit-of-work tracker. Use `Session::flush` to persist its
tracked changes, but do not use it as a transaction boundary. For multi-write
atomicity, perform the writes through `DatabaseConnection::atomic` and its
callback-owned executor. To abandon unflushed session state, discard and
recreate the `Session` instead of rolling it back.

`AsyncSession::begin` and `Engine::begin` are also removed. Use
`DatabaseConnection::atomic` for ORM transaction boundaries. `Engine` and raw
SQL remain available for operations outside the ORM atomic API.

The following public ORM APIs are removed:

- `TransactionScope` and `Atomic`
- free `atomic`, `atomic_with_isolation`, `transaction`, and
  `transaction_with_isolation` functions
- `DatabaseConnection::{begin_transaction, begin_transaction_with_isolation,
  commit_transaction, rollback_transaction, savepoint, release_savepoint,
  rollback_to_savepoint, begin, begin_with_isolation}`
- `Transaction::{begin_db, commit_db, rollback_db}`
- `Session::{begin, commit, rollback, has_transaction}` and
  `SessionError::TransactionError`
- `AsyncSession::begin`
- `Engine::begin`

Use `DatabaseConnection::atomic_with_isolation` when the outer transaction
requires a particular isolation level. `Transaction`, `Savepoint`, and
`IsolationLevel` remain available only as synchronous SQL-builder types; they
do not own or execute ORM transactions.

Callback failures roll back the active transaction. If rollback or savepoint
cleanup also fails, the cleanup error is returned because it is the most useful
signal that database state could not be restored. Panics and task cancellation
are not recoverable callback results; do not rely on them for rollback control
flow. MySQL implicitly commits many DDL statements, so do not put schema changes
inside an atomic callback and expect them to roll back.

## Typed ORM aggregates and annotations

The standard ORM aggregate vocabulary in 0.4 is the typed [`orm::func`] module.
Generated model fields and relation paths replace string operands, and labels
are validated with the fallible `.label(...)` method. Terminal aggregation and
row annotations are separate operations: `aggregate(...).await` executes one
aggregate row and returns `AggregateResult`, while `annotate(...)` returns a
fallible chainable `QuerySet` builder. `QuerySet::all()` still deserializes only
the model and intentionally ignores computed annotation columns.

| 0.3 API | 0.4 typed API |
| --- | --- |
| `Aggregate::Count` / `Aggregate::CountAll` | `func::count(field)` / `func::count_all::<Model>()`, then `.label("name")?` |
| `Aggregate::Sum`, `Aggregate::Avg`, `Aggregate::Min`, `Aggregate::Max` | `func::sum(Model::field_amount())`, `func::avg(...)`, `func::min(...)`, `func::max(...)`, then `.label("name")?` |
| `AggregateFunc` or `AggregateExpr` | the corresponding typed `func::*` expression; do not construct aggregate nodes directly |
| `AnnotationValue::Aggregate(...)` | `func::*` expression with `.label(...)`, passed to `QuerySet::annotate` or terminal `aggregate` |
| `"amount"` or `"posts__id"` aggregate operands | `Model::field_amount()` or a generated `Model::rel_posts().field_id()` path |
| `.with_alias("total")` for an aggregate alias | `.label("total")?` |
| `.having_avg("amount", ...)`, `.having_count(...)`, `.having_sum(...)`, `.having_min(...)`, `.having_max(...)` | `.having(func::avg(Model::field_amount()).gt(value))` (and the matching `func::*` constructor) |

For multi-valued relations, a count retains duplicate joined rows. Call
`.distinct()` on the operand aggregate when each related value should be
counted once:

```rust,ignore
let rows = User::objects()
    .all()
    .aggregate(func::count(User::rel_posts().field_id()).label("post_rows")?)
    .await?;
let unique = User::objects()
    .all()
    .aggregate(
        func::count(User::rel_posts().field_id())
            .distinct()
            .label("unique_posts")?,
    )
    .await?;
```

`reinhardt-query` remains the dynamic SQL-builder boundary for raw expressions
and statements. It is intentionally separate from the typed `func` vocabulary
and does not provide type-safe aggregate result decoding. PostgreSQL-only
projections such as `ArrayAgg`, `StringAgg`, and `JsonbAgg` remain explicit
through `BackendAnnotation` and `QuerySet::annotate_backend`; they are not
silently accepted by portable typed annotations. Scalar raw subqueries stay at
the explicit `QuerySet::annotate_subquery` boundary and remain fallible.

## Typed intrinsic events

Standard intrinsic `page!` handlers no longer receive one raw event type.
Each catalog event selects an exact payload such as `ClickEvent`, `InputEvent`,
or `ChangeEvent`.

```rust,ignore
// Before
fn handle_input(event: reinhardt_pages::platform::Event) {
    // Browser-only target cast.
}

// After
fn handle_input(event: reinhardt_pages::event::InputEvent) {
    match event.value() {
        Ok(value) => save(value),
        Err(error) => report(error),
    }
}
```

Inferred closures normally need no annotation:

```rust,ignore
page!({ input { @input: |event| { let _ = event.value(); } } })
```

External functions and `Callback` values must use the exact payload selected by
the event name. A payload for another event is a compile-time error.

## Raw handlers and custom events

Use explicit raw adapters when low-level access is required:

```rust,ignore
use reinhardt_pages::{raw_event_handler, platform};

let handler = raw_event_handler(|event: platform::Event| inspect(event));
```

Arbitrary intrinsic names have adjacent raw and typed forms:

```rust,ignore
// Raw event transport.
button { @custom("item-selected"): |event: platform::Event| { inspect(event); } }

// Typed browser CustomEvent.detail decoding.
button { @custom::<ItemSelected>("item-selected"): |event| {
    if let Ok(detail) = event.detail() {
        select(detail.id);
    }
} }
```

`@custom("name")` receives the unmodified `platform::Event`, while
`@custom::<T>("name")` receives `CustomEvent<T>`. `detail()` borrows the
cached decoded detail; `into_detail()` consumes the event and returns the owned
detail. Decode failures are structured `CustomEventDetailError` values, so
match `NotCustomEvent` and `Deserialize` instead of parsing strings. The
decoder-specific `Deserialize::message` is not stable across native and WASM
targets.

Manouche integrations that match the public `IntrinsicEvent` AST must rename
the raw custom-event arm from `IntrinsicEvent::Custom` to
`IntrinsicEvent::RawCustom`. The new `IntrinsicEvent::TypedCustom` arm
represents `@custom::<T>("name")` and carries its payload type separately:

```rust,ignore
match event {
    // Before
    IntrinsicEvent::Custom { name, handler } => inspect_raw(name, handler),

    // After
    IntrinsicEvent::RawCustom { name, handler } => inspect_raw(name, handler),
    IntrinsicEvent::TypedCustom {
        name,
        payload_type,
        handler,
    } => inspect_typed(name, payload_type, handler),
    IntrinsicEvent::Standard { event, handler } => inspect_standard(event, handler),
}
```

`Element::add_typed_custom_event_listener` callbacks now receive the complete
event rather than a `Result<T, String>` detail value:

```rust,ignore
// Before
|detail: Result<ItemSelected, String>| match detail {
    Ok(detail) => consume(detail),
    Err(error) => report(error),
}

// After
|event: CustomEvent<ItemSelected>| match event.into_detail() {
    Ok(detail) => consume(detail),
    Err(error) => report(error),
}
```

For browser-only DOM interop, `CustomEvent::raw()` retains the underlying
`web_sys::Event` on WASM. Portable code should otherwise prefer payload methods
and owned target snapshots.

## Target extraction

Replace `event.target()` casts and unchecked `expect` calls with capability
methods. `value`, `checked`, `selected_values`, and `files` return
`Result<_, EventTargetError>`. They read the listener's captured
`current_target`, not an element recast after async work begins.

## Native events and tests

`DummyEvent` is removed. Low-level native handlers receive `NativeEvent`, while
standard handlers receive the same generated payload types as WASM. Enable the
`testing` feature and use `EventFixture` to supply family data and target state.
Call `Screen::settle()` after async handlers or reactive writes. See
[`native_component_testing.md`](../crates/reinhardt-pages/docs/native_component_testing.md).

## Low-level event names

`reinhardt_core::types::page::EventType` now aliases the complete catalog-backed
`KnownEvent` enum. Code that exhaustively matched the previous small enum must
handle the expanded standard event set. Use `EventName` when a value may be
either a catalog event or an explicit custom name.

Parsing a standard name now returns `UnknownEventName` instead of `()`:

```rust,ignore
use reinhardt_core::types::page::EventType;

let event = "click".parse::<EventType>()?;
let dom_name = event.as_str();
```

The former `From<EventType> for &'static str` conversion is removed. Replace
`let name: &'static str = event.into();` with `event.as_str()`.

## Component event props

Component `@event` props are not intrinsic DOM events. Keep the component prop's
declared domain type, `()`, or an explicit standard payload when that is truly
the component contract. `@custom("name")` is valid only on intrinsic elements.

## Migration scan

```bash
rg -n "DummyEvent|platform::Event|event\.target\(\)|dyn_into::<.*Html" src crates examples
```

Classify intentional raw custom-event and low-level integration code before
replacing it. Then run native component tests and a WASM target check.
