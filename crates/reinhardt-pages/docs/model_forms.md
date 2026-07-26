# Model-backed Pages forms

Pages can derive a rendered form and one typed submission payload from model
metadata. Model support is explicit: add `form = true` to `#[model]`.

```rust
use reinhardt::model;
use serde::{Deserialize, Serialize};

#[model(app_label = "polls", form = true)]
#[derive(Clone, Deserialize, Serialize)]
pub struct Question {
    #[field(primary_key = true)]
    id: Option<i64>,
    #[field(max_length = 200)]
    text: String,
    owner_id: i64,
    #[field(default = false)]
    published: bool,
}
```

This opt-in generates the target-neutral `QuestionFormSchema` and the generic
`QuestionModelFormData<P>` payload. A model without `form = true` has neither
symbol. The generated payload is the type accepted by the form's server
function:

```rust
use reinhardt::core::model_form::ModelFormPolicy;
use reinhardt::pages::ServerFnError;

async fn save_question<P: ModelFormPolicy>(
    mut payload: QuestionModelFormData<P>,
) -> Result<(), ServerFnError> {
    let owner_id = authenticated_owner_id()?;

    // Typed setters are a trusted server-side construction path. They do not
    // make the field part of the public wire contract.
    payload.set_owner_id(owner_id);
    persist_question(payload).await
}
```

The application-specific `authenticated_owner_id()` and
`persist_question()` boundaries above obtain request identity and a database
executor. Browser model mode calls `save_question` with exactly one argument:
one generated payload. It does not expand the model fields into positional
server-function arguments.

`form!` remains an expression macro. The form-specific policy and data alias
created inside the expression are implementation items. Do not try to name
types such as `QuestionFormPolicy` or `QuestionFormData` outside that
expression. The server function instead stays generic over
`QuestionModelFormData<P>`.

## Explicit fields

Use `fields: [...]` to expose an allowlist. This is the safer default because a
new editable model field is not added to the form until the declaration is
reviewed.

```rust
use reinhardt::pages::form;

let question_form = form! {
    name: QuestionForm,
    model: Question,
    fields: [text],
    server_fn: save_question,
    overrides: {
        text: {
            widget: TextArea,
            label: "Question",
            help_text: "Enter the question shown to voters",
        },
    },
};
```

`overrides` changes only display metadata. It accepts `widget`, `label`, and
`help_text`; it does not change the generated Rust value type, the model
schema, or the server-side field policy.

## Excluded fields

Use `exclude: [...]` when nearly every editable model field belongs in the
form:

```rust
use reinhardt::pages::form;

let editorial_form = form! {
    name: EditorialQuestionForm,
    model: Question,
    exclude: [owner_id],
    server_fn: save_question,
    overrides: {
        text: {
            label: "Question",
            help_text: "Use plain language",
        },
    },
};
```

`exclude` automatically includes future editable model fields that are not in
the denylist. That can unintentionally expand the public input surface after a
model change. Prefer `fields` for security-sensitive or long-lived forms, and
review every model change before relying on `exclude`.

Exactly one of bracketed `fields: [...]` and `exclude: [...]` is required in
model mode. Braced `fields: { ... }` remains the independent explicit-form
syntax and cannot be mixed with `model`.

## Public input and trusted values

The selected fields are enforced at both ends:

- HTML renders only selected editable fields.
- Payload serialization omits denied fields.
- Payload deserialization records known denied wire fields.
- Native `ModelForm` rejects any recorded denied field with
  `ModelFormError::ForbiddenInput`.

The server check is the security boundary. Removing a control from HTML does
not make a field safe by itself.

Generated typed setters such as `payload.set_owner_id(value)` are deliberately
available to trusted server code. A setter can populate an excluded editable
value after authentication or authorization has selected it. In contrast,
generic `set_json()` respects the active policy and rejected wire input remains
recorded even if the server later supplies a trusted value.

## Empty values and datetimes

Generated descriptors preserve model nullability separately from whether a
control is required. Clearing a nullable control supplies JSON `null`, so an
update changes the database column to `NULL`. Clearing a non-nullable control
that is optional because it is blank or has a default removes that value from
the payload; create defaults and existing update values therefore remain
available.

`DateTime<Utc>` and `NaiveDateTime` use distinct schema kinds. Both render as
`datetime-local` controls, whose browser value has no timezone offset. Reinhardt
uses the following explicit convention:

- A browser value for `DateTime<Utc>` is interpreted as UTC and serialized as
  RFC 3339 with `Z`.
- A browser value for `NaiveDateTime` remains offset-free and is serialized as
  `YYYY-MM-DDTHH:MM:SS`.

