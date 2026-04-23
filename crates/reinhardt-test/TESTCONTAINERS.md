# TestContainers Integration

TestContainers integration provides automatic Docker container management for database testing in Reinhardt.

## Features

- 🐘 **PostgreSQL** - Automatic PostgreSQL container management
- 🐬 **MySQL** - Automatic MySQL container management
- 🔴 **Redis** - Automatic Redis container management
- 🧹 **Auto Cleanup** - Containers are automatically removed after tests
- 🔌 **Easy Integration** - Works seamlessly with `APITestCase`

## Prerequisites

- Docker must be installed and running
- Enable the `testcontainers` feature in your `Cargo.toml`

## Usage

### 1. Enable the Feature

Add to your `Cargo.toml`:

```toml
[dev-dependencies]
reinhardt-test = { version = "0.1.0-rc.19", features = ["testcontainers"] }
```

### 2. Basic Usage

Container fixtures are provided by `reinhardt-test` using the `#[fixture]` macro from `rstest`.
Inject them as function parameters and use `#[future]` for async fixtures.

#### PostgreSQL

```rust
use reinhardt_test::postgres_container;
use rstest::*;
use std::sync::Arc;
use testcontainers::{ContainerAsync, GenericImage};

#[rstest]
#[tokio::test]
async fn test_with_postgres(
    #[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
    let (_container, pool, _port, database_url) = postgres_container.await;
    // Use pool or database_url...
}
```

#### MySQL

```rust
use reinhardt_test::fixtures::resources::{MySqlSuiteResource, mysql_suite};
use reinhardt_testkit::resource::SuiteGuard;
use rstest::*;

#[rstest]
#[tokio::test]
async fn test_with_mysql(mysql_suite: SuiteGuard<MySqlSuiteResource>) {
    let pool = &mysql_suite.pool;
    let database_url = &mysql_suite.database_url;
    // Use pool or database_url...
}
```

#### Redis

```rust
use reinhardt_test::redis_container;
use rstest::*;
use testcontainers::{ContainerAsync, GenericImage};

#[rstest]
#[tokio::test]
async fn test_with_redis(
    #[future] redis_container: (ContainerAsync<GenericImage>, u16, String),
) {
    let (_container, _port, url) = redis_container.await;
    // Use url...
}
```

### 3. Using with APITestCase

Combine the `postgres_container` fixture with `AsyncTeardownGuard<APITestCase>` via a custom
`#[fixture]` function:

```rust
use reinhardt_test::postgres_container;
use reinhardt_testkit::resource::{AsyncTeardownGuard, AsyncTestResource};
use reinhardt_testkit::testcase::APITestCase;
use rstest::*;
use std::sync::Arc;
use testcontainers::{ContainerAsync, GenericImage};

#[fixture]
async fn api_test_with_db(
    #[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) -> (AsyncTeardownGuard<APITestCase>, ContainerAsync<GenericImage>) {
    let (container, _pool, _port, database_url) = postgres_container.await;
    let case = AsyncTeardownGuard::<APITestCase>::new().await;
    case.set_database_url(database_url).await;
    (case, container)
}

#[rstest]
#[tokio::test]
async fn test_api_with_database(
    #[future] api_test_with_db: (AsyncTeardownGuard<APITestCase>, ContainerAsync<GenericImage>),
) {
    let (case, _container) = api_test_with_db.await;
    let client = case.client().await;
    let response = client.get("/api/users/").await.unwrap();
    response.assert_ok();
}
```

### 4. Using the `test_case_with_db!` Macro

The easiest way to use TestContainers:

```rust
use reinhardt_test::prelude::*;

// PostgreSQL
test_case_with_db! {
    postgres,
    async fn test_users_postgres(case: &APITestCase) {
        let db_url = case.database_url().await.unwrap();
        // Your test logic here...
    }
}

// MySQL
test_case_with_db! {
    mysql,
    async fn test_users_mysql(case: &APITestCase) {
        let db_url = case.database_url().await.unwrap();
        // Your test logic here...
    }
}
```

The macro automatically:

1. Starts a database container
2. Creates an `APITestCase` with the database URL
3. Runs `setup()`
4. Executes your test
5. Runs `teardown()`
6. Cleans up the container

### 5. Custom Configuration

```rust
use reinhardt_test::containers::PostgresContainer;
use rstest::rstest;

#[rstest]
#[tokio::test]
async fn test_custom_postgres() {
    let container = PostgresContainer::with_credentials(
        "my_user",
        "my_password",
        "my_database",
    ).await;

    // Container is already ready after construction
    let url = container.connection_url();
    // Use custom database...
}
```

## Docker Image Versions

The following Docker images are used by default:

- **PostgreSQL**: `postgres:17-alpine`
- **MySQL**: Default from `testcontainers-modules` crate (MySQL 8.x)
- **Redis**: Default from `testcontainers-modules` crate (Redis 7.x)

For exact MySQL/Redis versions, refer to the [`testcontainers-modules` documentation](https://docs.rs/testcontainers-modules/).

## Running Tests

```bash
# Run all tests (including TestContainer tests)
cargo test --features testcontainers -- --ignored

# Run specific test
cargo test --features testcontainers test_with_postgres -- --ignored

# Run the example
cargo test --example testcontainer_usage --features testcontainers -- --ignored
```

## CI/CD Integration

### GitHub Actions

```yaml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest

    services:
      docker:
        image: docker:dind
        options: --privileged

    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Run tests
        run: cargo test --features testcontainers -- --ignored
```

## Benefits

✅ **Isolated Testing** - Each test runs with a fresh database
✅ **Real Database** - Test against actual PostgreSQL/MySQL/Redis
✅ **No Mock Needed** - Use real database queries
✅ **Automatic Cleanup** - Containers removed after tests
✅ **CI/CD Ready** - Works seamlessly in CI pipelines
✅ **Parallel Tests** - Each test gets its own container

## Troubleshooting

### Docker not running

```
Error: Cannot connect to Docker daemon
```

**Solution**: Start Docker Desktop or the Docker daemon.

### Port conflicts

```
Error: Port 5432 already in use
```

**Solution**: TestContainers automatically assigns random ports. This usually indicates a system PostgreSQL is running. The container will use a different port.

### Slow tests

Container startup can take 2-5 seconds. Use `cargo nextest run --features testcontainers` to run TestContainers tests selectively:

```rust
use rstest::rstest;

#[rstest]
#[tokio::test]
async fn slow_integration_test() {
    // ...
}
```

Then run with the feature flag enabled:

```bash
cargo nextest run --features testcontainers
```

## Examples

See [`examples/testcontainer_usage.rs`](examples/testcontainer_usage.rs) for comprehensive examples.

## License

BSD 3-Clause License
