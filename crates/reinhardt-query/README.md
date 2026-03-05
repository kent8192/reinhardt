# reinhardt-query

A type-safe SQL query builder for the Reinhardt framework.

## Overview

`reinhardt-query` provides a fluent API for constructing SQL queries targeting PostgreSQL, MySQL, SQLite, and CockroachDB. It generates parameterized queries with proper identifier escaping and value placeholders for each backend.

## Features

### DML (Data Manipulation Language)
- **Type-safe query construction** - SELECT, INSERT, UPDATE, DELETE
- **DCL (Data Control Language) support** - GRANT and REVOKE statements
- **Expression system** - Arithmetic, comparison, logical, and pattern matching operators
- **Advanced SQL** - JOINs, GROUP BY, HAVING, DISTINCT, UNION, CTEs, Window functions
- **Parameterized queries** - `$1` for PostgreSQL/CockroachDB, `?` for MySQL/SQLite
- **CASE WHEN expressions** - Conditional expressions in queries
- **Subqueries** - EXISTS, IN, ALL, ANY, SOME operators

### DDL (Data Definition Language)
- **Table operations** - CREATE TABLE, ALTER TABLE, DROP TABLE
- **Index operations** - CREATE INDEX, ALTER INDEX, DROP INDEX, REINDEX
- **View operations** - CREATE VIEW, DROP VIEW
- **Schema management** - CREATE/ALTER/DROP SCHEMA (PostgreSQL, CockroachDB)
- **Sequence operations** - CREATE/ALTER/DROP SEQUENCE (PostgreSQL, CockroachDB)
- **Database operations** - CREATE/ALTER/DROP DATABASE (all backends)
- **Functions & Procedures** - CREATE/ALTER/DROP FUNCTION/PROCEDURE (PostgreSQL, MySQL, CockroachDB)
- **Custom types** - CREATE/ALTER/DROP TYPE (PostgreSQL, CockroachDB)
- **Materialized views** - CREATE/ALTER/DROP/REFRESH MATERIALIZED VIEW (PostgreSQL, CockroachDB)
- **Events** - CREATE/ALTER/DROP EVENT (MySQL)
- **Comments** - COMMENT ON for all database objects (PostgreSQL, CockroachDB)
- **Maintenance** - VACUUM, ANALYZE, OPTIMIZE/REPAIR/CHECK TABLE

### Multi-Backend Support
- **PostgreSQL** - Full DDL and DML support with advanced features
- **MySQL** - DML, Functions, Procedures, Events, and table maintenance
- **SQLite** - DML and basic DDL operations
- **CockroachDB** - Full PostgreSQL compatibility with distributed database features

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
reinhardt-query = { version = "0.1.0-alpha.1" }
```

## Quick Start

```rust
use reinhardt_query::prelude::*;

// Build a SELECT query
let mut stmt = Query::select();
stmt.column("name")
    .column("email")
    .from("users")
    .and_where(Expr::col("active").eq(true))
    .order_by("name", Order::Asc)
    .limit(10);

// Generate SQL for PostgreSQL
let builder = PostgresQueryBuilder::new();
let (sql, values) = builder.build_select(&stmt);
// sql = r#"SELECT "name", "email" FROM "users" WHERE "active" = $1 ORDER BY "name" ASC LIMIT $2"#
```

## Usage Examples

### INSERT

```rust
use reinhardt_query::prelude::*;

let mut stmt = Query::insert();
stmt.into_table("users")
    .columns(["name", "email", "active"])
    .values(["Alice", "alice@example.com", "true"]);

let builder = PostgresQueryBuilder::new();
let (sql, values) = builder.build_insert(&stmt);
```

### UPDATE

```rust
use reinhardt_query::prelude::*;

let mut stmt = Query::update();
stmt.table("users")
    .set("name", "Bob")
    .set("active", false)
    .and_where(Expr::col("id").eq(1i32));

let builder = PostgresQueryBuilder::new();
let (sql, values) = builder.build_update(&stmt);
```

### DELETE

```rust
use reinhardt_query::prelude::*;

let mut stmt = Query::delete();
stmt.from_table("users")
    .and_where(Expr::col("active").eq(false));

