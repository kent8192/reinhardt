# Migration Guide: 0.3.x to 0.4.0

This guide covers the Rust management-shell migration, breaking Reinhardt Pages
event API, and closure-scoped ORM transaction API introduced for 0.4.

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

## JSONB column and field type naming

The JSONB type variants and the DDL builder method are renamed for consistency
with PostgreSQL's `jsonb`/`JSONB` spelling and the surrounding type enums.
Generated SQL is unchanged: PostgreSQL and CockroachDB still emit `JSONB`, MySQL
still maps to `JSON`, and SQLite still maps to `TEXT`. Only the Rust names change.

| Previous API | New API |
| --- | --- |
| `reinhardt_query::types::ColumnType::JsonBinary` | `reinhardt_query::types::ColumnType::Jsonb` |
| `reinhardt_query::types::ColumnDef::json_binary()` | `reinhardt_query::types::ColumnDef::jsonb()` |
| `reinhardt_db::migrations::FieldType::JsonBinary` | `reinhardt_db::migrations::FieldType::Jsonb` |

Update the `ColumnDef` builder call and any `ColumnType` references:

```rust,ignore
use reinhardt_query::types::{ColumnDef, ColumnType};

// Before
let column = ColumnDef::new("metadata").json_binary();
let is_jsonb = matches!(column_type, ColumnType::JsonBinary);

// After
let column = ColumnDef::new("metadata").jsonb();
let is_jsonb = matches!(column_type, ColumnType::Jsonb);
```

Migration field types rename the same way:

```rust,ignore
use reinhardt_db::migrations::FieldType;

// Before
let field = FieldType::JsonBinary;

// After
let field = FieldType::Jsonb;
```

The `#[model]` field-type strings `"jsonb"` and `"JSONB"` are unchanged; only the
Rust variant they resolve to is renamed. No schema regeneration or data migration
is required, because the emitted column types are identical.

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

Arbitrary intrinsic names use `@custom("name")` and receive
`platform::Event`. The 0.4 event API does not add typed custom detail values;
that follow-up is tracked by #5636. Browser-only raw APIs remain available
through `payload.raw()` on WASM, but portable code should prefer payload
methods and owned target snapshots.

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
