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

#[model(app_label = "users", table_name = "users_account")]
#[derive(Serialize, Deserialize)]
pub struct Account {
    #[field(primary_key = true)]
    pub id: i64,

    #[field(max_length = 255)]
    pub email: String,
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
- Keep query and transaction code in an app service so endpoints, commands,
  and background jobs can share it without importing one another's views.
- Resolve `DatabaseConnection` through the generated dependency-injection
  context. Do not create a second pool inside a view or service.

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
cargo make makemigrations-app -- users

# Preview the generated operations without writing a file
cargo run --bin manage makemigrations users --dry-run

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

## API boundary

Keep serializers and response DTOs separate from persistence models when the
API exposes different writable and readable fields. Handlers should coordinate
request extraction, authorization, service calls, and response mapping; the
app service owns query and transaction details.
