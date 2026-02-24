+++
title = "Field Attributes"
description = "Reference for model field attributes."
weight = 30

[extra]
sidebar_weight = 30
+++

# Field Attributes Reference

This document lists all available attributes for the `#[field(...)]` macro.

## Overview

Reinhardt's `#[model(...)]` attribute macro automatically applies
`#[derive(Model)]` and provides fine-grained control over database schema
through field-level attributes. Currently, **42 attributes** are supported (20
existing + 22 newly implemented).

**Note:** When using `#[model(...)]`, you don't need to explicitly add
`#[derive(Model)]`.

## Attribute Classification

### Common to All DBMS

- Basic attributes (primary_key, unique, null, default, etc.)
- Generated Columns (generated, generated_stored)
- Collation (collate)

### PostgreSQL-Specific

- Identity columns (identity_always, identity_by_default)
- Storage optimization (storage, compression)

### MySQL-Specific

- Auto-increment (auto_increment)
- Character set (character_set)
- ON UPDATE (on_update_current_timestamp)
- Invisible columns (invisible)
- Numeric type attributes (unsigned, zerofill) ※Deprecated

### SQLite-Specific

- Auto-increment (autoincrement)
- Table-level attributes (strict, without_rowid)

### Multiple DBMS Support

- generated_virtual (MySQL, SQLite)
- comment (PostgreSQL, MySQL)
- fulltext (PostgreSQL, MySQL)

---

## Existing Attributes (20 items)

### Basic Constraints

#### `primary_key: bool`

Specifies the primary key.

```rust
#[field(primary_key = true)]
id: i32,
```

**Supported DBMS**: All **SQL Output**: `PRIMARY KEY`

#### `unique: bool`

Specifies a UNIQUE constraint.

```rust
#[field(unique = true)]
email: String,
```

**Supported DBMS**: All **SQL Output**: `UNIQUE`

#### `null: bool`

Allows NULL values. Default is `false` (NOT NULL).

```rust
#[field(null = true)]
optional_field: Option<String>,
```

**Supported DBMS**: All **SQL Output**: No output when NULL is allowed,
`NOT NULL` when not allowed

#### `default: &str`

Specifies a default value.

```rust
#[field(default = "'active'")]
status: String,
```

**Supported DBMS**: All **SQL Output**: `DEFAULT 'active'`

#### `db_default: &str`

Specifies a database function as the default value.

```rust
#[field(db_default = "CURRENT_TIMESTAMP")]
created_at: chrono::NaiveDateTime,
```

**Supported DBMS**: All **SQL Output**: `DEFAULT CURRENT_TIMESTAMP`

### Field Types and Length

#### `max_length: usize`

Specifies the maximum length for VARCHAR type.

```rust
#[field(max_length = 255)]
name: String,
```

**Supported DBMS**: All **SQL Output**: `VARCHAR(255)`

#### `min_length: usize`

Minimum length validation (application-level).

```rust
#[field(min_length = 3)]
username: String,
```

**Supported DBMS**: All (validation only) **SQL Output**: None

### Validation

#### `email: bool`

Enables email address format validation.

```rust
#[field(email = true)]
email: String,
```

**Supported DBMS**: All (validation only) **SQL Output**: None

#### `url: bool`

Enables URL format validation.

```rust
#[field(url = true)]
website: String,
```

**Supported DBMS**: All (validation only) **SQL Output**: None

#### `min_value: i64`

Minimum value validation (application-level).

```rust
#[field(min_value = 0)]
age: i32,
```

**Supported DBMS**: All (validation only) **SQL Output**: None

#### `max_value: i64`

Maximum value validation (application-level).

```rust
#[field(max_value = 150)]
age: i32,
```

**Supported DBMS**: All (validation only) **SQL Output**: None

#### `check: &str`

Specifies a CHECK constraint.

```rust
#[field(check = "age >= 0 AND age <= 150")]
age: i32,
```

**Supported DBMS**: All **SQL Output**: `CHECK (age >= 0 AND age <= 150)`

### Relations

#### `foreign_key: Type or &str`

Specifies a foreign key. Can be a type name or "app_label.ModelName" string
format.

```rust
#[field(foreign_key = User)]
user_id: i32,

// Or
#[field(foreign_key = "users.User")]
user_id: i32,
```

**Supported DBMS**: All **SQL Output**: `REFERENCES users(id)`

#### `on_delete: &str`

Specifies the action when the foreign key is deleted.

Values: `"CASCADE"`, `"SET NULL"`, `"RESTRICT"`, `"NO ACTION"`, `"SET DEFAULT"`

```rust
#[field(foreign_key = User, on_delete = "CASCADE")]
user_id: i32,
```

**Supported DBMS**: All **SQL Output**: `ON DELETE CASCADE`

### Other

#### `db_column: &str`

