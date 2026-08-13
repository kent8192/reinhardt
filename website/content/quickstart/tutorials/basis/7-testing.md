+++
title = "Part 7: Testing"
description = "Test the polling app with native integration tests, createsuperuser coverage, and WASM MSW mocks."
weight = 70

[extra]
sidebar_weight = 70
+++

# Part 7: Testing

The final slice is the test suite. The example covers native database/server-function behavior, `createsuperuser` macro wiring, and WASM components that call mocked server functions.

The tests are intentionally split by target. Native tests can open SQLite databases and call server functions directly. WASM tests run in a browser with MSW-backed mocks.

## Declare Test Targets

Open `Cargo.toml`. The native integration test requires the native framework feature:

```toml
[[test]]
name = "integration"
required-features = ["with-reinhardt"]

[[test]]
name = "contract_export"
required-features = ["with-reinhardt"]
```

The WASM tests live under `tests/wasm/`, so Cargo needs explicit `[[test]]` entries:

```toml
[[test]]
name = "polls_mock_test"
path = "tests/wasm/polls_mock_test.rs"
required-features = ["msw"]

[[test]]
name = "users_mock_test"
path = "tests/wasm/users_mock_test.rs"
required-features = ["msw"]
```

The `msw` feature forwards to the Reinhardt facade so `#[server_fn]` generates the mockable markers that the browser tests consume:

```toml
msw = ["reinhardt/msw"]
```

## Native Test Dependencies

The native test fixture uses `rstest`, `serial_test`, `tokio`, `sqlx`, `tempfile`, and Reinhardt's database connection APIs:

```toml
[dev-dependencies]
rstest = { version = "0.26", default-features = false }

[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]
serial_test = "3.2"
tokio = { version = "1.48.0", features = ["rt", "macros"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }
tempfile = "3.15"
jsonschema = "0.26"
```

```rust
use reinhardt::DatabaseConnection;
use reinhardt::db::orm::reinitialize_database;
use rstest::*;
use serial_test::serial;
use sqlx::SqlitePool;
use std::sync::Arc;
use tempfile::NamedTempFile;
```

Gate native tests so `wasm-pack test` does not try to link `sqlx` for `wasm32-unknown-unknown`:

```rust
#![cfg(server)]
```

Keep those dependencies native-only. If `tokio`, `sqlx`, or `tempfile` are
placed in the shared `[dev-dependencies]` table, `wasm-pack test` will try to
compile their native transitive dependencies for `wasm32-unknown-unknown`.

## Test the application contract consumer

The native `contract_export` target launches the already-built `manage` binary
with an explicit temporary SQLite URL twice. It asserts byte-for-byte
determinism, one trailing newline, non-empty contract sections, and
`applied: false` for a fresh recorder. It validates stdout with the published
v0 schema. A second run reserves the implicit `db.sqlite3` path as a directory
to force the best-effort database overlay to emit one redacted warning and
`applied: null` without failing the export. Run it with:

```bash
cargo test --manifest-path examples/Cargo.toml \
  -p examples-tutorial-basis --all-features --test contract_export
```

## Build an Isolated SQLite Fixture

The server-function fixture creates a temp SQLite database, initializes ORM state, and returns a `DatabaseConnection`:

```rust
#[fixture]
async fn sqlite_with_test_data() -> (NamedTempFile, Arc<SqlitePool>, DatabaseConnection) {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let db_path = temp_file.path().to_str().unwrap().to_string();

    let sqlx_url = format!("sqlite://{}?mode=rwc", db_path);
    let orm_url = format!("sqlite:///{}", db_path);

    let pool = SqlitePool::connect(&sqlx_url)
        .await
        .expect("Failed to connect to SQLite");
    let pool = Arc::new(pool);
```

The fixture inserts rows with `sqlx`, then calls:

```rust
reinitialize_database(&orm_url)
    .await
    .expect("Failed to initialize ORM database");

let db_conn = DatabaseConnection::connect_sqlite(&orm_url)
    .await
    .expect("Failed to create DatabaseConnection");
```

Use `NamedTempFile` so the database disappears with the fixture.

## Call Server Functions Directly

Server functions are plain async Rust functions after macro expansion, with injected values represented as arguments. The example tests the read path directly:

```rust
#[rstest]
#[tokio::test]
#[serial(server_fn_tests)]
async fn test_get_questions_server_fn(
    #[future] sqlite_with_test_data: (NamedTempFile, Arc<SqlitePool>, DatabaseConnection),
) {
    let (_file, _pool, db_conn) = sqlite_with_test_data.await;

    let result = get_questions(db_conn).await;
    let questions = result.expect("get_questions should succeed");
    assert_eq!(questions.len(), 2);
}
```

Detail, results, and vote tests follow the same pattern:

```rust
let result = get_question_detail(1, db_conn).await;
assert!(result.is_ok(), "get_question_detail should succeed");
```