let builder = PostgresQueryBuilder::new();
let (sql, values) = builder.build_delete(&stmt);
```

### CREATE TABLE

```rust
use reinhardt_query::prelude::*;
use reinhardt_query::types::{ColumnDef, ColumnType};

let mut stmt = Query::create_table();
stmt.table("users")
    .if_not_exists()
    .column(ColumnDef {
        name: "id".into_iden(),
        column_type: ColumnType::Integer,
        not_null: true,
        primary_key: true,
        auto_increment: true,
        ..Default::default()
    })
    .column(ColumnDef {
        name: "email".into_iden(),
        column_type: ColumnType::String(Some(255)),
        not_null: true,
        unique: true,
        ..Default::default()
    });

let builder = PostgresQueryBuilder::new();
let (sql, values) = builder.build_create_table(&stmt);
// sql = r#"CREATE TABLE IF NOT EXISTS "users" ("id" INTEGER NOT NULL PRIMARY KEY, "email" VARCHAR(255) NOT NULL UNIQUE)"#
```

### ALTER TABLE

```rust
use reinhardt_query::prelude::*;
use reinhardt_query::query::AlterTableOperation;
use reinhardt_query::types::{ColumnDef, ColumnType};

let mut stmt = Query::alter_table();
stmt.table("users")
    .add_column(ColumnDef {
        name: "created_at".into_iden(),
        column_type: ColumnType::Timestamp,
        not_null: true,
        ..Default::default()
    })
    .rename_column("email", "email_address");

let builder = PostgresQueryBuilder::new();
let (sql, values) = builder.build_alter_table(&stmt);
// sql = r#"ALTER TABLE "users" ADD COLUMN "created_at" TIMESTAMP NOT NULL, RENAME COLUMN "email" TO "email_address""#
```

### DROP TABLE

```rust
use reinhardt_query::prelude::*;

let mut stmt = Query::drop_table();
stmt.table("users")
    .if_exists()
    .cascade();  // PostgreSQL only

let builder = PostgresQueryBuilder::new();
let (sql, values) = builder.build_drop_table(&stmt);
// sql = "DROP TABLE IF EXISTS \"users\" CASCADE"
```

### CREATE INDEX

```rust
use reinhardt_query::prelude::*;

let mut stmt = Query::create_index();
stmt.name("idx_email")
    .table("users")
    .col("email")
    .unique()
    .if_not_exists();

let builder = PostgresQueryBuilder::new();
let (sql, values) = builder.build_create_index(&stmt);
// sql = r#"CREATE UNIQUE INDEX IF NOT EXISTS "idx_email" ON "users" ("email")"#
```

### DROP INDEX

```rust
use reinhardt_query::prelude::*;

let mut stmt = Query::drop_index();
stmt.name("idx_email")
    .if_exists();

let builder = PostgresQueryBuilder::new();
let (sql, values) = builder.build_drop_index(&stmt);
// sql = "DROP INDEX IF EXISTS \"idx_email\""
```

### JOINs

```rust
use reinhardt_query::prelude::*;
use reinhardt_query::types::{JoinType, JoinExpr, JoinOn};

let mut stmt = Query::select();
stmt.column("users.name")
    .column("orders.total")
    .from("users")
    .join(JoinExpr {
        join_type: JoinType::InnerJoin,
        table: "orders".into_table_ref(),
        on: Some(JoinOn::Condition(
            Expr::col("users.id").eq(Expr::col("orders.user_id")).into_condition()
        )),
    });
```

### Window Functions

```rust
use reinhardt_query::prelude::*;
use reinhardt_query::types::WindowStatement;

let mut stmt = Query::select();
stmt.column("name")
    .expr(Expr::col("salary").over(WindowStatement {
        partition_by: vec![Expr::col("department").into_simple_expr()],
        order_by: vec![],
        frame: None,
    }))
    .from("employees");
```

### Common Table Expressions (CTEs)

```rust
use reinhardt_query::prelude::*;
use reinhardt_query::query::CommonTableExpr;

let mut cte_query = Query::select();
cte_query.column("id").column("name").from("categories")
    .and_where(Expr::col("parent_id").is_null());