Preloaded UTC values render without the transport-only `Z`, because
`datetime-local` controls accept offset-free values only. Time and datetime
controls use `step="any"`, preserving seconds and fractional seconds supported
by the model fields.

Applications that need a user or venue timezone should convert that context
before setting the model-form control instead of relying on the UTC convention.

Editable assigned primary keys, such as natural string keys or integer keys
with `auto_increment = false`, are included in generated create-form schemas.
Database-generated primary keys remain excluded.

## Native create and update

Native model forms use the model-generated payload and the same policy type:

```rust
use reinhardt::core::model_form::ModelFormPolicy;
use reinhardt::forms::{ModelForm, ModelFormError};

fn create_form<P: ModelFormPolicy>(
    payload: QuestionModelFormData<P>,
) -> ModelForm<Question, P> {
    ModelForm::from_payload(payload)
}

fn update_form<P: ModelFormPolicy>(
    payload: QuestionModelFormData<P>,
    instance: Question,
) -> ModelForm<Question, P> {
    ModelForm::from_payload_and_instance(payload, instance)
}
```

The constructor carries explicit persistence intent:

- `from_payload` creates a new model and later emits an insert.
- `from_payload_and_instance` updates the supplied instance and later emits an
  update.

No database existence query or primary-key heuristic chooses between those
operations.

Persistence is asynchronous and uses an executor owned by the caller:

```rust
use reinhardt::db::orm::OrmExecutor;
use reinhardt::forms::{FormModel, ModelForm, ModelFormError};

async fn persist<P: ModelFormPolicy>(
    mut form: ModelForm<Question, P>,
    executor: &mut dyn OrmExecutor,
) -> Result<Question, ModelFormError> {
    let saved = form.save(executor).await?;
    Ok(saved)
}
```

The executor can be a normal connection or the transaction executor supplied
by `atomic`. Database failures remain
`ModelFormError::Persistence { source: DatabaseError }`; use
`ModelFormError::database_error()` to inspect the structured error kind.

## Build without saving

`build_instance()` is the `commit=False` equivalent:

```rust
use reinhardt::forms::{ModelForm, ModelFormError};

fn validate_without_saving<P: ModelFormPolicy>(
    payload: QuestionModelFormData<P>,
) -> Result<Question, ModelFormError> {
    let mut form = ModelForm::<Question, P>::from_payload(payload);
    let candidate = form.build_instance()?;
    Ok(candidate)
}
```

It performs field cleaning, applies model defaults, builds the candidate, and
runs the configured model validator without accessing the database. The
validated candidate is cached. Repeated builds and save retries reuse that
candidate, so a persistence error leaves the form retryable.

`build_instance()` returns a clone of the cached candidate. If caller code
mutates that clone or persists it outside the form, validating those mutations
is the caller's responsibility. Mutate the generated payload before building
when the changes should pass through the form validation pipeline.

## Required excluded and hidden values

Creation must resolve every required model field. An excluded editable field
can be populated with a trusted typed setter before
`ModelForm::from_payload`. A hidden or non-editable required field needs a
declared model default or another supported automatic construction path.
Ordinary unresolved required fields return
`ModelFormError::MissingModelField`; Reinhardt does not synthesize an empty
string or other placeholder.

Updates preserve omitted values from the existing instance. They do not
reapply model defaults over excluded data.

## Formsets

`ModelFormSet::save(executor).await` uses the same caller-owned executor and
returns saved models in form order. It builds every candidate before the first
write and stops persistence at the first error. `ModelFormSetConfig::min_num`
and `max_num` count only submission candidates. Populate generated extra forms
through `forms_mut` with `ModelForm::set_field_value`, or replace their data
with a payload created by `ModelForm::from_payload`.

Use `AdvancedModelFormSet` when forms must be added incrementally with
`add_form`. Both formset types check candidate-based cardinality first, then
validate and build every candidate before persistence. This preflight prevents
a later invalid form or cardinality failure from following earlier writes; use
a transaction when the complete multi-row operation must be atomic.

Untouched create-mode extra forms are excluded from cardinality, candidate
preflight, and persistence. An existing instance, supplied field, or forbidden
wire field marks a form as submitted. This prevents default-only models from
creating phantom rows while still preserving server-side forbidden-input
rejection.

Inline formsets also save asynchronously. They save the parent first, apply
the saved parent key to child payloads through the trusted construction path,
and then persist children with the same executor. Use
`InlineFormSet::for_create(parent, fk_field)` for a new parent, including one
with an assigned UUID, and `InlineFormSet::for_update(parent, fk_field)` for an
existing parent. The legacy `InlineFormSet::new` uses primary-key presence and
the numeric zero sentinel; it cannot distinguish an assigned primary key from
an existing row and therefore must not be used for assigned-key creation.
