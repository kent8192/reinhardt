# reinhardt-forms

Django-inspired form handling and validation for Rust

## Overview

`reinhardt-forms` provides a comprehensive form system for form handling and validation. Inspired by Django's forms framework, it offers both automatic form generation from models and manual form definitions with extensive validation capabilities.

This crate is designed to be **WASM-compatible**, providing a pure form processing layer without HTML generation or platform-specific features. For HTML rendering, see `reinhardt-pages`.

## Installation

Add `reinhardt` to your `Cargo.toml`:

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", features = ["forms"] }

# Or use a preset:
# reinhardt = { version = "0.1.0-alpha.1", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.1.0-alpha.1", features = ["full"] }      # All features

# Enable the form! macro:
# reinhardt = { version = "0.1.0-alpha.1", features = ["forms", "form-macros"] }
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

### Model Integration

#### Implemented ✓

- **ModelForm (`ModelForm<T>`)**: Automatic form generation from models
  - `FormModel` trait for model integration
  - Field type inference from model metadata
  - Field inclusion/exclusion configuration
  - Custom field override support
  - Model instance population from form data
  - Save functionality with validation

- **ModelFormBuilder**: Fluent API for ModelForm configuration
  - Field selection (include/exclude)
  - Widget customization
  - Label customization
  - Help text customization

- **ModelFormConfig**: Configuration structure for ModelForm behavior
  - Field mapping configuration
  - Validation rules
  - Save behavior customization

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
  - Queryset integration
  - Instance creation, update, and deletion
  - Inline formset support
  - Configuration via `ModelFormSetConfig`
  - Builder pattern API via `ModelFormSetBuilder`

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
    },
    client_validators: {
        password: [
            "value.length >= 8" => "Password must be at least 8 characters",
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

```rust
use reinhardt::forms::{ModelForm, ModelFormBuilder};

let form = ModelFormBuilder::<User>::new()
    .include_fields(vec!["name", "email"])
    .build();
```

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