let mut stmt = Query::select();
stmt.column("name")
    .from("top_categories")
    .with_cte(CommonTableExpr {
        name: "top_categories".into_iden(),
        columns: vec![],
        query: cte_query,
    });
```

### CREATE SCHEMA (PostgreSQL, CockroachDB)

```rust
use reinhardt_query::prelude::*;

let mut stmt = Query::create_schema();
stmt.name("app_schema")
    .if_not_exists()
    .authorization("owner_user");

let builder = PostgresQueryBuilder::new();
let (sql, _) = builder.build_create_schema(&stmt);
// sql = r#"CREATE SCHEMA IF NOT EXISTS "app_schema" AUTHORIZATION "owner_user""#
```

### CREATE SEQUENCE (PostgreSQL, CockroachDB)

```rust
use reinhardt_query::prelude::*;

let mut stmt = Query::create_sequence();
stmt.name("user_id_seq")
    .if_not_exists()
    .start_with(1000)
    .increment_by(1)
    .min_value(1)
    .max_value(999999);

let builder = PostgresQueryBuilder::new();
let (sql, _) = builder.build_create_sequence(&stmt);
```

### CREATE FUNCTION (PostgreSQL, MySQL, CockroachDB)

```rust
use reinhardt_query::prelude::*;

let mut stmt = Query::create_function();
stmt.name("add_numbers")
    .parameter("a", "INTEGER")
    .parameter("b", "INTEGER")
    .returns("INTEGER")
    .language_sql()
    .immutable()
    .body("SELECT $1 + $2");

let builder = PostgresQueryBuilder::new();
let (sql, _) = builder.build_create_function(&stmt);
```

### CREATE MATERIALIZED VIEW (PostgreSQL, CockroachDB)

```rust
use reinhardt_query::prelude::*;

let select = Query::select()
    .column(Expr::col("id"))
    .column(Expr::col("name"))
    .column(Expr::col("email"))
    .from("users")
    .and_where(Expr::col("active").eq(true));

let mut stmt = Query::create_materialized_view();
stmt.name("active_users")
    .as_select(select)
    .if_not_exists();

let builder = PostgresQueryBuilder::new();
let (sql, _) = builder.build_create_materialized_view(&stmt);
```

### COMMENT ON (PostgreSQL, CockroachDB)

```rust
use reinhardt_query::prelude::*;
use reinhardt_query::types::CommentTarget;

let mut stmt = Query::comment();
stmt.target(CommentTarget::Table("users".into_iden()))
    .comment("Stores user account information");

let builder = PostgresQueryBuilder::new();
let (sql, _) = builder.build_comment(&stmt);
// sql = r#"COMMENT ON TABLE "users" IS 'Stores user account information'"#
```

### GRANT (DCL)

```rust
use reinhardt_query::prelude::*;

let stmt = Query::grant()
    .privilege(Privilege::Select)
    .privilege(Privilege::Insert)
    .on_table("users")
    .to("app_user")
    .with_grant_option(true);

let builder = PostgresQueryBuilder::new();
let (sql, values) = builder.build_grant(&stmt);
// sql = r#"GRANT SELECT, INSERT ON TABLE "users" TO "app_user" WITH GRANT OPTION"#
```

### REVOKE (DCL)

```rust
use reinhardt_query::prelude::*;

let stmt = Query::revoke()
    .privilege(Privilege::Insert)
    .from_table("users")
    .from("app_user")
    .cascade(true);

let builder = PostgresQueryBuilder::new();
let (sql, values) = builder.build_revoke(&stmt);
// sql = r#"REVOKE INSERT ON TABLE "users" FROM "app_user" CASCADE"#
```

### GRANT Role Membership (DCL)

```rust
use reinhardt_query::dcl::{GrantRoleStatement, RoleSpecification};

let stmt = GrantRoleStatement::new()
    .role("developer")
    .to(RoleSpecification::new("alice"))
    .with_admin_option();

let builder = PostgresQueryBuilder::new();
let (sql, values) = builder.build_grant_role(&stmt);
// sql = r#"GRANT "developer" TO alice WITH ADMIN OPTION"#
```

### REVOKE Role Membership (DCL)

```rust
use reinhardt_query::dcl::{RevokeRoleStatement, RoleSpecification};

