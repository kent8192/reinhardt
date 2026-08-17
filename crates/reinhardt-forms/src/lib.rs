#![warn(missing_docs)]

//! # Reinhardt Forms
//!
//! Form processing and validation for the Reinhardt framework.
//!
//! ## Overview
//!
//! This crate provides comprehensive form processing capabilities inspired by Django's
//! form system, focusing on data validation and multi-step form wizards.
//!
//! Generated model schemas and payloads are target-neutral. Native candidate
//! construction and persistence use a caller-owned asynchronous ORM executor.
//! HTML rendering is provided by `reinhardt-pages`.
//!
//! ## Features
//!
//! - **[`Form`]**: Base form class with validation
//! - **[`ModelForm`]**: Auto-generated forms from model definitions
//! - **[`FormSet`]**: Handle multiple forms of the same type
//! - **[`FormWizard`]**: Multi-step form workflows
//! - **Field Types**: 20+ field types (CharField, IntegerField, EmailField, etc.)
//! - **WASM Support**: Compatible with WebAssembly targets via `wasm_compat` module
//!
//! ## Quick Start
//!
//! ### Basic Form
//!
//! ```rust,ignore
//! use reinhardt_forms::{Form, CharField, EmailField, IntegerField};
//!
//! // Build a form imperatively using add_field()
//! let mut form = Form::new();
//! form.add_field(Box::new(CharField::new("name")));
//! form.add_field(Box::new(EmailField::new("email")));
//! form.add_field(Box::new(IntegerField::new("age")));
//! form.add_field(Box::new(CharField::new("message")));
//!
//! // Validate form data
//! form.bind(&request_data);
//! if form.is_valid() {
//!     // Process the validated form...
//! } else {
//!     let errors = form.errors();
//! }
//! ```
//!
//! ### Prefixed Form Data
//!
//! A prefixed form expects submitted field names to use the prefix. The
//! validated values are exposed through canonical field names, while bound
//! fields continue to read the original submitted values for rerendering.
//!
//! ```rust
//! use reinhardt_forms::{CharField, Field, Form};
//! use serde_json::json;
//! use std::collections::HashMap;
//!
//! let mut form = Form::with_prefix("profile".to_string());
//! form.add_field(Box::new(CharField::new("name".to_string()).required()));
//! form.bind(HashMap::from([("profile-name".to_string(), json!("Ada"))]));
//!
//! assert!(form.is_valid());
//! assert_eq!(form.cleaned_data().get("name"), Some(&json!("Ada")));
//! assert_eq!(
//!     form.get_bound_field("name").unwrap().value(),
//!     Some(&json!("Ada"))
//! );
//! ```
//!
//! ### Model Form
//!
//! ```rust,no_run
//! use reinhardt_core::model_form::ModelFormPolicy;
//! use reinhardt_db::orm::OrmExecutor;
//! use reinhardt_forms::{ModelForm, ModelFormError};
//! use reinhardt_macros::model;
//! use serde::{Deserialize, Serialize};
//! # mod model_form {
//! #     pub use reinhardt_forms::model_form::*;
//! # }
//!
//! #[model(
//!     app_label = "forms",
//!     table_name = "model_form_documented_users",
//!     form = true,
//!     info = false
//! )]
//! #[derive(Clone, Deserialize, Serialize)]
//! struct User {
//!     #[field(primary_key = true)]
//!     id: Option<i64>,
//!     #[field(max_length = 150)]
//!     username: String,
//!     #[field(max_length = 254)]
//!     email: String,
//! }
//!
//! struct PublicUserFields;
//!
//! impl ModelFormPolicy for PublicUserFields {
//!     fn allows(field: &str) -> bool {
//!         matches!(field, "username" | "email")
//!     }
//! }
//!
//! async fn create_user(
//!     executor: &mut dyn OrmExecutor,
//! ) -> Result<User, ModelFormError> {
//!     let mut data = UserModelFormData::<PublicUserFields>::empty();
//!     data.set_username("alice".to_owned());
//!     data.set_email("alice@example.com".to_owned());
//!
//!     let mut form = ModelForm::<User, PublicUserFields>::from_payload(data);
//!     let candidate = form.build_instance()?;
//!     assert_eq!(candidate.username, "alice");
//!     form.save(executor).await
//! }
//! # fn main() {}
//! ```
//!
//! `build_instance()` validates and caches a candidate without database
//! access. `save(executor).await` persists through the caller's executor and
//! preserves structured database failures in [`ModelFormError`].
//!
//! ## Available Field Types
//!
//! | Field | Description |
//! |-------|-------------|
//! | [`CharField`] | Text input with max_length validation |
//! | [`IntegerField`] | Integer input with min/max validation |
//! | [`FloatField`] | Floating-point number input |
//! | [`DecimalField`] | Decimal number with precision control |
//! | [`BooleanField`] | Checkbox input |
//! | [`EmailField`] | Email address validation |
//! | [`URLField`] | URL validation |
//! | [`DateField`] | Date input with format parsing |
//! | [`DateTimeField`] | DateTime input |
//! | [`TimeField`] | Time input |
//! | [`DurationField`] | Duration input |
//! | [`FileField`] | File upload |
//! | [`ImageField`] | Image upload with dimension validation |
//! | [`ChoiceField`] | Select dropdown |
//! | [`MultipleChoiceField`] | Multi-select |
//! | [`ModelChoiceField`] | Foreign key selection |
//! | [`ModelMultipleChoiceField`] | Multiple model selection with normalized dirty-state comparison |
//! | [`JSONField`] | JSON data input |
//! | [`UUIDField`] | UUID input |
//! | [`SlugField`] | URL-safe slug input |
//! | [`RegexField`] | Custom regex validation |
//!
//! `ModelMultipleChoiceField` compares selected values without considering
//! order when [`Form::has_changed`] runs. Numeric IDs and strings with the same
//! textual representation are equivalent, while booleans, nulls, arrays, and
//! objects remain distinct JSON types.
//!
//! ## FormSets
//!
//! Handle multiple forms of the same type:
//!
//! ```rust,ignore
//! use reinhardt_forms::{FormSet, FormSetFactory};
//!
//! // Create a formset with 3 forms
//! let formset = FormSetFactory::<ItemForm>::new()
//!     .extra(3)
//!     .min_num(1)
//!     .max_num(10)
//!     .build();
//!
//! if formset.is_valid() {
//!     for form in formset.forms() {
//!         // Process each form
//!     }
//! }
//! ```
//!
//! ## Form Wizard
//!
//! Multi-step forms:
//!
//! ```rust,ignore
//! use reinhardt_forms::{FormWizard, WizardStep};
//!
//! let wizard = FormWizard::new()
//!     .add_step(WizardStep::new("account", AccountForm::new()))
//!     .add_step(WizardStep::new("profile", ProfileForm::new()))
//!     .add_step(WizardStep::new("confirmation", ConfirmForm::new()));
//!
//! // Process wizard step
//! let result = wizard.process_step(&request).await?;
//! ```

