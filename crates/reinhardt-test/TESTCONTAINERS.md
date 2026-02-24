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
reinhardt-test = { version = "0.1.0-alpha.1", features = ["testcontainers"] }
```

### 2. Basic Usage

#### PostgreSQL

```rust
use reinhardt_test::containers::with_postgres;

#[tokio::test]
#[ignore] // Requires Docker
async fn test_with_postgres() {
    with_postgres(|db| async move {
        let url = db.connection_url();
        // Use the database connection URL...

        Ok(())
    }).await.unwrap();
}
```

#### MySQL

```rust
use reinhardt_test::containers::with_mysql;

#[tokio::test]
#[ignore] // Requires Docker
async fn test_with_mysql() {
    with_mysql(|db| async move {
        let url = db.connection_url();
        // Use the database connection URL...

        Ok(())
    }).await.unwrap();
}
```

#### Redis

```rust
use reinhardt_test::containers::with_redis;

#[tokio::test]
#[ignore] // Requires Docker
async fn test_with_redis() {
    with_redis(|redis| async move {
        let url = redis.connection_url();
        // Use the Redis connection URL...

        Ok(())
    }).await.unwrap();
}
```

### 3. Using with APITestCase

```rust
use reinhardt_test::prelude::*;
use reinhardt_test::containers::with_postgres;
use reinhardt_test::resource::AsyncTestResource;

#[tokio::test]
#[ignore] // Requires Docker
async fn test_api_with_database() {
    with_postgres(|db| async move {
        // Create APITestCase and set database URL
        let test_case = APITestCase::setup().await;
        test_case.set_database_url(db.connection_url()).await;

        // Get the database URL
        let db_url = test_case.database_url().await.unwrap();

        // Run your API tests...
        let client = test_case.client().await;
        let response = client.get("/api/users/").await.unwrap();

        test_case.teardown().await;
        Ok(())
    }).await.unwrap();
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

#[tokio::test]
#[ignore] // Requires Docker
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

Container startup can take 2-5 seconds. Use `#[ignore]` to skip in normal test runs:

```rust
#[tokio::test]
#[ignore] // Requires Docker
async fn slow_integration_test() {
    // ...
}
```

Then run explicitly:

```bash
cargo test -- --ignored
```

## Examples

See [`examples/testcontainer_usage.rs`](examples/testcontainer_usage.rs) for comprehensive examples.

## License

BSD 3-Clause License