let stmt = RevokeRoleStatement::new()
    .role("developer")
    .from(RoleSpecification::new("alice"))
    .cascade();

let builder = PostgresQueryBuilder::new();
let (sql, values) = builder.build_revoke_role(&stmt);
// sql = r#"REVOKE "developer" FROM alice CASCADE"#
```

### Extended Object Types (PostgreSQL)

PostgreSQL supports additional object types beyond tables and databases:

```rust
use reinhardt_query::prelude::*;

// Grant EXECUTE on function
let stmt = Query::grant()
    .privilege(Privilege::Execute)
    .on_function("calculate_total")
    .to("app_user");

let builder = PostgresQueryBuilder::new();
let (sql, values) = builder.build_grant(&stmt);
// sql = r#"GRANT EXECUTE ON FUNCTION "calculate_total" TO "app_user""#

// Grant USAGE on type
let stmt = Query::grant()
    .privilege(Privilege::Usage)
    .on_type("custom_type")
    .to("app_user");

let (sql, values) = builder.build_grant(&stmt);
// sql = r#"GRANT USAGE ON TYPE "custom_type" TO "app_user""#

// Grant SET on parameter
let stmt = Query::grant()
    .privilege(Privilege::Set)
    .on_parameter("work_mem")
    .to("app_user");