Explicitly specifies the database column name.

```rust
#[field(db_column = "user_name")]
username: String,
```

**Supported DBMS**: All **SQL Output**: Column name becomes `user_name`

#### `blank: bool`

Whether to allow empty strings (validation level).

```rust
#[field(blank = true)]
description: String,
```

**Supported DBMS**: All (validation only) **SQL Output**: None

#### `editable: bool`

Whether the field is editable (application-level).

```rust
#[field(editable = false)]
created_at: chrono::NaiveDateTime,
```

**Supported DBMS**: All (metadata only) **SQL Output**: None

#### `choices: Vec<(Value, Display)>`

Defines choices (application-level).

```rust
#[field(choices = vec![("active", "Active"), ("inactive", "Inactive")])]
status: String,
```

**Supported DBMS**: All (validation only) **SQL Output**: None

#### `help_text: &str`

Help text (for documentation).

```rust
#[field(help_text = "User's full name")]
name: String,
```

**Supported DBMS**: All (metadata only) **SQL Output**: None

---

## Newly Implemented Attributes (22 items)

### Core Features (10 attributes)

**Description**: Basic features used commonly across all or multiple DBMS

#### `generated: &str`

**Supported DBMS**: All **Feature Flag**: None

Specifies the expression for a generated (computed) column.

```rust
#[field(generated = "first_name || ' ' || last_name")]
full_name: String,
```

**SQL Output**:

- PostgreSQL: `GENERATED ALWAYS AS (first_name || ' ' || last_name)`
- MySQL: `GENERATED ALWAYS AS (first_name || ' ' || last_name)`
- SQLite: `AS (first_name || ' ' || last_name)`

#### `generated_stored: bool`

**Supported DBMS**: All **Feature Flag**: None

Specifies whether to physically store the generated column. Used with
`generated`.

```rust
#[field(generated = "price * quantity", generated_stored = true)]
total: f64,
```

**SQL Output**: `STORED`

#### `generated_virtual: bool`

**Supported DBMS**: MySQL, SQLite **Feature Flag**:
`#[cfg(any(feature = "db-mysql", feature = "db-sqlite"))]`

Defines the generated column as a virtual column. Used with `generated`.

```rust
#[cfg(any(feature = "db-mysql", feature = "db-sqlite"))]
#[field(generated = "YEAR(birth_date)", generated_virtual = true)]
birth_year: i32,
```

**SQL Output**: `VIRTUAL`

**Note**: PostgreSQL does not support virtual columns.

#### `identity_always: bool`

**Supported DBMS**: PostgreSQL **Feature Flag**:
`#[cfg(feature = "db-postgres")]`

Defines a PostgreSQL IDENTITY ALWAYS column.

```rust
#[cfg(feature = "db-postgres")]
#[field(identity_always = true)]
id: i64,
```

**SQL Output**: `GENERATED ALWAYS AS IDENTITY`

#### `identity_by_default: bool`

**Supported DBMS**: PostgreSQL **Feature Flag**:
`#[cfg(feature = "db-postgres")]`

Defines a PostgreSQL IDENTITY BY DEFAULT column.

```rust
#[cfg(feature = "db-postgres")]
#[field(identity_by_default = true)]
id: i64,
```

**SQL Output**: `GENERATED BY DEFAULT AS IDENTITY`

#### `auto_increment: bool`

**Supported DBMS**: MySQL **Feature Flag**: `#[cfg(feature = "db-mysql")]`

Specifies MySQL AUTO_INCREMENT attribute.

```rust
#[cfg(feature = "db-mysql")]
#[field(auto_increment = true)]
id: u32,
```

**SQL Output**: `AUTO_INCREMENT`

#### `autoincrement: bool`

**Supported DBMS**: SQLite **Feature Flag**: `#[cfg(feature = "db-sqlite")]`

Specifies SQLite AUTOINCREMENT attribute.

```rust
#[cfg(feature = "db-sqlite")]
#[field(primary_key = true, autoincrement = true)]
id: i64,
```

**SQL Output**: `AUTOINCREMENT`

**Mutual Exclusion**: `identity_always`, `identity_by_default`,
`auto_increment`, and `autoincrement` are mutually exclusive. Specifying
multiple on a single field will result in a compile error.

#### `collate: &str`

**Supported DBMS**: All **Feature Flag**: None

Specifies the collation.

```rust
#[field(collate = "utf8mb4_unicode_ci")]
name: String,
```

**SQL Output**: `COLLATE utf8mb4_unicode_ci`

#### `character_set: &str`

**Supported DBMS**: MySQL **Feature Flag**: `#[cfg(feature = "db-mysql")]`

Specifies MySQL character set.

```rust
#[cfg(feature = "db-mysql")]
#[field(character_set = "utf8mb4")]
description: String,
```

**SQL Output**: `CHARACTER SET utf8mb4`

