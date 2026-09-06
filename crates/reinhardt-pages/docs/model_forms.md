# Model-backed Pages forms

Pages can derive a rendered form and one typed submission payload from model
metadata. Use `form = true` for a legacy-only model-form declaration. A nested
named `form(...)` declaration generates its target-neutral contract and the
native legacy schema and generic payload needed by its adapter.

## Named target-neutral contracts

For new browser create forms, prefer one named contract on the real model:

```rust,ignore
#[model(
    app_label = "clusters",
    table_name = "clusters",
    form(name = ClusterCreateForm, fields(name, api_url))
)]
pub struct Cluster {
    #[field(primary_key = true)]
    id: Option<i64>,
    #[field(editable = false)]
    #[rel(foreign_key, related_name = "clusters")]
    organization: ForeignKeyField<Organization>,
    #[field(max_length = 63)]
    name: String,
    #[field(url = true, max_length = 2048)]
    api_url: String,
}
```

The nested declaration accepts exactly `name = Ident` and a non-empty,
source-ordered `fields(field, ...)` list. It generates these public names on
native and WASM: `ClusterCreateForm`, `ClusterCreateFormData`,
`ClusterCreateFormSchema`, and `ClusterCreateFormField`. The generated
`ClusterCreateFormPolicy` is public only because it is an associated type and
is hidden from documentation. The ORM model, legacy generic model-form types,
and the persistence adapter remain native-only.

Use that marker directly from `form!`; do not repeat `model`, `policy`, or a
field list:

```rust,ignore
let form = form! {
    name: CreateClusterForm,
    model_form: ClusterCreateForm,
    server_fn: create_cluster_for_current_org,
    overrides: {
        name: { label: "Name" },
        api_url: { label: "API URL" },
    },
};
```

`model_form:` cannot be combined with legacy `model`, `policy`, bracketed
`fields`, `exclude`, or braced manual `fields`. `overrides` remains valid and
is checked against the selected contract fields. The generated data type is a
strict JSON boundary: it serializes selected supplied fields only and rejects
unknown keys, duplicate keys, and incompatible values on deserialization. A
nullable selected value distinguishes omission from an explicit JSON `null`.

Selected fields may use `String`, `bool`, and the native ORM-supported numeric
primitives `i32`, `i64`, `f32`, and `f64`,
`rust_decimal::Decimal`, `uuid::Uuid`, `chrono::NaiveDate`,
`chrono::NaiveTime`, `chrono::NaiveDateTime`, `chrono::DateTime<chrono::Utc>`,
`serde_json::Value`, or one `Option<T>` layer. Relationships, generated
relationship identifiers, file/image fields, collections, and custom types
are intentionally unsupported.

`use_form(&form).build().set_value(field, value)` accepts UUID and chrono
values, including `Option<T>`, on native and WASM without enabling the Pages
`uuid` or `chrono` compatibility features. UTC datetimes use the `Z` suffix
and retain their fractional-second precision in the submission payload.

On native targets, construct the existing `ModelForm` from the concrete
payload and inject server-owned values only after authorization:

```rust,ignore
let mut payload = ClusterCreateFormData::default();
payload.set_name("Production".to_owned());
payload.set_api_url("https://api.example.com".to_owned());

let mut native_form = ClusterCreateForm::model_form(payload);
native_form.set_trusted_field_value("organization_id", serde_json::json!(organization_id))?;
let cluster = native_form.build_instance()?;
```

`model_form()` and `set_trusted_field_value()` are native-only trusted bridges;
they do not make server-owned values part of browser JSON. The named contract
replaces a WASM shadow model: compile the declaration module for both targets,
but keep relation imports and other ORM-only surrounding items explicitly
native-gated. WASM then needs only the contract types, not `Cluster`, its
relationship graph, or a database driver.

The native legacy schema retains target metadata for generated relationship
identifiers outside the public allowlist. This lets `InlineFormSet` validate a
server-owned relationship without exposing it in the named payload.