```rust
let result = get_question_results(1, db_conn).await;
assert!(result.is_ok(), "get_question_results should succeed");
```

```rust
let vote_request = VoteRequest {
    question_id: 1,
    choice_id: 1,
};
let result = vote(vote_request, db_conn).await;
```

Use `#[serial(server_fn_tests)]` for tests that share global ORM state.

## Test Auth Gates

Anonymous rejection is handled by the `CurrentUser<User>` injector before a server-function body runs. The direct-call CUD tests therefore focus on the handler-local inactive-user guard:

```rust
fn inactive_current_user() -> CurrentUser<User> {
    CurrentUser(
        User::build()
            .username("inactive-user".to_string())
            .password_hash(None)
            .is_active(false)
            .is_superuser(false)
            .finish(),
    )
}
```

Then they call CUD functions and assert a 403 before any database mutation path runs:

```rust
let result = create_question(
    "Inactive attempt".to_string(),
    db_conn,
    inactive_current_user(),
)
.await;

assert!(result.is_err());
```

Do not claim this fixture covers every ownership path. Author success, non-author 403 cases, and anonymous-injector 401 cases require a request/DI fixture with `users` plus `questions.author_id`.

## Test Createsuperuser Wiring

`tests/createsuperuser.rs` verifies the minimal `User` model participates in superuser creation:

```rust
#[rstest]
fn tutorial_user_auto_generates_superuser_init() {
    let username = "alice";
    let ignored_email = "";

    let user = User::init_superuser(username, ignored_email);

    assert_eq!(
        user.username, username,
        "username_field must be populated by init_superuser"
    );
    assert!(user.is_superuser);
    assert!(user.is_active);
}
```

The same file checks password hashing and `SuperuserCreatorRegistration` inventory. This protects the `#[user] + #[model]` macro wiring used by `cargo run --bin manage createsuperuser`.

## Add WASM Poll Tests

Add the browser test dependencies:

```toml
[target.'cfg(target_arch = "wasm32")'.dev-dependencies]
gloo-timers = { version = "0.3", features = ["futures"] }
wasm-bindgen-test = "0.3"
```

WASM tests run in a browser:

```rust
#![cfg(all(target_family = "wasm", target_os = "unknown"))]

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);
```

The polls test imports real components and server-function markers:

```rust
use tutorial::apps::polls::client::components::{
    polls_detail, polls_index, polls_results,
};
use tutorial::apps::polls::server_fn::{
    get_question_detail, get_question_results, get_questions, vote,
};
use reinhardt::test::msw::MockServiceWorker;
```

Mock the server function response through MSW:

```rust
#[wasm_bindgen_test]
async fn test_get_questions_returns_mocked_list() {
    let worker = MockServiceWorker::new();
    worker.handle_server_fn::<get_questions::marker>(|_args| Ok(mock_questions_list()));

    let questions = get_questions().await.expect("server_fn should succeed");

    assert_eq!(questions.len(), 3);
    assert_eq!(
        worker
            .calls_to_server_fn::<get_questions::marker>()
            .len(),
        1
    );
}
```

The rest of `polls_mock_test.rs` covers rendering, detail/results round trips, vote success/error paths, radio checked-state behavior, and serialization of the generated model info DTOs.

## Add WASM Users Tests

`tests/wasm/users_mock_test.rs` mirrors the polls test for auth:

```rust
use tutorial::apps::users::client::components::{
    login_form, logout_form, signup_form,
};
use tutorial::apps::users::server_fn::{current_user, login, logout, register};
use reinhardt::test::msw::MockServiceWorker;
```

It checks form rendering plus MSW round trips for `login`, `register`, `logout`, and `current_user`.

## Run the Tests

Run native tests:

```bash
cargo make test
```

Run browser/WASM tests:

```bash
cargo make wasm-test
```

The `wasm-test` task expands to:

```toml
[tasks.wasm-test]
description = "Run WASM tests in headless Chrome"
command = "wasm-pack"
args = ["test", "--headless", "--chrome", "--", "--no-default-features", "--features", "client-router,msw"]
```

Inside the standalone tutorial project, the focused native test command is:

```bash
cargo nextest run --all-features
```

The reference example lives in the repository's `examples` workspace. From a
repository checkout, enter that workspace before running the equivalent package
command:

```bash
cd examples
cargo nextest run -p examples-tutorial-basis --all-features
```

## Checkpoint

Before finishing the tutorial:

- Native tests run under `#[cfg(server)]`.
- Server functions are tested by direct invocation with injected arguments.
- Global ORM state tests use `#[serial(...)]`.
- Auth CUD tests cover anonymous rejection and do not overclaim non-author coverage.
- WASM tests use `MockServiceWorker` and real components.
- `cargo make test` and `cargo make wasm-test` are the example's local gates.