#### `comment: &str`

**Supported DBMS**: PostgreSQL, MySQL **Feature Flag**:
`#[cfg(any(feature = "db-postgres", feature = "db-mysql"))]`

Specifies column comment.

```rust
#[cfg(any(feature = "db-postgres", feature = "db-mysql"))]
#[field(comment = "User's email address")]
email: String,
```

**SQL Output**:

- PostgreSQL: Separate SQL statement `COMMENT ON COLUMN table.column IS 'text'`
- MySQL: `COMMENT 'text'`

---

### DBMS-Specific Features (5 attributes)

**Description**: Important features specific to particular DBMS (PostgreSQL, MySQL)

#### `storage: &str`

**Supported DBMS**: PostgreSQL **Feature Flag**:
`#[cfg(feature = "db-postgres")]`

Specifies PostgreSQL storage strategy.

Values: `"plain"`, `"extended"`, `"external"`, `"main"`

```rust
#[cfg(feature = "db-postgres")]
#[field(storage = "external")]
large_text: String,
```

**SQL Output**: `STORAGE EXTERNAL`

**Storage Strategy Descriptions**:

- `PLAIN`: Inline storage, no compression
- `EXTENDED`: Inline storage, with compression
- `EXTERNAL`: External table storage, no compression
- `MAIN`: External table storage, with compression

#### `compression: &str`

**Supported DBMS**: PostgreSQL **Feature Flag**:
`#[cfg(feature = "db-postgres")]`

Specifies PostgreSQL compression method.

Values: `"pglz"`, `"lz4"`

```rust
#[cfg(feature = "db-postgres")]
#[field(compression = "lz4")]
data: Vec<u8>,
```

**SQL Output**: `COMPRESSION lz4`

#### `on_update_current_timestamp: bool`

**Supported DBMS**: MySQL **Feature Flag**: `#[cfg(feature = "db-mysql")]`

Specifies MySQL ON UPDATE CURRENT_TIMESTAMP.

```rust
#[cfg(feature = "db-mysql")]
#[field(on_update_current_timestamp = true)]
updated_at: chrono::NaiveDateTime,
```

**SQL Output**: `ON UPDATE CURRENT_TIMESTAMP`

#### `invisible: bool`

**Supported DBMS**: MySQL 8.0.23+ **Feature Flag**:
`#[cfg(feature = "db-mysql")]`

Makes a MySQL column invisible.

```rust
#[cfg(feature = "db-mysql")]
#[field(invisible = true)]
internal_metadata: String,
```

**SQL Output**: `INVISIBLE`

**Use Case**: Columns you want to hide from `SELECT *` (e.g., audit metadata).

#### `fulltext: bool`

**Supported DBMS**: PostgreSQL, MySQL **Feature Flag**:
`#[cfg(any(feature = "db-postgres", feature = "db-mysql"))]`

Indicates that a full-text search index should be created.

```rust
#[cfg(any(feature = "db-postgres", feature = "db-mysql"))]
#[field(fulltext = true)]
content: String,
```

**SQL Output**: Not included in column definition; created separately as an
index.

---

### Legacy Compatibility (4 attributes)

**Description**: Features for backward compatibility (some deprecated)

#### `unsigned: bool` ⚠️ Deprecated

**Supported DBMS**: MySQL **Feature Flag**: `#[cfg(feature = "db-mysql")]`

Specifies MySQL unsigned integer type.

```rust
#[cfg(feature = "db-mysql")]
#[field(unsigned = true)]
count: i32,
```

**SQL Output**: `UNSIGNED`

**⚠️ Warning**: Deprecated since MySQL 8.0.17. Using CHECK constraints is
recommended.

#### `zerofill: bool` ⚠️ Deprecated

**Supported DBMS**: MySQL **Feature Flag**: `#[cfg(feature = "db-mysql")]`

Specifies MySQL zero-fill display.

```rust
#[cfg(feature = "db-mysql")]
#[field(zerofill = true)]
code: i32,
```

**SQL Output**: `ZEROFILL`

**⚠️ Warning**: Deprecated since MySQL 8.0.17. Application-level formatting is
recommended.

---

## Table-Level Attributes (`#[model(...)]`)

### `strict: bool`

**Supported DBMS**: SQLite **Feature Flag**: `#[cfg(feature = "db-sqlite")]`

Creates a SQLite STRICT table.

```rust
#[cfg(feature = "db-sqlite")]
#[model(table_name = "users", strict = true)]
struct User {
	// ...
}
```

**SQL Output**: `CREATE TABLE users (...) STRICT;`

**Use Case**: When you want strict type checking.

### `without_rowid: bool`

**Supported DBMS**: SQLite **Feature Flag**: `#[cfg(feature = "db-sqlite")]`

Creates a SQLite WITHOUT ROWID table.

