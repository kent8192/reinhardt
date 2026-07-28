# Migration Guide: 0.3.x to 0.4.0

This guide covers the breaking Reinhardt Pages event API and closure-scoped ORM
transaction API introduced for 0.4.

## Automatic server-function registration

Ordinary `#[server_fn]` declarations now register through the native inventory
by default. Keep each declaration and its `server_url_patterns` function below
one application's `#[app_config]` module, then replace explicit marker chains
with the fixed collector call.

```rust,ignore
// Before
use reinhardt::pages::server_fn::ServerFnRouterExt;

pub fn server_url_patterns() -> ServerRouter {
    ServerRouter::new()
        .server_fn(create_post::marker)
        .server_fn(delete_post::marker)
}

// After
use reinhardt::pages::server_fn::ServerFnRouterExt;

pub fn server_url_patterns() -> ServerRouter {
    ServerRouter::new().auto_server_fns(module_path!())
}
```

For framework-owned or other deliberately explicit routers, retain the marker
and opt out of inventory registration:

```rust,ignore
#[server_fn(auto_register = false)]
async fn internal_health() -> Result<(), ServerFnError> {
    Ok(())
}

let router = ServerRouter::new().server_fn(internal_health::marker);
```

Do not combine the automatic collector with an explicit registration of the
same endpoint. Native system checks report deterministic ownership and
duplicate errors before command execution.

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