The additive [`ModelFormContractSchema`](https://docs.rs/reinhardt-core/latest/reinhardt_core/model_form/trait.ModelFormContractSchema.html)
bridge uses `contract_fields()` and `contract_default_boolean_is_true()` so
legacy `ModelFormSchema::fields()` calls remain unambiguous for glob imports.
Existing legacy schemas are adapted automatically and keep their original
associated methods.

`form = true`, `QuestionFormSchema`, `QuestionModelFormData<P>`, and the
legacy `form! { model, policy, fields | exclude, ... }` form remain supported.
Use the legacy path when its generic policy behavior is required; do not mix it
with a named contract.

```rust
use reinhardt::model;
use reinhardt::db::associations::ForeignKeyField;
use serde::{Deserialize, Serialize};

#[model(app_label = "users", form = true)]
#[derive(Clone, Deserialize, Serialize)]
pub struct User {
    #[field(primary_key = true)]
    id: i64,
}

#[model(app_label = "polls", form = true)]
#[derive(Clone, Deserialize, Serialize)]
pub struct Question {
    #[field(primary_key = true)]
    id: Option<i64>,
    #[field(max_length = 200)]
    text: String,
    #[rel(foreign_key, related_name = "questions")]
    owner: ForeignKeyField<User>,
    #[field(default = false)]
    published: bool,
}
```

This legacy opt-in generates the ORM-coupled `QuestionFormSchema` and generic
`QuestionModelFormData<P>` payload. `QuestionFormSchema` implements
`ModelFormSchema` and therefore has an associated ORM `Model` type; it is not
the ORM-free target-neutral schema generated by a nested named declaration.
Use `QuestionCreateFormSchema` from
`form(name = QuestionCreateForm, fields(...))` when the schema must compile on
WASM without the model graph. A model with neither `form = true` nor
`form(...)` has no legacy model-form symbols. Each endpoint must name the
concrete policy it enforces:

```rust
use reinhardt::core::model_form::ModelFormPolicy;
use reinhardt::pages::ServerFnError;

struct QuestionSubmissionPolicy;

impl ModelFormPolicy for QuestionSubmissionPolicy {
    fn allows(field: &str) -> bool {
        matches!(field, "text")
    }
}

#[server_fn(model_form = true)]
async fn save_question(
    mut payload: QuestionModelFormData<QuestionSubmissionPolicy>,
) -> Result<(), ServerFnError> {
    let owner_id = authenticated_owner_id()?;

    // Trusted typed setters are a server-side construction path. They do not
	// make the field part of the public wire contract.
	payload.set_trusted_owner_id(owner_id);
    persist_question(payload).await
}
```

`model_form = true` is an explicit endpoint opt-in for native HTML form decoding; it
does not change ordinary JSON server functions, including those that happen to
name a parameter `payload`. The application-specific `authenticated_owner_id()` and
`persist_question()` boundaries above obtain request identity and a database
executor. JSON model mode calls `save_question` with exactly one generated
payload. It does not expand the model fields into positional server-function
arguments.

`form!` remains an expression macro. The form-specific policy and data alias
created inside the expression are implementation items. Do not try to name
types such as `QuestionFormPolicy` or `QuestionFormData` outside that
expression. The server function instead names the application policy in
`QuestionModelFormData<QuestionSubmissionPolicy>`.

## Explicit fields

Use `fields: [...]` to expose an allowlist. This is the safer default because a
new editable model field is not added to the form until the declaration is
reviewed.

```rust
use reinhardt::pages::form;

let question_form = form! {
    name: QuestionForm,
    model: Question,
    policy: QuestionSubmissionPolicy,
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

## File and image fields

When an explicit model-field selection includes a `File` or `Image` field,
`form!` renders a multipart form and calls a normal function-like
`#[server_fn]` directly. Use one client-visible server-function parameter for
each selected field, with the exact same field name, order, and count:

```rust
use reinhardt::core::parsers::UploadedFile;
use reinhardt::pages::form;
use reinhardt::pages::server_fn::{server_fn, ServerFnError};

#[server_fn]
async fn upload(
    title: String,
    document: UploadedFile,
    avatar: Option<UploadedFile>,
) -> Result<(), ServerFnError> {
    let _ = (title, document, avatar);
    Ok(())
}

let upload_form = form! {
    name: UploadForm,
    model: Upload,
    policy: UploadPolicy,
    fields: [title, document, avatar],
    server_fn: upload,
};
```

Scalar fields are encoded as JSON multipart parts, while `File` and `Image`
fields use `UploadedFile` or `Option<UploadedFile>`. The direct multipart
contract requires `fields: [...]`; `exclude: [...]` and
`ambient_arguments` (including its deprecated `strip_arguments` alias) are not
supported in model-backed forms. Obtain request-scoped values through normal
server-side request handling or injection instead. Ordinary forms may still
use `ambient_arguments` for non-field values.

Raw Rust identifiers use their unraw wire name throughout selection and
submission, so `r#type` is encoded and looked up as the multipart part `type`.

The selected model descriptor must match the typed server-function argument:
scalar descriptors use JSON arguments, required file/image descriptors use
`UploadedFile`, and optional file/image descriptors use
`Option<UploadedFile>`. A JSON model-form endpoint cannot submit file fields;
use the typed multipart form above instead.

On the browser, a failed submission retains selected files and the current
control values. A successful submission and a form reset clear the selected
file state and the corresponding file inputs. An unselected optional file is
omitted from multipart data; a selected zero-byte file remains a file.

Model forms without `File` or `Image` fields keep the JSON payload contract
shown above: the server function receives one generated payload, and both
explicit `fields: [...]` and `exclude: [...]` retain their existing behavior.

## Client-side responses and validation errors

The target-stable `submit()` method returns `Result<(), ServerFnError>`. On
WASM, `submit_response()` exposes the server function's typed success value;
attach the generated form to `use_form` and use `submit_server_fn` with that
method when the client needs the response:

```rust,ignore
use reinhardt_pages::{UseFormAsyncSubmitOutcome, form, use_form};

let create_form = form! {
    name: CreateQuestionForm,
    model: Question,
    policy: CreateQuestionPolicy,
    fields: [title],
    server_fn: create_question,
};
let runtime = use_form(&create_form).build();

match runtime.submit_server_fn(|| create_form.submit_response()).await? {
    UseFormAsyncSubmitOutcome::Submitted(response) => show_one_time_value(response),
    UseFormAsyncSubmitOutcome::AlreadyPending | UseFormAsyncSubmitOutcome::ValidationFailed => {}
}
```

Structured `ServerFnError` field errors are routed through the same runtime:
matching selected fields are available from `get_field_state`, while errors
for unselected or unknown fields remain in `form_state().form_error`. Explicit
field selections provide typed accessors such as `title_field()`; forms using
`exclude` can resolve a selected field with `form.field("title")`.

Model-form controls retain the raw browser value while the user is editing.
For example, opt-in trimming does not rewrite the mounted input, and an invalid
URL remains available for correction. Immediately before every generated
submission, Pages builds an owned snapshot in generated schema order, applies
field conversion and normalization, and runs the generated synchronous
validation pipeline under the form's `fields` or `exclude` selection policy.
Required fields outside that selection do not block the snapshot. The endpoint
payload keeps its declared policy, which the server validates independently.
The normalized raw payload is sent only when validation succeeds; the editable
control state remains unchanged.

Handwritten model payloads, including compile-test fixtures, must implement
`ModelFormValidatingPayload` in addition to `ModelFormPayload`. Its cleaned
payload must implement `ModelFormCleanedPayload` with `Raw` set to the original
payload type. Model-generated payloads provide both contracts automatically.

URL snapshot validation uses `reinhardt_core::validators::UrlValidator` on
native and WASM targets, matching generated server validation. Query strings
and fragments may follow the host directly, as in
`https://example.com?query=value` and `https://example.com#section`.

Snapshot errors combine conversion failures with generated validation errors
in schema order, with form-level errors last. A control that cannot be converted
keeps its conversion error instead of a secondary missing-value error.

Snapshot validation failures become the same structured `ServerFnError` used
for server responses. A recognized selected field is routed to
`get_field_state(field).error`; `_all`, excluded, and unknown field names are
routed to `form_state().form_error`. Automatic form submission stops before
the server-function adapter is called. The payload sent after successful
client-side validation is still an ordinary raw payload, so native server code
must independently call `clean_and_validate()` at its trust boundary.

## Excluded fields

Use `exclude: [...]` when nearly every editable model field belongs in the
form:

```rust
use reinhardt::pages::form;

struct EditorialQuestionPolicy;

impl ModelFormPolicy for EditorialQuestionPolicy {
    fn allows(field: &str) -> bool {
        matches!(field, "text" | "published")
    }
}

#[server_fn(model_form = true)]
async fn save_editorial_question(
    payload: QuestionModelFormData<EditorialQuestionPolicy>,
) -> Result<(), ServerFnError> {
    persist_question(payload).await
}

let editorial_form = form! {
    name: EditorialQuestionForm,
    model: Question,
    policy: EditorialQuestionPolicy,
    exclude: [owner_id],
    server_fn: save_editorial_question,
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

Policy-checked typed setters such as `payload.set_owner_id(value)` protect the
public field selection. Trusted server code can instead use the explicit
`payload.set_trusted_owner_id(value)` construction path after authentication or
authorization has selected an excluded editable value. Generic `set_json()`
also respects the active policy, and rejected wire input remains recorded even
if the server later supplies a trusted value.

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

## Scope boundaries

Named contracts remove schema and payload duplication only. Shared action
lifecycle work is tracked by #6220, mounted control binding and reset behavior
by #6221, and generated server-side validation execution by #6223. The current
form submit engine, control ownership, and server validation behavior are
unchanged.