```rust
#[cfg(feature = "db-sqlite")]
#[model(table_name = "cache", without_rowid = true)]
struct CacheEntry {
	#[field(primary_key = true)]
	key: String,
	value: String,
}
```

**SQL Output**: `CREATE TABLE cache (...) WITHOUT ROWID;`

**Use Case**: Performance optimization when the primary key is not an integer.

---

## Feature Flags

Enable the necessary feature flags in your project's `Cargo.toml`:

```toml
[dependencies]
reinhardt-macros = { version = "0.1", features = ["db-postgres", "db-mysql", "db-sqlite"] }
reinhardt-migrations = { version = "0.1", features = ["db-postgres", "db-mysql", "db-sqlite"] }
```

Available feature flags:

- `db-postgres`: Enable PostgreSQL-specific attributes
- `db-mysql`: Enable MySQL-specific attributes
- `db-sqlite`: Enable SQLite-specific attributes

---

## Usage Examples

### Complex Model Example

```rust
use reinhardt::db::orm::prelude::*;
use chrono::NaiveDateTime;

#[model(table_name = "articles")]
#[cfg_attr(feature = "db-sqlite", model(strict = true))]
struct Article {
	// PostgreSQL: IDENTITY BY DEFAULT
	#[cfg(feature = "db-postgres")]
	#[field(identity_by_default = true)]
	id: i64,

	// MySQL: AUTO_INCREMENT
	#[cfg(feature = "db-mysql")]
	#[field(auto_increment = true)]
	id: u32,

	// SQLite: AUTOINCREMENT
	#[cfg(feature = "db-sqlite")]
	#[field(primary_key = true, autoincrement = true)]
	id: i64,

	// All DBMS: Basic constraints
	#[field(max_length = 255, unique = true, collate = "utf8mb4_unicode_ci")]
	title: String,

	// Full-text search target
	#[cfg(any(feature = "db-postgres", feature = "db-mysql"))]
	#[field(fulltext = true)]
	content: String,

	// Generated column (stored)
	#[field(generated = "UPPER(title)", generated_stored = true)]
	title_upper: String,

	// With comment
	#[cfg(any(feature = "db-postgres", feature = "db-mysql"))]
	#[field(comment = "Article creation timestamp")]
	created_at: NaiveDateTime,

	// MySQL: ON UPDATE CURRENT_TIMESTAMP
	#[cfg(feature = "db-mysql")]
	#[field(on_update_current_timestamp = true)]
	updated_at: NaiveDateTime,

	// PostgreSQL: Compression and storage strategy
	#[cfg(feature = "db-postgres")]
	#[field(storage = "external", compression = "lz4")]
	large_data: Vec<u8>,
}
```

---

## Migrations

When using new attributes, the migration system automatically generates the
appropriate SQL:

```bash
# Generate migration files
cargo make makemigrations

# Apply migrations
cargo make migrate
```

Example generated SQL (PostgreSQL):

```sql
CREATE TABLE articles (
	id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
	title VARCHAR(255) UNIQUE COLLATE "utf8mb4_unicode_ci",
	content TEXT,
	title_upper TEXT GENERATED ALWAYS AS (UPPER(title)) STORED,
	created_at TIMESTAMP NOT NULL,
	large_data BYTEA STORAGE EXTERNAL COMPRESSION lz4
);

COMMENT ON COLUMN articles.created_at IS 'Article creation timestamp';
CREATE INDEX idx_articles_content_fulltext ON articles USING GIN (to_tsvector('english', content));
```

---

## Troubleshooting

### Compile Error: Attribute Not Recognized

**Cause**: Required feature flag is not enabled.

**Solution**: Enable the appropriate feature flag in `Cargo.toml`.

### Mutual Exclusion Error: Multiple Auto-Increment Attributes

```
error: Only one auto-increment attribute can be specified per field
```

**Cause**: Multiple attributes from `identity_always`, `identity_by_default`,
`auto_increment`, or `autoincrement` are specified.

**Solution**: Specify only one.

### Conflict Between Generated Column and Default Value

```
error: Generated columns cannot have default values
```

**Cause**: Both `generated` and `default` are specified simultaneously.

**Solution**: Generated columns cannot have default values. Remove one of them.

---

## References

- [PostgreSQL Generated Columns](https://www.postgresql.org/docs/current/ddl-generated-columns.html)
- [MySQL Generated Columns](https://dev.mysql.com/doc/refman/8.0/en/create-table-generated-columns.html)
- [SQLite Generated Columns](https://www.sqlite.org/gencol.html)
- [PostgreSQL STORAGE](https://www.postgresql.org/docs/current/sql-altertable.html#SQL-ALTERTABLE-DESC-SET-STORAGE)
- [MySQL INVISIBLE Columns](https://dev.mysql.com/doc/refman/8.0/en/invisible-columns.html)
