+++
title = "Part 5: Ownership and Poll CRUD"
description = "Add question ownership with migration 0002, then build ownership-checked CRUD for polls and choices."
weight = 50

[extra]
sidebar_weight = 50
+++

# Part 5: Ownership and Poll CRUD

The app now has users, but polls are still anonymous. In this part you will add `Question.author`, generate migration `0002_question_author`, and build create/edit/delete flows for questions and choices.

The client hides controls that do not apply to the current user, but that is only ergonomics. The server functions enforce ownership every time.

## Add the Author Field

Open `src/apps/polls/models.rs`. Import the user model and add an author foreign key to `Question`:

```rust
use crate::apps::users::models::User;
```

```rust
#[model(app_label = "polls", table_name = "questions")]
#[derive(Serialize, Deserialize)]
pub struct Question {
    #[field(primary_key = true)]
    pub id: i64,

    #[field(max_length = 200)]
    pub question_text: String,

    #[field(auto_now_add = true)]
    pub pub_date: DateTime<Utc>,

    #[rel(foreign_key, related_name = "questions")]
    pub author: ForeignKeyField<User>,
}
```

This is the schema change you intentionally deferred in Part 2.

`#[model]` regenerates `QuestionInfo` with an `author: RelationInfo<User>` field, so the client can tell whether the current user owns a question. No hand-written DTO edit is needed:

```rust
use crate::apps::polls::models::QuestionInfo;
```

## Generate Migration 0002

Run migrations again:

```bash
cargo make makemigrations
cargo make migrate
```

The new migration should add `author_id` to `questions`, not rewrite `0001_initial.rs`. The reference migration is `migrations/polls/0002_question_author.rs`:

```rust
pub(super) fn migration() -> Migration {
    Migration {
        app_label: "polls".to_string(),
        name: "0002_question_author".to_string(),
        operations: vec![Operation::AddColumn {
            table: "questions".to_string(),
            column: ColumnDefinition {
                name: "author_id".to_string(),
                type_definition: FieldType::BigInteger,
                not_null: true,
                unique: false,
                primary_key: false,
                auto_increment: false,
                default: None,
            },
            mysql_options: None,
        }],
        dependencies: vec![("polls".to_string(), "0001_initial".to_string())],
        atomic: true,
        replaces: vec![],
        initial: None,
        state_only: false,
        database_only: false,
        swappable_dependencies: vec![],
        optional_dependencies: vec![],
    }
}
```

For a tutorial database, it is fine to reset and reseed. For real existing data, a non-null column without a default needs a backfill plan before the constraint can be applied safely.

## Inject the Current User

The polls server functions need an authenticated user. Inject `CurrentUser<User>` directly; `SessionMiddleware` loads the session, `TutorialSessionAuthMiddleware` validates the current account and publishes its auth state, and the framework resolves the full `User` before the handler body runs:

```rust
use crate::apps::users::models::User;
use reinhardt::CurrentUser;

#[server_fn]
pub async fn create_question(
    question_text: String,
    #[inject] _db: DatabaseConnection,
    #[inject] CurrentUser(user): CurrentUser<User>,
) -> Result<QuestionInfo, ServerFnError> {
    require_active_user(&user)?;
    // ...
}
```

Anonymous users fail at injection time with 401. The tutorial still keeps a small handler-local active-user check so inactive accounts become 403:

```rust
#[cfg(server)]
fn require_active_user(user: &User) -> Result<(), ServerFnError> {
    if user.is_active {
        Ok(())
    } else {
        Err(ServerFnError::server(403, "User account is inactive"))
    }
}
```

## Create Questions

Add the create server function:

```rust
use crate::apps::polls::models::Question;
use reinhardt::DatabaseConnection;
use reinhardt::CurrentUser;
use std::result::Result;

#[server_fn]
pub async fn create_question(
    question_text: String,
    #[inject] _db: DatabaseConnection,
    #[inject] CurrentUser(user): CurrentUser<User>,
) -> Result<QuestionInfo, ServerFnError> {
    require_active_user(&user)?;

    let trimmed = question_text.trim();
    if trimmed.is_empty() || trimmed.len() > 200 {
        return Err(ServerFnError::server(
            400,
            "Question text must be between 1 and 200 characters",
        ));
    }

    let manager = Question::objects();
    let new_question = Question::build()
        .question_text(trimmed)
        .author(user.id())
        .finish();
    let saved = manager
        .create(&new_question)
        .await
        .map_err(|e| ServerFnError::application(format!("Database error: {}", e)))?;

    Ok(QuestionInfo::from(saved))
}
```

The `author(user.id())` builder call is the new ownership link.

## Update and Delete Questions

Update and delete both load the row, compare author ids, and reject non-authors:

```rust
if *question.author_id() != user.id() {
    return Err(ServerFnError::server(
        403,
        "Only the question's author can edit it",
    ));
}
```

Then update:

```rust
question.question_text = trimmed.to_string();

let updated = manager
    .update(&question)
    .await
    .map_err(|e| ServerFnError::application(format!("Database error: {}", e)))?;

Ok(QuestionInfo::from(updated))
```

Or delete:

```rust
manager
    .delete(question.id())
    .await
    .map_err(|e| ServerFnError::application(format!("Database error: {}", e)))?;

Ok(())
```

Do not rely on the client to protect these actions. The comparison in the server function is the rule.

## Manage Choices Through the Parent Question

`Choice` does not have its own author field. Ownership comes from the parent `Question`:

```rust
use crate::apps::polls::models::Question;
use std::result::Result;

#[cfg(server)]
async fn require_question_author(
    question_id: i64,
    user: &User,
) -> Result<Question, ServerFnError> {
    let question = Question::objects()
        .get(question_id)
        .first()
        .await
        .map_err(|e| ServerFnError::application(format!("Database error: {}", e)))?
        .ok_or_else(|| ServerFnError::server(404, "Question not found"))?;

    if *question.author_id() != user.id() {
        return Err(ServerFnError::server(
            403,
            "Only the question's author can manage its choices",
        ));
    }

    Ok(question)
}
```

This stays as a helper rather than an injectable provider because the rule depends on a route/form argument (`question_id`) and the loaded row. A route-wide `guard!` is useful for broad permissions, but this object-level check needs the current function's resource id and database lookup. The authentication part is already injected through `CurrentUser<User>`.

`create_choice` verifies the parent question first:

```rust
require_active_user(&user)?;
let question = require_question_author(question_id, &user).await?;

let new_choice = Choice::build()
    .choice_text(trimmed)
    .votes(0)
    .question(question.id())
    .finish();
```

`update_choice` and `delete_choice` load the choice, then call `require_question_author(*choice.question_id(), &user)` before mutating the row.

## Register the CRUD Server Functions

Update `src/apps/polls/urls/server_router.rs`:

```rust
ServerRouter::new()
    .server_fn(get_questions::marker)
    .server_fn(get_question_detail::marker)
    .server_fn(get_question_results::marker)
    .server_fn(vote::marker)
    .server_fn(submit_vote::marker)
    .server_fn(create_question::marker)
    .server_fn(update_question::marker)
    .server_fn(delete_question::marker)
    .server_fn(create_choice::marker)
    .server_fn(update_choice::marker)
    .server_fn(delete_choice::marker)
```

## Add Client CRUD Routes

Add question and choice routes in `src/apps/polls/urls/client_router.rs`:

```rust
ClientRouter::new()
    .component(components::polls_index::polls_index)
    .component(components::question_new::question_new)
    .component(components::choice_new::choice_new)
    .component(components::choice_edit::choice_edit)
    .component(components::choice_delete::choice_delete)
```

The existing detail/results routes remain below these.

## Add Question Forms

The new-question page submits to `create_question`:

```rust
pub fn question_new() -> Page {
    let new_form = form! {
        name: NewQuestionForm,
        server_fn: create_question,
        method: Post,
        redirect_on_success: "/",
        fields: {
            question_text: CharField {
                label: "Question",
                placeholder: "What do you want to ask?",
                max_length: 200,
                class: "form-control",
            }
        }
    };
```

The edit page loads the current question, then pre-fills the form:

```rust
let edit_form = form! {
    name: EditQuestionForm,
    server_fn: update_question,
    method: Post,
    redirect_on_success: "/",
    fields: {
        question_id: HiddenField<i64> {
            initial: qid,
        }
        question_text: CharField {
            label: "Question",
            placeholder: "Updated question text",
            max_length: 200,
            class: "form-control",
        }
    }
};
```

The delete page follows the same pattern with `delete_question` and a confirmation button.

## Add Choice Forms

Choice creation posts to `create_choice` and returns to the parent detail page:

```rust
let new_form = form! {
    name: NewChoiceForm,
    server_fn: create_choice,
    method: Post,
    success_url: |_form| polls_routes::reverse("detail", &[("question_id", qid.to_string().as_str())]),
    fields: {
        question_id: HiddenField<i64> {
            initial: qid,
        },
        choice_text: CharField {
            label: "Choice text",
            placeholder: "An answer option",
            required,
            max_length: 200,
            class: "form-control",
        },
    },
};
```

Choice edit/delete pages include both route ids so the client can return to the parent question:

```rust
polls_routes::reverse("detail", &[("question_id", question_id.to_string().as_str())])
```

## Hide Owner-Only Controls

Detail and results pages should load the current user and compare it with `q.author.id`:

```rust
let load_current_user = use_resource(
    || async move { current_user().await.map_err(|e| e.to_string()) },
    (),
);
```

```rust
let is_author = matches!(
    load_current_user.get(),
    ResourceState::Success(Some(ref u)) if u.id == q.author.id
);
```

Use `is_author` to show edit/delete/add-choice links only to the owner. This improves the UI, but the server checks above remain authoritative.

## Checkpoint

Run the app:

```bash
cargo make dev
```

Create two accounts. With account A, create a poll and choices. Log out, log in as account B, and try to open account A's edit/delete URLs directly. The forms may render, but submits must fail with an authorization error.

Before continuing:

- `0001_initial.rs` is still the anonymous poll schema.
- `0002_question_author.rs` adds `questions.author_id`.
- Question create/update/delete require `CurrentUser<User>`.
- Choice create/update/delete call `require_question_author`.
- Owner-only UI is only a convenience; server functions enforce the rule.
