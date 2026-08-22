# reinhardt-forms

Django-inspired form handling and validation for Rust

## Overview

`reinhardt-forms` provides a comprehensive form system for form handling and
validation. Inspired by Django's forms framework, it offers generated
model-backed forms and manual form definitions with extensive validation
capabilities.

Target-neutral model schemas and payloads can be shared with WASM code. Native
candidate construction and persistence use the caller's asynchronous ORM
executor. For HTML rendering and WASM form submission, see `reinhardt-pages`.

## Installation

Add `reinhardt` to your `Cargo.toml`:

<!-- reinhardt-version-sync:3 -->
```toml
[dependencies]
reinhardt = { version = "0.4.0-alpha.9", features = ["forms"] }

# Or use a preset:
# reinhardt = { version = "0.4.0-alpha.9", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.4.0-alpha.9", features = ["full"] }      # All features

# Forms is included in the standard preset
```

Then import form features:

```rust
use reinhardt::forms::{Form, Field, CharField, IntegerField};
```

**Note:** Form features are included in the `standard` and `full` feature presets.

## Features Status

### Core Form System

#### Implemented ✓

- **Form Base (`Form`)**: Complete form data structure with binding and validation
  - Form creation with initial data and field prefix support
  - Data binding and validation lifecycle
  - Custom clean functions for form-level and field-level validation
  - Field access and manipulation (add, remove, get)
  - Initial data and change detection
  - Error handling and reporting
  - Client-side validation rules (for WASM integration)

- **BoundField**: Field bound to form data
  - Field data and error binding
  - Label and help text support

- **WASM Compatibility (`wasm_compat`)**: WASM-compatible form metadata
  - `FormMetadata`: Serializable form state for client-side processing
  - `FieldMetadata`: Field information for client-side rendering
  - `ValidationRule`: Client-side validation rule definitions

### Field Types

#### Implemented ✓

**Basic Fields:**

- `CharField`: Text input with min/max length, stripping, null character validation
- `IntegerField`: Integer input with min/max value constraints, string parsing
- `BooleanField`: Boolean/checkbox input with flexible type coercion
- `EmailField`: Email validation with regex, length constraints

**Advanced Fields:**

- `FloatField`: Floating-point number validation with min/max constraints
- `DecimalField`: Precise decimal number handling with scale and precision
- `DateField`: Date input with multiple format support and locale handling
- `TimeField`: Time input with format parsing
- `DateTimeField`: Combined date and time validation
- `URLField`: URL validation with scheme and max length checks
- `JSONField`: JSON data validation and parsing
- `FileField`: File upload handling with size validation
- `ImageField`: Image file validation with dimension checks
- `ChoiceField`: Selection from predefined choices
- `MultipleChoiceField`: Multiple selection support
- `RegexField`: Pattern-based validation with custom regex
- `SlugField`: URL slug validation
- `GenericIPAddressField`: IPv4/IPv6 address validation
- `UUIDField`: UUID format validation
- `DurationField`: Time duration parsing
- `ComboField`: Multiple field validation combination
- `MultiValueField`: Composite field handling (base for split fields)
- `SplitDateTimeField`: Separate date and time inputs

**Model-Related Fields:**

- `ModelChoiceField`: Foreign key selection with queryset support
- `ModelMultipleChoiceField`: Many-to-many selection
  - `Form::has_changed` ignores selection order
  - Numeric IDs and strings with the same textual representation are treated as equivalent
  - Boolean, null, array, and object values retain their JSON type distinctions

### Model Integration

#### Implemented ✓

- **Generated model forms (`ModelForm<T, P>`)**: Descriptor-driven model
  validation and persistence
  - Explicit `#[model(form = true)]` opt-in
  - Generated `{Model}FormSchema` metadata and
    `{Model}ModelFormData<P>` typed payload
  - `ModelFormPolicy`-controlled public field selection
  - Typed trusted setters for server-owned values
  - `from_payload` for explicit create intent
  - `from_payload_and_instance` for explicit update intent
  - Database-free, cached `build_instance()` candidate construction
  - Caller-owned asynchronous `save(executor)` persistence
  - Structured `ModelFormError`, including retained database errors

Public JSON fields denied by the active policy are recorded during
deserialization and rejected by native candidate construction. Hiding a field
in HTML is not the security boundary. Server code may use the generated typed
setter to supply an excluded editable value from a trusted source.

### Formsets

