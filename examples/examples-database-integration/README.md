# Database Integration Example

This example demonstrates database ORM, migrations, and database connection
integration in the Reinhardt framework.

## Features

- **Django-style project structure**: Uses config/, settings/, apps.rs
- **Database configuration management**: Environment-specific database
  connection settings
- **Migration system**: Database schema version control
- **manage CLI**: Database management commands (`cargo run --bin manage`)

## Project Structure

```
src/
├── config/
│   ├── apps.rs              # Installed apps definition
│   ├── settings.rs          # Environment-based settings loader
│   ├── settings/
│   │   ├── base.rs          # Common settings for all environments
│   │   ├── local.rs         # Local development settings (including DB config)
│   │   ├── staging.rs       # Staging environment settings
│   │   └── production.rs    # Production environment settings
│   └── urls.rs              # URL routing configuration
├── migrations.rs            # Migration definitions
├── migrations/              # Migration files
│   └── 0001_initial.rs      # Initial migration
├── apps.rs                  # App registry
├── config.rs                # config module declaration
├── main.rs                  # Application entry point
└── bin/
    └── manage.rs            # Management CLI tool
```

## Setup

### Prerequisites

- Rust 2024 edition or later
- PostgreSQL, MySQL, or SQLite
- Cargo

### Database Setup

#### PostgreSQL (Recommended)

```bash
# Start PostgreSQL server
docker run -d \
  --name reinhardt-postgres \
  -e POSTGRES_USER=reinhardt \
  -e POSTGRES_PASSWORD=reinhardt_dev \
  -e POSTGRES_DB=reinhardt_examples \
  -p 5432:5432 \
  postgres:17
```

#### MySQL

```bash
# Start MySQL server
docker run -d \
  --name reinhardt-mysql \
  -e MYSQL_ROOT_PASSWORD=rootpass \
  -e MYSQL_DATABASE=reinhardt_examples \
  -e MYSQL_USER=reinhardt \
  -e MYSQL_PASSWORD=reinhardt_dev \
  -p 3306:3306 \
  mysql:8
```

#### SQLite

No additional setup required for SQLite.

### Build

```bash
# From project root
cargo build --package examples-database-integration
```

**Note**: This example will be buildable after reinhardt is published to
crates.io (version ^0.1).

## Usage

### Environment Variables

```bash
# PostgreSQL (default)
export DATABASE_URL="postgres://reinhardt:reinhardt_dev@localhost:5432/reinhardt_examples"

# MySQL
export DATABASE_URL="mysql://reinhardt:reinhardt_dev@localhost:3306/reinhardt_examples"

# SQLite
export DATABASE_URL="sqlite://./db.sqlite3"
```

### Migration Management

```bash
# Create new migration (auto-detects app if single app has models)
cargo run --bin manage makemigrations

# Create new migration for specific app (when multiple apps exist)
cargo run --bin manage makemigrations <app_label>

# Apply migrations
cargo run --bin manage migrate

# View migration plan (dry-run)
cargo run --bin manage migrate --plan

# Apply up to specific migration
cargo run --bin manage migrate app_name migration_name
```

**Auto-Detection Behavior:**

- If your project has only one app with models, the app label is automatically
  detected
- If multiple apps have models, you must specify the app label explicitly:
  ```bash
  cargo run --bin manage makemigrations users
  cargo run --bin manage makemigrations posts
  ```
- If no models are found, an error will be displayed with usage instructions

### Running Application

```bash
cargo run --package examples-database-integration
```

Output example:

```
Database Integration Example
✅ Application initialized
Debug mode: true
Database URL: postgres://reinhardt:reinhardt_dev@localhost:5432/reinhardt_examples
✅ Application started successfully
```

## Database Configuration

### Configuration in local.rs

```rust
use reinhardt::DatabaseConfig;

settings.database = Some(DatabaseConfig {
    url: database_url,
    max_connections: 10,
    min_connections: 1,
    connect_timeout: std::time::Duration::from_secs(30),
    idle_timeout: Some(std::time::Duration::from_secs(600)),
});
```

### Environment-Specific Settings

| Environment | File          | Database URL       | Connection Pool |
| ----------- | ------------- | ------------------ | --------------- |
| local       | local.rs      | Env var or default | 10 connections  |
| staging     | staging.rs    | Env var required   | 20 connections  |
| production  | production.rs | Env var required   | 50 connections  |

