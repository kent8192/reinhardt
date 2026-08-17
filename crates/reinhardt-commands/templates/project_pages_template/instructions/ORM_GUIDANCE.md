# ORM and Migrations Guidance

Keep each model and its database behavior inside the app that owns the
feature. Use `startapp` to create the app, then expose only explicit services
or serializable DTOs to other apps. This keeps the feature extractable into a
different project.

## Model ownership

Define persisted models in the app's `models.rs` and give them a stable app
label and table name:

```rust,ignore
use reinhardt::prelude::*;
use reinhardt::{Deserialize, Serialize};

#[model(app_label = "notes", table_name = "notes_item")]
#[derive(Serialize, Deserialize)]
pub struct Note {
    #[field(primary_key = true)]
    pub id: i64,

    #[field(max_length = 255)]
    pub title: String,

    pub score: i64,

    pub published: bool,
}
```

- Keep `app_label` stable after migrations are published. Changing it moves
  the model to another migration history.
- Treat `table_name` as a database contract. A rename needs an intentional
  migration, not an accidental model/table mismatch.
- Prefer the generated `Model::build()` typestate builder when constructing a
  record. Keep validation and business rules in the owning app's services.
- Do not import another app's model internals into a view or service. Define a
  DTO or service method when an app needs data owned by another app.

## Database and query boundaries

- Use `reinhardt-db` for CRUD, schema management, raw queries, and migration
  execution.
- Use `reinhardt-query` to construct SQL expressions instead of importing
  SeaQuery directly.
- Keep connections, models, and filesystem access in native/server modules.
  Pages client code should consume serializable DTOs or server-function
  results, never a database connection or ORM model.
- Keep query and transaction code in an app service so endpoints, commands,
  and background jobs can share it without importing one another's views.

## CRUD with `Model::objects()`

`Model::objects()` returns the model's manager. Keep this chain in the owning
app's service so views and commands do not share persistence details:

```rust,ignore
use reinhardt::db::orm::Model;

async fn save_note() -> Result<(), Box<dyn std::error::Error>> {
    let draft = Note::build()
        .title("Rust ORM")
        .score(10)
        .published(true)
        .finish();

    let created = Note::objects().create(&draft).await?;
    let mut loaded = Note::objects().get(created.id).get().await?;
    loaded.title = "Updated title".to_owned();
    let updated = Note::objects().update(&loaded).await?;

    Note::objects().delete(updated.id).await?;
    Ok(())
}
```

`create`, `update`, and `delete` use the configured ORM connection. The
`get(pk)` call builds a filtered `QuerySet`; its terminal `get().await` expects
exactly one row. Reads can keep the same typed field boundary:

```rust,ignore
let notes = Note::objects()
    .filter(Note::field_title().contains("Rust"))
    .order_by(&["-id"])
    .limit(20)
    .all()
    .await?;
```

## SQLAlchemy-style queries

The `sqlalchemy_query` module mirrors SQLAlchemy's `select().where()` style
and renders a SQL statement. It is a SQL builder, not a model-hydration API;
use `Model::objects()` when the result should be decoded as `Note` values.

```rust,ignore
use reinhardt::db::orm::{select, Model, Q};
use reinhardt::db::orm::sqlalchemy_query::column;

let sql = select::<Note>()
    .columns(vec![column("id"), column("title")])
    .where_clause(Q::new("published", "=", "true"))
    .order_by("id", false)
    .limit(20)
    .to_sql();
// SELECT id, title FROM notes_item WHERE published = true ORDER BY id DESC LIMIT 20
```

This builder accepts SQL fragments as strings, so do not concatenate request
values into `Q::new`. Use the typed manager filters for application input, or
`reinhardt-query` when a low-level parameterized statement is required.

## Typed aggregation queries (introduced in 0.4.0)

Use `reinhardt::db::orm::func` for typed `COUNT`, `SUM`, `AVG`, `MIN`, and
`MAX` expressions. Every aggregate needs a validated label, and terminal
`aggregate()` returns an `AggregateResult` instead of hydrating `Note` rows:

```rust,ignore
use reinhardt::db::orm::{func, Model};

async fn summarize_notes() -> Result<(), Box<dyn std::error::Error>> {
    let summary = Note::objects()
        .filter(Note::field_published().eq(true))
        .aggregate([
            func::count_all::<Note>().label("note_count")?,
            func::sum(Note::field_score()).label("score_total")?,
            func::avg(Note::field_score()).label("score_average")?,
        ])
        .await?;

    let note_count = summary.get_i64("note_count")?;
    let score_total = summary.get_i64("score_total")?;
    let score_average = summary.get_f64("score_average")?;
    let _ = (note_count, score_total, score_average);
    Ok(())
}
```

`SUM` and `AVG` can return SQL `NULL` for an empty filtered set; when that is
possible, inspect `summary.get()` and handle `AggregateValue::Null` instead of
using a numeric accessor unconditionally.

For grouped results, add an explicit projection before an aggregate
annotation. `annotate()` adds a computed SQL column; `to_sql()` shows the
statement, while `all().await` hydrates only the model fields:

```rust,ignore
use reinhardt::db::orm::{func, Model};

let grouped_sql = Note::objects()
    .all()
    .values(&["published"])
    .annotate(func::count_all::<Note>().label("note_count")?)?
    .having(func::count_all::<Note>().gt(0_i64))
    .to_sql()?;
```

## Migration lifecycle

`makemigrations` compares the registered model state with the state represented
by existing migration files and writes a new migration under
`migrations/<app_label>/`. It does not apply SQL to the database. Run
`migrate` separately.

After changing a model, use this sequence:

```bash
# Generate migrations for every installed app
cargo make makemigrations

# Or generate only one app's migration
cargo make makemigrations-app -- notes

# Preview the generated operations without writing a file
cargo run --bin manage makemigrations notes --dry-run

# Inspect and apply the resulting migration graph
cargo run --bin manage showmigrations --plan
cargo run --bin manage migrate
```

Review the generated `.rs` file before applying it. A normal change should
follow this order:

1. Change the model and its service/DTO contracts together.
2. Run `makemigrations <app_label>` and inspect every operation and dependency.
3. Use `showmigrations --plan` (and `sqlmigrate` when SQL needs review).
4. Apply with `migrate` in a development database, then in deployment.
5. Commit the model change and its migration in the same change.

Useful options are:

| Option | Use |
| --- | --- |
| `<APP_LABEL>` | Limit generation to one app; omit it to inspect all apps. |
| `--dry-run` | Show the migration that would be created without writing files. |
| `--empty` | Create an intentional empty migration for a data or manual operation. |
| `--merge` | Create a merge migration after independent migration branches conflict. |
| `-n, --name <NAME>` | Give a generated migration a stable descriptive suffix. |
| `--force-empty-state` | Treat the previous state as empty; use only for a genuinely new history. |

The default state builder uses the project's local infrastructure when
available and can fall back to the configured database or migration files.
`--force-empty-state` skips that history. On a project with existing
migrations it can regenerate tables and create duplicate operations, so fix
the database/container setup instead of using the flag as a general fallback.

## Migration history rules

- Do not rewrite or delete an applied migration. Add a new migration that
  changes the schema from the current state.
- Keep migration dependencies and app labels intact when resolving conflicts;
  use `makemigrations --merge` only after reviewing both branch histories.
- Treat destructive operations (dropping a column/table, narrowing a field,
  or replacing a table) as a deployment change. Back up data and add an
  explicit data migration when a transformation is required.
- Run management commands from the generated project root, where
  `src/bin/manage.rs` and `migrations/` are present.

## Pages boundary

The Pages template has native and WASM targets. Keep ORM models, migrations,
and database access on the native/server side. Shared request/response types
may live in target-neutral modules only when they are serializable and do not
carry ORM state. This lets the browser remain independent of the database
while the app's server functions and services own persistence.