#### Implemented ✓

- **FormSet**: Managing multiple forms together
  - Form collection management
  - Validation across multiple forms
  - Extra form generation
  - Min/max form count constraints
  - Deletion and ordering support
  - Management form handling
  - Non-form error tracking

- **ModelFormSet**: Formset for model instances
  - Generated payload and policy integration
  - Candidate-based `min_num` and `max_num` validation
  - Asynchronous ordered persistence through a caller-owned executor
  - Full candidate preflight before the first write
  - Untouched create-mode extra forms are excluded from cardinality, preflight,
    and persistence
  - Mutable extra-form access through `forms_mut` for submitted payloads
  - Persistence stops at the first error
  - Inline formset support
  - Configuration via `ModelFormSetConfig`
  - Builder pattern API via `ModelFormSetBuilder`

- **AdvancedModelFormSet**: Cardinality-aware model formset
  - `min_num` and `max_num` validation before candidate preflight
  - Incremental form insertion through `add_form`
  - Asynchronous ordered persistence through a caller-owned executor
  - Untouched create-mode extra forms are excluded from cardinality, preflight,
    and persistence; supplied or forbidden input marks an extra as submitted
  - Inline parent persistence uses explicit `InlineFormSet::for_create` or
    `InlineFormSet::for_update` intent

### Advanced Features

#### Implemented ✓

- **Form Wizard (`FormWizard`)**: Multi-step form flow
  - Step definition and management (`WizardStep`)
  - Conditional step availability
  - Session data storage across steps
  - Step navigation (next, previous, jump)
  - Final data compilation
  - Progress tracking

- **form! Macro** (with `macros` feature): Declarative form definition
  - DSL for defining forms with fields, validators, and client validators
  - Server-side and client-side validation rules
  - Field property configuration

### Validation

#### Implemented ✓

- **Field Validation**: Individual field cleaning and validation
  - Required field checking
  - Type conversion and coercion
  - Length constraints (CharField)
  - Value range constraints (IntegerField, FloatField, DecimalField)
  - Format validation (EmailField, URLField, DateField, etc.)
  - Pattern matching (RegexField)
  - Custom validators

- **Form Validation**: Multi-field validation
  - Custom clean methods (`add_clean_function`)
  - Field-specific clean methods (`add_field_clean_function`)
  - Cross-field validation
  - Error aggregation
  - Non-field errors

- **Error Handling**: Comprehensive error reporting
  - `FieldError` types (Required, Invalid, Validation)
  - `FormError` types (Field, Validation)
  - Custom error messages
  - Error message internationalization support

### Related Crates

Security and UI features have been moved to dedicated crates:

- **CSRF Protection**: Use `reinhardt-middleware::csrf`
- **Rate Limiting**: Use `reinhardt-middleware::rate_limit`
- **Honeypot Fields**: Use `reinhardt-middleware::honeypot`
- **XSS Protection**: Use `reinhardt-middleware::xss`
- **HTML Rendering**: Use `reinhardt-pages` for form rendering

## Usage Examples

### Basic Form

```rust
use reinhardt::forms::{Form, CharField, IntegerField, FormField};
use std::collections::HashMap;
use serde_json::json;

let mut form = Form::new();
form.add_field(Box::new(CharField::new("name".to_string())));
form.add_field(Box::new(IntegerField::new("age".to_string())));

let mut data = HashMap::new();
data.insert("name".to_string(), json!("John"));
data.insert("age".to_string(), json!(30));

form.bind(data);
assert!(form.is_valid());
```

### Prefixed Forms

Prefixed forms require submitted field names to include the prefix. After
validation, `cleaned_data()` uses canonical field names, while `BoundField`
continues to use the original submitted values so invalid forms can be
rerendered without losing user input.

```rust
use reinhardt::forms::{CharField, Field, Form};
use serde_json::json;
use std::collections::HashMap;

let mut form = Form::with_prefix("profile".to_string());
form.add_field(Box::new(CharField::new("name".to_string()).required()));
form.bind(HashMap::from([("profile-name".to_string(), json!("Ada"))]));

assert!(form.is_valid());
assert_eq!(form.cleaned_data().get("name"), Some(&json!("Ada")));
assert_eq!(
    form.get_bound_field("name").unwrap().value(),
    Some(&json!("Ada"))
);
```

### Using the form! Macro