## Creating Migrations

### 1. Create Migration File

```bash
cargo run --bin manage makemigrations --name create_users_table
```

### 2. Create File in migrations/ Directory

```rust
// migrations/0002_create_users_table.rs
use reinhardt::prelude::*;

pub struct Migration;

impl MigrationTrait for Migration {
	fn name(&self) -> &str {
		"0002_create_users_table"
	}

	async fn up(&self, db: &Database) -> Result<()> {
		db.execute(r#"
			CREATE TABLE users (
				id SERIAL PRIMARY KEY,
				name VARCHAR(255) NOT NULL,
				email VARCHAR(255) UNIQUE NOT NULL,
				created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
			)
		"#).await?;
		Ok(())
	}

	async fn down(&self, db: &Database) -> Result<()> {
		db.execute("DROP TABLE users").await?;
		Ok(())
	}
}
```

### 3. Register in migrations.rs

```rust
// src/migrations.rs
mod _0001_initial;
mod _0002_create_users_table;

pub fn all_migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(_0001_initial::Migration),
        Box::new(_0002_create_users_table::Migration),
    ]
}
```

## Running Tests

This example uses **standard fixtures** from `reinhardt-test` for database
testing with automatic TestContainers management.

### Integration Tests

```bash
# Run all integration tests
cargo nextest run --features with-reinhardt --test database_tests

# Run specific test
cargo nextest run --features with-reinhardt --test database_tests test_database_connection
```

### Test Coverage

**Database Connection Tests:**

- ✅ Basic database connection verification
- ✅ Database readiness check
- ✅ Connection pool functionality

**Schema Tests:**

- ✅ Table creation and schema verification
- ✅ Column structure validation

**CRUD Operations:**

- ✅ CREATE: User insertion with RETURNING clause
- ✅ READ: User querying and filtering
- ✅ UPDATE: User data modification
- ✅ DELETE: User removal with verification

**Transaction Tests:**

- ✅ Transaction commit verification
- ✅ Transaction rollback verification
- ✅ Data consistency after transactions

### Standard Fixtures Used

**`postgres_container`** - PostgreSQL TestContainer fixture from
`reinhardt-test`

- Automatically starts PostgreSQL 17 Alpine container
- Provides connection pool (`Arc<sqlx::PgPool>`)
- Provides connection URL and port
- Automatic cleanup via RAII (container dropped after test)

**Usage Example:**

```rust
use reinhardt::test::fixtures::postgres_container;
use rstest::*;

#[rstest]
#[tokio::test]
async fn test_with_database(
    #[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
    let (_container, pool, port, database_url) = postgres_container.await;

    // Use pool for database operations
    let result = sqlx::query("SELECT 1").fetch_one(pool.as_ref()).await;
    assert!(result.is_ok());

    // Container is automatically cleaned up when dropped
}
```

### Testing Best Practices

**✅ GOOD - Using Standard Fixture:**

```rust
#[rstest]
#[tokio::test]
async fn test_with_standard_fixture(
    #[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
    let (_container, pool, _port, _url) = postgres_container.await;
    // Test code with automatic cleanup
}
```

**❌ BAD - Manual Container Management:**

```rust
#[tokio::test]
async fn test_with_manual_setup() {
    let docker = Cli::default();
    let container = docker.run(postgres_image);
    // Test code
    drop(container); // Manual cleanup required
}
```

See [Testing Standards](../../../docs/TESTING_STANDARDS.md) for comprehensive
guidelines.

## Troubleshooting

### Connection Errors

```
Error: Database connection failed
```

**Solutions:**

1. Verify database server is running
2. Check DATABASE_URL environment variable is set correctly
3. Verify credentials (username, password) are correct

### Migration Errors

```
Error: Migration failed: table already exists
```

**Solutions:**

1. Mark migration as applied using `--fake` option
2. Or skip initial migration only with `--fake-initial`

```bash
cargo run --bin manage migrate --fake-initial
```

## References

- [Reinhardt ORM Documentation](https://docs.rs/reinhardt-orm)
- [Reinhardt Migrations Guide](https://docs.rs/reinhardt-migrations)
- [Django Migrations](https://docs.djangoproject.com/en/stable/topics/migrations/)
- [SQLAlchemy](https://www.sqlalchemy.org/)

## License

This example is provided as part of the Reinhardt project under the BSD 3-Clause License.