/// Bound field rendering with data and errors attached.
pub mod bound_field;
/// Core form field trait and error types.
pub mod field;
/// Built-in field types (text, email, integer, choice, etc.).
pub mod fields;
/// Form trait and validation logic.
pub mod form;
/// Formset for managing multiple form instances.
pub mod formset;
/// Built-in formset types (inline, base).
pub mod formsets;
/// Model-backed form with automatic field generation.
pub mod model_form;
/// Model-backed formset for bulk editing.
pub mod model_formset;
/// Field-level and form-level validators.
pub mod validators;
/// WASM compatibility layer for client-side forms.
pub mod wasm_compat;
/// Multi-step form wizard.
pub mod wizard;

pub use bound_field::BoundField;
pub use field::{
	ErrorType,
	FieldError,
	FieldResult,
	FormField as Field, // Alias for compatibility
	FormField,
	Widget,
	escape_attribute,
	html_escape,
};
pub use fields::{
	BooleanField, CharField, ChoiceField, ColorField, ComboField, DateField, DateTimeField,
	DecimalField, DurationField, EmailField, FileField, FloatField, GenericIPAddressField,
	IPProtocol, ImageField, IntegerField, JSONField, ModelChoiceField, ModelMultipleChoiceField,
	MultiValueField, MultipleChoiceField, PASSWORD_REDACTED, PasswordField, RegexField, SlugField,
	SplitDateTimeField, TimeField, URLField, UUIDField,
};
pub use form::{Form, FormError, FormResult};
pub use formset::FormSet;
pub use formsets::{
	FormSetFactory,
	InlineFormSet,
	ModelFormSet as AdvancedModelFormSet, // Renamed to avoid conflict
};
pub use model_form::{FormModel, ModelForm, ModelFormError, ModelFormPersistenceMode};
pub use model_formset::{ModelFormSet, ModelFormSetBuilder, ModelFormSetConfig};
pub use validators::{SlugValidator, UrlValidator};
pub use wizard::{FormWizard, WizardStep};