```rust
use reinhardt::forms::form;
use std::collections::HashMap;
use serde_json::json;

let mut form = form! {
    fields: {
        username: CharField {
            required,
            max_length: 150,
        },
        password: CharField {
            required,
            widget: PasswordInput,
        },
    },
    validators: {
        username: [
            |v: &serde_json::Value| v.as_str().map_or(false, |s| s.len() >= 3)
                => "Username must be at least 3 characters",
        ],
        password: [
            // Unscoped rule: executed by server-side `form.is_valid()`.
            // Add `#[client(on = input)]` on a separate rule to also run
            // the check in the browser (see reinhardt-pages docs).
            |v: &serde_json::Value| v.as_str().map_or(false, |s| s.len() >= 8)
                => "Password must be at least 8 characters",
        ],
    },
};

let mut data = HashMap::new();
data.insert("username".to_string(), json!("john"));
data.insert("password".to_string(), json!("secret123"));
form.bind(data);

assert!(form.is_valid());
```

### ModelForm

```rust,no_run
use reinhardt::core::model_form::{ModelFormPolicy, ModelFormSchema};
use reinhardt::db::associations::ForeignKeyField;
use reinhardt::db::orm::OrmExecutor;
use reinhardt::forms::{FormModel, ModelForm, ModelFormError};
use reinhardt::model;
use serde::{Deserialize, Serialize};

#[model(app_label = "users", form = true)]
#[derive(Clone, Deserialize, Serialize)]
struct User {
    #[field(primary_key = true)]
    id: i64,
}

#[model(app_label = "polls", form = true)]
#[derive(Clone, Deserialize, Serialize)]
struct Question {
    #[field(primary_key = true)]
    id: Option<i64>,
    #[field(max_length = 200)]
    text: String,
    #[rel(foreign_key, related_name = "questions")]
    owner: ForeignKeyField<User>,
}

struct PublicQuestionFields;

impl ModelFormPolicy for PublicQuestionFields {
    fn allows(field: &str) -> bool {
        field == "text"
    }
}

async fn create_question(
    executor: &mut dyn OrmExecutor,
    owner_id: i64,
) -> Result<Question, ModelFormError> {
    let mut payload = QuestionModelFormData::<PublicQuestionFields>::empty();
    payload.set_text("Which framework should we use?".to_owned());

    // This typed setter is trusted server-side construction. The same field is
    // still rejected if it arrives in the public JSON payload.
    payload.set_trusted_owner_id(owner_id);

    let mut form = ModelForm::<Question, PublicQuestionFields>::from_payload(payload);
    let candidate = form.build_instance()?;
    assert_eq!(candidate.owner_id, owner_id);

    form.save(executor).await
}

fn check_generated_contract<T, P>()
where
    T: FormModel,
    P: ModelFormPolicy,
    T::Schema: ModelFormSchema<Model = T>,
{
}
```

Use `from_payload_and_instance(payload, instance)` for an update. Create and
update intent is selected by the constructor, not by a database existence
query or a primary-key guess.

`build_instance()` is the equivalent of Django's `commit=False`: it validates
and caches a model candidate without database access. Repeated calls and a
failed `save()` reuse that candidate, which makes persistence retryable.
Mutations made directly to the returned clone after `build_instance()` are the
caller's validation responsibility.

An excluded required value must have a declared model default, an automatic
model construction path, or a value supplied by a trusted typed setter before
construction. Otherwise `build_instance()` returns
`ModelFormError::MissingModelField`. Persistence failures remain
`ModelFormError::Persistence`, and `database_error()` returns the structured
`DatabaseError`.

### Custom Validation

```rust
use reinhardt::forms::{Form, FormError};

let mut form = Form::new();
form.add_clean_function(|data| {
    if data.get("password") != data.get("confirm_password") {
        Err(FormError::Validation("Passwords do not match".to_string()))
    } else {
        Ok(())
    }
});
```

## Architecture

- **Field Layer**: Individual field types with validation logic
- **Form Layer**: Form structure, binding, and validation
- **Model Layer**: ORM integration and automatic form generation
- **Formset Layer**: Multiple form management
- **Wizard Layer**: Multi-step form flows
- **WASM Layer**: Serializable metadata for client-side integration

## Design Philosophy

This crate follows Django's forms philosophy:

- Declarative field definitions
- Separation of validation logic
- Model integration
- Extensible and customizable
- WASM-compatible core

## License

Licensed under the BSD 3-Clause License.