let (sql, values) = builder.build_grant(&stmt);
// sql = r#"GRANT SET ON PARAMETER "work_mem" TO "app_user""#
```

Supported object types: Function, Procedure, Routine, Type, Domain, ForeignDataWrapper, ForeignServer, Language, LargeObject, Tablespace, Parameter

## Backend Differences

### DML Features
| Feature | PostgreSQL | MySQL | SQLite | CockroachDB |
|---------|-----------|-------|--------|-------------|
| Identifier quoting | `"name"` | `` `name` `` | `"name"` | `"name"` |
| Placeholders | `$1, $2, ...` | `?, ?, ...` | `?, ?, ...` | `$1, $2, ...` |
| NULLS FIRST/LAST | ✅ Native | ❌ | ✅ Native | ✅ Native |
| DISTINCT ON | ✅ | ❌ | ❌ | ✅ |
| Window functions | ✅ Full | ✅ Full | ✅ Full | ✅ Full |
| CTEs (WITH) | ✅ | ✅ | ✅ | ✅ |

### DDL Features
| Feature | PostgreSQL | MySQL | SQLite | CockroachDB |
|---------|-----------|-------|--------|-------------|
| CREATE/ALTER/DROP SCHEMA | ✅ | ❌ | ❌ | ✅ |
| CREATE/ALTER/DROP SEQUENCE | ✅ | ❌ | ❌ | ✅ |
| CREATE/ALTER/DROP DATABASE | ✅ | ✅ | ✅ | ✅ |
| CREATE/ALTER/DROP FUNCTION | ✅ | ✅ | ❌ | ✅ |
| CREATE/ALTER/DROP PROCEDURE | ✅ | ✅ | ❌ | ✅ |
| CREATE/ALTER/DROP TYPE | ✅ | ❌ | ❌ | ✅ |
| CREATE/ALTER/DROP EVENT | ❌ | ✅ | ❌ | ❌ |
| MATERIALIZED VIEW | ✅ | ❌ | ❌ | ✅ |
| COMMENT ON | ✅ | ❌ | ❌ | ✅ |
| VACUUM/ANALYZE | ✅ | ❌ | ✅ | ✅ |
| OPTIMIZE/REPAIR/CHECK | ❌ | ✅ | ❌ | ❌ |

### DCL Features
| Feature | PostgreSQL | MySQL | SQLite | CockroachDB |
|---------|-----------|-------|--------|-------------|
| GRANT/REVOKE | ✅ | ✅ | ❌ | ✅ |

## SQL Generation Notes

### Identifier Quoting

Each database backend uses different quoting styles for SQL identifiers (table names, column names, index names, etc.):

| Backend | Quote Style | Example |
|---------|-------------|---------|
| PostgreSQL | Double quotes | `"table_name"` |
| MySQL | Backticks | `` `table_name` `` |
| SQLite | Double quotes | `"table_name"` |
| CockroachDB | Double quotes | `"table_name"` |

#### Quoting Behavior

All backends automatically escape special characters within identifiers:

- **PostgreSQL/SQLite/CockroachDB**: Double quotes within identifiers are escaped by doubling (`"` becomes `""`)
- **MySQL**: Backticks within identifiers are escaped by doubling (`` ` `` becomes `` `` ``)

#### Testing Generated SQL

When writing tests for generated SQL, ensure you account for identifier quoting:

```rust
use reinhardt_query::backend::{PostgresQueryBuilder, QueryBuilder};
use reinhardt_query::prelude::*;

let builder = PostgresQueryBuilder::new();
let stmt = Query::select().column("name").from("users");
let (sql, _) = builder.build_select(&stmt);

// PostgreSQL uses double quotes
assert_eq!(sql, r#"SELECT "name" FROM "users""#);

// MySQL would generate: SELECT `name` FROM `users`
// SQLite would generate: SELECT "name" FROM "users"
```

#### Escape Methods

Each query builder provides an `escape_identifier` method for manual escaping:

```rust
use reinhardt_query::backend::{PostgresQueryBuilder, QueryBuilder};

let builder = PostgresQueryBuilder::new();
let escaped = builder.escape_identifier("user");
assert_eq!(escaped, "\"user\"");
```

### Parameter Placeholders

Each backend uses different placeholder styles for parameterized queries:

| Backend | Placeholder Style | Example |
|---------|------------------|---------|
| PostgreSQL | Numbered (`$1`, `$2`, ...) | `WHERE id = $1` |
| MySQL | Question mark (`?`) | `WHERE id = ?` |
| SQLite | Question mark (`?`) | `WHERE id = ?` |
| CockroachDB | Numbered (`$1`, `$2`, ...) | `WHERE id = $1` |

### Case Sensitivity

When identifiers are quoted:
- **Case is preserved** exactly as specified
- PostgreSQL/MySQL/SQLite are case-sensitive for quoted identifiers
- Unquoted identifiers may be case-folded (uppercased or lowercased) depending on the backend

### Reserved Keywords

All backends automatically quote identifiers, which allows using reserved keywords as identifiers:

```rust
use reinhardt_query::prelude::*;

// "user" is a reserved keyword in PostgreSQL, but quoting allows its use
let stmt = Query::select().column("name").from("user");
// Generates: SELECT "name" FROM "user"
```

## Feature Flags

| Flag | Description |
|------|-------------|
| `thread-safe` | Use `Arc` instead of `Rc` for `DynIden` |
| `with-chrono` | Enable chrono date/time types in `Value` |
| `with-uuid` | Enable UUID type in `Value` |
| `with-json` | Enable JSON type in `Value` |
| `with-rust_decimal` | Enable Decimal type in `Value` |
| `with-bigdecimal` | Enable BigDecimal type in `Value` |
| `full` | Enable all optional features |

## Security Considerations

### SQL Injection Prevention

This library uses parameterized queries by default, which protects against SQL
injection attacks. However, some APIs allow raw SQL strings that can be
vulnerable if misused.

### Unsafe APIs

The following APIs accept raw SQL strings and should be used with caution:

| API | Severity | Alternative |
|-----|----------|-------------|
| `Expr::cust()` / `SimpleExpr::Custom` | Medium | Use `Expr::cust_with_values()` |
| `CreateFunctionStatement::body()` | High | Only use with trusted input |
| `CreateProcedureStatement::body()` | High | Only use with trusted input |
| `ColumnType::Custom` | Low | Only use with trusted input |
| `FunctionLanguage::Custom` | Low | Only use with trusted input |

### Best Practices

1. **Never** pass user input directly to custom SQL APIs
2. **Always** use parameterized queries (`cust_with_values()`) for dynamic values
3. **Validate and sanitize** any input before using it in custom SQL
4. **Use** the type-safe query builder API when possible

## License

See the repository root for license information.
